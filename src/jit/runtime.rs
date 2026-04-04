//! JIT Runtime — safe wrapper for compiled native functions.
//!
//! Provides `compile_and_run` which compiles a Fajar Lang program via Cranelift
//! and executes a named function natively. Feature-gated under `--features native`.

use crate::parser::ast::Program;

/// Result of JIT compilation + execution.
#[derive(Debug)]
pub struct JitResult {
    /// The native return value (i64 for integer functions).
    pub value: i64,
    /// Compilation time in microseconds.
    pub compile_time_us: u64,
    /// Number of functions compiled.
    pub functions_compiled: usize,
}

/// Compile a program and execute a named function with i64 args via Cranelift JIT.
///
/// Returns the native function's return value. Only available with `--features native`.
#[cfg(feature = "native")]
pub fn compile_and_run(
    program: &Program,
    fn_name: &str,
    args: &[i64],
) -> Result<JitResult, String> {
    use crate::codegen::cranelift::CraneliftCompiler;

    let start = std::time::Instant::now();

    // Create optimizing compiler
    let mut compiler =
        CraneliftCompiler::with_opt_level("speed").map_err(|e| format!("JIT init: {e}"))?;

    // Compile all functions in the program
    compiler
        .compile_program(program)
        .map_err(|errs| format!("JIT compile: {:?}", errs))?;

    let compile_time = start.elapsed();
    let functions_compiled = program
        .items
        .iter()
        .filter(|i| matches!(i, crate::parser::ast::Item::FnDef(_)))
        .count();

    // Get native function pointer
    let fn_ptr = compiler
        .get_fn_ptr(fn_name)
        .map_err(|e| format!("JIT get_fn_ptr: {e}"))?;

    // Call the native function based on argument count
    // SAFETY: Cranelift generates valid machine code for the host ISA.
    // The function signature must match — we only support i64 args/return for now.
    let value = unsafe {
        match args.len() {
            0 => {
                let f: fn() -> i64 = std::mem::transmute(fn_ptr);
                f()
            }
            1 => {
                let f: fn(i64) -> i64 = std::mem::transmute(fn_ptr);
                f(args[0])
            }
            2 => {
                let f: fn(i64, i64) -> i64 = std::mem::transmute(fn_ptr);
                f(args[0], args[1])
            }
            3 => {
                let f: fn(i64, i64, i64) -> i64 = std::mem::transmute(fn_ptr);
                f(args[0], args[1], args[2])
            }
            n => return Err(format!("JIT: unsupported arg count {n} (max 3)")),
        }
    };

    Ok(JitResult {
        value,
        compile_time_us: compile_time.as_micros() as u64,
        functions_compiled,
    })
}

/// Stub when native feature is not enabled.
#[cfg(not(feature = "native"))]
pub fn compile_and_run(
    _program: &Program,
    _fn_name: &str,
    _args: &[i64],
) -> Result<JitResult, String> {
    Err("JIT compilation requires --features native (Cranelift)".to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn jit_runtime_stub_without_native() {
        // Without native feature, compile_and_run should return an error
        #[cfg(not(feature = "native"))]
        {
            let program = crate::parser::ast::Program {
                items: vec![],
                span: crate::lexer::token::Span::new(0, 0),
            };
            let result = super::compile_and_run(&program, "main", &[]);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("native"));
        }
    }
}
