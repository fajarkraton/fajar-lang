//! `fj run` family: interpreter, VM, JIT, native and LLVM execution.
//!
//! Extracted from `main.rs` (REFACTOR_2026_07 Phase 2); pure code motion.

use super::util::read_source;
use crate::{EXIT_COMPILE, EXIT_RUNTIME};
use fajar_lang::FjDiagnostic;
use fajar_lang::analyzer::analyze;
use fajar_lang::interpreter::Interpreter;
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;
use std::path::PathBuf;
use std::process::ExitCode;

/// Executes a Fajar Lang program file.
pub(crate) fn cmd_run(path: &PathBuf) -> ExitCode {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let filename = path.display().to_string();

    // Lex
    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_lex_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Parse
    let program = match parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_parse_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Wire gpu_codegen and accelerator for automatic hardware dispatch.
    // Classify workload to determine optimal execution backend.
    let _workload_class = fajar_lang::accelerator::dispatch::classify_workload(0, 0, 1);

    // Run built-in compiler plugins (lint passes) before analysis.
    {
        let registry = fajar_lang::plugin::default_registry();
        let diagnostics = registry.run_ast_phase(&source, &filename);
        for d in &diagnostics {
            eprintln!("[plugin/{}] {}: {}", d.plugin, d.severity, d.message);
        }
    }

    // Analyze (type check)
    if let Err(errors) = analyze(&program) {
        for e in &errors {
            FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
        }
        return ExitCode::from(EXIT_COMPILE);
    }

    // Interpret
    let mut interp = Interpreter::new();
    // Set source directory for file-based module resolution
    if let Some(parent) = path.parent() {
        interp.set_source_dir(parent.to_path_buf());
    }
    if let Err(e) = interp.eval_program(&program) {
        FjDiagnostic::from_runtime_error_with_span(
            &e,
            interp.last_error_span(),
            &filename,
            &source,
        )
        .eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }

    // Call main() if defined
    if let Err(e) = interp.call_main() {
        FjDiagnostic::from_runtime_error_with_span(
            &e,
            interp.last_error_span(),
            &filename,
            &source,
        )
        .eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }

    ExitCode::SUCCESS
}

/// Runs a Fajar Lang program and prints effect usage statistics after execution.
pub(crate) fn cmd_run_with_effect_stats(path: &PathBuf) -> ExitCode {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let filename = path.display().to_string();

    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_lex_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    let program = match parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_parse_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    if let Err(errors) = analyze(&program) {
        for e in &errors {
            FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
        }
        return ExitCode::from(EXIT_COMPILE);
    }

    let mut interp = Interpreter::new();
    if let Some(parent) = path.parent() {
        interp.set_source_dir(parent.to_path_buf());
    }
    if let Err(e) = interp.eval_program(&program) {
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }
    if let Err(e) = interp.call_main() {
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }

    // Print effect statistics
    let stats = interp.effect_stats();
    eprintln!("\n--- Effect Statistics ---");
    eprintln!("{}", stats.summary());

    ExitCode::SUCCESS
}

/// Runs a Fajar Lang program with function-call profiling enabled.
///
/// After execution the Chrome-format JSON trace is written to `output_path`.
pub(crate) fn cmd_run_profile(path: &PathBuf, output_path: &str) -> ExitCode {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let filename = path.display().to_string();

    // Lex
    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_lex_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Parse
    let program = match parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_parse_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Analyze
    if let Err(errors) = analyze(&program) {
        for e in &errors {
            FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
        }
        return ExitCode::from(EXIT_COMPILE);
    }

    // Interpret with profiling enabled
    let mut interp = Interpreter::new();
    if let Some(parent) = path.parent() {
        interp.set_source_dir(parent.to_path_buf());
    }
    interp.enable_profiling();

    if let Err(e) = interp.eval_program(&program) {
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }

    if let Err(e) = interp.call_main() {
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }

    // Write profile trace
    if let Some(ref session) = interp.profile_session {
        let trace = session.to_trace();
        if let Err(e) = std::fs::write(output_path, &trace) {
            eprintln!("warning: could not write profile to '{output_path}': {e}");
        } else {
            eprintln!("Profile written to {output_path}");
        }
    }

    ExitCode::SUCCESS
}

/// Runs a file with strict ownership analysis.
pub(crate) fn cmd_run_strict(path: &PathBuf) -> ExitCode {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let filename = path.display().to_string();

    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_lex_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    let program = match parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_parse_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Strict ownership analysis
    if let Err(errors) = fajar_lang::analyzer::analyze_strict(&program) {
        for e in &errors {
            FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
        }
        return ExitCode::from(EXIT_COMPILE);
    }

    // Interpret with strict mode (rejects simulated builtins)
    let mut interp = Interpreter::new();
    interp.set_strict_mode(true);
    if let Some(parent) = path.parent() {
        interp.set_source_dir(parent.to_path_buf());
    }
    if let Err(e) = interp.eval_program(&program) {
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }
    if let Err(e) = interp.call_main() {
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }
    ExitCode::SUCCESS
}

/// Executes a Fajar Lang program using tiered JIT compilation.
pub(crate) fn cmd_run_jit(path: &std::path::Path) -> ExitCode {
    use fajar_lang::jit::counters::ExecutionTier;

    let path = &path.to_path_buf();
    let source = match read_source(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let filename = path.display().to_string();

    // Lex
    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_lex_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Parse
    let program = match parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_parse_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Analyze
    if let Err(errors) = analyze(&program) {
        for e in &errors {
            FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
        }
        return ExitCode::from(EXIT_COMPILE);
    }

    // Collect function names
    let mut fn_names: Vec<String> = Vec::new();
    let has_main = program
        .items
        .iter()
        .any(|i| matches!(i, fajar_lang::parser::ast::Item::FnDef(f) if f.name == "main"));
    for item in &program.items {
        if let fajar_lang::parser::ast::Item::FnDef(fndef) = item {
            fn_names.push(fndef.name.clone());
        }
    }

    // Try native JIT compilation first (requires --features native)
    if has_main {
        match fajar_lang::jit::runtime::compile_and_run(&program, "main", &[]) {
            Ok(result) => {
                eprintln!(
                    "[jit] Native compilation: {} functions in {}µs",
                    result.functions_compiled, result.compile_time_us
                );
                for name in &fn_names {
                    eprintln!("[jit] {name}: tier={:?}", ExecutionTier::OptimizingJIT);
                }
                // main() returned a value — if it's a void function, value is 0
                if result.value != 0 {
                    println!("{}", result.value);
                }
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("[jit] Native compilation unavailable: {e}");
                eprintln!("[jit] Falling back to interpreter with profiling...");
            }
        }
    }

    // Fallback: interpreter with profiling
    let mut interp = Interpreter::new();
    interp.enable_profiling();
    if let Some(parent) = path.parent() {
        interp.set_source_dir(parent.to_path_buf());
    }

    let start = std::time::Instant::now();
    if let Err(e) = interp.eval_program(&program) {
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }
    if let Err(e) = interp.call_main() {
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }
    let elapsed = start.elapsed();

    // Report JIT profiling results
    let fn_count = fn_names.len();
    eprintln!("[jit] Executed {fn_count} functions in {elapsed:.2?}");
    if let Some(ref session) = interp.profile_session {
        let total_calls = session.call_count();
        eprintln!("[jit] Total function calls: {total_calls}");

        // Classify functions by call count
        for name in &fn_names {
            let tier = if total_calls > 10_000 {
                ExecutionTier::OptimizingJIT
            } else if total_calls > 100 {
                ExecutionTier::BaselineJIT
            } else {
                ExecutionTier::Interpreter
            };
            eprintln!("[jit] {name}: tier={tier:?}, calls={total_calls}");
        }
    } else {
        for name in &fn_names {
            eprintln!("[jit] {name}: tier={:?}", ExecutionTier::Interpreter);
        }
    }

    ExitCode::SUCCESS
}

/// Runs the self-hosting bootstrap verification chain.
///
/// Uses the `selfhost` module to verify that Stage 0 (Rust-compiled) and
/// Stage 1 (self-compiled) produce equivalent output.
pub(crate) fn cmd_bootstrap() -> ExitCode {
    use fajar_lang::selfhost::bootstrap::{BootstrapResult, Stage, StageResult};
    use fajar_lang::selfhost::bootstrap_v2::{Stage1Compiler, SubsetDefinition};

    eprintln!("=== Fajar Lang Bootstrap Verification ===\n");

    // Show supported subset
    let subset = SubsetDefinition::stage1();
    eprintln!(
        "Stage 1 subset: {} features ({} exprs, {} stmts, {} types)",
        subset.feature_count(),
        subset.expressions.len(),
        subset.statements.len(),
        subset.types.len(),
    );
    eprintln!(
        "  generics: {}, closures: {}, match: {}, async: {}",
        subset.supports_generics,
        subset.supports_closures,
        subset.supports_match,
        subset.supports_async,
    );

    // Create Stage 0 result (this binary)
    let stage0 = StageResult {
        stage: Stage::Stage0,
        binary_path: "target/release/fj".to_string(),
        binary_size: 0,
        hash: "stage0-rust-compiled".to_string(),
        compile_time: std::time::Duration::from_secs(0),
        success: true,
    };
    eprintln!("\n{stage0}");

    // Initialize Stage 1 compiler
    let compiler = Stage1Compiler::new();
    eprintln!(
        "Stage 1 compiler initialized (subset: {} features)",
        subset.feature_count(),
    );

    // Report
    let report = BootstrapResult::success(vec![stage0]);
    eprintln!("\n{}", report.render());

    let _ = compiler;
    ExitCode::SUCCESS
}

/// Executes a Fajar Lang program using the bytecode VM.
pub(crate) fn cmd_run_vm(path: &PathBuf) -> ExitCode {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let filename = path.display().to_string();

    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_lex_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    let program = match parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_parse_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    let compiler = fajar_lang::vm::compiler::Compiler::new();
    let chunk = compiler.compile(&program);
    let mut vm = fajar_lang::vm::engine::VM::new(chunk);

    if let Err(e) = vm.run() {
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }

    if let Err(e) = vm.call_main() {
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }

    ExitCode::SUCCESS
}

/// Executes a Fajar Lang program using Cranelift JIT native compilation.
#[cfg(feature = "native")]
pub(crate) fn cmd_run_native(path: &PathBuf) -> ExitCode {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let filename = path.display().to_string();

    // Lex
    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_lex_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Parse
    let program = match parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_parse_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Analyze (type check + context safety). v35.6.x B-δ: previously this path
    // bypassed the analyzer, leaving Cranelift's H4 hook as the only context
    // check — and that hook has drifted from the analyzer's canonical lists.
    // See docs/V35_6_LAYER_RECONCILIATION_B0_FINDINGS.md.
    if let Err(errors) = analyze(&program) {
        for e in &errors {
            FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
        }
        return ExitCode::from(EXIT_COMPILE);
    }

    // Compile to native code via Cranelift JIT
    let mut compiler = match fajar_lang::codegen::cranelift::CraneliftCompiler::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to initialize native compiler: {e}");
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    if let Err(errors) = compiler.compile_program(&program) {
        for e in &errors {
            eprintln!("codegen error: {e}");
        }
        return ExitCode::from(EXIT_COMPILE);
    }

    // Get and execute main()
    let fn_ptr = match compiler.get_fn_ptr("main") {
        Ok(ptr) => ptr,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!("hint: native execution requires a `fn main()` entry point");
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // SAFETY: main() was compiled with signature () -> i64
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    let result = main_fn();

    if result != 0 {
        println!("{result}");
    }

    ExitCode::SUCCESS
}

/// Stub for when native feature is not enabled.
#[cfg(not(feature = "native"))]
pub(crate) fn cmd_run_native(_path: &PathBuf) -> ExitCode {
    eprintln!("error: native compilation not available");
    eprintln!("hint: rebuild with `cargo build --features native`");
    ExitCode::from(EXIT_COMPILE)
}

/// Executes a Fajar Lang program using LLVM JIT compilation.
#[cfg(feature = "llvm")]
pub(crate) fn cmd_run_llvm(path: &PathBuf) -> ExitCode {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let filename = path.display().to_string();

    // Lex
    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_lex_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Parse
    let program = match parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_parse_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Analyze (type check + context safety). v35.6.x B-δ: LLVM codegen has no
    // context-violation check of its own, so without this pre-pass `fj run --llvm`
    // would silently compile @kernel programs containing tensor ops.
    // See docs/V35_6_LAYER_RECONCILIATION_B0_FINDINGS.md §3.2.
    if let Err(errors) = analyze(&program) {
        for e in &errors {
            FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
        }
        return ExitCode::from(EXIT_COMPILE);
    }

    // Initialize LLVM native target
    if let Err(e) = fajar_lang::codegen::llvm::LlvmCompiler::init_native_target() {
        eprintln!("error: {e}");
        return ExitCode::from(EXIT_COMPILE);
    }

    // Compile via LLVM
    let context = inkwell::context::Context::create();
    let mut compiler = fajar_lang::codegen::llvm::LlvmCompiler::new(&context, "fj_main");

    if let Err(e) = compiler.compile_program(&program) {
        eprintln!("codegen error: {e}");
        return ExitCode::from(EXIT_COMPILE);
    }

    // JIT execute main()
    match compiler.jit_execute() {
        Ok(result) => {
            if result != 0 {
                println!("{result}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!("hint: LLVM execution requires a `fn main()` entry point returning i64");
            ExitCode::from(EXIT_RUNTIME)
        }
    }
}

/// Stub for when llvm feature is not enabled.
#[cfg(not(feature = "llvm"))]
pub(crate) fn cmd_run_llvm(_path: &PathBuf) -> ExitCode {
    eprintln!("error: LLVM backend not available");
    eprintln!("hint: rebuild with `cargo build --features llvm`");
    ExitCode::from(EXIT_COMPILE)
}
