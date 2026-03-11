//! Polonius-style borrow checker for Fajar Lang.
//!
//! Implements a Datalog-based borrow checker inspired by Rust's Polonius project.
//! This module provides an upgrade path from the existing NLL borrow checker
//! (`borrow_lite.rs`) with more precise analysis via fact generation and
//! iterative fixed-point solving.
//!
//! # Architecture
//!
//! ```text
//! AST → FactGenerator → PoloniusFacts → PoloniusSolver → PoloniusResult
//!                                              ↓
//!                                    ErrorFormatter → Diagnostics
//! ```
//!
//! # Modules
//!
//! - [`facts`] — Fact types (`Origin`, `Loan`, `Point`, `Place`) and `FactGenerator`
//! - [`solver`] — Iterative fixed-point Datalog solver
//! - [`two_phase`] — Two-phase borrowing and reborrowing support
//! - [`errors`] — Error formatting, suggestions, and loan timeline visualization

pub mod errors;
pub mod facts;
pub mod solver;
pub mod two_phase;

pub use errors::{PoloniusConfig, PoloniusDiagnostic, Suggestion};
pub use facts::{
    FactGenerator, Loan, Mutability, Origin, Place, PlaceProjection, Point, PointKind,
    PoloniusFacts,
};
pub use solver::{BorrowError, BorrowErrorKind, PoloniusResult, PoloniusSolver};
pub use two_phase::{BorrowPhase, TwoPhaseAnalyzer};
