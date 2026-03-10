//! Tree-walking interpreter for Fajar Lang.
//!
//! Evaluates AST nodes and produces runtime [`Value`]s.
//! Phase 1: accepts `&Program` directly (no analyzer).
//! Phase 2+: accepts `&TypedProgram` after semantic analysis.

pub mod env;
pub mod eval;
pub mod ffi;
pub mod value;

pub use eval::{EvalError, Interpreter, RuntimeError};
pub use ffi::FfiManager;
pub use value::Value;
