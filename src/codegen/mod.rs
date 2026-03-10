//! Native code generation via Cranelift.
//!
//! Feature-gated behind `native`. When enabled, provides JIT compilation
//! of Fajar Lang programs to native machine code.
//!
//! # Architecture
//!
//! ```text
//! AST (Program)
//!     │
//!     ▼
//! CraneliftCompiler
//!     ├── types.rs    — Fajar types → Cranelift types
//!     ├── abi.rs      — calling convention, value layout
//!     └── cranelift.rs — IR generation, function compilation
//!     │
//!     ▼
//! JIT execution (cranelift-jit)
//! ```

pub mod analysis;
pub mod nostd;

#[cfg(feature = "native")]
pub mod abi;
#[cfg(feature = "native")]
pub mod cranelift;
#[cfg(feature = "native")]
pub mod linker;
#[cfg(feature = "native")]
pub mod target;
#[cfg(feature = "native")]
pub mod types;

/// Error type for code generation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CodegenError {
    /// CE001: Unsupported expression for native compilation.
    #[error("[CE001] unsupported expression for native codegen: {0}")]
    UnsupportedExpr(String),

    /// CE002: Unsupported statement for native compilation.
    #[error("[CE002] unsupported statement for native codegen: {0}")]
    UnsupportedStmt(String),

    /// CE003: Type lowering failure.
    #[error("[CE003] cannot lower type to native: {0}")]
    TypeLoweringError(String),

    /// CE004: Function definition error.
    #[error("[CE004] function codegen error: {0}")]
    FunctionError(String),

    /// CE005: Undefined variable in native codegen.
    #[error("[CE005] undefined variable in codegen: {0}")]
    UndefinedVariable(String),

    /// CE006: Undefined function in native codegen.
    #[error("[CE006] undefined function in codegen: {0}")]
    UndefinedFunction(String),

    /// CE007: ABI error (calling convention mismatch).
    #[error("[CE007] ABI error: {0}")]
    AbiError(String),

    /// CE008: Module error (JIT/object finalization).
    #[error("[CE008] module error: {0}")]
    ModuleError(String),

    /// CE009: Internal compiler error.
    #[error("[CE009] internal codegen error: {0}")]
    Internal(String),

    /// CE010: Feature not yet implemented in native codegen.
    #[error("[CE010] not yet implemented in native codegen: {0}")]
    NotImplemented(String),
}
