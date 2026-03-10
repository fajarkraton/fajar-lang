//! Ownership and borrow analysis (lite version).
//!
//! Implements move semantics and borrow rules without lifetime annotations.
//! Copy types (integers, floats, booleans, char) are implicitly copied on assignment.
//! Move types (String, Array, Struct with non-Copy fields) transfer ownership.
//!
//! Borrow rules (scope-based, simpler than Rust NLL):
//! - Multiple `&T` (immutable borrows) are allowed simultaneously.
//! - Only one `&mut T` (mutable borrow) at a time, exclusive of all others.
//! - Cannot move a variable while it has active borrows.
//! - Borrows expire at the end of the scope where the borrow binding lives.

use std::collections::HashMap;

use crate::analyzer::type_check::Type;
use crate::lexer::token::Span;

/// Ownership state of a variable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OwnershipState {
    /// Variable owns its value.
    Owned,
    /// Variable's value has been moved to another binding.
    Moved,
}

/// Borrow state of a variable (tracked separately from ownership).
#[derive(Debug, Clone, PartialEq)]
pub enum BorrowState {
    /// No active borrows.
    Unborrowed,
    /// One or more immutable borrows active.
    ImmBorrowed {
        /// Number of active immutable borrows.
        count: usize,
        /// Span of first immutable borrow.
        first_span: Span,
    },
    /// One mutable borrow active (exclusive).
    MutBorrowed {
        /// Span of the mutable borrow.
        span: Span,
    },
}

/// Borrow conflict errors returned from borrow operations.
#[derive(Debug, Clone, PartialEq)]
pub enum BorrowError {
    /// Cannot take immutable borrow: variable is mutably borrowed.
    ImmWhileMutBorrowed { mut_span: Span },
    /// Cannot take mutable borrow: variable is immutably borrowed.
    MutWhileImmBorrowed { imm_span: Span },
    /// Cannot take mutable borrow: already mutably borrowed.
    DoubleMutBorrow { existing_span: Span },
    /// Cannot take mutable borrow: variable is not declared `mut`.
    NotMutable,
}

/// Move analysis errors.
#[derive(Debug, Clone, PartialEq)]
pub enum MoveError {
    /// ME001: Use after move.
    UseAfterMove {
        /// Variable name.
        name: String,
        /// Where it was used after move.
        use_span: Span,
        /// Where it was moved.
        move_span: Span,
    },
}

/// Tracks variable ownership and borrow states within scoped regions.
#[derive(Debug)]
pub struct MoveTracker {
    /// Variable name → (ownership state, span where moved).
    states: Vec<HashMap<String, (OwnershipState, Span)>>,
    /// Variable name → borrow state (parallels `states` scope stack).
    borrows: Vec<HashMap<String, BorrowState>>,
    /// Borrow binding → (target variable name, is_mutable).
    /// Tracks which variables hold borrows of which targets.
    borrow_refs: Vec<HashMap<String, (String, bool)>>,
}

impl MoveTracker {
    /// Creates a new move tracker with a global scope.
    pub fn new() -> Self {
        Self {
            states: vec![HashMap::new()],
            borrows: vec![HashMap::new()],
            borrow_refs: vec![HashMap::new()],
        }
    }

    /// Pushes a new scope.
    pub fn push_scope(&mut self) {
        self.states.push(HashMap::new());
        self.borrows.push(HashMap::new());
        self.borrow_refs.push(HashMap::new());
    }

    /// Pops a scope, releasing any borrows held by variables in the popped scope.
    pub fn pop_scope(&mut self) {
        // Release borrows held by variables going out of scope
        if let Some(refs) = self.borrow_refs.last() {
            let releases: Vec<(String, bool)> = refs.values().cloned().collect();
            for (target, is_mut) in releases {
                self.release_borrow(&target, is_mut);
            }
        }
        self.states.pop();
        self.borrows.pop();
        self.borrow_refs.pop();
    }

    /// Declares a variable as Owned.
    pub fn declare(&mut self, name: &str, span: Span) {
        if let Some(scope) = self.states.last_mut() {
            scope.insert(name.to_string(), (OwnershipState::Owned, span));
        }
    }

    /// Marks a variable as moved.
    pub fn mark_moved(&mut self, name: &str, span: Span) {
        for scope in self.states.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), (OwnershipState::Moved, span));
                return;
            }
        }
    }

    /// Checks if a variable is in a Moved state.
    /// Returns the move span if the variable was moved.
    pub fn check_use(&self, name: &str) -> Option<Span> {
        for scope in self.states.iter().rev() {
            if let Some((state, span)) = scope.get(name) {
                if *state == OwnershipState::Moved {
                    return Some(*span);
                }
                return None;
            }
        }
        None
    }

    /// Creates an immutable borrow of a variable.
    pub fn borrow_imm(&mut self, name: &str, span: Span) -> Result<(), BorrowError> {
        let state = self.get_borrow_state(name);
        match state {
            BorrowState::MutBorrowed { span: mut_span } => {
                Err(BorrowError::ImmWhileMutBorrowed { mut_span })
            }
            BorrowState::ImmBorrowed { count, first_span } => {
                self.set_borrow_state(
                    name,
                    BorrowState::ImmBorrowed {
                        count: count + 1,
                        first_span,
                    },
                );
                Ok(())
            }
            BorrowState::Unborrowed => {
                self.set_borrow_state(
                    name,
                    BorrowState::ImmBorrowed {
                        count: 1,
                        first_span: span,
                    },
                );
                Ok(())
            }
        }
    }

    /// Creates a mutable borrow of a variable.
    pub fn borrow_mut(&mut self, name: &str, span: Span) -> Result<(), BorrowError> {
        let state = self.get_borrow_state(name);
        match state {
            BorrowState::MutBorrowed { span: existing } => Err(BorrowError::DoubleMutBorrow {
                existing_span: existing,
            }),
            BorrowState::ImmBorrowed { first_span, .. } => Err(BorrowError::MutWhileImmBorrowed {
                imm_span: first_span,
            }),
            BorrowState::Unborrowed => {
                self.set_borrow_state(name, BorrowState::MutBorrowed { span });
                Ok(())
            }
        }
    }

    /// Registers that `borrow_var` holds a borrow of `target_var`.
    pub fn register_borrow_ref(&mut self, borrow_var: &str, target_var: &str, is_mutable: bool) {
        if let Some(scope) = self.borrow_refs.last_mut() {
            scope.insert(borrow_var.to_string(), (target_var.to_string(), is_mutable));
        }
    }

    /// Checks if a variable can be moved (not actively borrowed).
    /// Returns `Some(borrow_span)` if the variable has active borrows.
    pub fn check_can_move(&self, name: &str) -> Option<Span> {
        let state = self.get_borrow_state(name);
        match state {
            BorrowState::ImmBorrowed { first_span, .. } => Some(first_span),
            BorrowState::MutBorrowed { span } => Some(span),
            BorrowState::Unborrowed => None,
        }
    }

    /// Returns the names of all active borrow bindings across all scopes.
    pub fn active_borrow_refs(&self) -> Vec<String> {
        let mut refs = Vec::new();
        for scope in &self.borrow_refs {
            for name in scope.keys() {
                refs.push(name.clone());
            }
        }
        refs
    }

    /// Releases the borrow held by `borrow_var` (NLL: borrow past last use).
    ///
    /// Looks up which target variable `borrow_var` borrows and releases that borrow.
    /// Also removes the borrow ref entry so it won't be released again on scope pop.
    pub fn release_borrow_by_ref(&mut self, borrow_var: &str) {
        let mut target_info = None;
        for scope in self.borrow_refs.iter() {
            if let Some((target, is_mut)) = scope.get(borrow_var) {
                target_info = Some((target.clone(), *is_mut));
                break;
            }
        }
        if let Some((target, is_mut)) = target_info {
            self.release_borrow(&target, is_mut);
            // Remove the borrow ref entry
            for scope in self.borrow_refs.iter_mut() {
                if scope.remove(borrow_var).is_some() {
                    break;
                }
            }
        }
    }

    /// Gets the borrow state of a variable, searching scope chain.
    pub fn get_borrow_state(&self, name: &str) -> BorrowState {
        for scope in self.borrows.iter().rev() {
            if let Some(state) = scope.get(name) {
                return state.clone();
            }
        }
        BorrowState::Unborrowed
    }

    /// Sets borrow state in the innermost scope that has the variable,
    /// or inserts into the current scope.
    fn set_borrow_state(&mut self, name: &str, state: BorrowState) {
        for scope in self.borrows.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), state);
                return;
            }
        }
        // Variable not in any borrow scope yet — add to innermost
        if let Some(scope) = self.borrows.last_mut() {
            scope.insert(name.to_string(), state);
        }
    }

    /// Releases a borrow on a target variable.
    fn release_borrow(&mut self, target: &str, is_mutable: bool) {
        for scope in self.borrows.iter_mut().rev() {
            if let Some(state) = scope.get(target) {
                let new_state = match state {
                    BorrowState::MutBorrowed { .. } if is_mutable => BorrowState::Unborrowed,
                    BorrowState::ImmBorrowed { count, first_span } if !is_mutable => {
                        if *count <= 1 {
                            BorrowState::Unborrowed
                        } else {
                            BorrowState::ImmBorrowed {
                                count: count - 1,
                                first_span: *first_span,
                            }
                        }
                    }
                    other => other.clone(),
                };
                scope.insert(target.to_string(), new_state);
                return;
            }
        }
    }
}

impl Default for MoveTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns true if a type is Copy (implicitly cloned on assignment).
///
/// Copy types: all integer types, float types, bool, char, void, never.
/// Move types: String, Array, Struct, Enum, Tensor, Function, etc.
/// Shared references (`&T`) are Copy. Mutable references (`&mut T`) are not.
pub fn is_copy_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Void
            | Type::Never
            | Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::I128
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::U128
            | Type::ISize
            | Type::USize
            | Type::F32
            | Type::F64
            | Type::IntLiteral
            | Type::FloatLiteral
            | Type::Bool
            | Type::Char
            | Type::Str
            | Type::Unknown
            | Type::TypeVar(_)
            | Type::Ref(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_types_are_copy() {
        assert!(is_copy_type(&Type::I32));
        assert!(is_copy_type(&Type::I64));
        assert!(is_copy_type(&Type::F64));
        assert!(is_copy_type(&Type::Bool));
        assert!(is_copy_type(&Type::Char));
        assert!(is_copy_type(&Type::Void));
        assert!(is_copy_type(&Type::IntLiteral));
    }

    #[test]
    fn move_types_are_not_copy() {
        // Str is now Copy (runtime uses Rc-based cloning for strings)
        assert!(is_copy_type(&Type::Str));
        assert!(!is_copy_type(&Type::Array(Box::new(Type::I32))));
        assert!(!is_copy_type(&Type::Tuple(vec![Type::Str])));
    }

    #[test]
    fn ref_is_copy_refmut_is_not() {
        assert!(is_copy_type(&Type::Ref(Box::new(Type::Str))));
        assert!(!is_copy_type(&Type::RefMut(Box::new(Type::Str))));
    }

    #[test]
    fn tracker_declare_and_check() {
        let mut tracker = MoveTracker::new();
        let span = Span::new(0, 1);
        tracker.declare("x", span);
        assert!(tracker.check_use("x").is_none());
    }

    #[test]
    fn tracker_mark_moved() {
        let mut tracker = MoveTracker::new();
        let decl_span = Span::new(0, 5);
        let move_span = Span::new(10, 15);
        tracker.declare("s", decl_span);
        tracker.mark_moved("s", move_span);
        assert_eq!(tracker.check_use("s"), Some(move_span));
    }

    #[test]
    fn tracker_scope_isolation() {
        let mut tracker = MoveTracker::new();
        let span = Span::new(0, 1);
        tracker.declare("x", span);
        tracker.push_scope();
        tracker.declare("y", span);
        tracker.mark_moved("y", Span::new(5, 10));
        assert!(tracker.check_use("y").is_some());
        assert!(tracker.check_use("x").is_none()); // outer scope unaffected
        tracker.pop_scope();
        assert!(tracker.check_use("x").is_none()); // still owned
    }

    #[test]
    fn tracker_inner_moves_outer() {
        let mut tracker = MoveTracker::new();
        let span = Span::new(0, 5);
        tracker.declare("s", span);
        tracker.push_scope();
        // Moving an outer variable from inner scope
        tracker.mark_moved("s", Span::new(10, 15));
        assert!(tracker.check_use("s").is_some());
        tracker.pop_scope();
        // After inner scope pops, outer "s" still moved
        // (because mark_moved updates in-place)
        assert!(tracker.check_use("s").is_some());
    }

    // ── Borrow tracking tests ──────────────────────────────────────────

    #[test]
    fn borrow_imm_on_unborrowed_succeeds() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        assert!(tracker.borrow_imm("x", Span::new(5, 6)).is_ok());
        assert!(matches!(
            tracker.get_borrow_state("x"),
            BorrowState::ImmBorrowed { count: 1, .. }
        ));
    }

    #[test]
    fn multiple_imm_borrows_succeed() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        assert!(tracker.borrow_imm("x", Span::new(5, 6)).is_ok());
        assert!(tracker.borrow_imm("x", Span::new(10, 11)).is_ok());
        assert!(matches!(
            tracker.get_borrow_state("x"),
            BorrowState::ImmBorrowed { count: 2, .. }
        ));
    }

    #[test]
    fn borrow_mut_on_unborrowed_succeeds() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        assert!(tracker.borrow_mut("x", Span::new(5, 6)).is_ok());
        assert!(matches!(
            tracker.get_borrow_state("x"),
            BorrowState::MutBorrowed { .. }
        ));
    }

    #[test]
    fn borrow_mut_on_imm_borrowed_fails() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_imm("x", Span::new(5, 6)).unwrap();
        let err = tracker.borrow_mut("x", Span::new(10, 11)).unwrap_err();
        assert!(matches!(err, BorrowError::MutWhileImmBorrowed { .. }));
    }

    #[test]
    fn borrow_imm_on_mut_borrowed_fails() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_mut("x", Span::new(5, 6)).unwrap();
        let err = tracker.borrow_imm("x", Span::new(10, 11)).unwrap_err();
        assert!(matches!(err, BorrowError::ImmWhileMutBorrowed { .. }));
    }

    #[test]
    fn double_mut_borrow_fails() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_mut("x", Span::new(5, 6)).unwrap();
        let err = tracker.borrow_mut("x", Span::new(10, 11)).unwrap_err();
        assert!(matches!(err, BorrowError::DoubleMutBorrow { .. }));
    }

    #[test]
    fn check_can_move_returns_none_when_unborrowed() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        assert!(tracker.check_can_move("x").is_none());
    }

    #[test]
    fn check_can_move_returns_span_when_imm_borrowed() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_imm("x", Span::new(5, 6)).unwrap();
        assert!(tracker.check_can_move("x").is_some());
    }

    #[test]
    fn check_can_move_returns_span_when_mut_borrowed() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_mut("x", Span::new(5, 6)).unwrap();
        assert!(tracker.check_can_move("x").is_some());
    }

    #[test]
    fn borrow_released_on_scope_pop() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.push_scope();
        // Create borrow and register ref
        tracker.borrow_imm("x", Span::new(5, 6)).unwrap();
        tracker.register_borrow_ref("r", "x", false);
        assert!(tracker.check_can_move("x").is_some());
        // Pop scope releases the borrow
        tracker.pop_scope();
        assert!(tracker.check_can_move("x").is_none());
    }

    #[test]
    fn mut_borrow_released_on_scope_pop() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.push_scope();
        tracker.borrow_mut("x", Span::new(5, 6)).unwrap();
        tracker.register_borrow_ref("r", "x", true);
        assert!(tracker.check_can_move("x").is_some());
        tracker.pop_scope();
        assert!(tracker.check_can_move("x").is_none());
    }

    #[test]
    fn multiple_borrows_released_on_scope_pop() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.push_scope();
        tracker.borrow_imm("x", Span::new(5, 6)).unwrap();
        tracker.register_borrow_ref("r1", "x", false);
        tracker.borrow_imm("x", Span::new(7, 8)).unwrap();
        tracker.register_borrow_ref("r2", "x", false);
        assert!(matches!(
            tracker.get_borrow_state("x"),
            BorrowState::ImmBorrowed { count: 2, .. }
        ));
        tracker.pop_scope();
        // Both borrows released
        assert!(matches!(
            tracker.get_borrow_state("x"),
            BorrowState::Unborrowed
        ));
    }

    #[test]
    fn default_borrow_state_is_unborrowed() {
        let tracker = MoveTracker::new();
        assert!(matches!(
            tracker.get_borrow_state("anything"),
            BorrowState::Unborrowed
        ));
    }
}
