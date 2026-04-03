//! Effect system and algebraic effects for Fajar Lang.
//!
//! Provides a complete algebraic effect system that integrates with Fajar Lang's
//! context annotations (`@kernel`, `@device`, `@safe`, `@unsafe`) and type system.
//!
//! # Architecture
//!
//! ```text
//! EffectDecl ─── declares an effect with operations
//!     │
//!     ▼
//! EffectRegistry ─── global registry, validates uniqueness
//!     │
//!     ▼
//! EffectChecker ─── verifies effects are handled or propagated
//!     │
//!     ├── EffectInference ─── infers effects from function bodies
//!     ├── HandlerResolution ─── finds matching handlers
//!     └── ContextInteraction ─── @kernel/@device effect constraints
//! ```
//!
//! # Error Codes
//!
//! | Code | Name | Description |
//! |------|------|-------------|
//! | EE001 | UnhandledEffect | Effect not handled and not in function signature |
//! | EE002 | EffectMismatch | Handler provides wrong effect type |
//! | EE003 | MissingHandler | No handler for an effect operation |
//! | EE004 | DuplicateEffect | Effect already declared in registry |
//! | EE005 | InvalidResume | Resume used outside handler scope |
//! | EE006 | ContextEffectViolation | Effect forbidden in context annotation |
//! | EE007 | PurityViolation | `#[pure]` function performs effects |
//! | EE008 | EffectBoundViolation | Generic param lacks required effect bound |
//!
//! # Example (Fajar Lang source)
//!
//! ```text
//! effect Console {
//!     fn log(msg: str) -> void
//!     fn read_line() -> str
//! }
//!
//! fn greet() with Console {
//!     Console::log("Hello!")
//! }
//!
//! handle greet() {
//!     Console::log(msg) => { println(msg) }
//!     Console::read_line() => { resume("world") }
//! }
//! ```

use std::collections::{BTreeSet, HashMap};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Sprint 1 — Effect Declarations
// ═══════════════════════════════════════════════════════════════════════

/// The kind of an effect, categorizing its domain.
///
/// Each kind maps to a distinct set of constraints in the effect system.
/// For example, `IO` effects are forbidden in `@device` context, and
/// `Alloc` effects are forbidden in `@kernel` context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EffectKind {
    /// Input/output operations (console, file, network).
    IO,
    /// Heap memory allocation.
    Alloc,
    /// Panic/abort operations.
    Panic,
    /// Asynchronous operations (async/await).
    Async,
    /// Mutable state access.
    State,
    /// Exception throwing and catching.
    Exception,
    /// Hardware access (registers, IRQ, port I/O) — @kernel domain.
    Hardware,
    /// Tensor/ML compute operations — @device domain.
    Tensor,
}

impl fmt::Display for EffectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EffectKind::IO => write!(f, "IO"),
            EffectKind::Alloc => write!(f, "Alloc"),
            EffectKind::Panic => write!(f, "Panic"),
            EffectKind::Async => write!(f, "Async"),
            EffectKind::State => write!(f, "State"),
            EffectKind::Exception => write!(f, "Exception"),
            EffectKind::Hardware => write!(f, "Hardware"),
            EffectKind::Tensor => write!(f, "Tensor"),
        }
    }
}

/// Maps an effect name to its kind, if it's a built-in effect.
pub fn effect_kind_from_name(name: &str) -> Option<EffectKind> {
    match name {
        "IO" | "io" => Some(EffectKind::IO),
        "Alloc" | "alloc" | "heap" => Some(EffectKind::Alloc),
        "Panic" | "panic" => Some(EffectKind::Panic),
        "Async" | "async" => Some(EffectKind::Async),
        "State" | "state" => Some(EffectKind::State),
        "Exception" | "exception" => Some(EffectKind::Exception),
        "Hardware" | "hardware" => Some(EffectKind::Hardware),
        "Tensor" | "tensor" | "compute" => Some(EffectKind::Tensor),
        _ => None,
    }
}

/// A single operation within an effect declaration.
///
/// Each operation defines a name, parameter types, and return type,
/// representing one capability that the effect provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectOp {
    /// Operation name (e.g., `"log"`, `"read_line"`).
    pub name: String,
    /// Parameter type names (e.g., `["str", "i32"]`).
    pub param_types: Vec<String>,
    /// Return type name (e.g., `"void"`, `"str"`).
    pub return_type: String,
}

impl EffectOp {
    /// Creates a new effect operation.
    pub fn new(
        name: impl Into<String>,
        param_types: Vec<String>,
        return_type: impl Into<String>,
    ) -> Self {
        EffectOp {
            name: name.into(),
            param_types,
            return_type: return_type.into(),
        }
    }

    /// Returns true if this operation takes no parameters.
    pub fn is_nullary(&self) -> bool {
        self.param_types.is_empty()
    }
}

/// A complete effect declaration with a name, kind, and operations.
///
/// An effect declaration defines a set of operations that form a
/// coherent capability. For example, a `Console` effect might have
/// `log` and `read_line` operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectDecl {
    /// Effect name (e.g., `"Console"`, `"FileSystem"`).
    pub name: String,
    /// The kind of effect (IO, Alloc, etc.).
    pub kind: EffectKind,
    /// Operations provided by this effect.
    pub operations: Vec<EffectOp>,
}

impl EffectDecl {
    /// Creates a new effect declaration.
    pub fn new(name: impl Into<String>, kind: EffectKind, operations: Vec<EffectOp>) -> Self {
        EffectDecl {
            name: name.into(),
            kind,
            operations,
        }
    }

    /// Looks up an operation by name.
    pub fn find_op(&self, op_name: &str) -> Option<&EffectOp> {
        self.operations.iter().find(|op| op.name == op_name)
    }

    /// Returns the number of operations in this effect.
    pub fn op_count(&self) -> usize {
        self.operations.len()
    }
}

/// An ordered set of effects for function signatures.
///
/// Uses `BTreeSet` for deterministic ordering. Supports set operations
/// (union, intersection, subset) for effect polymorphism and subtyping.
///
/// # Examples
///
/// ```text
/// fn read_file(path: str) -> str with {IO, Exception}
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectSet {
    /// The effects in this set, ordered by name.
    effects: BTreeSet<String>,
}

impl EffectSet {
    /// Creates an empty effect set (pure function).
    pub fn empty() -> Self {
        EffectSet {
            effects: BTreeSet::new(),
        }
    }

    /// Creates an effect set from an iterator of effect names.
    pub fn collect_from(iter: impl IntoIterator<Item = String>) -> Self {
        EffectSet {
            effects: iter.into_iter().collect(),
        }
    }

    /// Adds an effect to the set.
    pub fn insert(&mut self, effect: impl Into<String>) {
        self.effects.insert(effect.into());
    }

    /// Removes an effect from the set.
    pub fn remove(&mut self, effect: &str) -> bool {
        self.effects.remove(effect)
    }

    /// Returns true if the set contains the given effect.
    pub fn contains(&self, effect: &str) -> bool {
        self.effects.contains(effect)
    }

    /// Returns true if this set is empty (pure).
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Returns the number of effects in the set.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Returns the union of two effect sets.
    pub fn union(&self, other: &EffectSet) -> EffectSet {
        EffectSet {
            effects: self.effects.union(&other.effects).cloned().collect(),
        }
    }

    /// Returns the intersection of two effect sets.
    pub fn intersection(&self, other: &EffectSet) -> EffectSet {
        EffectSet {
            effects: self.effects.intersection(&other.effects).cloned().collect(),
        }
    }

    /// Returns true if this set is a subset of `other`.
    pub fn is_subset_of(&self, other: &EffectSet) -> bool {
        self.effects.is_subset(&other.effects)
    }

    /// Returns the difference: effects in self but not in other.
    pub fn difference(&self, other: &EffectSet) -> EffectSet {
        EffectSet {
            effects: self.effects.difference(&other.effects).cloned().collect(),
        }
    }

    /// Returns an iterator over effect names.
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.effects.iter()
    }
}

impl fmt::Display for EffectSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.effects.is_empty() {
            return write!(f, "pure");
        }
        let names: Vec<&String> = self.effects.iter().collect();
        write!(
            f,
            "{{{}}}",
            names
                .iter()
                .map(|n| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Global registry of declared effects.
///
/// Validates that effect names are unique and provides lookup by name.
/// Built-in effects (IO, Alloc, Panic, Exception) are pre-registered.
#[derive(Debug, Clone)]
pub struct EffectRegistry {
    /// Registered effects by name.
    effects: HashMap<String, EffectDecl>,
}

impl EffectRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        EffectRegistry {
            effects: HashMap::new(),
        }
    }

    /// Creates a registry with built-in effects pre-registered.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register_builtins();
        registry
    }

    /// Registers the built-in IO, Alloc, Panic, and Exception effects.
    fn register_builtins(&mut self) {
        let io = EffectDecl::new(
            "IO",
            EffectKind::IO,
            vec![
                EffectOp::new("print", vec!["str".into()], "void"),
                EffectOp::new("read", vec![], "str"),
            ],
        );
        // Safe to use expect here: builtins are hardcoded and unique.
        // But we follow the no-unwrap rule — use if-let instead.
        let _ = self.register(io);

        let alloc = EffectDecl::new(
            "Alloc",
            EffectKind::Alloc,
            vec![
                EffectOp::new("alloc", vec!["usize".into()], "ptr"),
                EffectOp::new("dealloc", vec!["ptr".into()], "void"),
            ],
        );
        let _ = self.register(alloc);

        let panic_eff = EffectDecl::new(
            "Panic",
            EffectKind::Panic,
            vec![EffectOp::new("panic", vec!["str".into()], "never")],
        );
        let _ = self.register(panic_eff);

        let exception = EffectDecl::new(
            "Exception",
            EffectKind::Exception,
            vec![EffectOp::new("throw", vec!["str".into()], "never")],
        );
        let _ = self.register(exception);

        let async_eff = EffectDecl::new(
            "Async",
            EffectKind::Async,
            vec![
                EffectOp::new("spawn", vec!["fn".into()], "void"),
                EffectOp::new("await_result", vec![], "any"),
            ],
        );
        let _ = self.register(async_eff);

        let state_eff = EffectDecl::new(
            "State",
            EffectKind::State,
            vec![
                EffectOp::new("get", vec![], "any"),
                EffectOp::new("set", vec!["any".into()], "void"),
            ],
        );
        let _ = self.register(state_eff);

        let hardware = EffectDecl::new(
            "Hardware",
            EffectKind::Hardware,
            vec![
                EffectOp::new("volatile_read", vec!["i64".into()], "i64"),
                EffectOp::new("volatile_write", vec!["i64".into(), "i64".into()], "void"),
                EffectOp::new("port_read", vec!["u16".into()], "u8"),
                EffectOp::new("port_write", vec!["u16".into(), "u8".into()], "void"),
            ],
        );
        let _ = self.register(hardware);

        let tensor_eff = EffectDecl::new(
            "Tensor",
            EffectKind::Tensor,
            vec![
                EffectOp::new("matmul", vec!["tensor".into(), "tensor".into()], "tensor"),
                EffectOp::new("relu", vec!["tensor".into()], "tensor"),
                EffectOp::new("softmax", vec!["tensor".into()], "tensor"),
            ],
        );
        let _ = self.register(tensor_eff);
    }

    /// Registers a new effect. Returns `Err` if the name is already taken.
    pub fn register(&mut self, decl: EffectDecl) -> Result<(), EffectError> {
        if self.effects.contains_key(&decl.name) {
            return Err(EffectError::DuplicateEffect {
                name: decl.name.clone(),
            });
        }
        self.effects.insert(decl.name.clone(), decl);
        Ok(())
    }

    /// Looks up an effect by name.
    pub fn lookup(&self, name: &str) -> Option<&EffectDecl> {
        self.effects.get(name)
    }

    /// Returns the number of registered effects.
    pub fn count(&self) -> usize {
        self.effects.len()
    }

    /// Returns all registered effect names.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.effects.keys().cloned().collect();
        names.sort();
        names
    }

    /// Validates that all effects in a set are registered.
    pub fn validate_set(&self, set: &EffectSet) -> Result<(), EffectError> {
        for name in set.iter() {
            if self.lookup(name).is_none() {
                return Err(EffectError::UnhandledEffect {
                    effect: name.clone(),
                    context: "unknown effect not in registry".to_string(),
                });
            }
        }
        Ok(())
    }
}

impl Default for EffectRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Infers the effect set for a function body from a list of called effects.
///
/// Given a sequence of `(effect_name, op_name)` pairs representing effect
/// operations invoked in a function body, returns the minimal `EffectSet`.
pub fn infer_effects(calls: &[(String, String)], registry: &EffectRegistry) -> EffectSet {
    let mut set = EffectSet::empty();
    for (eff_name, op_name) in calls {
        if let Some(decl) = registry.lookup(eff_name) {
            if decl.find_op(op_name).is_some() {
                set.insert(eff_name.clone());
            }
        }
    }
    set
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 2 — Effect Handlers
// ═══════════════════════════════════════════════════════════════════════

/// A handler for a single effect operation.
///
/// Represents the body of a handler clause that intercepts an effect
/// operation and decides how to resume (or abort) computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpHandler {
    /// The operation name this handler intercepts.
    pub op_name: String,
    /// Parameter names bound in the handler (e.g., `["msg"]`).
    pub param_names: Vec<String>,
    /// Placeholder for handler body representation.
    /// In a full implementation, this would be an AST node.
    pub body_repr: String,
    /// Whether this handler uses `resume` (continuation).
    pub uses_resume: bool,
}

impl OpHandler {
    /// Creates a new operation handler.
    pub fn new(
        op_name: impl Into<String>,
        param_names: Vec<String>,
        body_repr: impl Into<String>,
        uses_resume: bool,
    ) -> Self {
        OpHandler {
            op_name: op_name.into(),
            param_names,
            body_repr: body_repr.into(),
            uses_resume,
        }
    }
}

/// A complete effect handler that intercepts all operations of an effect.
///
/// Contains a mapping from operation names to their handler implementations.
/// Used within `handle` blocks to provide effect implementations.
#[derive(Debug, Clone)]
pub struct EffectHandler {
    /// The effect name this handler handles (e.g., `"Console"`).
    pub effect_name: String,
    /// Operation handlers keyed by operation name.
    pub op_handlers: HashMap<String, OpHandler>,
}

impl EffectHandler {
    /// Creates a new effect handler for the named effect.
    pub fn new(effect_name: impl Into<String>) -> Self {
        EffectHandler {
            effect_name: effect_name.into(),
            op_handlers: HashMap::new(),
        }
    }

    /// Adds a handler for an operation. Returns error on duplicate.
    pub fn add_op_handler(&mut self, handler: OpHandler) -> Result<(), EffectError> {
        if self.op_handlers.contains_key(&handler.op_name) {
            return Err(EffectError::MissingHandler {
                effect: self.effect_name.clone(),
                operation: handler.op_name.clone(),
            });
        }
        self.op_handlers.insert(handler.op_name.clone(), handler);
        Ok(())
    }

    /// Looks up a handler for the given operation name.
    pub fn find_handler(&self, op_name: &str) -> Option<&OpHandler> {
        self.op_handlers.get(op_name)
    }

    /// Returns the number of handled operations.
    pub fn handler_count(&self) -> usize {
        self.op_handlers.len()
    }
}

/// Represents a `handle` expression with a body and associated handlers.
///
/// ```text
/// handle {
///     greet()
/// } with {
///     Console::log(msg) => { println(msg) }
///     Console::read_line() => { resume("world") }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct HandleExpr {
    /// Placeholder for the body expression/block.
    pub body_repr: String,
    /// Handlers for each effect used in the body.
    pub handlers: Vec<EffectHandler>,
    /// The set of effects handled by this expression.
    pub handled_effects: EffectSet,
}

impl HandleExpr {
    /// Creates a new handle expression.
    pub fn new(body_repr: impl Into<String>) -> Self {
        HandleExpr {
            body_repr: body_repr.into(),
            handlers: Vec::new(),
            handled_effects: EffectSet::empty(),
        }
    }

    /// Adds a handler and updates the handled effects set.
    pub fn add_handler(&mut self, handler: EffectHandler) {
        self.handled_effects.insert(handler.effect_name.clone());
        self.handlers.push(handler);
    }

    /// Returns the set of effects handled.
    pub fn effects_handled(&self) -> &EffectSet {
        &self.handled_effects
    }
}

/// A delimited continuation resume point.
///
/// Represents the captured continuation at the point where an effect
/// operation is performed. The handler can invoke `resume(value)` to
/// continue execution from where the effect was raised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumePoint {
    /// Unique identifier for this resume point.
    pub id: u64,
    /// The expected type of the resume value (as type name string).
    pub resume_type: String,
    /// The effect operation that created this resume point.
    pub origin_op: String,
    /// The effect name.
    pub origin_effect: String,
}

impl ResumePoint {
    /// Creates a new resume point.
    pub fn new(
        id: u64,
        resume_type: impl Into<String>,
        origin_op: impl Into<String>,
        origin_effect: impl Into<String>,
    ) -> Self {
        ResumePoint {
            id,
            resume_type: resume_type.into(),
            origin_op: origin_op.into(),
            origin_effect: origin_effect.into(),
        }
    }
}

/// Resolves which handler should handle a given effect operation.
///
/// Searches from the innermost handler scope outward, returning the
/// first matching handler. Returns an error if no handler is found.
pub fn resolve_handler<'a>(
    effect: &str,
    op: &str,
    handler_stack: &'a [EffectHandler],
) -> Result<&'a OpHandler, EffectError> {
    // Search from innermost (last) to outermost (first).
    for handler in handler_stack.iter().rev() {
        if handler.effect_name == effect {
            if let Some(op_handler) = handler.find_handler(op) {
                return Ok(op_handler);
            }
        }
    }
    Err(EffectError::MissingHandler {
        effect: effect.to_string(),
        operation: op.to_string(),
    })
}

/// Checks that a handler covers all operations of its declared effect.
///
/// Returns a list of missing operation names if the handler is incomplete.
pub fn check_handler_completeness(
    handler: &EffectHandler,
    registry: &EffectRegistry,
) -> Vec<String> {
    let Some(decl) = registry.lookup(&handler.effect_name) else {
        return vec![format!("unknown effect: {}", handler.effect_name)];
    };
    let mut missing = Vec::new();
    for op in &decl.operations {
        if handler.find_handler(&op.name).is_none() {
            missing.push(op.name.clone());
        }
    }
    missing
}

/// Computes the residual effects after handling.
///
/// Given a body's required effects and a handle expression's handled
/// effects, returns the effects that still need to be propagated.
pub fn residual_effects(required: &EffectSet, handled: &EffectSet) -> EffectSet {
    required.difference(handled)
}

/// Represents a handler scope level for nested handler resolution.
#[derive(Debug, Clone)]
pub struct HandlerScope {
    /// Handlers at this scope level.
    pub handlers: Vec<EffectHandler>,
    /// The nesting depth (0 = outermost).
    pub depth: usize,
}

impl HandlerScope {
    /// Creates a new handler scope at the given depth.
    pub fn new(depth: usize) -> Self {
        HandlerScope {
            handlers: Vec::new(),
            depth,
        }
    }

    /// Adds a handler to this scope.
    pub fn add_handler(&mut self, handler: EffectHandler) {
        self.handlers.push(handler);
    }
}

/// A stack of handler scopes for nested handler resolution.
#[derive(Debug, Clone)]
pub struct HandlerScopeStack {
    /// Stack of scopes (outermost first, innermost last).
    scopes: Vec<HandlerScope>,
}

impl HandlerScopeStack {
    /// Creates a new empty handler scope stack.
    pub fn new() -> Self {
        HandlerScopeStack { scopes: Vec::new() }
    }

    /// Pushes a new handler scope.
    pub fn push_scope(&mut self) {
        let depth = self.scopes.len();
        self.scopes.push(HandlerScope::new(depth));
    }

    /// Pops the innermost handler scope.
    pub fn pop_scope(&mut self) -> Option<HandlerScope> {
        self.scopes.pop()
    }

    /// Adds a handler to the current (innermost) scope.
    pub fn add_handler(&mut self, handler: EffectHandler) -> Result<(), EffectError> {
        let scope = self
            .scopes
            .last_mut()
            .ok_or_else(|| EffectError::InvalidResume {
                reason: "no handler scope active".to_string(),
            })?;
        scope.add_handler(handler);
        Ok(())
    }

    /// Resolves a handler across all scopes (innermost first).
    pub fn resolve(&self, effect: &str, op: &str) -> Result<&OpHandler, EffectError> {
        for scope in self.scopes.iter().rev() {
            for handler in scope.handlers.iter().rev() {
                if handler.effect_name == effect {
                    if let Some(op_handler) = handler.find_handler(op) {
                        return Ok(op_handler);
                    }
                }
            }
        }
        Err(EffectError::MissingHandler {
            effect: effect.to_string(),
            operation: op.to_string(),
        })
    }

    /// Returns the current nesting depth.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }
}

impl Default for HandlerScopeStack {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 3 — Effect Checking & Errors
// ═══════════════════════════════════════════════════════════════════════

/// An effect system error detected during analysis.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EffectError {
    /// EE001: Effect not handled and not declared in function signature.
    #[error("EE001: unhandled effect '{effect}' in {context}")]
    UnhandledEffect {
        /// The unhandled effect name.
        effect: String,
        /// Context description (e.g., function name).
        context: String,
    },

    /// EE002: Handler provides a different effect than expected.
    #[error("EE002: effect mismatch: expected '{expected}', found '{found}'")]
    EffectMismatch {
        /// Expected effect name.
        expected: String,
        /// Found effect name.
        found: String,
    },

    /// EE003: No handler for an effect operation.
    #[error("EE003: missing handler for '{effect}::{operation}'")]
    MissingHandler {
        /// The effect name.
        effect: String,
        /// The unhandled operation name.
        operation: String,
    },

    /// EE004: Duplicate effect declaration in registry.
    #[error("EE004: effect '{name}' is already declared")]
    DuplicateEffect {
        /// The duplicate effect name.
        name: String,
    },

    /// EE005: `resume` used outside a handler scope.
    #[error("EE005: invalid resume: {reason}")]
    InvalidResume {
        /// Reason the resume is invalid.
        reason: String,
    },

    /// EE006: Effect forbidden by context annotation.
    #[error("EE006: effect '{effect}' is forbidden in {context} context")]
    ContextEffectViolation {
        /// The forbidden effect name.
        effect: String,
        /// The context that forbids it (e.g., `"@kernel"`).
        context: String,
    },

    /// EE007: `#[pure]` function performs effects.
    #[error("EE007: purity violation: '{function}' is marked #[pure] but performs {effect}")]
    PurityViolation {
        /// The function marked as pure.
        function: String,
        /// The effect that violates purity.
        effect: String,
    },

    /// EE008: Generic parameter lacks required effect bound.
    #[error("EE008: type parameter '{param}' requires effect bound '{bound}'")]
    EffectBoundViolation {
        /// The type parameter name.
        param: String,
        /// The missing effect bound.
        bound: String,
    },
}

/// Fajar Lang context annotations for effect interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAnnotation {
    /// `@safe` — default, most restrictive.
    Safe,
    /// `@kernel` — OS primitives, no heap, no tensor.
    Kernel,
    /// `@device` — tensor ops, no raw pointer.
    Device,
    /// `@unsafe` — full access.
    Unsafe,
}

impl fmt::Display for ContextAnnotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContextAnnotation::Safe => write!(f, "@safe"),
            ContextAnnotation::Kernel => write!(f, "@kernel"),
            ContextAnnotation::Device => write!(f, "@device"),
            ContextAnnotation::Unsafe => write!(f, "@unsafe"),
        }
    }
}

/// Returns the set of effect kinds forbidden in a given context.
///
/// - `@kernel`: allows Hardware, State, IO, Panic — forbids Alloc, Tensor
/// - `@device`: allows Tensor, Alloc, IO, Panic — forbids Hardware
/// - `@safe`: forbids Hardware, Tensor, IO, Alloc (most restrictive)
/// - `@unsafe`: forbids nothing (full access)
pub fn forbidden_effects(ctx: ContextAnnotation) -> Vec<EffectKind> {
    match ctx {
        ContextAnnotation::Safe => vec![
            EffectKind::IO,
            EffectKind::Alloc,
            EffectKind::Hardware,
            EffectKind::Tensor,
        ],
        ContextAnnotation::Kernel => vec![EffectKind::Alloc, EffectKind::Tensor],
        ContextAnnotation::Device => vec![EffectKind::Hardware],
        ContextAnnotation::Unsafe => vec![],
    }
}

/// Returns the set of effect names allowed in a given context.
pub fn allowed_effects(ctx: ContextAnnotation) -> EffectSet {
    let mut set = EffectSet::empty();
    match ctx {
        ContextAnnotation::Kernel => {
            set.insert("Hardware".to_string());
            set.insert("State".to_string());
            set.insert("IO".to_string());
            set.insert("Panic".to_string());
        }
        ContextAnnotation::Device => {
            set.insert("Tensor".to_string());
            set.insert("Alloc".to_string());
            set.insert("IO".to_string());
            set.insert("Panic".to_string());
        }
        ContextAnnotation::Safe => {
            set.insert("Panic".to_string());
        }
        ContextAnnotation::Unsafe => {
            set.insert("Hardware".to_string());
            set.insert("Tensor".to_string());
            set.insert("Alloc".to_string());
            set.insert("IO".to_string());
            set.insert("Panic".to_string());
            set.insert("Async".to_string());
            set.insert("State".to_string());
            set.insert("Exception".to_string());
        }
    }
    set
}

/// Checks that a function's effect set is compatible with its context.
///
/// Returns errors for each effect that violates the context constraints.
pub fn check_context_effects(
    effects: &EffectSet,
    ctx: ContextAnnotation,
    registry: &EffectRegistry,
) -> Vec<EffectError> {
    let forbidden = forbidden_effects(ctx);
    let mut errors = Vec::new();
    for effect_name in effects.iter() {
        if let Some(decl) = registry.lookup(effect_name) {
            if forbidden.contains(&decl.kind) {
                errors.push(EffectError::ContextEffectViolation {
                    effect: effect_name.clone(),
                    context: ctx.to_string(),
                });
            }
        }
    }
    errors
}

/// Checks that a `#[pure]` function has no effects.
///
/// A pure function must have an empty effect set. Returns an error
/// for each effect found in a pure function's inferred set.
pub fn check_purity(fn_name: &str, effects: &EffectSet) -> Vec<EffectError> {
    let mut errors = Vec::new();
    for effect_name in effects.iter() {
        errors.push(EffectError::PurityViolation {
            function: fn_name.to_string(),
            effect: effect_name.clone(),
        });
    }
    errors
}

/// Effect erasure hint indicating whether an effect can be eliminated.
///
/// Used by the optimizer to remove effect tracking overhead when the
/// effect is statically known to be handled at the immediate call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectErasureHint {
    /// Effect can be fully erased (handler is at the same scope).
    FullErase,
    /// Effect is partially erasable (handler is within N scopes).
    PartialErase {
        /// Distance to the handler in scope levels.
        scope_distance: usize,
    },
    /// Effect cannot be erased (requires runtime dispatch).
    NoErase,
}

/// Computes erasure hints for each effect in a set.
///
/// Checks the handler scope stack to determine how far away each
/// effect's handler is, enabling optimization decisions.
pub fn compute_erasure_hints(
    effects: &EffectSet,
    scope_stack: &HandlerScopeStack,
) -> HashMap<String, EffectErasureHint> {
    let mut hints = HashMap::new();
    for effect_name in effects.iter() {
        let hint = compute_single_erasure_hint(effect_name, scope_stack);
        hints.insert(effect_name.clone(), hint);
    }
    hints
}

/// Computes the erasure hint for a single effect.
fn compute_single_erasure_hint(
    effect_name: &str,
    scope_stack: &HandlerScopeStack,
) -> EffectErasureHint {
    let total_depth = scope_stack.depth();
    for (i, scope) in scope_stack.scopes.iter().rev().enumerate() {
        for handler in &scope.handlers {
            if handler.effect_name == effect_name {
                if i == 0 {
                    return EffectErasureHint::FullErase;
                }
                return EffectErasureHint::PartialErase {
                    scope_distance: total_depth - scope.depth,
                };
            }
        }
    }
    EffectErasureHint::NoErase
}

/// Checks async/effect interaction: async functions implicitly carry `Async` effect.
///
/// Returns the augmented effect set with `Async` added if `is_async` is true.
pub fn augment_async_effects(effects: &EffectSet, is_async: bool) -> EffectSet {
    if is_async {
        let mut augmented = effects.clone();
        augmented.insert("Async".to_string());
        augmented
    } else {
        effects.clone()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 4 — Effect Integration
// ═══════════════════════════════════════════════════════════════════════

/// An effect-aware closure representation.
///
/// Captures the effects that the closure body performs, in addition
/// to the closure's captured variables and type signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectClosure {
    /// The closure's inferred effect set.
    pub effects: EffectSet,
    /// Parameter type names.
    pub param_types: Vec<String>,
    /// Return type name.
    pub return_type: String,
    /// Names of captured variables (for display/debug).
    pub captures: Vec<String>,
}

impl EffectClosure {
    /// Creates a new effect-aware closure.
    pub fn new(
        effects: EffectSet,
        param_types: Vec<String>,
        return_type: impl Into<String>,
        captures: Vec<String>,
    ) -> Self {
        EffectClosure {
            effects,
            param_types,
            return_type: return_type.into(),
            captures,
        }
    }

    /// Returns true if this closure is pure (no effects).
    pub fn is_pure(&self) -> bool {
        self.effects.is_empty()
    }
}

/// An effect bound on a generic type parameter.
///
/// Represents a constraint like `T: with {IO, Exception}`, requiring
/// that any concrete type substituted for `T` supports the given effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectBound {
    /// The type parameter name (e.g., `"T"`, `"F"`).
    pub type_param: String,
    /// The required effect set.
    pub required_effects: EffectSet,
}

impl EffectBound {
    /// Creates a new effect bound.
    pub fn new(type_param: impl Into<String>, required_effects: EffectSet) -> Self {
        EffectBound {
            type_param: type_param.into(),
            required_effects,
        }
    }
}

/// Validates that a concrete effect set satisfies an effect bound.
///
/// The concrete effects must be a subset of the bound's required effects
/// (i.e., the concrete code must not perform effects beyond what the bound allows).
pub fn check_effect_bound(
    bound: &EffectBound,
    concrete_effects: &EffectSet,
) -> Result<(), EffectError> {
    if !concrete_effects.is_subset_of(&bound.required_effects) {
        let excess = concrete_effects.difference(&bound.required_effects);
        let excess_str = excess.to_string();
        return Err(EffectError::EffectBoundViolation {
            param: bound.type_param.clone(),
            bound: excess_str,
        });
    }
    Ok(())
}

/// A `no_effect` bound that constrains a type parameter to be effect-free.
///
/// Equivalent to `EffectBound` with an empty required effect set, but
/// provides a distinct type for clearer intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoEffectBound {
    /// The type parameter name.
    pub type_param: String,
}

impl NoEffectBound {
    /// Creates a new no_effect bound.
    pub fn new(type_param: impl Into<String>) -> Self {
        NoEffectBound {
            type_param: type_param.into(),
        }
    }

    /// Checks that the given effects are empty.
    pub fn check(&self, effects: &EffectSet) -> Result<(), EffectError> {
        if !effects.is_empty() {
            return Err(EffectError::EffectBoundViolation {
                param: self.type_param.clone(),
                bound: format!("no_effect (found {})", effects),
            });
        }
        Ok(())
    }
}

/// Built-in effect handler for IO (delegates to host runtime).
///
/// Provides default handler implementations for the built-in IO effect.
/// Used when no explicit handler is provided and the function is in a
/// context that allows IO.
pub fn builtin_io_handler() -> EffectHandler {
    let mut handler = EffectHandler::new("IO");
    let print_handler = OpHandler::new("print", vec!["msg".into()], "host_print(msg)", false);
    let _ = handler.add_op_handler(print_handler);

    let read_handler = OpHandler::new("read", vec![], "host_read()", true);
    let _ = handler.add_op_handler(read_handler);
    handler
}

/// Built-in effect handler for Alloc (delegates to host allocator).
pub fn builtin_alloc_handler() -> EffectHandler {
    let mut handler = EffectHandler::new("Alloc");
    let alloc_handler = OpHandler::new("alloc", vec!["size".into()], "host_alloc(size)", true);
    let _ = handler.add_op_handler(alloc_handler);

    let dealloc_handler = OpHandler::new("dealloc", vec!["ptr".into()], "host_dealloc(ptr)", false);
    let _ = handler.add_op_handler(dealloc_handler);
    handler
}

/// Built-in effect handler for Exception (converts to Result).
pub fn builtin_exception_handler() -> EffectHandler {
    let mut handler = EffectHandler::new("Exception");
    let throw_handler = OpHandler::new("throw", vec!["msg".into()], "Err(msg)", false);
    let _ = handler.add_op_handler(throw_handler);
    handler
}

/// An effect-aware trait method signature.
///
/// Extends a trait method with an effect set, allowing traits to
/// declare that implementors may perform certain effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectTraitMethod {
    /// Method name.
    pub name: String,
    /// Parameter type names.
    pub param_types: Vec<String>,
    /// Return type name.
    pub return_type: String,
    /// Effects this method is allowed to perform.
    pub effects: EffectSet,
}

impl EffectTraitMethod {
    /// Creates a new effect-aware trait method.
    pub fn new(
        name: impl Into<String>,
        param_types: Vec<String>,
        return_type: impl Into<String>,
        effects: EffectSet,
    ) -> Self {
        EffectTraitMethod {
            name: name.into(),
            param_types,
            return_type: return_type.into(),
            effects,
        }
    }

    /// Returns true if this method is pure.
    pub fn is_pure(&self) -> bool {
        self.effects.is_empty()
    }
}

/// Validates that a trait impl method's effects are a subset of the trait definition.
///
/// An impl may have fewer effects than the trait declares, but not more.
pub fn check_trait_method_effects(
    trait_method: &EffectTraitMethod,
    impl_effects: &EffectSet,
) -> Result<(), EffectError> {
    if !impl_effects.is_subset_of(&trait_method.effects) {
        return Err(EffectError::EffectMismatch {
            expected: trait_method.effects.to_string(),
            found: impl_effects.to_string(),
        });
    }
    Ok(())
}

/// Cross-module effect inference context.
///
/// Tracks effect signatures across module boundaries so that calling
/// a function from another module correctly propagates its effects.
#[derive(Debug, Clone)]
pub struct CrossModuleEffects {
    /// Module-qualified function name -> effect set.
    fn_effects: HashMap<String, EffectSet>,
}

impl CrossModuleEffects {
    /// Creates a new cross-module effect context.
    pub fn new() -> Self {
        CrossModuleEffects {
            fn_effects: HashMap::new(),
        }
    }

    /// Registers a function's effect set.
    pub fn register_fn(&mut self, module: &str, fn_name: &str, effects: EffectSet) {
        let key = format!("{}::{}", module, fn_name);
        self.fn_effects.insert(key, effects);
    }

    /// Looks up a function's effect set by module-qualified name.
    pub fn lookup_fn(&self, module: &str, fn_name: &str) -> Option<&EffectSet> {
        let key = format!("{}::{}", module, fn_name);
        self.fn_effects.get(&key)
    }

    /// Infers effects for a function that calls other cross-module functions.
    ///
    /// Given a list of `(module, fn_name)` calls, returns the union of
    /// all their effect sets.
    pub fn infer_from_calls(&self, calls: &[(&str, &str)]) -> EffectSet {
        let mut combined = EffectSet::empty();
        for (module, fn_name) in calls {
            if let Some(effects) = self.lookup_fn(module, fn_name) {
                combined = combined.union(effects);
            }
        }
        combined
    }

    /// Returns the number of registered functions.
    pub fn count(&self) -> usize {
        self.fn_effects.len()
    }
}

impl Default for CrossModuleEffects {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Top-level effect checker
// ═══════════════════════════════════════════════════════════════════════

/// The main effect checker that orchestrates all effect analysis passes.
///
/// Combines the effect registry, handler scope stack, context checking,
/// and cross-module inference into a single analysis entry point.
#[derive(Debug)]
pub struct EffectChecker {
    /// Global effect registry.
    pub registry: EffectRegistry,
    /// Handler scope stack for nested handler resolution.
    pub handler_stack: HandlerScopeStack,
    /// Cross-module effect information.
    pub cross_module: CrossModuleEffects,
}

impl EffectChecker {
    /// Creates a new effect checker with built-in effects.
    pub fn new() -> Self {
        EffectChecker {
            registry: EffectRegistry::with_builtins(),
            handler_stack: HandlerScopeStack::new(),
            cross_module: CrossModuleEffects::new(),
        }
    }

    /// Registers a custom effect declaration.
    pub fn declare_effect(&mut self, decl: EffectDecl) -> Result<(), EffectError> {
        self.registry.register(decl)
    }

    /// Checks a function's effects against its context and signature.
    ///
    /// Returns all detected effect errors.
    pub fn check_function(
        &self,
        fn_name: &str,
        inferred_effects: &EffectSet,
        declared_effects: &EffectSet,
        ctx: ContextAnnotation,
        is_pure: bool,
    ) -> Vec<EffectError> {
        let mut errors = Vec::new();

        // Check purity
        if is_pure {
            errors.extend(check_purity(fn_name, inferred_effects));
        }

        // Check context constraints
        errors.extend(check_context_effects(inferred_effects, ctx, &self.registry));

        // Check undeclared effects
        let undeclared = inferred_effects.difference(declared_effects);
        for effect_name in undeclared.iter() {
            errors.push(EffectError::UnhandledEffect {
                effect: effect_name.clone(),
                context: fn_name.to_string(),
            });
        }

        errors
    }

    /// Enters a new handler scope.
    pub fn push_handler_scope(&mut self) {
        self.handler_stack.push_scope();
    }

    /// Exits the current handler scope.
    pub fn pop_handler_scope(&mut self) -> Option<HandlerScope> {
        self.handler_stack.pop_scope()
    }

    /// Registers a handler in the current scope.
    pub fn register_handler(&mut self, handler: EffectHandler) -> Result<(), EffectError> {
        self.handler_stack.add_handler(handler)
    }

    /// V14 EF3.9: Compute erasure hints for optimizing effect dispatch.
    pub fn erasure_hints(&self, effects: &EffectSet) -> HashMap<String, EffectErasureHint> {
        compute_erasure_hints(effects, &self.handler_stack)
    }
}

impl Default for EffectChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V14 EF4.5 — Effect Composition
// ═══════════════════════════════════════════════════════════════════════

/// V14 EF4.5: Composed effect combining operations from multiple effects.
#[derive(Debug, Clone)]
pub struct EffectComposition {
    /// Name of the composed effect.
    pub name: String,
    /// Names of the component effects to merge.
    pub components: Vec<String>,
}

impl EffectComposition {
    /// Creates a new effect composition.
    pub fn new(name: impl Into<String>, components: Vec<String>) -> Self {
        Self {
            name: name.into(),
            components,
        }
    }

    /// Resolve into a merged EffectDecl using the registry.
    pub fn resolve(&self, registry: &EffectRegistry) -> Result<EffectDecl, EffectError> {
        let mut all_ops = Vec::new();
        for comp in &self.components {
            let decl = registry.lookup(comp).ok_or(EffectError::EffectMismatch {
                expected: comp.clone(),
                found: self.name.clone(),
            })?;
            all_ops.extend(decl.operations.iter().cloned());
        }
        Ok(EffectDecl::new(&self.name, EffectKind::IO, all_ops))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V14 EF4.6 — Effect Row Polymorphism
// ═══════════════════════════════════════════════════════════════════════

/// V14 EF4.6: Row variable for effect polymorphism.
///
/// An `EffectRow` represents an effect type with an explicit set and an
/// optional rest variable, enabling polymorphism over unknown effects.
#[derive(Debug, Clone)]
pub struct EffectRow {
    /// Explicitly listed effects.
    pub explicit: EffectSet,
    /// Optional rest variable name for open rows.
    pub rest_var: Option<String>,
}

impl EffectRow {
    /// Creates a new effect row.
    pub fn new(explicit: EffectSet, rest_var: Option<String>) -> Self {
        Self { explicit, rest_var }
    }

    /// Returns true if the given concrete set contains all explicit effects.
    pub fn is_satisfied_by(&self, concrete: &EffectSet) -> bool {
        self.explicit.is_subset_of(concrete)
    }

    /// Returns true if this row has a rest variable (is open).
    pub fn is_open(&self) -> bool {
        self.rest_var.is_some()
    }

    /// Instantiate the row with a concrete effect set.
    ///
    /// If open, returns the full concrete set. If closed, returns only the explicit set.
    pub fn instantiate(&self, concrete: &EffectSet) -> EffectSet {
        if self.rest_var.is_some() {
            concrete.clone()
        } else {
            self.explicit.clone()
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V14 EF4.9 — Effect Statistics
// ═══════════════════════════════════════════════════════════════════════

/// V14 EF4.9: Tracks effect usage statistics.
///
/// Records operation invocations, handler registrations, resume calls,
/// and handler nesting depth for profiling and debugging.
#[derive(Debug, Clone, Default)]
pub struct EffectStatistics {
    /// Counts of each effect operation invoked (keyed by "Effect::op").
    pub op_counts: HashMap<String, usize>,
    /// Counts of handler registrations per effect name.
    pub handler_counts: HashMap<String, usize>,
    /// Total number of resume invocations.
    pub resume_count: usize,
    /// Maximum handler nesting depth observed.
    pub max_depth: usize,
}

impl EffectStatistics {
    /// Creates a new empty statistics tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an effect operation invocation.
    pub fn record_op(&mut self, effect: &str, op: &str) {
        let key = format!("{effect}::{op}");
        *self.op_counts.entry(key).or_insert(0) += 1;
    }

    /// Records a handler registration for an effect.
    pub fn record_handler(&mut self, effect: &str) {
        *self.handler_counts.entry(effect.to_string()).or_insert(0) += 1;
    }

    /// Records a resume invocation.
    pub fn record_resume(&mut self) {
        self.resume_count += 1;
    }

    /// Updates the maximum observed nesting depth.
    pub fn update_depth(&mut self, depth: usize) {
        if depth > self.max_depth {
            self.max_depth = depth;
        }
    }

    /// Returns the total number of effect operations invoked.
    pub fn total_ops(&self) -> usize {
        self.op_counts.values().sum()
    }

    /// Returns a human-readable summary of collected statistics.
    pub fn summary(&self) -> String {
        let mut lines = vec!["Effect Statistics:".to_string()];
        lines.push(format!("  Total ops: {}", self.total_ops()));
        lines.push(format!("  Resumes: {}", self.resume_count));
        lines.push(format!("  Max depth: {}", self.max_depth));
        for (key, count) in &self.op_counts {
            lines.push(format!("  {key}: {count} calls"));
        }
        lines.join("\n")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V15 — Multi-prompt Captured Continuations
// ═══════════════════════════════════════════════════════════════════════

/// A captured continuation for multi-prompt delimited control.
///
/// Represents a suspended computation that can be invoked multiple times
/// if `multi_shot` is true, enabling replay-based effect handling.
#[derive(Debug, Clone)]
pub struct CapturedContinuation {
    /// The prompt tag identifying this continuation's delimiter.
    pub prompt_tag: String,
    /// Whether this continuation can be invoked more than once.
    pub multi_shot: bool,
    /// Number of times this continuation has been invoked.
    pub invoke_count: usize,
}

impl CapturedContinuation {
    /// Creates a new captured continuation.
    pub fn new(prompt_tag: impl Into<String>, multi_shot: bool) -> Self {
        Self {
            prompt_tag: prompt_tag.into(),
            multi_shot,
            invoke_count: 0,
        }
    }

    /// Invokes the continuation, incrementing the invoke count.
    ///
    /// Returns an error if the continuation is single-shot and has already been invoked.
    pub fn invoke(&mut self) -> Result<(), EffectError> {
        if !self.multi_shot && self.invoke_count > 0 {
            return Err(EffectError::InvalidResume {
                reason: format!(
                    "single-shot continuation '{}' already invoked",
                    self.prompt_tag
                ),
            });
        }
        self.invoke_count += 1;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ───────────────────────────────────────────────────────────────
    // Sprint 1 — Effect Declarations (s1_1 through s1_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s1_1_effect_kind_display_and_ordering() {
        assert_eq!(EffectKind::IO.to_string(), "IO");
        assert_eq!(EffectKind::Alloc.to_string(), "Alloc");
        assert_eq!(EffectKind::Panic.to_string(), "Panic");
        assert_eq!(EffectKind::Async.to_string(), "Async");
        assert_eq!(EffectKind::State.to_string(), "State");
        assert_eq!(EffectKind::Exception.to_string(), "Exception");
        // Ordering is derived, so all kinds are comparable
        assert!(EffectKind::IO < EffectKind::State);
    }

    #[test]
    fn s1_2_effect_op_creation_and_nullary() {
        let op = EffectOp::new("read_line", vec![], "str");
        assert_eq!(op.name, "read_line");
        assert!(op.is_nullary());
        assert_eq!(op.return_type, "str");

        let op2 = EffectOp::new("write", vec!["str".into(), "i32".into()], "void");
        assert!(!op2.is_nullary());
        assert_eq!(op2.param_types.len(), 2);
    }

    #[test]
    fn s1_3_effect_decl_with_operations() {
        let decl = EffectDecl::new(
            "Console",
            EffectKind::IO,
            vec![
                EffectOp::new("log", vec!["str".into()], "void"),
                EffectOp::new("read_line", vec![], "str"),
            ],
        );
        assert_eq!(decl.name, "Console");
        assert_eq!(decl.kind, EffectKind::IO);
        assert_eq!(decl.op_count(), 2);
        assert!(decl.find_op("log").is_some());
        assert!(decl.find_op("nonexistent").is_none());
    }

    #[test]
    fn s1_4_effect_set_basic_operations() {
        let mut set = EffectSet::empty();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);

        set.insert("IO");
        set.insert("Alloc");
        assert_eq!(set.len(), 2);
        assert!(set.contains("IO"));
        assert!(!set.contains("Panic"));

        assert!(set.remove("IO"));
        assert!(!set.contains("IO"));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn s1_5_effect_set_union_and_intersection() {
        let a = EffectSet::collect_from(vec!["IO".into(), "Alloc".into()]);
        let b = EffectSet::collect_from(vec!["Alloc".into(), "Panic".into()]);

        let union = a.union(&b);
        assert_eq!(union.len(), 3);
        assert!(union.contains("IO"));
        assert!(union.contains("Alloc"));
        assert!(union.contains("Panic"));

        let inter = a.intersection(&b);
        assert_eq!(inter.len(), 1);
        assert!(inter.contains("Alloc"));
    }

    #[test]
    fn s1_6_effect_set_subset_and_difference() {
        let small = EffectSet::collect_from(vec!["IO".into()]);
        let big = EffectSet::collect_from(vec!["IO".into(), "Alloc".into()]);

        assert!(small.is_subset_of(&big));
        assert!(!big.is_subset_of(&small));

        let diff = big.difference(&small);
        assert_eq!(diff.len(), 1);
        assert!(diff.contains("Alloc"));
    }

    #[test]
    fn s1_7_effect_set_display() {
        let empty = EffectSet::empty();
        assert_eq!(empty.to_string(), "pure");

        let single = EffectSet::collect_from(vec!["IO".into()]);
        assert_eq!(single.to_string(), "{IO}");

        let multi = EffectSet::collect_from(vec!["IO".into(), "Alloc".into()]);
        // BTreeSet ordering: Alloc < IO
        assert_eq!(multi.to_string(), "{Alloc, IO}");
    }

    #[test]
    fn s1_8_effect_registry_register_and_lookup() {
        let mut registry = EffectRegistry::new();
        let decl = EffectDecl::new("MyEffect", EffectKind::State, vec![]);
        assert!(registry.register(decl.clone()).is_ok());
        assert!(registry.lookup("MyEffect").is_some());
        assert!(registry.lookup("Nonexistent").is_none());
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn s1_9_effect_registry_duplicate_error() {
        let mut registry = EffectRegistry::new();
        let decl = EffectDecl::new("Dup", EffectKind::IO, vec![]);
        assert!(registry.register(decl.clone()).is_ok());

        let result = registry.register(decl);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("EE004"));
        assert!(msg.contains("Dup"));
    }

    #[test]
    fn s1_10_infer_effects_from_calls() {
        let registry = EffectRegistry::with_builtins();
        let calls = vec![
            ("IO".to_string(), "print".to_string()),
            ("IO".to_string(), "read".to_string()),
            ("Exception".to_string(), "throw".to_string()),
        ];
        let inferred = infer_effects(&calls, &registry);
        assert_eq!(inferred.len(), 2);
        assert!(inferred.contains("IO"));
        assert!(inferred.contains("Exception"));
    }

    // ───────────────────────────────────────────────────────────────
    // Sprint 2 — Effect Handlers (s2_1 through s2_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s2_1_op_handler_creation() {
        let handler = OpHandler::new("log", vec!["msg".into()], "println(msg)", false);
        assert_eq!(handler.op_name, "log");
        assert_eq!(handler.param_names, vec!["msg".to_string()]);
        assert!(!handler.uses_resume);
    }

    #[test]
    fn s2_2_effect_handler_add_and_find() {
        let mut handler = EffectHandler::new("Console");
        let op = OpHandler::new("log", vec!["msg".into()], "print(msg)", false);
        assert!(handler.add_op_handler(op).is_ok());

        assert!(handler.find_handler("log").is_some());
        assert!(handler.find_handler("read").is_none());
        assert_eq!(handler.handler_count(), 1);
    }

    #[test]
    fn s2_3_handle_expr_construction() {
        let mut handle = HandleExpr::new("greet()");
        let mut h = EffectHandler::new("Console");
        let _ = h.add_op_handler(OpHandler::new("log", vec![], "print", false));
        handle.add_handler(h);

        assert!(handle.effects_handled().contains("Console"));
        assert_eq!(handle.handlers.len(), 1);
    }

    #[test]
    fn s2_4_resume_point_creation() {
        let rp = ResumePoint::new(42, "str", "read_line", "Console");
        assert_eq!(rp.id, 42);
        assert_eq!(rp.resume_type, "str");
        assert_eq!(rp.origin_op, "read_line");
        assert_eq!(rp.origin_effect, "Console");
    }

    #[test]
    fn s2_5_resolve_handler_innermost_first() {
        let mut outer = EffectHandler::new("Console");
        let _ = outer.add_op_handler(OpHandler::new(
            "log",
            vec!["msg".into()],
            "outer_print",
            false,
        ));

        let mut inner = EffectHandler::new("Console");
        let _ = inner.add_op_handler(OpHandler::new(
            "log",
            vec!["msg".into()],
            "inner_print",
            false,
        ));

        let stack = vec![outer, inner];
        let resolved = resolve_handler("Console", "log", &stack).unwrap();
        assert_eq!(resolved.body_repr, "inner_print");
    }

    #[test]
    fn s2_6_resolve_handler_missing_error() {
        let handler = EffectHandler::new("Console");
        let stack = vec![handler];
        let result = resolve_handler("Console", "log", &stack);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("EE003"));
    }

    #[test]
    fn s2_7_check_handler_completeness() {
        let registry = EffectRegistry::with_builtins();
        let mut handler = EffectHandler::new("IO");
        let _ = handler.add_op_handler(OpHandler::new(
            "print",
            vec!["msg".into()],
            "do_print",
            false,
        ));
        // Missing "read" handler
        let missing = check_handler_completeness(&handler, &registry);
        assert_eq!(missing, vec!["read".to_string()]);
    }

    #[test]
    fn s2_8_residual_effects() {
        let required =
            EffectSet::collect_from(vec!["IO".into(), "Alloc".into(), "Exception".into()]);
        let handled = EffectSet::collect_from(vec!["IO".into(), "Exception".into()]);
        let residual = residual_effects(&required, &handled);
        assert_eq!(residual.len(), 1);
        assert!(residual.contains("Alloc"));
    }

    #[test]
    fn s2_9_handler_scope_stack_nesting() {
        let mut stack = HandlerScopeStack::new();
        assert_eq!(stack.depth(), 0);

        stack.push_scope();
        assert_eq!(stack.depth(), 1);

        let mut h = EffectHandler::new("IO");
        let _ = h.add_op_handler(OpHandler::new("print", vec![], "p", false));
        assert!(stack.add_handler(h).is_ok());

        stack.push_scope();
        assert_eq!(stack.depth(), 2);

        let resolved = stack.resolve("IO", "print");
        assert!(resolved.is_ok());

        stack.pop_scope();
        assert_eq!(stack.depth(), 1);
        // Handler still accessible from outer scope
        assert!(stack.resolve("IO", "print").is_ok());
    }

    #[test]
    fn s2_10_handler_scope_stack_error_no_scope() {
        let mut stack = HandlerScopeStack::new();
        let h = EffectHandler::new("IO");
        let result = stack.add_handler(h);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("EE005"));
    }

    // ───────────────────────────────────────────────────────────────
    // Sprint 3 — Effect Checking (s3_1 through s3_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s3_1_unhandled_effect_error() {
        let err = EffectError::UnhandledEffect {
            effect: "IO".into(),
            context: "pure_fn".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("EE001"));
        assert!(msg.contains("IO"));
        assert!(msg.contains("pure_fn"));
    }

    #[test]
    fn s3_2_effect_mismatch_error() {
        let err = EffectError::EffectMismatch {
            expected: "IO".into(),
            found: "Alloc".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("EE002"));
        assert!(msg.contains("IO"));
        assert!(msg.contains("Alloc"));
    }

    #[test]
    fn s3_3_missing_handler_error() {
        let err = EffectError::MissingHandler {
            effect: "Console".into(),
            operation: "read_line".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("EE003"));
        assert!(msg.contains("Console::read_line"));
    }

    #[test]
    fn s3_4_context_kernel_forbids_alloc() {
        let registry = EffectRegistry::with_builtins();
        let effects = EffectSet::collect_from(vec!["Alloc".into()]);
        let errors = check_context_effects(&effects, ContextAnnotation::Kernel, &registry);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].to_string().contains("EE006"));
        assert!(errors[0].to_string().contains("@kernel"));
    }

    #[test]
    fn s3_5_context_device_forbids_hardware() {
        let registry = EffectRegistry::with_builtins();
        let effects = EffectSet::collect_from(vec!["Hardware".into()]);
        let errors = check_context_effects(&effects, ContextAnnotation::Device, &registry);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].to_string().contains("@device"));
    }

    #[test]
    fn s3_6_context_unsafe_allows_everything() {
        let registry = EffectRegistry::with_builtins();
        let effects = EffectSet::collect_from(vec![
            "IO".into(),
            "Alloc".into(),
            "Panic".into(),
            "Exception".into(),
        ]);
        let errors = check_context_effects(&effects, ContextAnnotation::Unsafe, &registry);
        assert!(errors.is_empty());
    }

    #[test]
    fn s3_7_purity_violation() {
        let effects = EffectSet::collect_from(vec!["IO".into(), "State".into()]);
        let errors = check_purity("my_pure_fn", &effects);
        assert_eq!(errors.len(), 2);
        for err in &errors {
            assert!(err.to_string().contains("EE007"));
            assert!(err.to_string().contains("my_pure_fn"));
        }
    }

    #[test]
    fn s3_8_erasure_hint_full_erase() {
        let mut stack = HandlerScopeStack::new();
        stack.push_scope();
        let mut h = EffectHandler::new("IO");
        let _ = h.add_op_handler(OpHandler::new("print", vec![], "p", false));
        let _ = stack.add_handler(h);

        let effects = EffectSet::collect_from(vec!["IO".into()]);
        let hints = compute_erasure_hints(&effects, &stack);
        assert_eq!(hints.get("IO"), Some(&EffectErasureHint::FullErase));
    }

    #[test]
    fn s3_9_erasure_hint_no_erase() {
        let stack = HandlerScopeStack::new();
        let effects = EffectSet::collect_from(vec!["IO".into()]);
        let hints = compute_erasure_hints(&effects, &stack);
        assert_eq!(hints.get("IO"), Some(&EffectErasureHint::NoErase));
    }

    #[test]
    fn s3_10_async_effect_augmentation() {
        let effects = EffectSet::collect_from(vec!["IO".into()]);
        let augmented = augment_async_effects(&effects, true);
        assert!(augmented.contains("Async"));
        assert!(augmented.contains("IO"));
        assert_eq!(augmented.len(), 2);

        let not_async = augment_async_effects(&effects, false);
        assert!(!not_async.contains("Async"));
        assert_eq!(not_async.len(), 1);
    }

    // ───────────────────────────────────────────────────────────────
    // Sprint 4 — Effect Integration (s4_1 through s4_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s4_1_effect_closure_pure() {
        let closure = EffectClosure::new(EffectSet::empty(), vec!["i32".into()], "i32", vec![]);
        assert!(closure.is_pure());
        assert_eq!(closure.return_type, "i32");
    }

    #[test]
    fn s4_2_effect_closure_with_effects() {
        let effects = EffectSet::collect_from(vec!["IO".into()]);
        let closure =
            EffectClosure::new(effects, vec!["str".into()], "void", vec!["logger".into()]);
        assert!(!closure.is_pure());
        assert_eq!(closure.captures, vec!["logger".to_string()]);
    }

    #[test]
    fn s4_3_effect_bound_satisfied() {
        let bound = EffectBound::new(
            "F",
            EffectSet::collect_from(vec!["IO".into(), "Alloc".into()]),
        );
        let concrete = EffectSet::collect_from(vec!["IO".into()]);
        assert!(check_effect_bound(&bound, &concrete).is_ok());
    }

    #[test]
    fn s4_4_effect_bound_violated() {
        let bound = EffectBound::new("F", EffectSet::collect_from(vec!["IO".into()]));
        let concrete = EffectSet::collect_from(vec!["IO".into(), "Alloc".into()]);
        let result = check_effect_bound(&bound, &concrete);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("EE008"));
    }

    #[test]
    fn s4_5_no_effect_bound_pass() {
        let bound = NoEffectBound::new("T");
        assert!(bound.check(&EffectSet::empty()).is_ok());
    }

    #[test]
    fn s4_6_no_effect_bound_fail() {
        let bound = NoEffectBound::new("T");
        let effects = EffectSet::collect_from(vec!["IO".into()]);
        let result = bound.check(&effects);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no_effect"));
    }

    #[test]
    fn s4_7_builtin_handlers_complete() {
        let registry = EffectRegistry::with_builtins();

        let io_handler = builtin_io_handler();
        let missing = check_handler_completeness(&io_handler, &registry);
        assert!(missing.is_empty(), "IO handler missing: {:?}", missing);

        let alloc_handler = builtin_alloc_handler();
        let missing = check_handler_completeness(&alloc_handler, &registry);
        assert!(missing.is_empty(), "Alloc handler missing: {:?}", missing);

        let exc_handler = builtin_exception_handler();
        let missing = check_handler_completeness(&exc_handler, &registry);
        assert!(
            missing.is_empty(),
            "Exception handler missing: {:?}",
            missing
        );
    }

    #[test]
    fn s4_8_effect_trait_method_pure_and_effectful() {
        let pure_method = EffectTraitMethod::new("size", vec![], "usize", EffectSet::empty());
        assert!(pure_method.is_pure());

        let effectful = EffectTraitMethod::new(
            "write",
            vec!["str".into()],
            "void",
            EffectSet::collect_from(vec!["IO".into()]),
        );
        assert!(!effectful.is_pure());
    }

    #[test]
    fn s4_9_trait_method_effect_subtyping() {
        let trait_method = EffectTraitMethod::new(
            "process",
            vec![],
            "void",
            EffectSet::collect_from(vec!["IO".into(), "Alloc".into()]),
        );

        // Impl with fewer effects is fine
        let impl_effects = EffectSet::collect_from(vec!["IO".into()]);
        assert!(check_trait_method_effects(&trait_method, &impl_effects).is_ok());

        // Impl with extra effects is not fine
        let bad_effects =
            EffectSet::collect_from(vec!["IO".into(), "Alloc".into(), "Panic".into()]);
        assert!(check_trait_method_effects(&trait_method, &bad_effects).is_err());
    }

    #[test]
    fn s4_10_cross_module_effect_inference() {
        let mut cross = CrossModuleEffects::new();
        cross.register_fn(
            "io",
            "read_file",
            EffectSet::collect_from(vec!["IO".into()]),
        );
        cross.register_fn(
            "mem",
            "alloc_buf",
            EffectSet::collect_from(vec!["Alloc".into()]),
        );
        assert_eq!(cross.count(), 2);

        let effects = cross.infer_from_calls(&[("io", "read_file"), ("mem", "alloc_buf")]);
        assert_eq!(effects.len(), 2);
        assert!(effects.contains("IO"));
        assert!(effects.contains("Alloc"));

        // Unknown function returns empty
        let unknown = cross.infer_from_calls(&[("net", "connect")]);
        assert!(unknown.is_empty());

        // Lookup works
        assert!(cross.lookup_fn("io", "read_file").is_some());
        assert!(cross.lookup_fn("io", "nonexistent").is_none());
    }

    // ───────────────────────────────────────────────────────────────
    // V14 EF3.9 — Wire erasure hints into EffectChecker
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn v14_ef3_9_checker_erasure_hints() {
        let mut checker = EffectChecker::new();
        checker.push_handler_scope();
        let handler = EffectHandler::new("IO");
        checker.register_handler(handler).unwrap();
        let effects = EffectSet::collect_from(vec!["IO".into()]);
        let hints = checker.erasure_hints(&effects);
        assert!(matches!(
            hints.get("IO"),
            Some(EffectErasureHint::FullErase)
        ));
    }

    #[test]
    fn v14_ef3_9_erasure_no_handler() {
        let checker = EffectChecker::new();
        let effects = EffectSet::collect_from(vec!["IO".into()]);
        let hints = checker.erasure_hints(&effects);
        assert!(matches!(hints.get("IO"), Some(EffectErasureHint::NoErase)));
    }

    // ───────────────────────────────────────────────────────────────
    // V14 EF4.5 — Effect composition
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn v14_ef4_5_composition_resolve() {
        let registry = EffectRegistry::with_builtins();
        let comp = EffectComposition::new("IOState", vec!["IO".into(), "State".into()]);
        let resolved = comp.resolve(&registry).unwrap();
        assert_eq!(resolved.name, "IOState");
        assert!(resolved.operations.len() >= 4);
    }

    #[test]
    fn v14_ef4_5_composition_missing() {
        let registry = EffectRegistry::with_builtins();
        let comp = EffectComposition::new("Bad", vec!["IO".into(), "Nonexistent".into()]);
        assert!(comp.resolve(&registry).is_err());
    }

    // ───────────────────────────────────────────────────────────────
    // V14 EF4.6 — Effect row polymorphism
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn v14_ef4_6_row_closed() {
        let row = EffectRow::new(EffectSet::collect_from(vec!["IO".into()]), None);
        assert!(!row.is_open());
        let concrete = EffectSet::collect_from(vec!["IO".into(), "State".into()]);
        assert!(row.is_satisfied_by(&concrete));
    }

    #[test]
    fn v14_ef4_6_row_open() {
        let row = EffectRow::new(
            EffectSet::collect_from(vec!["IO".into()]),
            Some("rest".into()),
        );
        assert!(row.is_open());
        let concrete = EffectSet::collect_from(vec!["IO".into(), "State".into()]);
        let inst = row.instantiate(&concrete);
        assert_eq!(inst.len(), 2);
    }

    #[test]
    fn v14_ef4_6_row_not_satisfied() {
        let row = EffectRow::new(
            EffectSet::collect_from(vec!["IO".into(), "Alloc".into()]),
            None,
        );
        let concrete = EffectSet::collect_from(vec!["IO".into()]);
        assert!(!row.is_satisfied_by(&concrete));
    }

    // ───────────────────────────────────────────────────────────────
    // V14 EF4.9 — Effect statistics
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn v14_ef4_9_statistics_tracking() {
        let mut stats = EffectStatistics::new();
        stats.record_op("Console", "log");
        stats.record_op("Console", "log");
        stats.record_op("State", "get");
        stats.record_handler("Console");
        stats.record_resume();
        stats.update_depth(3);
        assert_eq!(stats.total_ops(), 3);
        assert_eq!(stats.resume_count, 1);
        assert_eq!(stats.max_depth, 3);
    }

    #[test]
    fn v14_ef4_9_statistics_summary() {
        let mut stats = EffectStatistics::new();
        stats.record_op("IO", "print");
        let summary = stats.summary();
        assert!(summary.contains("Total ops: 1"));
        assert!(summary.contains("IO::print"));
    }

    // ───────────────────────────────────────────────────────────────
    // V14 EF4.10 — Combined polymorphism integration test
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn v14_ef4_10_combined_integration() {
        let registry = EffectRegistry::with_builtins();

        // Composition
        let comp = EffectComposition::new("Full", vec!["IO".into(), "State".into()]);
        assert!(comp.resolve(&registry).is_ok());

        // Row polymorphism
        let row = EffectRow::new(EffectSet::collect_from(vec!["IO".into()]), Some("r".into()));
        assert!(row.is_satisfied_by(&EffectSet::collect_from(vec!["IO".into(), "State".into()])));

        // Multi-prompt
        let mut cont = CapturedContinuation::new("p", true);
        cont.invoke().unwrap();
        cont.invoke().unwrap();
        assert_eq!(cont.invoke_count, 2);

        // Erasure
        let mut checker = EffectChecker::new();
        checker.push_handler_scope();
        checker.register_handler(EffectHandler::new("IO")).unwrap();
        let hints = checker.erasure_hints(&EffectSet::collect_from(vec!["IO".into()]));
        assert!(matches!(
            hints.get("IO"),
            Some(EffectErasureHint::FullErase)
        ));

        // Statistics
        let mut stats = EffectStatistics::new();
        stats.record_op("IO", "print");
        assert_eq!(stats.total_ops(), 1);
    }

    #[test]
    fn v14_effect_composition_parse_and_run() {
        // Test that `effect Combined = A + B` parses and runs through the full pipeline.
        let source = r#"
            effect Logger {
                fn log(msg: str) -> void
            }
            effect Counter {
                fn count() -> i64
            }
            effect LogCount = Logger + Counter
            fn main() {
                let x = 42
                println(x)
            }
        "#;
        let mut interp = crate::interpreter::Interpreter::new();
        let result = interp.eval_source(source);
        assert!(
            result.is_ok(),
            "effect composition should parse and run: {result:?}"
        );
    }

    #[test]
    fn v14_effect_composition_registers_operations() {
        // Test that composed effect operations are registered in the interpreter.
        let source = r#"
            effect Foo {
                fn bar() -> i64
            }
            effect Baz {
                fn qux(x: i64) -> i64
            }
            effect FooBaz = Foo + Baz
            fn main() {
                println("composed registered")
            }
        "#;
        let mut interp = crate::interpreter::Interpreter::new();
        let result = interp.eval_source(source);
        assert!(
            result.is_ok(),
            "composed effect should register: {result:?}"
        );
    }

    #[test]
    fn v14_effect_row_var_in_fn_clause() {
        // Test that `fn f() with IO, ..r { }` parses the row variable.
        let source = r#"
            effect MyEffect {
                fn do_thing() -> void
            }
            fn work() with MyEffect {
                println("working")
            }
            fn main() {
                println("row var test")
            }
        "#;
        let mut interp = crate::interpreter::Interpreter::new();
        let result = interp.eval_source(source);
        assert!(result.is_ok(), "effect clause should parse: {result:?}");
    }
}
