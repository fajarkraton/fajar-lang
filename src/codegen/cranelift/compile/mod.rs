//! Free-standing codegen functions for Fajar Lang compilation.
//!
//! These functions take `&mut FunctionBuilder` and `&mut CodegenCtx` as parameters
//! (avoids lifetime issues with mutable builder borrows).
//!
//! Sub-modules:
//! - `call` — function call compilation (builtins, enum constructors, generic dispatch)
//! - `method` — method call compilation (string, array, tensor, struct impl methods)
//! - `asm` — inline assembly compilation
//! - `expr` — expression compilation (binary, unary, cast, literals, identifiers)
//! - `stmt` — statement compilation (let, assignment, return, etc.)
//! - `control` — control flow (if, while, loop, for, match)
//! - `builtins` — math/assert/file builtins
//! - `arrays` — array operations
//! - `strings` — string operations
//! - `structs` — struct operations

mod arrays;
mod asm;
mod builtins;
mod call;
mod control;
mod expr;
mod method;
mod stmt;
mod strings;
mod structs;

pub(in crate::codegen::cranelift) use arrays::*;
pub(in crate::codegen::cranelift) use asm::*;
pub(in crate::codegen::cranelift) use builtins::*;
pub(in crate::codegen::cranelift) use call::*;
pub(in crate::codegen::cranelift) use control::*;
pub(in crate::codegen::cranelift) use expr::*;
pub(in crate::codegen::cranelift) use method::*;
pub(in crate::codegen::cranelift) use stmt::*;
pub(in crate::codegen::cranelift) use strings::*;
pub(in crate::codegen::cranelift) use structs::*;
