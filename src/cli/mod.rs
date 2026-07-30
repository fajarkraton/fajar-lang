//! CLI command handlers, one module per command family.
//!
//! Extracted from `main.rs` (REFACTOR_2026_07 Phase 2). `main.rs` keeps
//! only the clap definitions and the dispatch loop; handlers re-export
//! flat so dispatch call sites stay unchanged.

pub(crate) mod build;
pub(crate) mod check;
pub(crate) mod demo;
pub(crate) mod dev;
pub(crate) mod project;
pub(crate) mod registry;
pub(crate) mod repl;
pub(crate) mod run;
pub(crate) mod util;

pub(crate) use build::*;
pub(crate) use check::*;
pub(crate) use demo::*;
pub(crate) use dev::*;
pub(crate) use project::*;
pub(crate) use registry::*;
pub(crate) use repl::*;
pub(crate) use run::*;
