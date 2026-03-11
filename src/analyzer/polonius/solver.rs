//! Iterative fixed-point Datalog solver for Polonius borrow checking.
//!
//! Computes derived relations from the input facts and detects borrow errors.
//!
//! # Datalog Rules
//!
//! ```text
//! origin_contains_loan_on_entry(O, L, P2) :-
//!     origin_contains_loan_on_entry(O, L, P1),
//!     cfg_edge(P1, P2),
//!     !loan_killed_at(L, P1).
//!
//! origin_contains_loan_on_entry(O2, L, P) :-
//!     subset(O1, O2, P),
//!     origin_contains_loan_on_entry(O1, L, P).
//!
//! loan_live_at(L, P) :-
//!     origin_contains_loan_on_entry(O, L, P),
//!     origin_live_on_entry(O, P).
//!
//! errors(L, P) :-
//!     loan_live_at(L, P),
//!     loan_invalidated_at(L, P).
//! ```

use std::collections::{HashMap, HashSet};

use super::facts::{Loan, Mutability, Origin, Point, PoloniusFacts};

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// A borrow error detected by the Polonius solver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowError {
    /// The loan that caused the error.
    pub loan: Loan,
    /// The program point where the error was detected.
    pub point: Point,
    /// The kind of borrow error.
    pub kind: BorrowErrorKind,
}

/// Classification of borrow errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorrowErrorKind {
    /// Using a value after it was moved.
    UseAfterMove,
    /// A reference outlives the value it borrows.
    DanglingReference,
    /// Two incompatible borrows overlap (e.g., shared + mutable).
    ConflictingBorrow,
}

impl std::fmt::Display for BorrowErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BorrowErrorKind::UseAfterMove => write!(f, "use after move"),
            BorrowErrorKind::DanglingReference => write!(f, "dangling reference"),
            BorrowErrorKind::ConflictingBorrow => write!(f, "conflicting borrow"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PoloniusResult
// ═══════════════════════════════════════════════════════════════════════

/// The result of running the Polonius solver.
#[derive(Debug, Clone)]
pub struct PoloniusResult {
    /// Detected borrow errors.
    pub errors: Vec<BorrowError>,
    /// The set of (loan, point) pairs where a loan is live.
    pub loan_live_at: HashSet<(Loan, Point)>,
    /// Number of iterations the solver took to reach fixpoint.
    pub iterations: u32,
}

impl PoloniusResult {
    /// Returns true if no borrow errors were detected.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the number of detected errors.
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PoloniusSolver — iterative fixed-point computation
// ═══════════════════════════════════════════════════════════════════════

/// Iterative fixed-point Datalog solver for Polonius.
///
/// Repeatedly applies Datalog rules to derive new facts until no new
/// facts are produced (fixpoint). Then detects errors by finding loans
/// that are both live and invalidated at the same program point.
pub struct PoloniusSolver {
    /// Maximum number of iterations before giving up.
    max_iterations: u32,
}

impl PoloniusSolver {
    /// Creates a new solver with default max iterations (1000).
    pub fn new() -> Self {
        Self {
            max_iterations: 1000,
        }
    }

    /// Creates a solver with a custom iteration limit.
    pub fn with_max_iterations(max_iterations: u32) -> Self {
        Self { max_iterations }
    }

    /// Runs the solver on the given facts and returns the result.
    pub fn solve(&self, facts: &PoloniusFacts) -> PoloniusResult {
        let mut state = SolverState::from_facts(facts);
        let iterations = state.run_to_fixpoint(self.max_iterations);
        let loan_live_at = state.compute_loan_live_at(facts);
        let errors = state.compute_errors(facts, &loan_live_at);

        PoloniusResult {
            errors,
            loan_live_at,
            iterations,
        }
    }
}

impl Default for PoloniusSolver {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SolverState — internal state for fixed-point iteration
// ═══════════════════════════════════════════════════════════════════════

/// Internal solver state tracking derived relations during iteration.
struct SolverState {
    /// Derived: origin_contains_loan_on_entry(origin, loan, point).
    origin_contains: HashSet<(Origin, Loan, Point)>,
    /// Input: cfg_edge(from, to).
    cfg_edges: HashSet<(Point, Point)>,
    /// Input: loan_killed_at(loan, point).
    killed: HashSet<(Loan, Point)>,
    /// Input: subset(origin1, origin2, point).
    subsets: HashSet<(Origin, Origin, Point)>,
}

impl SolverState {
    /// Initializes solver state from input facts.
    fn from_facts(facts: &PoloniusFacts) -> Self {
        let mut origin_contains = HashSet::new();

        // Seed from loan_issued_at.
        for (origin, loan, point) in &facts.loan_issued_at {
            origin_contains.insert((*origin, loan.clone(), *point));
        }

        // Also seed from explicit origin_contains_loan_on_entry.
        for (origin, loan, point) in &facts.origin_contains_loan_on_entry {
            origin_contains.insert((*origin, loan.clone(), *point));
        }

        let cfg_edges: HashSet<_> = facts.cfg_edge.iter().copied().collect();
        let killed: HashSet<_> = facts.loan_killed_at.iter().cloned().collect();
        let subsets: HashSet<_> = facts.subset.iter().copied().collect();

        Self {
            origin_contains,
            cfg_edges,
            killed,
            subsets,
        }
    }

    /// Runs the solver to fixpoint, returning the iteration count.
    fn run_to_fixpoint(&mut self, max_iterations: u32) -> u32 {
        let mut iteration = 0;
        loop {
            iteration += 1;
            if iteration > max_iterations {
                break;
            }

            let added = self.step();
            if !added {
                break;
            }
        }
        iteration
    }

    /// Performs one iteration of the Datalog rules.
    ///
    /// Returns `true` if any new facts were derived.
    fn step(&mut self) -> bool {
        let mut new_facts: Vec<(Origin, Loan, Point)> = Vec::new();

        self.propagate_along_edges(&mut new_facts);
        self.propagate_subsets(&mut new_facts);

        let mut added = false;
        for fact in new_facts {
            if self.origin_contains.insert(fact) {
                added = true;
            }
        }
        added
    }

    /// Rule: propagate loans along CFG edges unless killed.
    ///
    /// ```text
    /// origin_contains_loan_on_entry(O, L, P2) :-
    ///     origin_contains_loan_on_entry(O, L, P1),
    ///     cfg_edge(P1, P2),
    ///     !loan_killed_at(L, P1).
    /// ```
    fn propagate_along_edges(&self, new_facts: &mut Vec<(Origin, Loan, Point)>) {
        let current: Vec<_> = self.origin_contains.iter().cloned().collect();

        for (origin, loan, p1) in &current {
            // Skip if the loan is killed at this point.
            if self.killed.contains(&(loan.clone(), *p1)) {
                continue;
            }

            // Propagate to successors.
            for &(from, to) in &self.cfg_edges {
                if from == *p1 {
                    let new = (*origin, loan.clone(), to);
                    if !self.origin_contains.contains(&new) {
                        new_facts.push(new);
                    }
                }
            }
        }
    }

    /// Rule: propagate loans through subset relationships.
    ///
    /// ```text
    /// origin_contains_loan_on_entry(O2, L, P) :-
    ///     subset(O1, O2, P),
    ///     origin_contains_loan_on_entry(O1, L, P).
    /// ```
    fn propagate_subsets(&self, new_facts: &mut Vec<(Origin, Loan, Point)>) {
        for &(o1, o2, p) in &self.subsets {
            let current: Vec<_> = self.origin_contains.iter().cloned().collect();
            for (origin, loan, point) in &current {
                if *origin == o1 && *point == p {
                    let new = (o2, loan.clone(), p);
                    if !self.origin_contains.contains(&new) {
                        new_facts.push(new);
                    }
                }
            }
        }
    }

    /// Computes loan_live_at from the derived facts and input liveness.
    ///
    /// ```text
    /// loan_live_at(L, P) :-
    ///     origin_contains_loan_on_entry(O, L, P),
    ///     origin_live_on_entry(O, P).
    /// ```
    fn compute_loan_live_at(&self, facts: &PoloniusFacts) -> HashSet<(Loan, Point)> {
        let live_origins: HashSet<(Origin, Point)> =
            facts.origin_live_on_entry.iter().copied().collect();

        let mut loan_live = HashSet::new();
        for (origin, loan, point) in &self.origin_contains {
            if live_origins.contains(&(*origin, *point)) {
                loan_live.insert((loan.clone(), *point));
            }
        }
        loan_live
    }

    /// Detects errors: loans that are live and invalidated at the same point.
    ///
    /// ```text
    /// errors(L, P) :-
    ///     loan_live_at(L, P),
    ///     loan_invalidated_at(L, P).
    /// ```
    fn compute_errors(
        &self,
        facts: &PoloniusFacts,
        loan_live_at: &HashSet<(Loan, Point)>,
    ) -> Vec<BorrowError> {
        let mut errors = Vec::new();

        for (loan, point) in &facts.loan_invalidated_at {
            if loan_live_at.contains(&(loan.clone(), *point)) {
                let kind = classify_error(loan);
                errors.push(BorrowError {
                    loan: loan.clone(),
                    point: *point,
                    kind,
                });
            }
        }

        self.detect_conflicting_borrows(facts, &mut errors);
        errors
    }

    /// Detects conflicting borrows at the same point.
    fn detect_conflicting_borrows(&self, _facts: &PoloniusFacts, errors: &mut Vec<BorrowError>) {
        // Group loans by place and point.
        let mut place_loans: HashMap<(String, Point), Vec<Loan>> = HashMap::new();

        for (_, loan, point) in &self.origin_contains {
            let key = (loan.place.base.clone(), *point);
            place_loans.entry(key).or_default().push(loan.clone());
        }

        for ((_place, point), loans) in &place_loans {
            if loans.len() < 2 {
                continue;
            }

            let has_mutable = loans.iter().any(|l| l.mutability == Mutability::Mutable);
            let has_shared = loans.iter().any(|l| l.mutability == Mutability::Shared);
            let mut_count = loans
                .iter()
                .filter(|l| l.mutability == Mutability::Mutable)
                .count();

            if (has_mutable && has_shared) || mut_count > 1 {
                if let Some(mut_loan) = loans.iter().find(|l| l.mutability == Mutability::Mutable) {
                    errors.push(BorrowError {
                        loan: mut_loan.clone(),
                        point: *point,
                        kind: BorrowErrorKind::ConflictingBorrow,
                    });
                }
            }
        }
    }
}

/// Classifies a borrow error based on the loan's properties.
fn classify_error(loan: &Loan) -> BorrowErrorKind {
    match loan.mutability {
        Mutability::Mutable => BorrowErrorKind::ConflictingBorrow,
        Mutability::Shared => BorrowErrorKind::DanglingReference,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::polonius::facts::{
        Mutability, Origin, Place, Point, PointKind, PoloniusFacts,
    };
    use crate::lexer::token::Span;

    fn span(s: usize, e: usize) -> Span {
        Span::new(s, e)
    }

    fn point(block: u32, stmt: u32) -> Point {
        Point {
            block,
            statement: stmt,
            kind: PointKind::Start,
        }
    }

    fn mid_point(block: u32, stmt: u32) -> Point {
        Point {
            block,
            statement: stmt,
            kind: PointKind::Mid,
        }
    }

    fn make_loan(id: u32, place: &str, mutability: Mutability) -> Loan {
        Loan {
            id,
            place: Place::from_var(place),
            mutability,
            span: span(0, 10),
        }
    }

    // ── S10.1: Simple correct code has no errors ──────────────────────

    #[test]
    fn s10_1_correct_code_no_errors() {
        // let x = 42
        // let r = &x  (at point 0)
        // use r        (at point 1)
        // -- r not used after this, no invalidation --
        let mut facts = PoloniusFacts::new();
        let o0 = Origin(0);
        let loan = make_loan(0, "x", Mutability::Shared);

        facts.loan_issued_at.push((o0, loan.clone(), point(0, 0)));
        facts.cfg_edge.push((point(0, 0), mid_point(0, 0)));
        facts.cfg_edge.push((mid_point(0, 0), point(0, 1)));
        facts.origin_live_on_entry.push((o0, point(0, 1)));

        let solver = PoloniusSolver::new();
        let result = solver.solve(&facts);

        assert!(result.is_ok(), "expected no errors for correct code");
    }

    // ── S10.2: Use after invalidation (dangling reference) ────────────

    #[test]
    fn s10_2_dangling_reference_detected() {
        // let x = 42           (point 0,0)
        // let r = &x           (point 0,1) -- loan L0
        // x = 100              (point 0,2) -- invalidates L0
        // use r                (point 0,3) -- error: L0 live + invalidated
        let mut facts = PoloniusFacts::new();
        let o0 = Origin(0);
        let loan = make_loan(0, "x", Mutability::Shared);

        facts.loan_issued_at.push((o0, loan.clone(), point(0, 1)));
        facts.loan_invalidated_at.push((loan.clone(), point(0, 2)));
        facts.origin_live_on_entry.push((o0, point(0, 2)));

        // CFG edges: 0→1→2→3
        facts.cfg_edge.push((point(0, 0), point(0, 1)));
        facts.cfg_edge.push((point(0, 1), point(0, 2)));
        facts.cfg_edge.push((point(0, 2), point(0, 3)));

        let solver = PoloniusSolver::new();
        let result = solver.solve(&facts);

        assert!(!result.is_ok(), "expected error for dangling reference");
        assert!(result.errors.iter().any(|e| e.loan.place.base == "x"));
    }

    // ── S10.3: Conflicting borrows (shared + mutable) ─────────────────

    #[test]
    fn s10_3_conflicting_borrow_detected() {
        // let mut x = 42       (point 0,0)
        // let r1 = &x          (point 0,1) -- shared loan L0
        // let r2 = &mut x      (point 0,1) -- mutable loan L1
        // -- both active at same point --
        let mut facts = PoloniusFacts::new();
        let o0 = Origin(0);
        let o1 = Origin(1);
        let shared_loan = make_loan(0, "x", Mutability::Shared);
        let mut_loan = make_loan(1, "x", Mutability::Mutable);

        facts
            .loan_issued_at
            .push((o0, shared_loan.clone(), point(0, 1)));
        facts
            .loan_issued_at
            .push((o1, mut_loan.clone(), point(0, 1)));

        facts.cfg_edge.push((point(0, 0), point(0, 1)));
        facts.cfg_edge.push((point(0, 1), point(0, 2)));

        facts.origin_live_on_entry.push((o0, point(0, 1)));
        facts.origin_live_on_entry.push((o1, point(0, 1)));

        let solver = PoloniusSolver::new();
        let result = solver.solve(&facts);

        assert!(
            result
                .errors
                .iter()
                .any(|e| e.kind == BorrowErrorKind::ConflictingBorrow),
            "expected ConflictingBorrow error"
        );
    }

    // ── S10.4: Loan killed stops propagation ──────────────────────────

    #[test]
    fn s10_4_killed_loan_does_not_propagate() {
        // let r = &x           (point 0,0) -- loan L0
        // x goes out of scope  (point 0,0) -- kill L0
        // next point           (point 0,1)
        let mut facts = PoloniusFacts::new();
        let o0 = Origin(0);
        let loan = make_loan(0, "x", Mutability::Shared);

        facts.loan_issued_at.push((o0, loan.clone(), point(0, 0)));
        facts.loan_killed_at.push((loan.clone(), point(0, 0)));
        facts.cfg_edge.push((point(0, 0), point(0, 1)));
        facts.origin_live_on_entry.push((o0, point(0, 1)));

        let solver = PoloniusSolver::new();
        let result = solver.solve(&facts);

        // The loan should NOT be live at point(0,1) because it was killed.
        let live_at_1 = result
            .loan_live_at
            .iter()
            .any(|(l, p)| l.id == 0 && *p == point(0, 1));
        assert!(
            !live_at_1,
            "killed loan should not propagate past kill point"
        );
    }

    // ── S10.5: Cross-block propagation ────────────────────────────────

    #[test]
    fn s10_5_loan_propagates_across_blocks() {
        // Block 0: let r = &x         (point 0,0) -- loan L0
        // Edge: (0,0) → (1,0)
        // Block 1: use r              (point 1,0)
        let mut facts = PoloniusFacts::new();
        let o0 = Origin(0);
        let loan = make_loan(0, "x", Mutability::Shared);

        facts.loan_issued_at.push((o0, loan.clone(), point(0, 0)));
        facts.cfg_edge.push((point(0, 0), point(1, 0)));
        facts.origin_live_on_entry.push((o0, point(1, 0)));

        let solver = PoloniusSolver::new();
        let result = solver.solve(&facts);

        // Loan should be live in block 1.
        let live_at_b1 = result
            .loan_live_at
            .iter()
            .any(|(l, p)| l.id == 0 && p.block == 1);
        assert!(live_at_b1, "loan should propagate to block 1 via CFG edge");
    }

    // ── S10.6: Subset propagation ─────────────────────────────────────

    #[test]
    fn s10_6_subset_propagates_loans() {
        // origin_contains(O0, L0, P0)
        // subset(O0, O1, P0)
        // => origin_contains(O1, L0, P0)
        let mut facts = PoloniusFacts::new();
        let o0 = Origin(0);
        let o1 = Origin(1);
        let loan = make_loan(0, "x", Mutability::Shared);

        facts.loan_issued_at.push((o0, loan.clone(), point(0, 0)));
        facts.subset.push((o0, o1, point(0, 0)));
        facts.origin_live_on_entry.push((o1, point(0, 0)));

        let solver = PoloniusSolver::new();
        let result = solver.solve(&facts);

        // Loan should be live via subset propagation to O1.
        let live_via_subset = result.loan_live_at.iter().any(|(l, _)| l.id == 0);
        assert!(
            live_via_subset,
            "loan should be live via subset propagation"
        );
    }

    // ── S10.7: Multiple loans on same variable ────────────────────────

    #[test]
    fn s10_7_multiple_shared_borrows_ok() {
        // let r1 = &x  (L0 shared)
        // let r2 = &x  (L1 shared)
        // Multiple shared borrows should be fine.
        let mut facts = PoloniusFacts::new();
        let o0 = Origin(0);
        let o1 = Origin(1);
        let loan0 = make_loan(0, "x", Mutability::Shared);
        let loan1 = make_loan(1, "x", Mutability::Shared);

        facts.loan_issued_at.push((o0, loan0.clone(), point(0, 0)));
        facts.loan_issued_at.push((o1, loan1.clone(), point(0, 1)));
        facts.cfg_edge.push((point(0, 0), point(0, 1)));

        let solver = PoloniusSolver::new();
        let result = solver.solve(&facts);

        // Multiple shared borrows should not produce conflicting borrow errors.
        let has_conflict = result
            .errors
            .iter()
            .any(|e| e.kind == BorrowErrorKind::ConflictingBorrow);
        assert!(!has_conflict, "multiple shared borrows should not conflict");
    }

    // ── S10.8: Double mutable borrow conflict ─────────────────────────

    #[test]
    fn s10_8_double_mutable_borrow_conflict() {
        // let r1 = &mut x  (L0 mutable)
        // let r2 = &mut x  (L1 mutable) at same point
        let mut facts = PoloniusFacts::new();
        let o0 = Origin(0);
        let o1 = Origin(1);
        let loan0 = make_loan(0, "x", Mutability::Mutable);
        let loan1 = make_loan(1, "x", Mutability::Mutable);

        // Both issued at same point.
        facts.loan_issued_at.push((o0, loan0.clone(), point(0, 0)));
        facts.loan_issued_at.push((o1, loan1.clone(), point(0, 0)));

        facts.origin_live_on_entry.push((o0, point(0, 0)));
        facts.origin_live_on_entry.push((o1, point(0, 0)));

        let solver = PoloniusSolver::new();
        let result = solver.solve(&facts);

        assert!(
            result
                .errors
                .iter()
                .any(|e| e.kind == BorrowErrorKind::ConflictingBorrow),
            "double mutable borrow should be a conflict"
        );
    }

    // ── S10.9: Fixpoint terminates ────────────────────────────────────

    #[test]
    fn s10_9_solver_terminates_on_loop_cfg() {
        // CFG with a loop: 0 → 1 → 0
        let mut facts = PoloniusFacts::new();
        let o0 = Origin(0);
        let loan = make_loan(0, "x", Mutability::Shared);

        facts.loan_issued_at.push((o0, loan.clone(), point(0, 0)));
        facts.cfg_edge.push((point(0, 0), point(1, 0)));
        facts.cfg_edge.push((point(1, 0), point(0, 0)));

        let solver = PoloniusSolver::with_max_iterations(100);
        let result = solver.solve(&facts);

        // Should terminate without hitting max iterations excessively.
        assert!(
            result.iterations <= 100,
            "solver should reach fixpoint within iteration limit"
        );
    }

    // ── S10.10: Empty facts produce no errors ─────────────────────────

    #[test]
    fn s10_10_empty_facts_no_errors() {
        let facts = PoloniusFacts::new();
        let solver = PoloniusSolver::new();
        let result = solver.solve(&facts);

        assert!(result.is_ok());
        assert_eq!(result.error_count(), 0);
        assert!(result.loan_live_at.is_empty());
    }
}
