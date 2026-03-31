//! Self-Hosting v2 — analyzer in .fj, codegen in .fj,
//! bootstrap chain, and reproducible builds.
//!
//! Option G additions (Sprints S1-S5):
//! - `ast_tree`: Tree-based AST with 28 Expr variants, visitor, pretty printer
//! - `parser_v2`: 19-level Pratt parser with error recovery
//! - `analyzer_v2`: Type checker with HM inference, scope, trait resolution
//! - `codegen_v2`: 46-opcode bytecode codegen with register allocation
//! - `bootstrap_v2`: Stage 1 bootstrap compiler with verification

pub mod analyzer_fj;
pub mod analyzer_v2;
pub mod ast_tree;
pub mod bootstrap;
pub mod bootstrap_v2;
pub mod codegen_fj;
pub mod codegen_v2;
pub mod diagnostics;
pub mod optimizer;
pub mod parser_v2;
pub mod reproducible;
pub mod self_bench;
pub mod stage2;
pub mod stdlib_self;
