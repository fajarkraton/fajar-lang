//! LSP server for Fajar Lang.
//!
//! Provides IDE features via the Language Server Protocol:
//! - Real-time diagnostics (lex, parse, semantic errors)
//! - Hover information (types, function signatures)
//! - Go-to-definition
//! - Completions (keywords, builtins, variables)

mod server;

pub use server::run_lsp;
