//! Code formatter for Fajar Lang.
//!
//! Formats `.fj` source files with consistent style:
//! - 4-space indentation (no tabs)
//! - Opening brace on same line
//! - Spaces around binary operators
//! - Space after commas
//! - Max 1 consecutive blank line
//! - Trailing newline

pub mod pretty;

use crate::lexer::tokenize_with_comments;
use crate::parser::parse;
use crate::FjError;

/// Formats Fajar Lang source code to canonical style.
///
/// Parses the source, then re-emits it with consistent formatting.
/// Comments are preserved in their relative positions.
///
/// # Returns
///
/// * `Ok(String)` - The formatted source code.
/// * `Err(FjError)` - If the source has lex or parse errors.
pub fn format(source: &str) -> Result<String, FjError> {
    let (tokens, comments) = tokenize_with_comments(source)?;
    let program = parse(tokens)?;
    let mut formatter = pretty::Formatter::new(source, comments);
    formatter.format_program(&program);
    Ok(formatter.finish())
}
