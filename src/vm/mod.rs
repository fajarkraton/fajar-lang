//! Bytecode virtual machine for Fajar Lang.
//!
//! Provides a stack-based VM that executes compiled bytecode,
//! offering significant performance improvements over tree-walking interpretation.
//!
//! # Architecture
//!
//! ```text
//! AST → Compiler → Chunk (bytecode + constants) → VM → Value
//! ```

pub mod chunk;
pub mod compiler;
pub mod engine;
pub mod instruction;

use crate::interpreter::RuntimeError;
use crate::interpreter::value::Value;
use crate::parser::ast::Program;

/// Compiles and runs a program using the bytecode VM.
///
/// This is the high-level entry point for VM execution.
pub fn run_program(program: &Program) -> Result<Value, RuntimeError> {
    let compiler = compiler::Compiler::new();
    let chunk = compiler.compile(program);
    let mut vm = engine::VM::new(chunk);
    vm.run()?;
    vm.call_main()
}

/// Compiles and runs a program using the bytecode VM, capturing output.
pub fn run_program_capturing(program: &Program) -> Result<(Value, Vec<String>), RuntimeError> {
    let compiler = compiler::Compiler::new();
    let chunk = compiler.compile(program);
    let mut vm = engine::VM::new_capturing(chunk);
    vm.run()?;
    let result = vm.call_main()?;
    let output = vm.get_output().to_vec();
    Ok((result, output))
}
