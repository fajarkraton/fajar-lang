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

use fajar_lang::analyzer::analyze;
use fajar_lang::interpreter::Interpreter;
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;
use fajar_lang::FjDiagnostic;

/// Fajar Lang — A systems programming language for OS and AI/ML.
#[derive(Parser)]
#[command(name = "fj", version = "0.5.0", about)]
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
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Run { file, vm, native } => {
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
            if native {
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
        Command::New { name } => cmd_new(&name),
        Command::Build {
            file,
            target,
            output,
            no_std,
            linker_script,
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
            cmd_build(&path, &target, output.as_deref(), no_std, ls.as_deref())
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
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("error: cannot read '{}': {e}", path.display());
        ExitCode::from(EXIT_USAGE)
    })
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

/// Starts an interactive REPL.
fn cmd_repl() -> ExitCode {
    println!("Fajar Lang v0.3.0 — Interactive REPL");
    println!("Type expressions to evaluate. Type 'exit' or Ctrl-D to quit.");
    println!();

    let mut rl = match rustyline::DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("error: failed to initialize REPL: {e}");
            return ExitCode::from(1);
        }
    };

    let mut interp = Interpreter::new();

    loop {
        let line = match rl.readline("fj> ") {
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
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "exit" || trimmed == "quit" {
            println!("Bye!");
            break;
        }

        let _ = rl.add_history_entry(&line);

        // Lex → Parse → Analyze → Eval (via eval_source)
        match interp.eval_source(trimmed) {
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
) -> ExitCode {
    cmd_build_native(path, target, output, no_std, linker_script)
}

/// Stub when native feature is not enabled.
#[cfg(not(feature = "native"))]
fn cmd_build(
    path: &PathBuf,
    _target: &str,
    _output: Option<&std::path::Path>,
    _no_std: bool,
    _linker_script: Option<&str>,
) -> ExitCode {
    eprintln!("error: native compilation not available");
    eprintln!("hint: rebuild with `cargo build --features native`");
    let _ = path;
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
            eprintln!("hint: supported targets: x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, aarch64-unknown-none, riscv64gc-unknown-linux-gnu, riscv64gc-unknown-none-elf");
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

    // Enable no_std mode: --no-std flag or @no_std annotation in source
    if no_std {
        compiler.set_no_std(true);
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

    // Determine linker command
    let linker = if is_cross {
        cross_linker(&target)
    } else {
        "cc".to_string()
    };

    // Resolve linker script: explicit > auto-generated for bare-metal
    let generated_script_path;
    let script_path = if let Some(ls) = linker_script {
        Some(std::path::PathBuf::from(ls))
    } else if target.is_bare_metal {
        // Auto-generate a default linker script for bare-metal targets
        let config = fajar_lang::codegen::linker::LinkerConfig::for_target(&target);
        match fajar_lang::codegen::linker::generate_linker_script(&config) {
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

    if target.is_bare_metal {
        // Bare-metal: no standard libs, use linker script
        link_cmd.arg("-nostdlib").arg("-nostartfiles");
        if let Some(ref sp) = script_path {
            link_cmd.arg("-T").arg(sp);
        }
    } else {
        link_cmd.arg("-lm");
    }

    // Add platform-specific dead-code stripping flags
    if cfg!(target_os = "macos") {
        link_cmd.arg("-Wl,-dead_strip");
    } else {
        link_cmd.arg("-Wl,--gc-sections");
    }
    let status = link_cmd.status();

    // Clean up object file and generated linker script
    let _ = std::fs::remove_file(&obj_path);
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
