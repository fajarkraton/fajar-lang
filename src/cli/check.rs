//! Static analysis commands: check, dump-tokens, dump-ast, call-graph.
//!
//! Extracted from `main.rs` (REFACTOR_2026_07 Phase 2); pure code motion.

use super::util::read_source;
use crate::EXIT_COMPILE;
use fajar_lang::FjDiagnostic;
use fajar_lang::analyzer::analyze;
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;
use std::path::PathBuf;
use std::process::ExitCode;

/// Checks a file for lex/parse errors without executing.
/// Prints cross-context call graph analysis.
pub(crate) fn cmd_call_graph(path: &std::path::Path) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };

    println!("Cross-Context Call Graph: {}", path.display());
    println!("═══════════════════════════════════════════");

    let mut kernel_fns = Vec::new();
    let mut device_fns = Vec::new();
    let mut safe_fns = Vec::new();

    // Extract annotated functions
    let mut current_annotation: Option<String> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("@kernel") {
            current_annotation = Some("kernel".to_string());
        } else if trimmed.starts_with("@device") {
            current_annotation = Some("device".to_string());
        } else if trimmed.starts_with("@safe") {
            current_annotation = Some("safe".to_string());
        }

        if trimmed.contains("fn ") && trimmed.contains('(') {
            let fn_name = trimmed
                .split("fn ")
                .nth(1)
                .and_then(|s| s.split('(').next())
                .map(|s| s.trim().to_string());

            if let Some(name) = fn_name {
                match current_annotation.as_deref() {
                    Some("kernel") => kernel_fns.push(name),
                    Some("device") => device_fns.push(name),
                    Some("safe") => safe_fns.push(name),
                    _ => safe_fns.push(name), // default = safe
                }
            }
            current_annotation = None;
        }
    }

    println!("\n@kernel functions ({}):", kernel_fns.len());
    for f in &kernel_fns {
        println!("  {f}");
    }
    println!("\n@device functions ({}):", device_fns.len());
    for f in &device_fns {
        println!("  {f}");
    }
    println!("\n@safe functions ({}):", safe_fns.len());
    for f in &safe_fns {
        println!("  {f}");
    }

    println!(
        "\nTotal: {} @kernel, {} @device, {} @safe functions",
        kernel_fns.len(),
        device_fns.len(),
        safe_fns.len()
    );
    println!(
        "Context enforcement: checked by analyzer (SE020 for @safe hw access; KE/DE codes for @kernel/@device strict boundaries)"
    );
}

pub(crate) fn cmd_check(path: &PathBuf) -> ExitCode {
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

    match analyze(&program) {
        Ok(()) => {
            println!("OK: {} — no errors found", path.display());
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
            }
            ExitCode::from(EXIT_COMPILE)
        }
    }
}

/// Checks a file with strict ownership mode enabled.
pub(crate) fn cmd_check_strict(path: &PathBuf) -> ExitCode {
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

    match fajar_lang::analyzer::analyze_strict(&program) {
        Ok(()) => {
            println!(
                "OK: {} — no errors found (strict ownership)",
                path.display()
            );
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
            }
            ExitCode::from(EXIT_COMPILE)
        }
    }
}

/// Dumps lexer tokens for a file.
pub(crate) fn cmd_dump_tokens(path: &PathBuf) -> ExitCode {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let filename = path.display().to_string();

    match tokenize(&source) {
        Ok(tokens) => {
            for tok in &tokens {
                println!("  {:>4}:{:<3}  {:?}", tok.line, tok.col, tok.kind);
            }
            println!("({} tokens)", tokens.len());
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_lex_error(e, &filename, &source).eprint();
            }
            ExitCode::from(EXIT_COMPILE)
        }
    }
}

/// Dumps parser AST for a file.
pub(crate) fn cmd_dump_ast(path: &PathBuf) -> ExitCode {
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

    match parse(tokens) {
        Ok(program) => {
            println!("{program:#?}");
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for e in &errors {
                FjDiagnostic::from_parse_error(e, &filename, &source).eprint();
            }
            ExitCode::from(EXIT_COMPILE)
        }
    }
}
