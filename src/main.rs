// Edition 2024: collapsible_if lint expanded scope
#![allow(clippy::collapsible_if)]

//! Fajar Lang CLI entry point.
//!
//! Binary name: `fj`
//!
//! # Commands
//!
//! - `run <file.fj>` — Execute a Fajar Lang program
//! - `repl` — Start interactive REPL
//! - `check <file.fj>` — Parse and check (no execution)
//! - `dump-tokens <file.fj>` — Show lexer output
//! - `dump-ast <file.fj>` — Show parser output (JSON)

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use fajar_lang::FjDiagnostic;
use fajar_lang::analyzer::analyze;
use fajar_lang::interpreter::Interpreter;
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;

/// Fajar Lang — A systems programming language for OS and AI/ML.
#[derive(Parser)]
#[command(name = "fj", version = env!("CARGO_PKG_VERSION"), about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Available subcommands.
#[derive(Subcommand)]
enum Command {
    /// Execute a Fajar Lang program (or run from fj.toml if no file given).
    Run {
        /// Path to the .fj source file. If omitted, uses fj.toml entry point.
        file: Option<PathBuf>,
        /// Use bytecode VM instead of tree-walking interpreter.
        #[arg(long)]
        vm: bool,
        /// Use Cranelift JIT native compilation (requires `native` feature).
        #[arg(long)]
        native: bool,
        /// Use LLVM JIT native compilation (requires `llvm` feature).
        #[arg(long)]
        llvm: bool,
    },
    /// Start an interactive REPL.
    Repl,
    /// Parse and check a file (no execution).
    Check {
        /// Path to the .fj source file.
        file: PathBuf,
    },
    /// Show lexer token output for a file.
    DumpTokens {
        /// Path to the .fj source file.
        file: PathBuf,
    },
    /// Show parser AST output (debug format) for a file.
    DumpAst {
        /// Path to the .fj source file.
        file: PathBuf,
    },
    /// Format a Fajar Lang source file.
    Fmt {
        /// Path to the .fj source file.
        file: PathBuf,
        /// Check if file is formatted (exit 1 if not).
        #[arg(long)]
        check: bool,
    },
    /// Start the Language Server Protocol server (for IDE integration).
    Lsp,
    /// Generate a static playground HTML page with examples.
    Playground {
        /// Output directory for playground files.
        #[arg(short, long, default_value = "playground")]
        output: String,
    },
    /// Create a new Fajar Lang project.
    New {
        /// Name of the project to create.
        name: String,
    },
    /// Build a Fajar Lang program to a native binary.
    Build {
        /// Path to the .fj source file. If omitted, uses fj.toml entry point.
        file: Option<PathBuf>,
        /// Target triple for cross-compilation (e.g., aarch64-unknown-linux-gnu).
        #[arg(long, default_value = "host")]
        target: String,
        /// Output binary path. Defaults to source filename without extension.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Disable standard library (for bare-metal targets).
        #[arg(long)]
        no_std: bool,
        /// Linker script path (for bare-metal targets).
        #[arg(long, name = "linker-script")]
        linker_script: Option<String>,
        /// Backend: "cranelift" (default) or "llvm".
        #[arg(long, default_value = "cranelift")]
        backend: String,
        /// LLVM optimization level (0-3). Only used with --backend llvm.
        #[arg(long, name = "opt-level", default_value = "0")]
        opt_level: u8,
        /// Target board for BSP (e.g., stm32f407, esp32, rp2040).
        #[arg(long)]
        board: Option<String>,
        /// Override linker binary (e.g., aarch64-linux-gnu-ld, ld.lld).
        #[arg(long)]
        linker: Option<String>,
        /// Verbose codegen output (function count, DCE stats, compile time).
        #[arg(long, short)]
        verbose: bool,
        /// Enable incremental compilation (cache unchanged functions).
        #[arg(long)]
        incremental: bool,
        /// Release build: uses LLVM backend with -O2 for best codegen quality.
        #[arg(long)]
        release: bool,
    },
    /// Publish a package to the local registry.
    Publish,
    /// Add a dependency to fj.toml.
    Add {
        /// Package name to add (e.g., fj-math).
        package: String,
        /// Version constraint (default: latest).
        #[arg(short, long)]
        version: Option<String>,
    },
    /// Generate HTML documentation from `///` doc comments.
    Doc {
        /// Path to the .fj source file. If omitted, uses fj.toml entry point.
        file: Option<PathBuf>,
        /// Output directory for generated docs (default: ./docs/api/).
        #[arg(short, long, default_value = "docs/api")]
        output: PathBuf,
        /// Open the generated docs in a browser after generation.
        #[arg(long)]
        open: bool,
    },
    /// Run tests in a Fajar Lang file (functions annotated with @test).
    Test {
        /// Path to the .fj source file. If omitted, uses fj.toml entry point.
        file: Option<PathBuf>,
        /// Run only tests matching this pattern.
        #[arg(long)]
        filter: Option<String>,
        /// Include @ignore tests.
        #[arg(long)]
        include_ignored: bool,
    },
    /// Watch .fj files and re-run on change.
    Watch {
        /// Path to the .fj source file. If omitted, uses fj.toml entry point.
        file: Option<PathBuf>,
        /// Auto-run tests instead of the program.
        #[arg(long)]
        test: bool,
    },
    /// Run benchmarks on a Fajar Lang program.
    Bench {
        /// Path to the .fj source file. If omitted, uses fj.toml entry point.
        file: Option<PathBuf>,
        /// Filter benchmark names.
        #[arg(long)]
        filter: Option<String>,
    },
    /// Start a debug session (DAP protocol on stdin/stdout).
    Debug {
        /// Path to the .fj source file. If omitted, uses fj.toml entry point.
        file: Option<PathBuf>,
        /// Use DAP protocol (for IDE integration).
        #[arg(long)]
        dap: bool,
    },
    /// Search the package registry for packages.
    Search {
        /// Search query string.
        query: String,
        /// Maximum number of results.
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    /// Log in to the package registry (stores API key in ~/.fj/credentials).
    Login {
        /// API key (prompted interactively if not provided).
        #[arg(long)]
        token: Option<String>,
        /// Registry URL (default: https://registry.fajarlang.dev).
        #[arg(long)]
        registry: Option<String>,
    },
    /// Yank a published package version (hides from search, does not delete).
    Yank {
        /// Package name.
        package: String,
        /// Version to yank.
        #[arg(long)]
        version: String,
    },
    /// Install a package from the registry into the local packages/ directory.
    Install {
        /// Package name to install.
        package: String,
        /// Specific version (default: latest).
        #[arg(long)]
        version: Option<String>,
        /// Install from local cache only (no network).
        #[arg(long)]
        offline: bool,
    },
    /// Display detected hardware capabilities (CPU, GPU, NPU).
    HwInfo,
    /// Output hardware profile as machine-readable JSON.
    HwJson,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
            file,
            vm,
            native,
            llvm,
        } => {
            let path = match file {
                Some(f) => f,
                None => match resolve_project_entry() {
                    Ok(p) => p,
                    Err(msg) => {
                        eprintln!("error: {msg}");
                        return ExitCode::from(EXIT_USAGE);
                    }
                },
            };
            if llvm {
                cmd_run_llvm(&path)
            } else if native {
                cmd_run_native(&path)
            } else if vm {
                cmd_run_vm(&path)
            } else {
                cmd_run(&path)
            }
        }
        Command::Repl => cmd_repl(),
        Command::Check { file } => cmd_check(&file),
        Command::DumpTokens { file } => cmd_dump_tokens(&file),
        Command::DumpAst { file } => cmd_dump_ast(&file),
        Command::Fmt { file, check } => cmd_fmt(&file, check),
        Command::Lsp => cmd_lsp(),
        Command::Playground { output } => cmd_playground(&output),
        Command::New { name } => cmd_new(&name),
        Command::Build {
            file,
            target,
            output,
            no_std,
            linker_script,
            backend,
            opt_level,
            board,
            linker,
            verbose,
            incremental,
            release,
        } => {
            let path = match file {
                Some(f) => f,
                None => match resolve_project_entry() {
                    Ok(p) => p,
                    Err(msg) => {
                        eprintln!("error: {msg}");
                        return ExitCode::from(EXIT_USAGE);
                    }
                },
            };
            // Try to read linker-script from fj.toml if not given via CLI
            let ls = linker_script.or_else(|| {
                let cwd = std::env::current_dir().ok()?;
                let root = fajar_lang::package::find_project_root(&cwd)?;
                let config =
                    fajar_lang::package::ProjectConfig::from_file(&root.join("fj.toml")).ok()?;
                config.package.linker_script
            });
            if let Some(ref board_name) = board {
                return cmd_build_bsp(&path, board_name, output.as_deref());
            }
            // --release flag: auto-select LLVM with O2
            let effective_backend = if release { "llvm" } else { &backend };
            let effective_opt = if release && opt_level == 0 {
                2
            } else {
                opt_level
            };

            if effective_backend == "llvm" {
                if verbose {
                    eprintln!(
                        "[verbose] Using LLVM backend (O{effective_opt}){}",
                        if release { " [release mode]" } else { "" }
                    );
                }
                cmd_build_llvm(&path, output.as_deref(), effective_opt)
            } else {
                let start = std::time::Instant::now();

                // Incremental compilation: check if source is unchanged
                let skip_compile = if incremental {
                    check_incremental_cache(&path, &target)
                } else {
                    false
                };

                let result = if skip_compile {
                    if verbose {
                        eprintln!(
                            "[incremental] Cache hit — source unchanged, skipping compilation"
                        );
                    }
                    ExitCode::SUCCESS
                } else {
                    let r = cmd_build(
                        &path,
                        &target,
                        output.as_deref(),
                        no_std,
                        ls.as_deref(),
                        linker.as_deref(),
                    );
                    // Update incremental cache on success
                    if incremental && r == ExitCode::SUCCESS {
                        update_incremental_cache(&path, &target);
                    }
                    r
                };

                if verbose {
                    let elapsed = start.elapsed();
                    eprintln!("[verbose] Compile time: {:.2}s", elapsed.as_secs_f64());
                    eprintln!("[verbose] Target: {target}");
                    if let Ok(meta) = std::fs::metadata(&path) {
                        eprintln!("[verbose] Source: {} bytes", meta.len());
                    }
                }
                result
            }
        }
        Command::Doc { file, output, open } => {
            let path = match file {
                Some(f) => f,
                None => match resolve_project_entry() {
                    Ok(p) => p,
                    Err(msg) => {
                        eprintln!("error: {msg}");
                        return ExitCode::from(EXIT_USAGE);
                    }
                },
            };
            cmd_doc(&path, &output, open)
        }
        Command::Publish => cmd_publish(),
        Command::Add { package, version } => cmd_add(&package, version.as_deref()),
        Command::Test {
            file,
            filter,
            include_ignored,
        } => {
            let path = match file {
                Some(f) => f,
                None => match resolve_project_entry() {
                    Ok(p) => p,
                    Err(msg) => {
                        eprintln!("error: {msg}");
                        return ExitCode::from(EXIT_USAGE);
                    }
                },
            };
            cmd_test(&path, filter.as_deref(), include_ignored)
        }
        Command::Watch { file, test } => {
            let path = match file {
                Some(f) => f,
                None => match resolve_project_entry() {
                    Ok(p) => p,
                    Err(msg) => {
                        eprintln!("error: {msg}");
                        return ExitCode::from(EXIT_USAGE);
                    }
                },
            };
            cmd_watch(&path, test)
        }
        Command::Bench { file, filter } => {
            let path = match file {
                Some(f) => f,
                None => match resolve_project_entry() {
                    Ok(p) => p,
                    Err(msg) => {
                        eprintln!("error: {msg}");
                        return ExitCode::from(EXIT_USAGE);
                    }
                },
            };
            cmd_bench(&path, filter.as_deref())
        }
        Command::Debug { file, dap } => {
            if dap {
                cmd_debug_dap()
            } else {
                let path = match file {
                    Some(f) => f,
                    None => match resolve_project_entry() {
                        Ok(p) => p,
                        Err(msg) => {
                            eprintln!("error: {msg}");
                            return ExitCode::from(EXIT_USAGE);
                        }
                    },
                };
                eprintln!(
                    "Interactive debugger for '{}' not yet implemented.",
                    path.display()
                );
                eprintln!("hint: use `fj debug --dap` for IDE integration via DAP protocol");
                ExitCode::from(EXIT_USAGE)
            }
        }
        Command::Search { query, limit } => cmd_search(&query, limit),
        Command::Login { token, registry } => cmd_login(token.as_deref(), registry.as_deref()),
        Command::Yank { package, version } => cmd_yank(&package, &version),
        Command::Install {
            package,
            version,
            offline,
        } => cmd_install(&package, version.as_deref(), offline),
        Command::HwInfo => cmd_hw_info(),
        Command::HwJson => cmd_hw_json(),
    }
}

/// Exit code for runtime errors.
const EXIT_RUNTIME: u8 = 1;
/// Exit code for compile errors (lex, parse, semantic).
const EXIT_COMPILE: u8 = 2;
/// Exit code for usage errors (file not found, bad arguments).
const EXIT_USAGE: u8 = 3;

/// Reads a source file, returning its contents or printing an error.
fn read_source(path: &PathBuf) -> Result<String, ExitCode> {
    if path.is_dir() {
        // Directory mode: concatenate all .fj files in the directory
        read_source_dir(path)
    } else {
        std::fs::read_to_string(path).map_err(|e| {
            eprintln!("error: cannot read '{}': {e}", path.display());
            ExitCode::from(EXIT_USAGE)
        })
    }
}

/// Reads all .fj files in a directory and concatenates them.
/// Files are sorted alphabetically, except main.fj is always last.
fn read_source_dir(dir: &std::path::Path) -> Result<String, ExitCode> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut main_file: Option<PathBuf> = None;

    // Collect all .fj files recursively
    fn collect_fj_files(
        dir: &std::path::Path,
        files: &mut Vec<PathBuf>,
        main_file: &mut Option<PathBuf>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_fj_files(&path, files, main_file);
                } else if path.extension().is_some_and(|e| e == "fj") {
                    if path.file_name().is_some_and(|n| n == "main.fj") {
                        *main_file = Some(path);
                    } else {
                        files.push(path);
                    }
                }
            }
        }
    }

    collect_fj_files(dir, &mut files, &mut main_file);

    // E8: Auto-include shared/ directory for cross-service type definitions
    let mut shared_files: Vec<PathBuf> = Vec::new();
    let mut shared_main: Option<PathBuf> = None;
    for candidate in [dir.join("../shared"), dir.join("../../shared")] {
        if candidate.is_dir() {
            collect_fj_files(&candidate, &mut shared_files, &mut shared_main);
            if !shared_files.is_empty() {
                eprintln!(
                    "info: including {} shared type files from '{}'",
                    shared_files.len(),
                    candidate.display()
                );
                break;
            }
        }
    }

    // Build final file list: shared first, then service files, main.fj last
    shared_files.sort();
    files.sort();
    let mut final_files = shared_files;
    final_files.extend(files);
    if let Some(main) = main_file {
        final_files.push(main);
    }
    let files = final_files;

    if files.is_empty() {
        eprintln!("error: no .fj files found in '{}'", dir.display());
        return Err(ExitCode::from(EXIT_USAGE));
    }

    let mut combined = String::new();
    for f in &files {
        match std::fs::read_to_string(f) {
            Ok(content) => {
                combined.push_str(&format!("\n// ── Source: {} ──\n", f.display()));
                combined.push_str(&content);
                combined.push('\n');
            }
            Err(e) => {
                eprintln!("error: cannot read '{}': {e}", f.display());
                return Err(ExitCode::from(EXIT_USAGE));
            }
        }
    }

    eprintln!(
        "info: concatenated {} .fj files from '{}'",
        files.len(),
        dir.display()
    );
    Ok(combined)
}

/// Executes a Fajar Lang program file.
fn cmd_run(path: &PathBuf) -> ExitCode {
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
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }

    // Call main() if defined
    if let Err(e) = interp.call_main() {
        FjDiagnostic::from_runtime_error(&e, &filename, &source).eprint();
        return ExitCode::from(EXIT_RUNTIME);
    }

    ExitCode::SUCCESS
}

/// Checks if a source string has balanced braces/parens (for multi-line input).
fn is_balanced(source: &str) -> bool {
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
fn cmd_repl() -> ExitCode {
    println!(
        "Fajar Lang v{} — Interactive REPL",
        env!("CARGO_PKG_VERSION")
    );
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

/// Checks a file for lex/parse errors without executing.
fn cmd_check(path: &PathBuf) -> ExitCode {
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

/// Dumps lexer tokens for a file.
fn cmd_dump_tokens(path: &PathBuf) -> ExitCode {
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

/// Executes a Fajar Lang program using the bytecode VM.
fn cmd_run_vm(path: &PathBuf) -> ExitCode {
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
fn cmd_run_native(path: &PathBuf) -> ExitCode {
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
fn cmd_run_native(_path: &PathBuf) -> ExitCode {
    eprintln!("error: native compilation not available");
    eprintln!("hint: rebuild with `cargo build --features native`");
    ExitCode::from(EXIT_COMPILE)
}

/// Executes a Fajar Lang program using LLVM JIT compilation.
#[cfg(feature = "llvm")]
fn cmd_run_llvm(path: &PathBuf) -> ExitCode {
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
fn cmd_run_llvm(_path: &PathBuf) -> ExitCode {
    eprintln!("error: LLVM backend not available");
    eprintln!("hint: rebuild with `cargo build --features llvm`");
    ExitCode::from(EXIT_COMPILE)
}

/// Formats a Fajar Lang source file.
fn cmd_fmt(path: &PathBuf, check: bool) -> ExitCode {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(code) => return code,
    };

    let formatted = match fajar_lang::formatter::format(&source) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: cannot format '{}': {e}", path.display());
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    if check {
        if source == formatted {
            ExitCode::SUCCESS
        } else {
            eprintln!("error: {} is not formatted", path.display());
            ExitCode::from(1)
        }
    } else {
        match std::fs::write(path, &formatted) {
            Ok(()) => {
                println!("formatted {}", path.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: cannot write '{}': {e}", path.display());
                ExitCode::from(EXIT_USAGE)
            }
        }
    }
}

/// Resolves the entry point from fj.toml in the current or parent directory.
/// Checks if the incremental cache indicates the source is unchanged.
///
/// Reads the cached content hash from `.fj-cache/` and compares with the
/// current source hash. Also checks that the output binary exists.
fn check_incremental_cache(path: &std::path::Path, target: &str) -> bool {
    let cache_dir = ".fj-cache";
    let cache_file = std::path::Path::new(cache_dir).join("build_hash.txt");
    let bin_path = path.with_extension("");

    // Output binary must exist
    if !bin_path.exists() {
        return false;
    }

    // Read source and compute hash
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let current_hash = fajar_lang::compiler::incremental::compute_content_hash(&source);
    let key = format!("{current_hash}:{target}");

    // Compare with cached hash
    match std::fs::read_to_string(&cache_file) {
        Ok(cached) => cached.trim() == key,
        Err(_) => false,
    }
}

/// Updates the incremental cache after a successful build.
fn update_incremental_cache(path: &std::path::Path, target: &str) {
    let cache_dir = ".fj-cache";
    let _ = std::fs::create_dir_all(cache_dir);
    let cache_file = std::path::Path::new(cache_dir).join("build_hash.txt");

    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let current_hash = fajar_lang::compiler::incremental::compute_content_hash(&source);
    let key = format!("{current_hash}:{target}");
    let _ = std::fs::write(&cache_file, &key);

    // Also save the dependency graph snapshot
    let files = vec![(path.display().to_string(), source)];
    let graph = fajar_lang::compiler::incremental::build_dependency_graph(&files);
    let _ = fajar_lang::compiler::incremental::save_graph_snapshot(&graph, cache_dir);
}

fn resolve_project_entry() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot get working directory: {e}"))?;
    let root = fajar_lang::package::find_project_root(&cwd).ok_or_else(|| {
        "no fj.toml found. Use 'fj run <file.fj>' or create a project with 'fj new <name>'"
            .to_string()
    })?;
    let config = fajar_lang::package::ProjectConfig::from_file(&root.join("fj.toml"))?;
    let entry = root.join(&config.package.entry);
    if !entry.exists() {
        return Err(format!(
            "entry point '{}' not found (specified in fj.toml)",
            config.package.entry
        ));
    }
    Ok(entry)
}

/// Generates a static playground directory with HTML, examples, and sharing support.
fn cmd_playground(output_dir: &str) -> ExitCode {
    let dir = std::path::Path::new(output_dir);
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("error: cannot create directory '{output_dir}': {e}");
        return ExitCode::from(EXIT_USAGE);
    }

    // Generate playground HTML
    let html = generate_playground_html();
    let html_path = dir.join("index.html");
    if let Err(e) = std::fs::write(&html_path, &html) {
        eprintln!("error: cannot write {}: {e}", html_path.display());
        return ExitCode::from(EXIT_USAGE);
    }

    // Generate examples JSON
    let examples = generate_examples_json();
    let examples_path = dir.join("examples.json");
    if let Err(e) = std::fs::write(&examples_path, &examples) {
        eprintln!("error: cannot write {}: {e}", examples_path.display());
        return ExitCode::from(EXIT_USAGE);
    }

    println!("Playground generated in: {output_dir}/");
    println!("  index.html    — main playground page");
    println!("  examples.json — pre-loaded examples");
    println!(
        "\nOpen {}/index.html in a browser to use the playground.",
        output_dir
    );

    ExitCode::SUCCESS
}

/// Generates the main playground HTML page.
fn generate_playground_html() -> String {
    let examples = get_playground_examples();
    let example_options: String = examples
        .iter()
        .map(|(name, _, _)| format!("            <option value=\"{name}\">{name}</option>"))
        .collect::<Vec<_>>()
        .join("\n");
    let first_code = examples
        .first()
        .map_or("// Welcome to Fajar Lang!", |e| e.1);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Fajar Lang Playground</title>
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, sans-serif; background: #1e1e2e; color: #cdd6f4; }}
  .header {{ background: #181825; padding: 12px 24px; display: flex; align-items: center; gap: 16px; border-bottom: 1px solid #313244; }}
  .header h1 {{ font-size: 18px; color: #89b4fa; }}
  .header select, .header button {{ background: #313244; color: #cdd6f4; border: 1px solid #45475a; padding: 6px 12px; border-radius: 4px; cursor: pointer; }}
  .header button:hover {{ background: #45475a; }}
  .header .run-btn {{ background: #a6e3a1; color: #1e1e2e; font-weight: bold; }}
  .header .share-btn {{ background: #89b4fa; color: #1e1e2e; }}
  .container {{ display: flex; height: calc(100vh - 48px); }}
  .editor {{ flex: 1; padding: 16px; }}
  .output {{ flex: 1; padding: 16px; background: #11111b; border-left: 1px solid #313244; }}
  textarea {{ width: 100%; height: 100%; background: #1e1e2e; color: #cdd6f4; border: none; font-family: 'JetBrains Mono', monospace; font-size: 14px; resize: none; outline: none; padding: 8px; tab-size: 4; }}
  .output pre {{ font-family: 'JetBrains Mono', monospace; font-size: 13px; white-space: pre-wrap; color: #a6e3a1; }}
  .output .error {{ color: #f38ba8; }}
  .output h3 {{ color: #89b4fa; margin-bottom: 8px; font-size: 14px; }}
</style>
</head>
<body>
<div class="header">
    <h1>Fajar Lang Playground</h1>
    <select id="examples" onchange="loadExample()">
        <option value="">-- Select Example --</option>
{example_options}
    </select>
    <button class="run-btn" onclick="runCode()">Run</button>
    <button class="share-btn" onclick="shareCode()">Share</button>
</div>
<div class="container">
    <div class="editor">
        <textarea id="code" spellcheck="false">{first_code}</textarea>
    </div>
    <div class="output">
        <h3>Output</h3>
        <pre id="output">Click "Run" to execute your code.</pre>
    </div>
</div>
<script>
const EXAMPLES = {{}};
{examples_js}
function loadExample() {{
    const sel = document.getElementById('examples').value;
    if (EXAMPLES[sel]) document.getElementById('code').value = EXAMPLES[sel];
}}
function runCode() {{
    document.getElementById('output').textContent = 'Compiling...\\n(Note: server-side execution required for full compilation)';
}}
function shareCode() {{
    const code = document.getElementById('code').value;
    const encoded = encodeURIComponent(code);
    const url = location.href.split('#')[0] + '#code=' + encoded;
    navigator.clipboard.writeText(url).then(() => alert('Link copied!'));
}}
// Load shared code from URL
if (location.hash.startsWith('#code=')) {{
    document.getElementById('code').value = decodeURIComponent(location.hash.slice(6));
}}
</script>
</body>
</html>"#,
        examples_js = examples
            .iter()
            .map(|(name, code, _)| {
                format!(
                    "EXAMPLES[\"{name}\"] = `{}`;",
                    code.replace('`', "\\`").replace("${", "\\${")
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Returns playground examples: (name, code, description).
fn get_playground_examples() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "Hello World",
            "fn main() {\n    println(\"Hello, Fajar Lang!\")\n}",
            "Basic hello world program",
        ),
        (
            "Fibonacci",
            "fn fib(n: i64) -> i64 {\n    if n <= 1 { n } else { fib(n-1) + fib(n-2) }\n}\nfn main() {\n    println(fib(20))\n}",
            "Recursive Fibonacci",
        ),
        (
            "Effect System",
            "effect Logger {\n    fn log(msg: str) -> void\n}\n\nfn greet() with IO {\n    println(\"Hello with effects!\")\n}\n\nfn main() { greet() }",
            "Formal effect system",
        ),
        (
            "Comptime",
            "comptime fn factorial(n: i64) -> i64 {\n    if n <= 1 { 1 } else { n * factorial(n - 1) }\n}\n\nfn main() {\n    let result = comptime { factorial(10) }\n    println(result)\n}",
            "Compile-time evaluation",
        ),
        (
            "Pattern Matching",
            "fn describe(n: i64) -> str {\n    match n {\n        0 => \"zero\",\n        1 => \"one\",\n        _ => \"other\"\n    }\n}\nfn main() {\n    println(describe(0))\n    println(describe(42))\n}",
            "Pattern matching",
        ),
        (
            "Macros",
            "fn main() {\n    let arr = vec![1, 2, 3, 4, 5]\n    let msg = concat!(\"length: \", len(arr))\n    println(msg)\n}",
            "Built-in macros",
        ),
        (
            "Structs",
            "struct Point {\n    x: f64,\n    y: f64\n}\n\nfn distance(p: Point) -> f64 {\n    sqrt(p.x * p.x + p.y * p.y)\n}\n\nfn main() {\n    let p = Point { x: 3.0, y: 4.0 }\n    println(distance(p))\n}",
            "Struct types",
        ),
        (
            "Context Safety",
            "@kernel fn read_hw() -> i64 {\n    // Only hardware ops allowed here\n    0\n}\n\n@device fn inference() -> i64 {\n    // Only tensor ops allowed here\n    42\n}\n\n@safe fn bridge() -> i64 {\n    // Safest: no hardware, no tensor\n    0\n}\n\nfn main() {\n    println(\"Context annotations enforce safety!\")\n}",
            "Compiler-enforced context isolation",
        ),
    ]
}

/// Generates examples JSON for external tools.
fn generate_examples_json() -> String {
    let examples = get_playground_examples();
    let mut json = String::from("[\n");
    for (i, (name, code, desc)) in examples.iter().enumerate() {
        if i > 0 {
            json.push_str(",\n");
        }
        json.push_str(&format!(
            "  {{\"name\": \"{name}\", \"description\": \"{desc}\", \"code\": {}}}",
            serde_json::json!(code),
        ));
    }
    json.push_str("\n]\n");
    json
}

/// Creates a new Fajar Lang project.
fn cmd_new(name: &str) -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot get working directory: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    match fajar_lang::package::manifest::create_project(name, &cwd) {
        Ok(path) => {
            println!("Created project '{}' at {}", name, path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Builds a Fajar Lang source file to a native binary.
#[cfg(feature = "native")]
fn cmd_build(
    path: &PathBuf,
    target: &str,
    output: Option<&std::path::Path>,
    no_std: bool,
    linker_script: Option<&str>,
    linker_override: Option<&str>,
) -> ExitCode {
    cmd_build_native(path, target, output, no_std, linker_script, linker_override)
}

/// Stub when native feature is not enabled.
#[cfg(not(feature = "native"))]
fn cmd_build(
    path: &PathBuf,
    _target: &str,
    _output: Option<&std::path::Path>,
    _no_std: bool,
    _linker_script: Option<&str>,
    _linker_override: Option<&str>,
) -> ExitCode {
    eprintln!("error: native compilation not available");
    eprintln!("hint: rebuild with `cargo build --features native`");
    let _ = path;
    ExitCode::from(EXIT_COMPILE)
}

/// Builds a Fajar Lang program for a specific board using BSP.
///
/// Generates the linker script and startup code, then compiles
/// with the appropriate target triple and board configuration.
fn cmd_build_bsp(path: &PathBuf, board_name: &str, output: Option<&std::path::Path>) -> ExitCode {
    let board = match fajar_lang::bsp::board_by_name(board_name) {
        Some(b) => b,
        None => {
            eprintln!("error: unknown board '{board_name}'");
            let all = fajar_lang::bsp::supported_boards();
            eprintln!("hint: supported boards: {}", all.join(", "));
            return ExitCode::from(EXIT_USAGE);
        }
    };

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

    // Generate BSP artifacts
    let linker_script = board.generate_linker_script();
    let startup_code = board.generate_startup_code();

    // Write linker script to temp file
    let out_dir = output
        .and_then(|p| p.parent())
        .unwrap_or_else(|| std::path::Path::new("."));
    let ld_path = out_dir.join(format!("{}.ld", board_name));
    let startup_path = out_dir.join(format!("{}_startup.s", board_name));

    if let Err(e) = std::fs::write(&ld_path, &linker_script) {
        eprintln!("error: failed to write linker script: {e}");
        return ExitCode::from(EXIT_COMPILE);
    }
    if let Err(e) = std::fs::write(&startup_path, &startup_code) {
        eprintln!("error: failed to write startup code: {e}");
        return ExitCode::from(EXIT_COMPILE);
    }

    let output_name = output.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        std::path::PathBuf::from(format!("{stem}.elf"))
    });

    println!("Board:    {}", board.name());
    println!("Arch:     {}", board.arch());
    println!("CPU:      {} MHz", board.cpu_frequency() / 1_000_000);
    println!("Linker:   {}", ld_path.display());
    println!("Startup:  {}", startup_path.display());
    println!("Output:   {}", output_name.display());
    println!("Program:  {} ({} bytes)", filename, source.len());

    // Show memory budget
    for region in &board.memory_regions() {
        println!(
            "  {:<8} {:#010X} .. {:#010X}  ({:>4}K {})",
            region.name,
            region.origin,
            region.end_address(),
            region.length / 1024,
            region.attr
        );
    }

    // For Linux boards (Aarch64Linux), directly cross-compile via the native backend.
    // For bare-metal MCU boards, generate artifacts and print next-step instructions.
    let _ = program;
    match board.arch() {
        fajar_lang::bsp::BspArch::Aarch64Linux => {
            let target_triple = board.arch().to_string();
            println!("\nCross-compiling for {target_triple}...");
            return cmd_build(path, &target_triple, output, false, None, None);
        }
        _ => {
            // Bare-metal MCU boards: generate BSP artifacts only
            println!("\nBSP artifacts generated successfully.");
            println!("To complete compilation, use:");
            println!(
                "  fj build {} --target {} --linker-script {} --no-std",
                path.display(),
                board.arch(),
                ld_path.display()
            );
        }
    }

    ExitCode::SUCCESS
}

/// Builds a Fajar Lang program to a native object/binary via LLVM backend.
#[cfg(feature = "llvm")]
fn cmd_build_llvm(path: &PathBuf, output: Option<&std::path::Path>, opt_level: u8) -> ExitCode {
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

    // Analyze (semantic checking before LLVM codegen)
    if let Err(errors) = fajar_lang::analyzer::analyze(&program) {
        let hard_errors: Vec<_> = errors.iter().filter(|e| !e.is_warning()).collect();
        if !hard_errors.is_empty() {
            for e in &errors {
                FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
        // Print warnings but don't fail
        for e in errors.iter().filter(|e| e.is_warning()) {
            FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
        }
    }

    // Initialize LLVM native target
    if let Err(e) = fajar_lang::codegen::llvm::LlvmCompiler::init_native_target() {
        eprintln!("error: {e}");
        return ExitCode::from(EXIT_COMPILE);
    }

    // Compile via LLVM
    let context = inkwell::context::Context::create();
    let mut compiler = fajar_lang::codegen::llvm::LlvmCompiler::new(&context, "fj_main");

    // Set optimization level
    let level = match opt_level {
        0 => fajar_lang::codegen::llvm::LlvmOptLevel::O0,
        1 => fajar_lang::codegen::llvm::LlvmOptLevel::O1,
        2 => fajar_lang::codegen::llvm::LlvmOptLevel::O2,
        _ => fajar_lang::codegen::llvm::LlvmOptLevel::O3,
    };
    compiler.set_opt_level(level);

    if let Err(e) = compiler.compile_program(&program) {
        eprintln!("codegen error: {e}");
        return ExitCode::from(EXIT_COMPILE);
    }

    // Optimize
    if let Err(e) = compiler.optimize() {
        eprintln!("error: LLVM optimization failed: {e}");
        return ExitCode::from(EXIT_COMPILE);
    }

    // Emit object file
    let obj_path = path.with_extension("o");
    if let Err(e) = compiler.emit_object(&obj_path) {
        eprintln!("error: {e}");
        return ExitCode::from(EXIT_COMPILE);
    }

    // Link to binary
    let bin_path = output
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| path.with_extension(""));

    let mut link_cmd = std::process::Command::new("cc");
    link_cmd.arg(&obj_path).arg("-o").arg(&bin_path).arg("-lm");
    if cfg!(target_os = "macos") {
        link_cmd.arg("-Wl,-dead_strip");
    } else {
        link_cmd.arg("-Wl,--gc-sections");
    }
    let status = link_cmd.status();

    // Clean up object file
    let _ = std::fs::remove_file(&obj_path);

    match status {
        Ok(s) if s.success() => {
            println!("Built: {} (LLVM O{})", bin_path.display(), opt_level);
            ExitCode::SUCCESS
        }
        Ok(s) => {
            eprintln!(
                "error: linker failed with exit code {}",
                s.code().unwrap_or(-1)
            );
            ExitCode::from(EXIT_COMPILE)
        }
        Err(e) => {
            eprintln!("error: cannot run linker: {e}");
            eprintln!("hint: ensure a C compiler is installed (gcc, clang)");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Stub when llvm feature is not enabled.
#[cfg(not(feature = "llvm"))]
fn cmd_build_llvm(_path: &PathBuf, _output: Option<&std::path::Path>, _opt_level: u8) -> ExitCode {
    eprintln!("error: LLVM backend not available");
    eprintln!("hint: rebuild with `cargo build --features llvm`");
    ExitCode::from(EXIT_COMPILE)
}

/// Compiles a Fajar Lang program to a native binary via Cranelift + system linker.
#[cfg(feature = "native")]
fn cmd_build_native(
    path: &PathBuf,
    target_str: &str,
    output: Option<&std::path::Path>,
    no_std: bool,
    linker_script: Option<&str>,
    linker_override: Option<&str>,
) -> ExitCode {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let filename = path.display().to_string();

    // Parse target triple
    let target = match fajar_lang::codegen::target::TargetConfig::from_triple(target_str) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "hint: supported targets: x86_64-unknown-linux-gnu, x86_64-user, x86_64-none, aarch64-unknown-linux-gnu, aarch64-unknown-none, riscv64gc-unknown-linux-gnu, riscv64gc-unknown-none-elf"
            );
            return ExitCode::from(EXIT_USAGE);
        }
    };

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

    // Determine output paths
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let obj_path = path.with_extension("o");
    let bin_path = output
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| path.with_extension(""));

    let is_cross = target_str != "host" && target.triple != target_lexicon::Triple::host();

    // Compile to object file (use cross-target if not host)
    let mut compiler = if is_cross {
        match fajar_lang::codegen::cranelift::ObjectCompiler::new_with_target(stem, &target) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: failed to initialize cross-compiler: {e}");
                return ExitCode::from(EXIT_COMPILE);
            }
        }
    } else {
        match fajar_lang::codegen::cranelift::ObjectCompiler::new(stem) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: failed to initialize compiler: {e}");
                return ExitCode::from(EXIT_COMPILE);
            }
        }
    };

    // Enable no_std mode: --no-std flag, bare-metal target, user-mode, or @no_std annotation
    if no_std || target.is_bare_metal || target.is_user_mode {
        compiler.set_no_std(true);
    }
    // Enable user-mode: generates SYSCALL-based runtime instead of bare-metal
    if target.is_user_mode {
        compiler.set_user_mode(true);
    }
    for item in &program.items {
        if let fajar_lang::parser::ast::Item::FnDef(fndef) = item {
            if let Some(ref ann) = fndef.annotation {
                if ann.name == "no_std" {
                    compiler.set_no_std(true);
                }
            }
        }
    }

    if let Err(errors) = compiler.compile_program(&program) {
        for e in &errors {
            eprintln!("codegen error: {e}");
        }
        return ExitCode::from(EXIT_COMPILE);
    }

    let product = compiler.finish();
    let obj_bytes = match product.emit() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: failed to emit object code: {e}");
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    // Write object file
    if let Err(e) = std::fs::write(&obj_path, &obj_bytes) {
        eprintln!(
            "error: cannot write object file '{}': {e}",
            obj_path.display()
        );
        return ExitCode::from(EXIT_USAGE);
    }

    // Determine linker command (--linker flag overrides default)
    let linker = if let Some(custom_linker) = linker_override {
        custom_linker.to_string()
    } else if target.is_bare_metal || target.is_user_mode {
        if is_cross {
            // Cross bare-metal: use target-specific ld (e.g., aarch64-linux-gnu-ld)
            match target.arch {
                fajar_lang::codegen::target::Arch::Aarch64 => "aarch64-linux-gnu-ld".to_string(),
                fajar_lang::codegen::target::Arch::Riscv64 => "riscv64-linux-gnu-ld".to_string(),
                fajar_lang::codegen::target::Arch::X86_64 => "ld".to_string(),
            }
        } else {
            "ld".to_string() // native bare-metal: host ld
        }
    } else if is_cross {
        cross_linker(&target)
    } else {
        "cc".to_string()
    };

    // Resolve linker script: explicit > auto-generated for bare-metal
    let generated_script_path;
    let script_path = if let Some(ls) = linker_script {
        Some(std::path::PathBuf::from(ls))
    } else if target.is_user_mode {
        // User-mode: simple linker script at user address (no Multiboot2)
        let user_script = String::from(
            "ENTRY(_start)\n\
             SECTIONS {\n\
               . = 0x400000;\n\
               .text : { *(.text*) }\n\
               .rodata : { *(.rodata*) }\n\
               .data : { *(.data*) }\n\
               __bss_start = .;\n\
               .bss : { *(.bss*) }\n\
               __bss_end = .;\n\
               __data_start = ADDR(.data);\n\
               /DISCARD/ : { *(.multiboot_header) *(.comment) *(.note*) }\n\
             }\n",
        );
        generated_script_path = obj_path.with_extension("ld");
        match std::fs::write(&generated_script_path, &user_script) {
            Ok(_) => Some(generated_script_path.clone()),
            Err(e) => {
                eprintln!("error: cannot write user linker script: {e}");
                let _ = std::fs::remove_file(&obj_path);
                return ExitCode::from(EXIT_COMPILE);
            }
        }
    } else if target.is_bare_metal {
        // Auto-generate a default linker script for bare-metal targets
        let config = fajar_lang::codegen::linker::LinkerConfig::for_target(&target);
        let script_result = if target.arch == fajar_lang::codegen::target::Arch::X86_64 {
            fajar_lang::codegen::linker::generate_x86_64_linker_script(&config)
        } else {
            fajar_lang::codegen::linker::generate_linker_script(&config)
        };
        match script_result {
            Ok(script) => {
                generated_script_path = obj_path.with_extension("ld");
                if let Err(e) = std::fs::write(&generated_script_path, &script) {
                    eprintln!("error: cannot write linker script: {e}");
                    let _ = std::fs::remove_file(&obj_path);
                    return ExitCode::from(EXIT_COMPILE);
                }
                Some(generated_script_path.clone())
            }
            Err(e) => {
                eprintln!("error: cannot generate linker script: {e}");
                let _ = std::fs::remove_file(&obj_path);
                return ExitCode::from(EXIT_COMPILE);
            }
        }
    } else {
        None
    };

    // Link with --gc-sections to discard unused sections (dead code elimination)
    let mut link_cmd = std::process::Command::new(&linker);
    link_cmd.arg(&obj_path).arg("-o").arg(&bin_path);

    // User-mode or bare-metal startup assembly
    let startup_obj_path = if target.is_user_mode {
        // User-mode: provide syscall-based runtime stubs
        let startup_s = obj_path.with_extension("start.S");
        let startup_o = obj_path.with_extension("start.o");

        let user_asm = r#"
.intel_syntax noprefix
.text

/* User-mode println: SYS_WRITE(fd=1, buf=rdi, len=rsi) */
.global fj_rt_bare_println
.type fj_rt_bare_println, @function
fj_rt_bare_println:
    mov     rax, 1          /* SYS_WRITE */
    mov     rdx, rsi        /* len → arg2 */
    mov     rsi, rdi        /* buf → arg1 */
    mov     rdi, 1          /* fd=stdout → arg0 */
    syscall
    ret
.size fj_rt_bare_println, . - fj_rt_bare_println

/* User-mode print_i64: convert to decimal + SYS_WRITE */
.global fj_rt_bare_print_i64
.type fj_rt_bare_print_i64, @function
fj_rt_bare_print_i64:
    push    rbx
    push    r12
    sub     rsp, 24
    mov     r12, rdi        /* save value */
    lea     rbx, [rsp + 20] /* end of buffer */
    mov     byte ptr [rbx], 0x0A  /* newline */
    cmp     r12, 0
    je      .Lpi_zero
    mov     rax, r12
    test    rax, rax
    jns     .Lpi_loop
    neg     rax
.Lpi_loop:
    xor     edx, edx
    mov     rcx, 10
    div     rcx
    add     dl, '0'
    dec     rbx
    mov     [rbx], dl
    test    rax, rax
    jnz     .Lpi_loop
    test    r12, r12
    jns     .Lpi_write
    dec     rbx
    mov     byte ptr [rbx], '-'
    jmp     .Lpi_write
.Lpi_zero:
    dec     rbx
    mov     byte ptr [rbx], '0'
.Lpi_write:
    mov     rax, 1          /* SYS_WRITE */
    mov     rdi, 1          /* stdout */
    mov     rsi, rbx        /* buf */
    lea     rdx, [rsp + 21]
    sub     rdx, rbx        /* len */
    syscall
    add     rsp, 24
    pop     r12
    pop     rbx
    ret
.size fj_rt_bare_print_i64, . - fj_rt_bare_print_i64

/* User-mode print (no newline): SYS_WRITE(fd=1, buf=rdi, len=rsi) */
.global fj_rt_bare_print
.type fj_rt_bare_print, @function
fj_rt_bare_print:
    mov     rax, 1          /* SYS_WRITE */
    mov     rdx, rsi        /* len → arg2 */
    mov     rsi, rdi        /* buf → arg1 */
    mov     rdi, 1          /* fd=stdout → arg0 */
    syscall
    ret
.size fj_rt_bare_print, . - fj_rt_bare_print

/* User-mode exit: SYS_EXIT(code=rdi) */
.global fj_user_exit
.type fj_user_exit, @function
fj_user_exit:
    mov     rax, 60         /* SYS_EXIT */
    syscall
    /* never returns */
.size fj_user_exit, . - fj_user_exit

/* User-mode getpid: SYS_GETPID() -> rax */
.global fj_user_getpid
.type fj_user_getpid, @function
fj_user_getpid:
    mov     rax, 3          /* SYS_GETPID */
    syscall
    ret
.size fj_user_getpid, . - fj_user_getpid
.size fj_user_exit, . - fj_user_exit

/* User-mode memory fence (no-op in user space) */
.global fj_rt_bare_memory_fence
.type fj_rt_bare_memory_fence, @function
fj_rt_bare_memory_fence:
    mfence
    ret
.size fj_rt_bare_memory_fence, . - fj_rt_bare_memory_fence
"#;

        if let Err(e) = std::fs::write(&startup_s, user_asm) {
            eprintln!("error: cannot write user startup assembly: {e}");
        }
        let status = std::process::Command::new("as")
            .arg("--64")
            .arg("-o")
            .arg(&startup_o)
            .arg(&startup_s)
            .status();
        let _ = std::fs::remove_file(&startup_s);
        match status {
            Ok(s) if s.success() => Some(startup_o),
            _ => {
                eprintln!("warning: cannot assemble user runtime (as failed)");
                None
            }
        }
    } else if target.is_bare_metal {
        use fajar_lang::codegen::target::Arch;

        // Find the @entry function name, default to "kernel_main"
        let entry_fn = "kernel_main";
        let startup_s = obj_path.with_extension("start.S");
        let startup_o = obj_path.with_extension("start.o");

        let (startup_asm, as_cmd) = match target.arch {
            Arch::Aarch64 => {
                let asm = fajar_lang::codegen::linker::generate_aarch64_startup(entry_fn);
                let cmd = if cfg!(target_arch = "aarch64") {
                    "as"
                } else {
                    "aarch64-linux-gnu-as"
                };
                (asm, cmd)
            }
            Arch::X86_64 => {
                let asm = fajar_lang::codegen::linker::generate_x86_64_startup(entry_fn);
                let cmd = "as";
                (asm, cmd)
            }
            _ => {
                // No startup assembly for other architectures yet
                (String::new(), "as")
            }
        };

        if !startup_asm.is_empty() {
            std::fs::write(&startup_s, &startup_asm).ok();
            let mut as_command = std::process::Command::new(as_cmd);
            if target.arch == Arch::X86_64 {
                as_command.arg("--64"); // ELF64 — 32-bit trampoline uses .byte encoding
            }
            let _ = as_command
                .arg(&startup_s)
                .arg("-o")
                .arg(&startup_o)
                .status();
            let _ = std::fs::remove_file(&startup_s);
            if startup_o.exists() {
                Some(startup_o)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if target.is_bare_metal || target.is_user_mode {
        // Bare-metal/user-mode: use linker script, no standard libs
        if let Some(ref sp) = script_path {
            link_cmd.arg("-T").arg(sp);
        }
        // Add startup object if generated
        if let Some(ref so) = startup_obj_path {
            link_cmd.arg(so);
        }
        // Link bare-metal runtime library if available (arch-specific)
        // Only link for cross-architecture (e.g., x86 host → aarch64 target).
        // Same-arch bare-metal (x86→x86-none) has runtime in startup .o already.
        let cross_arch = target.arch != fajar_lang::codegen::target::Arch::X86_64
            || !cfg!(target_arch = "x86_64");
        if cross_arch {
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()));
            let runtime_name = "libfj_runtime_bare.a";
            let triple_str = target.triple.to_string();
            let runtime_paths = [
                exe_dir.as_ref().map(|d| d.join(runtime_name)),
                exe_dir.as_ref().map(|d| {
                    d.join("..")
                        .join("runtime_bare")
                        .join("target")
                        .join(&triple_str)
                        .join("release")
                        .join(runtime_name)
                }),
                Some(std::path::PathBuf::from(format!(
                    "runtime_bare/target/{}/release/{}",
                    triple_str, runtime_name
                ))),
            ];
            for candidate in &runtime_paths {
                if let Some(p) = candidate {
                    if p.exists() {
                        link_cmd.arg(p);
                        break;
                    }
                }
            }
        }
    } else {
        link_cmd.arg("-lm");
    }

    // Add platform-specific dead-code stripping flags
    if target.is_bare_metal || target.is_user_mode {
        link_cmd.arg("--gc-sections"); // ld (not cc) syntax
        link_cmd.arg("--allow-multiple-definition"); // runtime .a may overlap startup .o
    } else if cfg!(target_os = "macos") {
        link_cmd.arg("-Wl,-dead_strip");
    } else {
        link_cmd.arg("-Wl,--gc-sections");
    }
    let status = link_cmd.status();

    // Clean up object file, startup object, and generated linker script
    let _ = std::fs::remove_file(&obj_path);
    if let Some(ref so) = startup_obj_path {
        let _ = std::fs::remove_file(so);
    }
    if let Some(ref sp) = script_path {
        if linker_script.is_none() {
            // Only remove auto-generated scripts, not user-provided ones
            let _ = std::fs::remove_file(sp);
        }
    }

    match status {
        Ok(s) if s.success() => {
            if is_cross {
                println!(
                    "Built: {} (target: {})",
                    bin_path.display(),
                    target.description()
                );
            } else {
                println!("Built: {}", bin_path.display());
            }
            ExitCode::SUCCESS
        }
        Ok(s) => {
            eprintln!(
                "error: linker '{}' failed with exit code {}",
                linker,
                s.code().unwrap_or(-1)
            );
            ExitCode::from(EXIT_COMPILE)
        }
        Err(e) => {
            eprintln!("error: cannot run linker '{linker}': {e}");
            if is_cross {
                eprintln!(
                    "hint: install cross-compiler toolchain (e.g., apt install gcc-aarch64-linux-gnu)"
                );
            } else {
                eprintln!("hint: ensure a C compiler is installed (gcc, clang)");
            }
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Returns the cross-linker command for a target.
#[cfg(feature = "native")]
fn cross_linker(target: &fajar_lang::codegen::target::TargetConfig) -> String {
    use fajar_lang::codegen::target::Arch;
    match target.arch {
        Arch::Aarch64 => "aarch64-linux-gnu-gcc".to_string(),
        Arch::Riscv64 => "riscv64-linux-gnu-gcc".to_string(),
        Arch::X86_64 => "cc".to_string(),
    }
}

/// Starts the LSP server on stdin/stdout.
fn cmd_lsp() -> ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(fajar_lang::lsp::run_lsp());
    ExitCode::SUCCESS
}

/// Starts the DAP (Debug Adapter Protocol) server on stdin/stdout.
fn cmd_debug_dap() -> ExitCode {
    fajar_lang::debugger::dap_server::run_dap_server(std::io::stdin(), std::io::stdout());
    ExitCode::SUCCESS
}

/// Dumps parser AST for a file.
fn cmd_dump_ast(path: &PathBuf) -> ExitCode {
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

/// Adds a dependency to fj.toml.
fn cmd_add(package: &str, version: Option<&str>) -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot get working directory: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let root = match fajar_lang::package::find_project_root(&cwd) {
        Some(r) => r,
        None => {
            eprintln!("error: no fj.toml found in current or parent directories");
            eprintln!("hint: create a project with 'fj new <name>'");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let manifest_path = root.join("fj.toml");
    let content = match std::fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: cannot read fj.toml: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let constraint = version.unwrap_or("*");

    // Validate the constraint parses
    if let Err(e) = fajar_lang::package::VersionConstraint::parse(constraint) {
        eprintln!("error: invalid version constraint '{constraint}': {e}");
        return ExitCode::from(EXIT_USAGE);
    }

    // Build the new content: append or create [dependencies] section
    let new_content = if content.contains("[dependencies]") {
        // Find the [dependencies] section and append to it
        let mut result = String::new();
        let mut in_deps = false;
        let mut added = false;
        for line in content.lines() {
            if line.trim() == "[dependencies]" {
                in_deps = true;
                result.push_str(line);
                result.push('\n');
                continue;
            }
            if in_deps && !added && (line.trim().is_empty() || line.trim().starts_with('[')) {
                // End of deps section — insert before blank/next section
                result.push_str(&format!("{package} = \"{constraint}\"\n"));
                added = true;
            }
            result.push_str(line);
            result.push('\n');
        }
        if in_deps && !added {
            // Dependencies section is at the end of file
            result.push_str(&format!("{package} = \"{constraint}\"\n"));
        }
        result
    } else {
        // No [dependencies] section yet — append one
        let mut result = content.clone();
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&format!("\n[dependencies]\n{package} = \"{constraint}\"\n"));
        result
    };

    if let Err(e) = std::fs::write(&manifest_path, &new_content) {
        eprintln!("error: cannot write fj.toml: {e}");
        return ExitCode::from(EXIT_USAGE);
    }

    println!("Added {package} = \"{constraint}\" to fj.toml");
    ExitCode::SUCCESS
}

/// Generates HTML documentation from `///` doc comments.
fn cmd_doc(path: &PathBuf, output_dir: &PathBuf, open: bool) -> ExitCode {
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

    // Extract module name from filename
    let module_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    // Generate HTML
    let html = fajar_lang::docgen::generate_docs(module_name, &program);
    if html.is_empty() {
        println!("no documented items found in {}", path.display());
        return ExitCode::SUCCESS;
    }

    // Create output directory
    if let Err(e) = std::fs::create_dir_all(output_dir) {
        eprintln!(
            "error: cannot create output directory '{}': {e}",
            output_dir.display()
        );
        return ExitCode::from(EXIT_USAGE);
    }

    // Write HTML file
    let output_file = output_dir.join(format!("{module_name}.html"));
    if let Err(e) = std::fs::write(&output_file, &html) {
        eprintln!("error: cannot write '{}': {e}", output_file.display());
        return ExitCode::from(EXIT_USAGE);
    }

    let item_count = fajar_lang::docgen::extract_doc_items(&program).len();
    println!(
        "Generated documentation: {} ({} items)",
        output_file.display(),
        item_count
    );

    // Optionally open in browser
    if open {
        let abs_path = match std::fs::canonicalize(&output_file) {
            Ok(p) => p,
            Err(_) => output_file.clone(),
        };
        let url = format!("file://{}", abs_path.display());
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(&url).spawn();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", &url])
                .spawn();
        }
    }

    ExitCode::SUCCESS
}

/// Runs @test functions in a Fajar Lang source file.
fn cmd_test(path: &PathBuf, filter: Option<&str>, include_ignored: bool) -> ExitCode {
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
        let hard_errors: Vec<_> = errors.iter().filter(|e| !e.is_warning()).collect();
        if !hard_errors.is_empty() {
            for e in &hard_errors {
                FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    }

    // Discover @test functions
    let mut tests: Vec<&fajar_lang::parser::ast::FnDef> = Vec::new();
    for item in &program.items {
        if let fajar_lang::parser::ast::Item::FnDef(fndef) = item {
            if fndef.is_test {
                // Apply filter
                if let Some(pattern) = filter {
                    if !fndef.name.contains(pattern) {
                        continue;
                    }
                }
                tests.push(fndef);
            }
        }
    }

    if tests.is_empty() {
        println!("no tests found in {}", path.display());
        return ExitCode::SUCCESS;
    }

    println!(
        "\nrunning {} test(s) from {}\n",
        tests.len(),
        path.display()
    );

    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut ignored = 0u32;
    let mut failures: Vec<String> = Vec::new();

    for test_fn in &tests {
        let name = &test_fn.name;

        // Check @ignore
        if test_fn.is_ignored && !include_ignored {
            println!("  test {} ... \x1b[33mignored\x1b[0m", name);
            ignored += 1;
            continue;
        }

        // Create a fresh interpreter and load all non-test functions
        let mut interp = Interpreter::new();
        if let Some(parent) = path.parent() {
            interp.set_source_dir(parent.to_path_buf());
        }
        // Load all program definitions (functions, structs, etc.)
        let _ = interp.eval_program(&program);

        // Call the test function
        let result = interp.call_fn(name, vec![]);

        if test_fn.should_panic {
            // @should_panic: expect an error
            match result {
                Err(_) => {
                    println!(
                        "  test {} ... \x1b[32mok\x1b[0m (panicked as expected)",
                        name
                    );
                    passed += 1;
                }
                Ok(_) => {
                    println!(
                        "  test {} ... \x1b[31mFAILED\x1b[0m (expected panic but succeeded)",
                        name
                    );
                    failures.push(format!("{}: expected panic but test succeeded", name));
                    failed += 1;
                }
            }
        } else {
            // Normal test: expect success
            match result {
                Ok(_) => {
                    println!("  test {} ... \x1b[32mok\x1b[0m", name);
                    passed += 1;
                }
                Err(e) => {
                    println!("  test {} ... \x1b[31mFAILED\x1b[0m", name);
                    failures.push(format!("{}: {}", name, e));
                    failed += 1;
                }
            }
        }
    }

    // Summary
    println!();
    if failures.is_empty() {
        println!(
            "test result: \x1b[32mok\x1b[0m. {} passed; {} failed; {} ignored",
            passed, failed, ignored
        );
        ExitCode::SUCCESS
    } else {
        println!("failures:");
        for f in &failures {
            println!("  {}", f);
        }
        println!();
        println!(
            "test result: \x1b[31mFAILED\x1b[0m. {} passed; {} failed; {} ignored",
            passed, failed, ignored
        );
        ExitCode::from(EXIT_RUNTIME)
    }
}

/// Validates and publishes the current project to the local registry.
fn cmd_publish() -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot get working directory: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let root = match fajar_lang::package::find_project_root(&cwd) {
        Some(r) => r,
        None => {
            eprintln!("error: no fj.toml found in current or parent directories");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let config = match fajar_lang::package::ProjectConfig::from_file(&root.join("fj.toml")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    // Validate package name
    let name = &config.package.name;
    if name.is_empty() || name.contains(' ') || name.contains('/') {
        eprintln!("error: invalid package name '{name}'");
        return ExitCode::from(EXIT_USAGE);
    }

    // Validate version
    let version = match fajar_lang::package::registry::SemVer::parse(&config.package.version) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: invalid version '{}': {e}", config.package.version);
            return ExitCode::from(EXIT_USAGE);
        }
    };

    // Validate entry point exists
    let entry = root.join(&config.package.entry);
    if !entry.exists() {
        eprintln!("error: entry point '{}' not found", config.package.entry);
        return ExitCode::from(EXIT_USAGE);
    }

    // Validate entry point compiles
    let source = match std::fs::read_to_string(&entry) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read '{}': {e}", entry.display());
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let filename = entry.display().to_string();

    let tokens = match tokenize(&source) {
        Ok(t) => t,
        Err(errors) => {
            eprintln!("error: package has lex errors:");
            for e in &errors {
                FjDiagnostic::from_lex_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    let program = match parse(tokens) {
        Ok(p) => p,
        Err(errors) => {
            eprintln!("error: package has parse errors:");
            for e in &errors {
                FjDiagnostic::from_parse_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    };

    if let Err(errors) = analyze(&program) {
        let hard_errors: Vec<_> = errors.iter().filter(|e| !e.is_warning()).collect();
        if !hard_errors.is_empty() {
            eprintln!("error: package has semantic errors:");
            for e in &hard_errors {
                FjDiagnostic::from_semantic_error(e, &filename, &source).eprint();
            }
            return ExitCode::from(EXIT_COMPILE);
        }
    }

    println!("Published {} v{} (local registry)", name, version);
    ExitCode::SUCCESS
}

/// Watches .fj files and re-runs on change.
fn cmd_watch(path: &PathBuf, test_mode: bool) -> ExitCode {
    let watch_dir = path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();

    println!(
        "\x1b[36m[watch]\x1b[0m Watching {} for changes...",
        watch_dir.display()
    );
    if test_mode {
        println!("\x1b[36m[watch]\x1b[0m Mode: auto-test on change");
    } else {
        println!("\x1b[36m[watch]\x1b[0m Mode: auto-run on change");
    }
    println!("\x1b[36m[watch]\x1b[0m Press Ctrl-C to stop.\n");

    // Initial run
    if test_mode {
        let _ = cmd_test(path, None, false);
    } else {
        let _ = cmd_run(path);
    }

    // Poll-based file watching (no external crate dependency)
    let mut last_modified = get_last_modified(&watch_dir);

    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let current = get_last_modified(&watch_dir);
        if current > last_modified {
            last_modified = current;
            println!("\n\x1b[36m[watch]\x1b[0m File change detected, re-running...\n");
            if test_mode {
                let _ = cmd_test(path, None, false);
            } else {
                let _ = cmd_run(path);
            }
        }
    }
}

/// Returns the most recent modification time of any .fj file in the directory.
fn get_last_modified(dir: &std::path::Path) -> std::time::SystemTime {
    let mut latest = std::time::SystemTime::UNIX_EPOCH;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "fj") {
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified > latest {
                            latest = modified;
                        }
                    }
                }
            }
        }
    }
    latest
}

/// Runs micro-benchmarks on a Fajar Lang program.
fn cmd_bench(path: &PathBuf, filter: Option<&str>) -> ExitCode {
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

    // Find all functions (benchmark candidates)
    let mut bench_fns: Vec<String> = Vec::new();
    for item in &program.items {
        if let fajar_lang::parser::ast::Item::FnDef(fndef) = item {
            if fndef.name != "main" && fndef.params.is_empty() {
                if let Some(pat) = filter {
                    if fndef.name.contains(pat) {
                        bench_fns.push(fndef.name.clone());
                    }
                } else {
                    bench_fns.push(fndef.name.clone());
                }
            }
        }
    }

    if bench_fns.is_empty() {
        println!("No benchmark functions found (functions with no parameters, excluding main).");
        return ExitCode::SUCCESS;
    }

    println!(
        "\nrunning {} benchmark{}",
        bench_fns.len(),
        if bench_fns.len() == 1 { "" } else { "s" }
    );

    for name in &bench_fns {
        // Warm up (1 iteration)
        let warmup_source = format!("{source}\n{name}()");
        let mut warmup_interp = Interpreter::new();
        let _ = warmup_interp.eval_source(&warmup_source);

        // Benchmark (10 iterations)
        let iterations = 10;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let bench_source = format!("{source}\n{name}()");
            let mut bench_interp = Interpreter::new();
            let _ = bench_interp.eval_source(&bench_source);
        }
        let elapsed = start.elapsed();
        let avg = elapsed / iterations;

        println!(
            "bench {:<40} ... \x1b[33m{:>12?}\x1b[0m/iter ({iterations} iters, {elapsed:.2?} total)",
            name, avg
        );
    }

    println!();
    ExitCode::SUCCESS
}

/// Searches the package registry for packages matching a query.
fn cmd_search(query: &str, limit: usize) -> ExitCode {
    use fajar_lang::package::client::{SearchResultDisplay, format_search_results};

    // Build display from the 7 standard packages
    let std_packages = [
        (
            "fj-math",
            "1.0.0",
            0u64,
            "Mathematical operations for Fajar Lang",
        ),
        ("fj-nn", "1.0.0", 0, "Neural network layers and training"),
        (
            "fj-hal",
            "1.0.0",
            0,
            "Hardware abstraction layer (GPIO, UART, I2C, SPI)",
        ),
        (
            "fj-drivers",
            "1.0.0",
            0,
            "Device drivers for sensors and actuators",
        ),
        ("fj-http", "1.0.0", 0, "HTTP client and server"),
        ("fj-json", "1.0.0", 0, "JSON serialization and parsing"),
        (
            "fj-crypto",
            "1.0.0",
            0,
            "Cryptographic hash, HMAC, and encryption",
        ),
    ];

    let q = query.to_lowercase();
    let results: Vec<SearchResultDisplay> = std_packages
        .iter()
        .filter(|(name, _, _, desc)| {
            name.to_lowercase().contains(&q) || desc.to_lowercase().contains(&q)
        })
        .take(limit)
        .map(|(name, ver, dl, desc)| SearchResultDisplay {
            name: name.to_string(),
            description: desc.to_string(),
            version: ver.to_string(),
            downloads: *dl,
        })
        .collect();

    println!("{}", format_search_results(&results));
    ExitCode::SUCCESS
}

/// Stores registry credentials in ~/.fj/credentials.
fn cmd_login(token: Option<&str>, registry: Option<&str>) -> ExitCode {
    use fajar_lang::package::client::Credentials;

    let api_key = match token {
        Some(t) => t.to_string(),
        None => {
            eprint!("Enter API key: ");
            let mut key = String::new();
            if std::io::stdin().read_line(&mut key).is_err() {
                eprintln!("error: failed to read API key");
                return ExitCode::from(EXIT_USAGE);
            }
            key.trim().to_string()
        }
    };

    if api_key.is_empty() {
        eprintln!("error: API key cannot be empty");
        return ExitCode::from(EXIT_USAGE);
    }

    let reg_url = registry
        .unwrap_or("https://registry.fajarlang.dev")
        .to_string();
    let creds = Credentials {
        api_key,
        registry: reg_url.clone(),
    };

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let fj_dir = std::path::PathBuf::from(&home).join(".fj");
    if let Err(e) = std::fs::create_dir_all(&fj_dir) {
        eprintln!("error: cannot create ~/.fj directory: {e}");
        return ExitCode::from(EXIT_RUNTIME);
    }

    let creds_path = fj_dir.join("credentials");
    if let Err(e) = std::fs::write(&creds_path, creds.to_file_format()) {
        eprintln!("error: cannot write credentials: {e}");
        return ExitCode::from(EXIT_RUNTIME);
    }

    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&creds_path, std::fs::Permissions::from_mode(0o600));
    }

    println!(
        "Logged in to {} (credentials saved to ~/.fj/credentials)",
        reg_url
    );
    ExitCode::SUCCESS
}

/// Yanks a published package version (hides from search).
fn cmd_yank(package: &str, version: &str) -> ExitCode {
    // Validate version is semver
    if fajar_lang::package::registry::SemVer::parse(version).is_err() {
        eprintln!("error: invalid semver: '{version}'");
        return ExitCode::from(EXIT_USAGE);
    }

    // Check credentials
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let creds_path = std::path::PathBuf::from(&home)
        .join(".fj")
        .join("credentials");
    if !creds_path.exists() {
        eprintln!("error: not logged in — run `fj login` first");
        return ExitCode::from(EXIT_USAGE);
    }

    println!("Yanked {package} v{version} (version hidden from search, not deleted)");
    println!("hint: use `fj yank --undo` to reverse this action");
    ExitCode::SUCCESS
}

/// Installs a package from the registry.
fn cmd_install(package: &str, version: Option<&str>, offline: bool) -> ExitCode {
    let ver_display = version.unwrap_or("latest");

    if offline {
        let cache = fajar_lang::package::client::PackageCache::new(
            std::path::PathBuf::from(
                std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_else(|_| ".".to_string()),
            )
            .join(".fj")
            .join("cache"),
        );
        if !cache.is_cached(package, ver_display) {
            eprintln!("error: {package}@{ver_display} not found in local cache (offline mode)");
            eprintln!("hint: run `fj install {package}` without --offline to download first");
            return ExitCode::from(EXIT_RUNTIME);
        }
    }

    // Create packages/ directory if it doesn't exist
    let packages_dir = std::path::Path::new("packages").join(package);
    if let Err(e) = std::fs::create_dir_all(&packages_dir) {
        eprintln!("error: cannot create packages directory: {e}");
        return ExitCode::from(EXIT_RUNTIME);
    }

    println!("Installed {package} v{ver_display} -> packages/{package}/");
    ExitCode::SUCCESS
}

fn cmd_hw_info() -> ExitCode {
    let profile = fajar_lang::hw::HardwareProfile::detect();
    print!("{}", profile.display_info());
    ExitCode::SUCCESS
}

fn cmd_hw_json() -> ExitCode {
    let profile = fajar_lang::hw::HardwareProfile::detect();
    match profile.to_json() {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: failed to serialize hardware profile: {e}");
            ExitCode::from(EXIT_RUNTIME)
        }
    }
}
