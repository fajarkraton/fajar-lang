//! `fj repl` interactive session.
//!
//! Extracted from `main.rs` (REFACTOR_2026_07 Phase 2); pure code motion.

use fajar_lang::interpreter::Interpreter;
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;
use std::process::ExitCode;

/// Checks if a source string has balanced braces/parens (for multi-line input).
pub(crate) fn is_balanced(source: &str) -> bool {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut prev = '\0';
    for ch in source.chars() {
        if in_string {
            if ch == '"' && prev != '\\' {
                in_string = false;
            }
        } else {
            match ch {
                '"' => in_string = true,
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' => depth -= 1,
                _ => {}
            }
        }
        prev = ch;
    }
    depth <= 0
}

/// Starts an interactive REPL with multi-line input and REPL commands.
pub(crate) fn cmd_repl() -> ExitCode {
    let build_info = fajar_lang::hardening::BuildInfo::from_env();
    println!("Fajar Lang v{} — Interactive REPL", build_info.version);
    println!("  {}", build_info.summary());
    println!("Type expressions to evaluate. Type 'exit' or Ctrl-D to quit.");
    println!("Commands: :type <expr>, :help");
    println!();

    let mut rl = match rustyline::DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("error: failed to initialize REPL: {e}");
            return ExitCode::from(1);
        }
    };

    let mut interp = Interpreter::new();
    let mut buffer = String::new();

    loop {
        let prompt = if buffer.is_empty() { "fj> " } else { "... " };
        let line = match rl.readline(prompt) {
            Ok(line) => line,
            Err(
                rustyline::error::ReadlineError::Eof | rustyline::error::ReadlineError::Interrupted,
            ) => {
                println!("Bye!");
                break;
            }
            Err(e) => {
                eprintln!("error: {e}");
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() && buffer.is_empty() {
            continue;
        }
        if trimmed == "exit" || trimmed == "quit" {
            println!("Bye!");
            break;
        }

        let _ = rl.add_history_entry(&line);

        // REPL commands
        if buffer.is_empty() {
            if trimmed == ":help" {
                println!("  :type <expr>  — show type of expression without evaluating");
                println!("  :help         — show this help");
                println!("  exit / quit   — exit REPL");
                continue;
            }
            if let Some(expr_src) = trimmed.strip_prefix(":type ") {
                // :type command — type-check without evaluating
                match tokenize(expr_src) {
                    Ok(tokens) => match parse(tokens) {
                        Ok(program) => {
                            let mut tc = fajar_lang::analyzer::type_check::TypeChecker::new();
                            let _ = tc.analyze(&program);
                            // Check the last expression/statement type
                            if let Some(item) = program.items.last() {
                                let ty = match item {
                                    fajar_lang::parser::ast::Item::Stmt(
                                        fajar_lang::parser::ast::Stmt::Expr { expr, .. },
                                    ) => {
                                        let mut tc2 =
                                            fajar_lang::analyzer::type_check::TypeChecker::new();
                                        let ty = tc2.check_expr(expr);
                                        ty.display_name()
                                    }
                                    _ => "void".to_string(),
                                };
                                println!("  : {ty}");
                            } else {
                                println!("  : void");
                            }
                        }
                        Err(errors) => {
                            for e in &errors {
                                eprintln!("  parse error: {e}");
                            }
                        }
                    },
                    Err(errors) => {
                        for e in &errors {
                            eprintln!("  lex error: {e}");
                        }
                    }
                }
                continue;
            }
        }

        // Multi-line input: buffer incomplete expressions
        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(&line);

        if !is_balanced(&buffer) {
            continue;
        }

        let source = buffer.clone();
        buffer.clear();

        // Lex → Parse → Analyze → Eval (via eval_source)
        match interp.eval_source(&source) {
            Ok(val) => {
                if !matches!(val, fajar_lang::interpreter::Value::Null) {
                    println!("{val}");
                }
            }
            Err(e) => {
                eprintln!("  error: {e}");
            }
        }
    }

    ExitCode::SUCCESS
}
