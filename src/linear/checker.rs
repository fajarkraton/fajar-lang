//! Linearity checker — tracks usage of linear bindings, enforces exactly-once
//! semantics, handles control flow branches, loops, and function parameters.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S5.1 / S5.2: Linear Annotation & AST Representation
// ═══════════════════════════════════════════════════════════════════════

/// Linearity qualifier for types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Linearity {
    /// Must be used exactly once — cannot be dropped or duplicated.
    Linear,
    /// Can be used at most once (moved) — current Fajar Lang default.
    Affine,
    /// Can be used any number of times — Copy types.
    Unrestricted,
}

impl fmt::Display for Linearity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Linearity::Linear => write!(f, "linear"),
            Linearity::Affine => write!(f, "affine"),
            Linearity::Unrestricted => write!(f, "unrestricted"),
        }
    }
}

impl Linearity {
    /// Returns `true` if this linearity requires exactly-once usage.
    pub fn is_linear(&self) -> bool {
        *self == Linearity::Linear
    }

    /// Returns `true` if this linearity forbids duplication.
    pub fn forbids_copy(&self) -> bool {
        matches!(self, Linearity::Linear | Linearity::Affine)
    }

    /// Returns `true` if this linearity requires consumption.
    pub fn requires_consumption(&self) -> bool {
        *self == Linearity::Linear
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S5.3: Usage Tracking
// ═══════════════════════════════════════════════════════════════════════

/// Tracks the usage count and state of a linear binding.
#[derive(Debug, Clone)]
pub struct LinearBinding {
    /// Variable name.
    pub name: String,
    /// Type name.
    pub type_name: String,
    /// Linearity qualifier.
    pub linearity: Linearity,
    /// Number of times this binding has been used.
    pub use_count: u32,
    /// Whether the binding has been consumed (moved into a consuming call).
    pub consumed: bool,
    /// Whether the binding was returned from the function.
    pub returned: bool,
    /// Scope depth where the binding was created.
    pub scope_depth: u32,
}

impl LinearBinding {
    /// Creates a new linear binding with zero uses.
    pub fn new(name: &str, type_name: &str, linearity: Linearity, scope_depth: u32) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            linearity,
            use_count: 0,
            consumed: false,
            returned: false,
            scope_depth,
        }
    }

    /// Records a use of this binding.
    pub fn record_use(&mut self) {
        self.use_count += 1;
    }

    /// Marks this binding as consumed.
    pub fn mark_consumed(&mut self) {
        self.consumed = true;
        self.use_count += 1;
    }

    /// Marks this binding as returned.
    pub fn mark_returned(&mut self) {
        self.returned = true;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S5.4 / S5.5: Linear Errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors related to linear type violations.
#[derive(Debug, Clone, PartialEq)]
pub enum LinearError {
    /// A linear value was never consumed (S5.4).
    UnusedLinear {
        /// Variable name.
        name: String,
        /// Type name.
        type_name: String,
    },
    /// A linear value was used more than once (S5.5).
    DuplicateUse {
        /// Variable name.
        name: String,
        /// Type name.
        type_name: String,
        /// Number of uses.
        use_count: u32,
    },
    /// A linear value was not consumed in all control flow branches (S5.7).
    InconsistentBranch {
        /// Variable name.
        name: String,
        /// Branches where it was consumed.
        consumed_in: Vec<String>,
        /// Branches where it was not consumed.
        not_consumed_in: Vec<String>,
    },
    /// A linear value was used inside a loop without fresh rebinding (S5.8).
    LinearInLoop {
        /// Variable name.
        name: String,
        /// Type name.
        type_name: String,
    },
    /// A linear function parameter was not consumed in the body (S5.9).
    UnconsumedParam {
        /// Parameter name.
        name: String,
        /// Type name.
        type_name: String,
    },
    /// A function returning a linear type had its result discarded.
    DiscardedLinearResult {
        /// Function name.
        fn_name: String,
        /// Return type.
        return_type: String,
    },
    /// A linear value leaked — went out of scope without consumption.
    Leaked {
        /// Variable name.
        name: String,
        /// Type name.
        type_name: String,
    },
}

impl fmt::Display for LinearError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinearError::UnusedLinear { name, type_name } => {
                write!(
                    f,
                    "linear value `{name}` of type `{type_name}` was never consumed"
                )
            }
            LinearError::DuplicateUse {
                name,
                type_name,
                use_count,
            } => {
                write!(
                    f,
                    "linear value `{name}` of type `{type_name}` used {use_count} times (must be exactly 1)"
                )
            }
            LinearError::InconsistentBranch {
                name,
                consumed_in,
                not_consumed_in,
            } => {
                write!(
                    f,
                    "linear value `{name}` consumed in [{}] but not in [{}]",
                    consumed_in.join(", "),
                    not_consumed_in.join(", ")
                )
            }
            LinearError::LinearInLoop { name, type_name } => {
                write!(
                    f,
                    "linear value `{name}` of type `{type_name}` cannot be used in a loop"
                )
            }
            LinearError::UnconsumedParam { name, type_name } => {
                write!(
                    f,
                    "linear parameter `{name}` of type `{type_name}` not consumed in function body"
                )
            }
            LinearError::DiscardedLinearResult {
                fn_name,
                return_type,
            } => {
                write!(
                    f,
                    "result of `{fn_name}()` returning linear type `{return_type}` was discarded"
                )
            }
            LinearError::Leaked { name, type_name } => {
                write!(
                    f,
                    "linear value `{name}` of type `{type_name}` leaked (went out of scope)"
                )
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S5.6: Consume Syntax
// ═══════════════════════════════════════════════════════════════════════

/// Represents the `consume(handle)` built-in operation.
#[derive(Debug, Clone)]
pub struct ConsumeOp {
    /// The variable being consumed.
    pub target: String,
    /// Whether to extract inner data.
    pub extract: bool,
}

/// Checks that a consume target is valid (must be linear and unconsumed).
pub fn validate_consume(
    bindings: &HashMap<String, LinearBinding>,
    target: &str,
) -> Result<(), LinearError> {
    match bindings.get(target) {
        Some(binding) if binding.consumed => Err(LinearError::DuplicateUse {
            name: target.into(),
            type_name: binding.type_name.clone(),
            use_count: binding.use_count + 1,
        }),
        Some(binding) if !binding.linearity.is_linear() => Ok(()), // non-linear, fine
        Some(_) => Ok(()),
        None => Ok(()), // not tracked — assume non-linear
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S5.7 / S5.8 / S5.9: Control Flow, Loops, Parameters
// ═══════════════════════════════════════════════════════════════════════

/// The linearity checker context.
#[derive(Debug, Clone)]
pub struct LinearityChecker {
    /// All tracked linear bindings.
    pub bindings: HashMap<String, LinearBinding>,
    /// Current scope depth.
    pub scope_depth: u32,
    /// Whether we are inside a loop.
    pub in_loop: bool,
    /// Collected errors.
    pub errors: Vec<LinearError>,
}

impl LinearityChecker {
    /// Creates a new linearity checker.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            scope_depth: 0,
            in_loop: false,
            errors: Vec::new(),
        }
    }

    /// Declares a new linear binding.
    pub fn declare(&mut self, name: &str, type_name: &str, linearity: Linearity) {
        self.bindings.insert(
            name.into(),
            LinearBinding::new(name, type_name, linearity, self.scope_depth),
        );
    }

    /// Records a use of a binding. Checks for duplicate use of linear values.
    pub fn record_use(&mut self, name: &str) {
        if let Some(binding) = self.bindings.get_mut(name) {
            if binding.linearity.is_linear() {
                if binding.consumed {
                    self.errors.push(LinearError::DuplicateUse {
                        name: name.into(),
                        type_name: binding.type_name.clone(),
                        use_count: binding.use_count + 1,
                    });
                }
                if self.in_loop {
                    self.errors.push(LinearError::LinearInLoop {
                        name: name.into(),
                        type_name: binding.type_name.clone(),
                    });
                }
            }
            binding.record_use();
        }
    }

    /// Records consumption of a linear value.
    pub fn consume(&mut self, name: &str) {
        if let Some(binding) = self.bindings.get_mut(name) {
            if binding.consumed && binding.linearity.is_linear() {
                self.errors.push(LinearError::DuplicateUse {
                    name: name.into(),
                    type_name: binding.type_name.clone(),
                    use_count: binding.use_count + 1,
                });
            }
            binding.mark_consumed();
        }
    }

    /// Records a return of a linear value.
    pub fn mark_returned(&mut self, name: &str) {
        if let Some(binding) = self.bindings.get_mut(name) {
            binding.mark_returned();
        }
    }

    /// Enters a new scope.
    pub fn enter_scope(&mut self) {
        self.scope_depth += 1;
    }

    /// Exits a scope, checking all linear bindings at this depth.
    pub fn exit_scope(&mut self) {
        let depth = self.scope_depth;
        let leaked: Vec<(String, String)> = self
            .bindings
            .iter()
            .filter(|(_, b)| {
                b.scope_depth == depth && b.linearity.is_linear() && !b.consumed && !b.returned
            })
            .map(|(_, b)| (b.name.clone(), b.type_name.clone()))
            .collect();

        for (name, type_name) in leaked {
            self.errors
                .push(LinearError::UnusedLinear { name, type_name });
        }

        self.bindings.retain(|_, b| b.scope_depth < depth);
        self.scope_depth = self.scope_depth.saturating_sub(1);
    }

    /// Enters a loop context.
    pub fn enter_loop(&mut self) {
        self.in_loop = true;
    }

    /// Exits a loop context.
    pub fn exit_loop(&mut self) {
        self.in_loop = false;
    }

    /// Checks that all linear function parameters were consumed.
    pub fn check_params_consumed(&mut self, param_names: &[&str]) {
        for name in param_names {
            if let Some(binding) = self.bindings.get(*name) {
                if binding.linearity.is_linear() && !binding.consumed && !binding.returned {
                    self.errors.push(LinearError::UnconsumedParam {
                        name: (*name).into(),
                        type_name: binding.type_name.clone(),
                    });
                }
            }
        }
    }

    /// Checks consistency of linear consumption across two branches.
    pub fn check_branch_consistency(
        &mut self,
        before: &HashMap<String, bool>,
        then_consumed: &HashMap<String, bool>,
        else_consumed: &HashMap<String, bool>,
    ) {
        for name in before.keys() {
            if let Some(binding) = self.bindings.get(name) {
                if !binding.linearity.is_linear() {
                    continue;
                }
            }
            let in_then = then_consumed.get(name).copied().unwrap_or(false);
            let in_else = else_consumed.get(name).copied().unwrap_or(false);

            if in_then != in_else {
                let mut consumed_in = Vec::new();
                let mut not_consumed_in = Vec::new();
                if in_then {
                    consumed_in.push("then".into());
                } else {
                    not_consumed_in.push("then".into());
                }
                if in_else {
                    consumed_in.push("else".into());
                } else {
                    not_consumed_in.push("else".into());
                }
                self.errors.push(LinearError::InconsistentBranch {
                    name: name.clone(),
                    consumed_in,
                    not_consumed_in,
                });
            }
        }
    }

    /// Returns all collected errors.
    pub fn into_errors(self) -> Vec<LinearError> {
        self.errors
    }

    /// Returns `true` if no errors were collected.
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for LinearityChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S5.1 — Linear Annotation
    #[test]
    fn s5_1_linearity_display() {
        assert_eq!(Linearity::Linear.to_string(), "linear");
        assert_eq!(Linearity::Affine.to_string(), "affine");
        assert_eq!(Linearity::Unrestricted.to_string(), "unrestricted");
    }

    #[test]
    fn s5_1_linearity_properties() {
        assert!(Linearity::Linear.is_linear());
        assert!(!Linearity::Affine.is_linear());
        assert!(Linearity::Linear.forbids_copy());
        assert!(Linearity::Affine.forbids_copy());
        assert!(!Linearity::Unrestricted.forbids_copy());
        assert!(Linearity::Linear.requires_consumption());
        assert!(!Linearity::Affine.requires_consumption());
    }

    // S5.2 — AST Representation
    #[test]
    fn s5_2_linearity_enum_values() {
        let vals = [
            Linearity::Linear,
            Linearity::Affine,
            Linearity::Unrestricted,
        ];
        assert_eq!(vals.len(), 3);
        assert_ne!(vals[0], vals[1]);
        assert_ne!(vals[1], vals[2]);
    }

    // S5.3 — Usage Tracking
    #[test]
    fn s5_3_binding_tracks_usage() {
        let mut b = LinearBinding::new("handle", "FileHandle", Linearity::Linear, 0);
        assert_eq!(b.use_count, 0);
        b.record_use();
        assert_eq!(b.use_count, 1);
        b.record_use();
        assert_eq!(b.use_count, 2);
    }

    #[test]
    fn s5_3_binding_consumed() {
        let mut b = LinearBinding::new("buf", "GpuBuffer", Linearity::Linear, 0);
        assert!(!b.consumed);
        b.mark_consumed();
        assert!(b.consumed);
        assert_eq!(b.use_count, 1);
    }

    // S5.4 — Unused Linear Error
    #[test]
    fn s5_4_unused_linear_detected() {
        let mut checker = LinearityChecker::new();
        checker.enter_scope();
        checker.declare("h", "FileHandle", Linearity::Linear);
        checker.exit_scope();
        assert_eq!(checker.errors.len(), 1);
        assert!(matches!(
            &checker.errors[0],
            LinearError::UnusedLinear { name, .. } if name == "h"
        ));
    }

    #[test]
    fn s5_4_consumed_no_error() {
        let mut checker = LinearityChecker::new();
        checker.enter_scope();
        checker.declare("h", "FileHandle", Linearity::Linear);
        checker.consume("h");
        checker.exit_scope();
        assert!(checker.is_clean());
    }

    // S5.5 — Duplicate Use
    #[test]
    fn s5_5_duplicate_use_detected() {
        let mut checker = LinearityChecker::new();
        checker.declare("h", "FileHandle", Linearity::Linear);
        checker.consume("h");
        checker.consume("h"); // duplicate
        assert_eq!(checker.errors.len(), 1);
        assert!(matches!(
            &checker.errors[0],
            LinearError::DuplicateUse { name, .. } if name == "h"
        ));
    }

    // S5.6 — Consume Syntax
    #[test]
    fn s5_6_validate_consume_ok() {
        let mut bindings = HashMap::new();
        bindings.insert(
            "h".into(),
            LinearBinding::new("h", "FileHandle", Linearity::Linear, 0),
        );
        assert!(validate_consume(&bindings, "h").is_ok());
    }

    #[test]
    fn s5_6_validate_consume_already_consumed() {
        let mut bindings = HashMap::new();
        let mut b = LinearBinding::new("h", "FileHandle", Linearity::Linear, 0);
        b.mark_consumed();
        bindings.insert("h".into(), b);
        assert!(validate_consume(&bindings, "h").is_err());
    }

    // S5.7 — Control Flow Branches
    #[test]
    fn s5_7_consistent_branches_ok() {
        let mut checker = LinearityChecker::new();
        checker.declare("h", "FileHandle", Linearity::Linear);

        let before: HashMap<String, bool> = [("h".into(), false)].into();
        let then_consumed: HashMap<String, bool> = [("h".into(), true)].into();
        let else_consumed: HashMap<String, bool> = [("h".into(), true)].into();
        checker.check_branch_consistency(&before, &then_consumed, &else_consumed);
        assert!(checker.is_clean());
    }

    #[test]
    fn s5_7_inconsistent_branches_error() {
        let mut checker = LinearityChecker::new();
        checker.declare("h", "FileHandle", Linearity::Linear);

        let before: HashMap<String, bool> = [("h".into(), false)].into();
        let then_consumed: HashMap<String, bool> = [("h".into(), true)].into();
        let else_consumed: HashMap<String, bool> = [("h".into(), false)].into();
        checker.check_branch_consistency(&before, &then_consumed, &else_consumed);
        assert_eq!(checker.errors.len(), 1);
        assert!(matches!(
            &checker.errors[0],
            LinearError::InconsistentBranch { name, .. } if name == "h"
        ));
    }

    // S5.8 — Linear in Loops
    #[test]
    fn s5_8_linear_in_loop_error() {
        let mut checker = LinearityChecker::new();
        checker.declare("h", "FileHandle", Linearity::Linear);
        checker.enter_loop();
        checker.record_use("h");
        checker.exit_loop();
        assert_eq!(checker.errors.len(), 1);
        assert!(matches!(
            &checker.errors[0],
            LinearError::LinearInLoop { name, .. } if name == "h"
        ));
    }

    #[test]
    fn s5_8_non_linear_in_loop_ok() {
        let mut checker = LinearityChecker::new();
        checker.declare("x", "i32", Linearity::Unrestricted);
        checker.enter_loop();
        checker.record_use("x");
        checker.exit_loop();
        assert!(checker.is_clean());
    }

    // S5.9 — Function Parameters
    #[test]
    fn s5_9_unconsumed_param_error() {
        let mut checker = LinearityChecker::new();
        checker.declare("handle", "FileHandle", Linearity::Linear);
        checker.check_params_consumed(&["handle"]);
        assert_eq!(checker.errors.len(), 1);
        assert!(matches!(
            &checker.errors[0],
            LinearError::UnconsumedParam { name, .. } if name == "handle"
        ));
    }

    #[test]
    fn s5_9_consumed_param_ok() {
        let mut checker = LinearityChecker::new();
        checker.declare("handle", "FileHandle", Linearity::Linear);
        checker.consume("handle");
        checker.check_params_consumed(&["handle"]);
        assert!(checker.is_clean());
    }

    #[test]
    fn s5_9_returned_param_ok() {
        let mut checker = LinearityChecker::new();
        checker.declare("handle", "FileHandle", Linearity::Linear);
        checker.mark_returned("handle");
        checker.check_params_consumed(&["handle"]);
        assert!(checker.is_clean());
    }

    // S5.10 — Additional tests
    #[test]
    fn s5_10_affine_value_can_be_dropped() {
        let mut checker = LinearityChecker::new();
        checker.enter_scope();
        checker.declare("x", "String", Linearity::Affine);
        checker.exit_scope();
        assert!(checker.is_clean()); // Affine can be dropped.
    }

    #[test]
    fn s5_10_linear_error_display() {
        let err = LinearError::UnusedLinear {
            name: "h".into(),
            type_name: "FileHandle".into(),
        };
        assert!(err.to_string().contains("never consumed"));
    }

    #[test]
    fn s5_10_multiple_linear_bindings() {
        let mut checker = LinearityChecker::new();
        checker.enter_scope();
        checker.declare("a", "FileHandle", Linearity::Linear);
        checker.declare("b", "GpuBuffer", Linearity::Linear);
        checker.consume("a");
        // b is not consumed
        checker.exit_scope();
        assert_eq!(checker.errors.len(), 1);
        assert!(matches!(
            &checker.errors[0],
            LinearError::UnusedLinear { name, .. } if name == "b"
        ));
    }

    #[test]
    fn s5_10_nested_scopes() {
        let mut checker = LinearityChecker::new();
        checker.enter_scope();
        checker.declare("outer", "FileHandle", Linearity::Linear);
        checker.enter_scope();
        checker.declare("inner", "GpuBuffer", Linearity::Linear);
        checker.consume("inner");
        checker.exit_scope();
        checker.consume("outer");
        checker.exit_scope();
        assert!(checker.is_clean());
    }
}
