//! Code generation backends for Fajar Lang.
//!
//! Multiple backends are available, each feature-gated:
//! - `native` — Cranelift JIT/AOT compilation to native machine code
//! - `llvm` — LLVM-based compilation via inkwell
//! - `wasm` — WebAssembly compilation (WASI + browser targets)
//!
//! # Architecture
//!
//! ```text
//! AST (Program)
//!     │
//!     ├──► CraneliftCompiler  (native)   → JIT / .o file
//!     ├──► LlvmCompiler       (llvm)     → JIT / .o file
//!     └──► WasmCompiler       (wasm)     → .wasm binary
//! ```

pub mod aarch64_asm;
pub mod amx;
pub mod analysis;
pub mod avx10;
pub mod avx512;
pub mod benchmarks;
pub mod interop;
pub mod nostd;
pub mod optimizer;
pub mod pgo;
pub mod ptx;

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

#[cfg(feature = "llvm")]
pub mod llvm;

#[cfg(feature = "wasm")]
pub mod wasm;

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

    /// NS001: no_std violation in bare-metal/kernel compilation.
    #[error("[NS001] no_std violation: {0}")]
    NoStdViolation(String),

    /// CE011: Context annotation violation in codegen.
    #[error("[CE011] context violation: {0}")]
    ContextViolation(String),
}
