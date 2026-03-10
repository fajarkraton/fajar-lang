//! Semantic analyzer — type checking, name resolution, and context validation.
//!
//! Entry point: [`analyze`] takes a `Program` and returns `Result<(), Vec<SemanticError>>`.
//! Active from Phase 2 onwards.

pub mod borrow_lite;
pub mod cfg;
pub mod inference;
pub mod scope;
pub mod type_check;

pub use type_check::{SemanticError, Type, TypeChecker};

use crate::parser::ast::Program;

/// Analyzes a parsed program for semantic errors.
///
/// Performs type checking, name resolution, and mutability checks.
/// Returns `Ok(())` if the program is well-typed, or `Err(errors)` with
/// all collected semantic errors.
///
/// # Examples
///
/// ```
/// use fajar_lang::lexer::tokenize;
/// use fajar_lang::parser::parse;
/// use fajar_lang::analyzer::analyze;
///
/// let tokens = tokenize("let x: i64 = 42").unwrap();
/// let program = parse(tokens).unwrap();
/// assert!(analyze(&program).is_ok());
/// ```
pub fn analyze(program: &Program) -> Result<(), Vec<SemanticError>> {
    let mut checker = TypeChecker::new();
    checker.analyze(program)
}

/// Analyzes a parsed program with additional pre-defined symbol names.
///
/// Used by the REPL and `eval_source()` to inform the analyzer about
/// names that were defined in previous evaluation rounds.
pub fn analyze_with_known(
    program: &Program,
    known_names: &[String],
) -> Result<(), Vec<SemanticError>> {
    let mut checker = TypeChecker::new();
    checker.register_known_names(known_names);
    checker.analyze(program)
}
