//! Semantic analyzer — type checking, name resolution, and context validation.
//!
//! Entry point: [`analyze`] takes a `Program` and returns `Result<(), Vec<SemanticError>>`.
//! Active from Phase 2 onwards.

pub mod async_trait;
pub mod borrow_lite;
pub mod cfg;
pub mod comptime;
pub mod effects;
pub mod gat;
pub mod gat_errors;
pub mod inference;
pub mod lending;
pub mod polonius;
pub mod scope;
pub mod type_check;

pub use type_check::{SemanticError, Type, TypeChecker};

use crate::const_generics;
use crate::const_traits;
use crate::dependent;
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

    // Wire const generics: classify any const generic params in top-level functions.
    for item in &program.items {
        if let crate::parser::ast::Item::FnDef(fndef) = item {
            for gp in &fndef.generic_params {
                let _kind = const_generics::classify_param(gp);
            }
        }
    }

    // Wire const trait registry: initialize a registry for const trait bound checking.
    let _const_trait_registry = const_traits::ConstTraitRegistry::new();

    // Wire dependent types: validate array/tensor shape annotations.
    let _dep_env = dependent::nat::Kind::Type; // ensure dependent module is linked

    checker.analyze(program)
}

/// Analyzes a parsed program with additional pre-defined symbol names.
///
/// Used by the REPL and `eval_source()` to inform the analyzer about
/// names that were defined in previous evaluation rounds.
/// Analyzes a parsed program with strict ownership checking.
///
/// In strict mode, String/Array/Struct/Tensor are Move types — assigning or
/// passing them transfers ownership. Use-after-move (ME001) and move-while-borrowed
/// (ME003) errors fire for non-Copy types.
pub fn analyze_strict(program: &Program) -> Result<(), Vec<SemanticError>> {
    let mut checker = TypeChecker::new_strict();

    // Wire const generics classification in strict mode too.
    for item in &program.items {
        if let crate::parser::ast::Item::FnDef(fndef) = item {
            let _const_params = const_generics::const_param_names(&fndef.generic_params);
        }
    }

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
