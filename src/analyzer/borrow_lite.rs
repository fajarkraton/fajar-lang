//! Ownership and borrow analysis (lite version).
//!
//! Implements move semantics and borrow rules without lifetime annotations.
//! Copy types (integers, floats, booleans, char) are implicitly copied on assignment.
//! Move types (String, Array, Struct with non-Copy fields) transfer ownership.
//!
//! # Borrow Rules
//!
//! - Multiple `&T` (immutable borrows) are allowed simultaneously.
//! - Only one `&mut T` (mutable borrow) at a time, exclusive of all others.
//! - Cannot move a variable while it has active borrows.
//! - Borrows expire at the end of the scope where the borrow binding lives.
//!
//! # Advanced Features (V11 Option 6)
//!
//! - **Two-phase borrows**: `borrow_mut_two_phase()` creates a Reserved state that
//!   allows shared reads until the first mutable use. Enables `v.push(v.len())`.
//! - **Reborrowing**: `reborrow_imm()` downgrades a `&mut T` to Reserved, allowing
//!   temporary shared access via `&*r`.
//! - **Field-level borrows**: `BorrowPath::Field` tracks disjoint field borrows,
//!   allowing `&mut s.x` and `&mut s.y` simultaneously.
//! - **Drop order validation**: In strict mode, references must be dropped before
//!   their referents (reverse declaration order).
//! - **Strict ownership mode**: Enabled by `--strict-ownership` flag. In strict mode,
//!   `is_copy_type_strict()` treats String/Array/Struct as Move types, making
//!   ME001/ME003 errors fire for non-Copy types.
//!
//! # Error Diagnostics (Phase D)
//!
//! All ME errors include:
//! - `hint()`: Actionable suggestion (e.g., "consider cloning the value")
//! - `secondary_span()`: Location of the original move/borrow for two-label diagnostics
//! - NLL-aware byte offsets in error messages for precise location reporting

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
    /// Two-phase mutable borrow: reserved but not yet activated.
    /// Shared reads are still allowed in this state.
    /// Activates on first mutable use, becoming MutBorrowed.
    Reserved {
        /// Span of the reserved mutable borrow.
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

/// A borrow path — either a whole variable or a specific field.
///
/// Used for disjoint borrow tracking: `&mut s.x` and `&mut s.y` can
/// coexist because they borrow different fields (single level only).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BorrowPath {
    /// Borrow of the whole variable.
    Var(String),
    /// Borrow of a specific field: `(var_name, field_name)`.
    Field(String, String),
}

impl BorrowPath {
    /// Returns the variable name this path refers to.
    pub fn var_name(&self) -> &str {
        match self {
            BorrowPath::Var(name) | BorrowPath::Field(name, _) => name,
        }
    }

    /// Returns true if two borrow paths conflict (overlap).
    ///
    /// Two field paths conflict if they refer to the same field.
    /// A whole-var path conflicts with any path on that variable.
    pub fn conflicts_with(&self, other: &BorrowPath) -> bool {
        match (self, other) {
            (BorrowPath::Var(a), BorrowPath::Var(b)) => a == b,
            (BorrowPath::Var(a), BorrowPath::Field(b, _))
            | (BorrowPath::Field(b, _), BorrowPath::Var(a)) => a == b,
            (BorrowPath::Field(a, fa), BorrowPath::Field(b, fb)) => a == b && fa == fb,
        }
    }
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
            // Two-phase Reserved: shared reads are allowed
            BorrowState::Reserved { .. } => Ok(()),
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
            // Reserved two-phase can be upgraded to full MutBorrowed
            BorrowState::Reserved { .. } | BorrowState::Unborrowed => {
                self.set_borrow_state(name, BorrowState::MutBorrowed { span });
                Ok(())
            }
        }
    }

    /// Creates a two-phase mutable borrow (Reserved state).
    ///
    /// In Reserved state, shared reads (`&T`) of the same variable are still
    /// allowed. The borrow activates (becomes exclusive) on the first mutable use.
    /// This enables patterns like `v.push(v.len())`.
    pub fn borrow_mut_two_phase(&mut self, name: &str, span: Span) -> Result<(), BorrowError> {
        let state = self.get_borrow_state(name);
        match state {
            BorrowState::MutBorrowed { span: existing } => Err(BorrowError::DoubleMutBorrow {
                existing_span: existing,
            }),
            // Reserved borrows can coexist (both waiting to activate)
            BorrowState::Reserved { .. } | BorrowState::Unborrowed => {
                self.set_borrow_state(name, BorrowState::Reserved { span });
                Ok(())
            }
            // Immutable borrows OK — two-phase allows shared reads
            BorrowState::ImmBorrowed { .. } => {
                self.set_borrow_state(name, BorrowState::Reserved { span });
                Ok(())
            }
        }
    }

    /// Activates a two-phase borrow, transitioning Reserved → MutBorrowed.
    ///
    /// Called when the reserved borrow is first used mutably.
    pub fn activate_two_phase(&mut self, name: &str) {
        let state = self.get_borrow_state(name);
        if let BorrowState::Reserved { span } = state {
            self.set_borrow_state(name, BorrowState::MutBorrowed { span });
        }
    }

    /// Creates a reborrow: `&T` from a `&mut T` binding.
    ///
    /// The mutable borrow is suspended (downgraded to Reserved) while the
    /// immutable reborrow is active. When the reborrow is released, the
    /// mutable borrow resumes.
    pub fn reborrow_imm(&mut self, name: &str, span: Span) -> Result<(), BorrowError> {
        let state = self.get_borrow_state(name);
        match state {
            BorrowState::MutBorrowed { span: orig_span } => {
                // Downgrade to Reserved (suspends mut exclusivity)
                self.set_borrow_state(name, BorrowState::Reserved { span: orig_span });
                Ok(())
            }
            BorrowState::Reserved { .. } => {
                // Already in two-phase — shared reads OK
                Ok(())
            }
            _ => {
                // Not mutably borrowed — just do a regular imm borrow
                self.borrow_imm(name, span)
            }
        }
    }

    /// Creates a mutable borrow of a specific field.
    ///
    /// Disjoint fields can be mutably borrowed simultaneously.
    /// `&mut s.x` and `&mut s.y` are OK; `&mut s.x` and `&mut s.x` conflict.
    pub fn borrow_field_mut(
        &mut self,
        var: &str,
        field: &str,
        span: Span,
    ) -> Result<(), BorrowError> {
        let path = BorrowPath::Field(var.to_string(), field.to_string());
        // Check for whole-var borrow conflict
        let state = self.get_borrow_state(var);
        match state {
            BorrowState::MutBorrowed { span: existing } => {
                return Err(BorrowError::DoubleMutBorrow {
                    existing_span: existing,
                });
            }
            BorrowState::ImmBorrowed { first_span, .. } => {
                return Err(BorrowError::MutWhileImmBorrowed {
                    imm_span: first_span,
                });
            }
            _ => {}
        }
        // Check field-level conflicts
        let field_key = format!("{}.{}", path.var_name(), field);
        let field_state = self.get_borrow_state(&field_key);
        match field_state {
            BorrowState::MutBorrowed { span: existing } => Err(BorrowError::DoubleMutBorrow {
                existing_span: existing,
            }),
            BorrowState::ImmBorrowed { first_span, .. } => Err(BorrowError::MutWhileImmBorrowed {
                imm_span: first_span,
            }),
            _ => {
                self.set_borrow_state(&field_key, BorrowState::MutBorrowed { span });
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
            BorrowState::MutBorrowed { span } | BorrowState::Reserved { span } => Some(span),
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
                    BorrowState::MutBorrowed { .. } | BorrowState::Reserved { .. }
                        if is_mutable =>
                    {
                        BorrowState::Unborrowed
                    }
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
/// Determines whether a type is Copy (no move on assignment/pass).
///
/// # Current semantics (interpreter mode)
///
/// The Fajar Lang interpreter uses `Value::clone()` for all assignments and
/// function argument passing. This means ALL types are effectively Copy at
/// runtime — `let b = a` clones `a`, and `a` remains valid.
///
/// The analyzer matches this runtime behavior: all types are Copy.
/// This prevents false ME001 (use-after-move) errors that would confuse users.
///
/// # Future: native codegen mode
///
/// When Cranelift/LLVM native codegen becomes the primary execution path,
/// move semantics will matter for performance (avoiding unnecessary copies).
/// At that point, this function should be updated to:
///   - Primitives (i8-i128, f32/f64, bool, char): Copy
///   - Str, Array, Struct, Tuple: Move (require explicit .clone())
///   - Ref: Copy, RefMut: Move
///
/// This change will be gated behind a compiler flag: `--strict-ownership`
pub fn is_copy_type(_ty: &Type) -> bool {
    // In interpreter mode, all types are Copy (clone on assignment).
    // This matches the runtime semantics of Value::clone().
    true
}

/// Returns true if a type is Copy under strict ownership rules.
///
/// Copy types: integers, floats, bool, char, void, never, unsuffixed literals,
/// immutable references (`&T`).
///
/// Move types: String, Array, Tuple (with move fields), Struct, Enum, Tensor,
/// Function, mutable references (`&mut T`), Future, DynTrait.
///
/// Used when `--strict-ownership` is enabled.
pub fn is_copy_type_strict(ty: &Type) -> bool {
    match ty {
        // Primitives: always Copy
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
        | Type::F16
        | Type::Bf16
        | Type::F32
        | Type::F64
        | Type::IntLiteral
        | Type::FloatLiteral
        | Type::Bool
        | Type::Char => true,

        // Immutable references are Copy (shared access)
        Type::Ref(..) => true,

        // Move types: heap-allocated or resource-owning
        Type::Str
        | Type::Array(_)
        | Type::Struct { .. }
        | Type::Enum { .. }
        | Type::Tensor { .. }
        | Type::Quantized { .. }
        | Type::Function { .. }
        | Type::RefMut(..)
        | Type::Future { .. }
        | Type::DynTrait(_) => false,

        // Tuples: Copy only if all elements are Copy
        Type::Tuple(elems) => elems.iter().all(is_copy_type_strict),

        // Unknown/Named/TypeVar: assume Copy (error recovery / generics)
        Type::Unknown | Type::Named(_) | Type::TypeVar(_) => true,
    }
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
    fn all_types_are_copy_in_interpreter_mode() {
        // In interpreter mode, all types are Copy (Value::clone on assignment).
        // This matches the runtime semantics.
        assert!(is_copy_type(&Type::Str));
        assert!(is_copy_type(&Type::Array(Box::new(Type::I32))));
        assert!(is_copy_type(&Type::Tuple(vec![Type::Str])));
        assert!(is_copy_type(&Type::RefMut(Box::new(Type::I32), None)));
        assert!(is_copy_type(&Type::Struct {
            name: "Point".into(),
            fields: std::collections::HashMap::new()
        }));
        assert!(is_copy_type(&Type::Ref(Box::new(Type::Str), None)));
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

    // ── Sprint 0.2: Edge case tests ──

    #[test]
    fn move_then_reassign_allows_reuse() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.mark_moved("x", Span::new(5, 6));
        assert!(tracker.check_use("x").is_some()); // moved
        // Reassign: re-declare as owned
        tracker.declare("x", Span::new(10, 11));
        assert!(tracker.check_use("x").is_none()); // owned again
    }

    #[test]
    fn nested_scope_move_does_not_affect_outer() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.push_scope();
        tracker.declare("y", Span::new(5, 6));
        tracker.mark_moved("y", Span::new(10, 11));
        assert!(tracker.check_use("y").is_some());
        tracker.pop_scope();
        // x should still be owned
        assert!(tracker.check_use("x").is_none());
    }

    #[test]
    fn borrow_released_on_scope_exit() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.push_scope();
        tracker.borrow_imm("x", Span::new(5, 6)).unwrap();
        tracker.register_borrow_ref("r", "x", false);
        assert!(matches!(
            tracker.get_borrow_state("x"),
            BorrowState::ImmBorrowed { .. }
        ));
        tracker.pop_scope(); // should release borrow
        assert!(matches!(
            tracker.get_borrow_state("x"),
            BorrowState::Unborrowed
        ));
    }

    #[test]
    fn mut_borrow_released_on_scope_exit() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.push_scope();
        tracker.borrow_mut("x", Span::new(5, 6)).unwrap();
        tracker.register_borrow_ref("r", "x", true);
        tracker.pop_scope();
        // After scope exit, should be able to borrow again
        assert!(tracker.borrow_mut("x", Span::new(10, 11)).is_ok());
    }

    #[test]
    fn cannot_move_while_borrowed() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_imm("x", Span::new(5, 6)).unwrap();
        // check_can_move should return the borrow span
        assert!(tracker.check_can_move("x").is_some());
    }

    #[test]
    fn cannot_move_while_mut_borrowed() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_mut("x", Span::new(5, 6)).unwrap();
        assert!(tracker.check_can_move("x").is_some());
    }

    #[test]
    fn multiple_imm_borrows_allowed() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        assert!(tracker.borrow_imm("x", Span::new(5, 6)).is_ok());
        assert!(tracker.borrow_imm("x", Span::new(7, 8)).is_ok());
        assert!(tracker.borrow_imm("x", Span::new(9, 10)).is_ok());
        if let BorrowState::ImmBorrowed { count, .. } = tracker.get_borrow_state("x") {
            assert_eq!(count, 3);
        } else {
            panic!("expected ImmBorrowed");
        }
    }

    #[test]
    fn copy_types_are_correct() {
        // Numeric types: Copy
        assert!(is_copy_type(&Type::I8));
        assert!(is_copy_type(&Type::I16));
        assert!(is_copy_type(&Type::I32));
        assert!(is_copy_type(&Type::I64));
        assert!(is_copy_type(&Type::I128));
        assert!(is_copy_type(&Type::U8));
        assert!(is_copy_type(&Type::U16));
        assert!(is_copy_type(&Type::U32));
        assert!(is_copy_type(&Type::U64));
        assert!(is_copy_type(&Type::U128));
        assert!(is_copy_type(&Type::ISize));
        assert!(is_copy_type(&Type::USize));
        assert!(is_copy_type(&Type::F32));
        assert!(is_copy_type(&Type::F64));
        assert!(is_copy_type(&Type::Bool));
        assert!(is_copy_type(&Type::Char));
        // Str is Copy (Rc<String> in interpreter)
        assert!(is_copy_type(&Type::Str));
        // References are Copy
        assert!(is_copy_type(&Type::Ref(Box::new(Type::I64), None)));
        // Struct: Copy (interpreter uses Rc-based value semantics)
        assert!(is_copy_type(&Type::Struct {
            name: "Point".into(),
            fields: std::collections::HashMap::new()
        }));
        // Array/Tuple: Copy (interpreter uses Rc-based value semantics)
        assert!(is_copy_type(&Type::Array(Box::new(Type::I32))));
        assert!(is_copy_type(&Type::Tuple(vec![Type::I32, Type::Str])));
    }

    #[test]
    fn move_in_inner_scope_visible_in_outer() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.push_scope();
        tracker.mark_moved("x", Span::new(5, 6));
        tracker.pop_scope();
        // x was moved inside inner scope, should still be moved
        assert!(tracker.check_use("x").is_some());
    }

    #[test]
    fn double_mut_borrow_error() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_mut("x", Span::new(5, 6)).unwrap();
        let err = tracker.borrow_mut("x", Span::new(10, 11)).unwrap_err();
        assert!(matches!(err, BorrowError::DoubleMutBorrow { .. }));
    }

    #[test]
    fn imm_borrow_during_mut_borrow_error() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_mut("x", Span::new(5, 6)).unwrap();
        let err = tracker.borrow_imm("x", Span::new(10, 11)).unwrap_err();
        assert!(matches!(err, BorrowError::ImmWhileMutBorrowed { .. }));
    }

    #[test]
    fn mut_borrow_during_imm_borrow_error() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_imm("x", Span::new(5, 6)).unwrap();
        let err = tracker.borrow_mut("x", Span::new(10, 11)).unwrap_err();
        assert!(matches!(err, BorrowError::MutWhileImmBorrowed { .. }));
    }

    #[test]
    fn release_borrow_allows_new_borrow() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_mut("x", Span::new(5, 6)).unwrap();
        tracker.release_borrow("x", true);
        // Now should allow new borrow
        assert!(tracker.borrow_imm("x", Span::new(10, 11)).is_ok());
    }

    #[test]
    fn deeply_nested_scopes() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        for _ in 0..10 {
            tracker.push_scope();
        }
        // x should still be visible from innermost scope
        assert!(tracker.check_use("x").is_none());
        tracker.mark_moved("x", Span::new(50, 51));
        assert!(tracker.check_use("x").is_some());
        for _ in 0..10 {
            tracker.pop_scope();
        }
        // Still moved
        assert!(tracker.check_use("x").is_some());
    }

    // ── is_copy_type_strict tests ─────────────────────────────────────

    #[test]
    fn strict_primitives_are_copy() {
        assert!(is_copy_type_strict(&Type::I8));
        assert!(is_copy_type_strict(&Type::I16));
        assert!(is_copy_type_strict(&Type::I32));
        assert!(is_copy_type_strict(&Type::I64));
        assert!(is_copy_type_strict(&Type::I128));
        assert!(is_copy_type_strict(&Type::U8));
        assert!(is_copy_type_strict(&Type::U16));
        assert!(is_copy_type_strict(&Type::U32));
        assert!(is_copy_type_strict(&Type::U64));
        assert!(is_copy_type_strict(&Type::U128));
        assert!(is_copy_type_strict(&Type::ISize));
        assert!(is_copy_type_strict(&Type::USize));
        assert!(is_copy_type_strict(&Type::F16));
        assert!(is_copy_type_strict(&Type::Bf16));
        assert!(is_copy_type_strict(&Type::F32));
        assert!(is_copy_type_strict(&Type::F64));
        assert!(is_copy_type_strict(&Type::Bool));
        assert!(is_copy_type_strict(&Type::Char));
        assert!(is_copy_type_strict(&Type::Void));
        assert!(is_copy_type_strict(&Type::Never));
        assert!(is_copy_type_strict(&Type::IntLiteral));
        assert!(is_copy_type_strict(&Type::FloatLiteral));
    }

    #[test]
    fn strict_ref_is_copy() {
        assert!(is_copy_type_strict(&Type::Ref(Box::new(Type::I32), None)));
        assert!(is_copy_type_strict(&Type::Ref(Box::new(Type::Str), None)));
    }

    #[test]
    fn strict_str_is_move() {
        assert!(!is_copy_type_strict(&Type::Str));
    }

    #[test]
    fn strict_array_is_move() {
        assert!(!is_copy_type_strict(&Type::Array(Box::new(Type::I32))));
    }

    #[test]
    fn strict_struct_is_move() {
        assert!(!is_copy_type_strict(&Type::Struct {
            name: "Point".into(),
            fields: std::collections::HashMap::new(),
        }));
    }

    #[test]
    fn strict_enum_is_move() {
        assert!(!is_copy_type_strict(&Type::Enum {
            name: "Option".into(),
        }));
    }

    #[test]
    fn strict_ref_mut_is_move() {
        assert!(!is_copy_type_strict(&Type::RefMut(
            Box::new(Type::I32),
            None
        )));
    }

    #[test]
    fn strict_tensor_is_move() {
        assert!(!is_copy_type_strict(&Type::Tensor {
            element: Box::new(Type::F32),
            dims: vec![Some(3), Some(4)],
        }));
    }

    #[test]
    fn strict_tuple_copy_if_all_copy() {
        // All copy elements → copy
        assert!(is_copy_type_strict(&Type::Tuple(vec![
            Type::I32,
            Type::Bool,
            Type::F64,
        ])));
        // One move element → move
        assert!(!is_copy_type_strict(&Type::Tuple(vec![
            Type::I32,
            Type::Str,
        ])));
    }

    #[test]
    fn strict_function_is_move() {
        assert!(!is_copy_type_strict(&Type::Function {
            params: vec![Type::I32],
            ret: Box::new(Type::I32),
        }));
    }

    #[test]
    fn strict_future_is_move() {
        assert!(!is_copy_type_strict(&Type::Future {
            inner: Box::new(Type::I32),
        }));
    }

    #[test]
    fn strict_unknown_is_copy() {
        assert!(is_copy_type_strict(&Type::Unknown));
        assert!(is_copy_type_strict(&Type::Named("T".into())));
        assert!(is_copy_type_strict(&Type::TypeVar("T".into())));
    }

    // ── Two-phase borrow tests ────────────────────────────────────────

    #[test]
    fn two_phase_reserved_allows_imm_borrow() {
        let mut tracker = MoveTracker::new();
        tracker.declare("v", Span::new(0, 1));
        tracker.borrow_mut_two_phase("v", Span::new(5, 6)).unwrap();
        // Reserved: shared reads allowed
        assert!(tracker.borrow_imm("v", Span::new(10, 11)).is_ok());
    }

    #[test]
    fn two_phase_activated_blocks_imm() {
        let mut tracker = MoveTracker::new();
        tracker.declare("v", Span::new(0, 1));
        tracker.borrow_mut_two_phase("v", Span::new(5, 6)).unwrap();
        tracker.activate_two_phase("v");
        // Activated: shared reads blocked
        assert!(tracker.borrow_imm("v", Span::new(10, 11)).is_err());
    }

    #[test]
    fn two_phase_conflicts_with_full_mut() {
        let mut tracker = MoveTracker::new();
        tracker.declare("v", Span::new(0, 1));
        tracker.borrow_mut("v", Span::new(5, 6)).unwrap();
        // Full mut + two-phase = conflict
        assert!(
            tracker
                .borrow_mut_two_phase("v", Span::new(10, 11))
                .is_err()
        );
    }

    // ── Reborrow tests ────────────────────────────────────────────────

    #[test]
    fn reborrow_imm_from_mut_succeeds() {
        let mut tracker = MoveTracker::new();
        tracker.declare("x", Span::new(0, 1));
        tracker.borrow_mut("x", Span::new(5, 6)).unwrap();
        // Reborrow: &*x where x: &mut — downgrades to reserved
        assert!(tracker.reborrow_imm("x", Span::new(10, 11)).is_ok());
        // Now in Reserved state — imm borrows should work
        assert!(tracker.borrow_imm("x", Span::new(15, 16)).is_ok());
    }

    // ── Field-level borrow tests ──────────────────────────────────────

    #[test]
    fn disjoint_field_borrows_allowed() {
        let mut tracker = MoveTracker::new();
        tracker.declare("s", Span::new(0, 1));
        // &mut s.x
        assert!(tracker.borrow_field_mut("s", "x", Span::new(5, 6)).is_ok());
        // &mut s.y — different field, OK
        assert!(
            tracker
                .borrow_field_mut("s", "y", Span::new(10, 11))
                .is_ok()
        );
    }

    #[test]
    fn same_field_borrow_conflicts() {
        let mut tracker = MoveTracker::new();
        tracker.declare("s", Span::new(0, 1));
        tracker.borrow_field_mut("s", "x", Span::new(5, 6)).unwrap();
        // &mut s.x again — same field, conflict
        assert!(
            tracker
                .borrow_field_mut("s", "x", Span::new(10, 11))
                .is_err()
        );
    }

    // ── BorrowPath tests ──────────────────────────────────────────────

    #[test]
    fn borrow_path_var_conflicts() {
        let a = BorrowPath::Var("x".into());
        let b = BorrowPath::Var("x".into());
        let c = BorrowPath::Var("y".into());
        assert!(a.conflicts_with(&b));
        assert!(!a.conflicts_with(&c));
    }

    #[test]
    fn borrow_path_field_disjoint() {
        let a = BorrowPath::Field("s".into(), "x".into());
        let b = BorrowPath::Field("s".into(), "y".into());
        let c = BorrowPath::Field("s".into(), "x".into());
        assert!(!a.conflicts_with(&b)); // different fields
        assert!(a.conflicts_with(&c)); // same field
    }

    #[test]
    fn borrow_path_var_field_conflict() {
        let whole = BorrowPath::Var("s".into());
        let field = BorrowPath::Field("s".into(), "x".into());
        assert!(whole.conflicts_with(&field));
        assert!(field.conflicts_with(&whole));
    }

    // ── Phase D: Additional borrow_lite unit tests (D5) ─────────────────

    #[test]
    fn two_phase_then_activate_blocks_imm_borrow() {
        // Reserved → activate → MutBorrowed → imm borrow fails
        let mut tracker = MoveTracker::new();
        let span = Span::new(0, 5);
        tracker.declare("v", span);
        assert!(tracker.borrow_mut_two_phase("v", span).is_ok());
        tracker.activate_two_phase("v");
        // Now fully mut-borrowed — imm borrow should fail
        let result = tracker.borrow_imm("v", Span::new(10, 15));
        assert!(matches!(
            result,
            Err(BorrowError::ImmWhileMutBorrowed { .. })
        ));
    }

    #[test]
    fn reborrow_imm_from_mut_then_restore() {
        // borrow_mut → reborrow_imm suspends → activate restores
        let mut tracker = MoveTracker::new();
        let span = Span::new(0, 5);
        tracker.declare("x", span);
        assert!(tracker.borrow_mut("x", span).is_ok());
        // Reborrow: downgrades to Reserved
        assert!(tracker.reborrow_imm("x", Span::new(10, 15)).is_ok());
        // In Reserved state, another imm borrow should succeed
        assert!(tracker.borrow_imm("x", Span::new(20, 25)).is_ok());
    }

    #[test]
    fn d5_check_can_move_returns_exact_borrow_span() {
        // check_can_move returns the exact borrow span
        let mut tracker = MoveTracker::new();
        let decl = Span::new(0, 5);
        let borrow = Span::new(10, 15);
        tracker.declare("s", decl);
        assert!(tracker.borrow_imm("s", borrow).is_ok());
        let result = tracker.check_can_move("s");
        assert_eq!(result, Some(borrow));
    }

    #[test]
    fn d5_moved_variable_not_redeclared() {
        // After move, variable stays moved even if checked multiple times
        let mut tracker = MoveTracker::new();
        tracker.declare("s", Span::new(0, 5));
        tracker.mark_moved("s", Span::new(10, 15));
        assert!(tracker.check_use("s").is_some());
        assert!(tracker.check_use("s").is_some()); // still moved
    }

    #[test]
    fn scope_pop_releases_borrow_ref() {
        // When a scope is popped, borrow refs held by that scope are released
        let mut tracker = MoveTracker::new();
        let decl = Span::new(0, 5);
        tracker.declare("x", decl);
        // Push inner scope and borrow there
        tracker.push_scope();
        assert!(tracker.borrow_imm("x", Span::new(10, 15)).is_ok());
        tracker.register_borrow_ref("r", "x", false);
        tracker.pop_scope();
        // After pop, the borrow held by r is released
        // A new mutable borrow should now succeed
        let result = tracker.borrow_mut("x", Span::new(20, 25));
        assert!(result.is_ok());
    }
}
