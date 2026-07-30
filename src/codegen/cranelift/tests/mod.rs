// Test-only file: float literals like 3.14 / 6.28 are intentional approximations
// used as Fajar Lang source code inside r#""# blocks (where Rust clippy lints
// don't apply to the .fj DSL semantics) AND as matching Rust-side assertions
// at literal precision. Using std::f64::consts::PI/TAU on the Rust side would
// silently mismatch the .fj source's 3.14/6.28 literals.
#![allow(clippy::approx_constant)]

use super::*;
use crate::lexer::tokenize;
use crate::parser::parse;

mod baremetal_os_security;
mod basics;
mod builtins_methods;
mod collections_async;
mod floats_strings_arrays;
mod ml_repr_selfhost;
mod modules_sync_asm;
mod parity;
mod tensors_generics;
mod types_patterns_traits;
mod v04_generics_async;

/// Helper: compile source and execute `main()` -> i64.
fn compile_and_run(source: &str) -> i64 {
    let tokens = tokenize(source).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    // SAFETY: main() compiled with signature () -> i64
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    main_fn()
}

/// Helper: compile source with `fn main() -> f64` and execute.
fn compile_and_run_f64(source: &str) -> f64 {
    let tokens = tokenize(source).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    // SAFETY: main() compiled with signature () -> f64
    let main_fn: fn() -> f64 = unsafe { std::mem::transmute(fn_ptr) };
    main_fn()
}

fn compile_and_run_optimized(source: &str) -> i64 {
    let tokens = tokenize(source).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler =
        CraneliftCompiler::with_opt_level("speed").expect("compiler init with speed failed");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    // SAFETY: main() compiled with signature () -> i64
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    main_fn()
}

/// Helper: run source with interpreter, call main(), return i64 result.
fn interpret_main(source: &str) -> i64 {
    let mut interp = crate::interpreter::Interpreter::new();
    // First, evaluate the source to define all functions
    let _ = interp.eval_source(source).expect("interpreter failed");
    // Then call main() explicitly
    let result = interp
        .eval_source("main()")
        .expect("interpreter main() failed");
    match result {
        crate::interpreter::value::Value::Int(n) => n,
        _ => panic!("main() did not return Int, got: {:?}", result),
    }
}

/// Helper: compile and run with security enabled.
fn compile_and_run_with_security(src: &str) -> i64 {
    let tokens = crate::lexer::tokenize(src).expect("lex");
    let program = crate::parser::parse(tokens).expect("parse");
    let _ = crate::analyzer::analyze(&program);
    let mut compiler = super::CraneliftCompiler::new().expect("compiler");
    compiler.enable_security();
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main ptr");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    main_fn()
}
