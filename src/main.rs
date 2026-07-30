// Nightly clippy allow-list — lints that differ between stable and nightly.
// TODO: remove each allow when the lint stabilizes and code is updated.
// - collapsible_if: Edition 2024 expanded scope (nightly 2025-03+)
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

mod cli;
use cli::*;

/// Fajar Lang — A systems programming language for OS and AI/ML.
#[derive(Parser)]
#[command(name = "fj", version = env!("CARGO_PKG_VERSION"), about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Available subcommands.
#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
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
        /// Enable function-call profiling and write trace to --profile-output.
        #[arg(long)]
        profile: bool,
        /// Output file for the profile trace (Chrome JSON format).
        #[arg(long, default_value = "fj-profile.json")]
        profile_output: Option<String>,
        /// Enable strict ownership: String/Array/Struct are Move types (use-after-move errors).
        #[arg(long)]
        strict_ownership: bool,
        /// Use tiered JIT compilation (interpreter → baseline → optimizing).
        #[arg(long)]
        jit: bool,
        /// V15 B3.5: Parse + analyze without executing. Print "OK" or errors.
        #[arg(long)]
        check_only: bool,
        /// V14 EF4.9: Print effect usage statistics after execution.
        #[arg(long)]
        effect_stats: bool,
    },
    /// Start an interactive REPL.
    Repl,
    /// Parse and check a file (no execution).
    Check {
        /// Path to the .fj source file.
        file: PathBuf,
        /// Show cross-context call graph (which @safe/@kernel/@device functions call each other).
        #[arg(long)]
        call_graph: bool,
        /// Enable strict ownership: String/Array/Struct are Move types (use-after-move errors).
        #[arg(long)]
        strict_ownership: bool,
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
    /// Pack service ELFs into an initramfs archive.
    Pack {
        /// Output file for the initramfs archive.
        #[arg(short, long, default_value = "build/initramfs.img")]
        output: String,
        /// Service ELF files to pack (or auto-detect from build/services/).
        files: Vec<PathBuf>,
    },
    /// Generate a static playground HTML page with examples.
    Playground {
        /// Output directory for playground files.
        #[arg(short, long, default_value = "playground")]
        output: String,
    },
    /// Run a built-in demo (drone, os, network, ffi, database, web, embedded-ml, cli, ...).
    Demo {
        /// Demo name (omit to list all demos)
        name: Option<String>,
        /// List all available demos
        #[arg(long)]
        list: bool,
    },
    /// Generate deployment artifacts (Dockerfile, K8s manifests).
    Deploy {
        /// Deployment target: container, k8s
        #[arg(long, default_value = "container")]
        target: String,
        /// Source .fj file to deploy
        file: PathBuf,
        /// Output directory
        #[arg(short, long, default_value = ".")]
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
        /// LLVM optimization level (0-3, s, z). Only used with --backend llvm.
        #[arg(long, name = "opt-level", default_value = "0")]
        opt_level: String,
        /// Target CPU for LLVM codegen (e.g., "native", "skylake", "cortex-a76", "generic").
        #[arg(long, name = "target-cpu", default_value = "generic")]
        target_cpu: String,
        /// Target CPU features for LLVM (e.g., "+avx2,+fma,-sse4a").
        #[arg(long, name = "target-features", default_value = "")]
        target_features: String,
        /// Relocation model: default, static, pic, dynamic-no-pic.
        #[arg(long, default_value = "default")]
        reloc: String,
        /// Code model: default, small, medium, large, kernel.
        #[arg(long, name = "code-model", default_value = "default")]
        code_model: String,
        /// Link-time optimization: none, thin, or full. --release defaults to thin.
        #[arg(long, default_value = "none")]
        lto: String,
        /// Profile-guided optimization: none, generate, `generate=<dir>`, `use=<file.profdata>`.
        #[arg(long, default_value = "none")]
        pgo: String,
        /// Target board for BSP (e.g., stm32f407, esp32, rp2040).
        #[arg(long)]
        board: Option<String>,
        /// Override linker binary (e.g., aarch64-linux-gnu-ld, ld.lld).
        #[arg(long)]
        linker: Option<String>,
        /// Extra object files to link (e.g., runtime stubs for bare-metal).
        #[arg(long, name = "extra-objects", value_delimiter = ',')]
        extra_objects: Vec<String>,
        /// Verbose codegen output (function count, DCE stats, compile time).
        #[arg(long, short)]
        verbose: bool,
        /// Enable incremental compilation (cache unchanged functions).
        #[arg(long)]
        incremental: bool,
        /// Build all targets (kernel + services) defined in fj.toml.
        #[arg(long)]
        all: bool,
        /// Release build: uses LLVM backend with -O2 for best codegen quality.
        #[arg(long)]
        release: bool,
        /// Enable runtime security hardening (bounds checks, overflow checks).
        #[arg(long)]
        security: bool,
        /// Enable security linter pre-pass before compilation.
        #[arg(long)]
        lint: bool,
    },
    /// Publish a package to the local registry.
    Publish {
        /// V15 B3.9: Publish to a local file-based registry instead of default.
        #[arg(long)]
        local: bool,
        /// Path to local registry directory (used with --local).
        #[arg(long)]
        registry: Option<PathBuf>,
    },
    /// V15 B3.8: Initialize a local file-based package registry.
    RegistryInit {
        /// Path where the registry directory should be created.
        path: PathBuf,
    },
    /// V14 PR1.9: Start a local package registry HTTP server.
    #[command(name = "registry-serve")]
    RegistryServe {
        /// Port to listen on (default: 8080).
        #[arg(long, default_value = "8080")]
        port: u16,
    },
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
    /// Manage compiler plugins.
    Plugin {
        /// Action: list, load
        action: String,
        /// Path to plugin .so/.dylib (for load action)
        path: Option<PathBuf>,
    },
    /// Start a debug session (DAP protocol on stdin/stdout).
    Debug {
        /// Path to the .fj source file. If omitted, uses fj.toml entry point.
        file: Option<PathBuf>,
        /// Use DAP protocol (for IDE integration).
        #[arg(long)]
        dap: bool,
        /// Record execution trace to a JSON file.
        #[arg(long)]
        record: Option<PathBuf>,
        /// Replay execution trace from a JSON file.
        #[arg(long)]
        replay: Option<PathBuf>,
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
        /// Registry URL (default: <https://registry.fajarlang.dev>).
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
    /// V12: Update all dependencies to latest compatible versions.
    Update,
    /// V12: Display dependency tree.
    Tree,
    /// V12: Check dependencies for known vulnerabilities.
    Audit,
    /// Run the self-hosting bootstrap verification chain (Stage 0 → Stage 1 → Stage 2).
    Bootstrap,
    /// Launch a Fajar Lang program with GUI windowing (requires `gui` feature).
    Gui {
        /// Path to the .fj source file.
        file: PathBuf,
    },
    /// Display detected hardware capabilities (CPU, GPU, NPU).
    HwInfo,
    /// Output hardware profile as machine-readable JSON.
    HwJson,
    /// Generate Software Bill of Materials (CycloneDX or SPDX).
    Sbom {
        /// Output format: "cyclonedx" (default) or "spdx".
        #[arg(long, default_value = "cyclonedx")]
        format: String,
        /// Output file path (default: stdout).
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Verify a Fajar Lang source file using formal verification.
    Verify {
        /// Path to the .fj source file.
        file: PathBuf,
        /// Output format: "text" (default), "json", or "smtlib2".
        #[arg(long, default_value = "text")]
        format: String,
        /// Verbose: show each verification condition.
        #[arg(long, short)]
        verbose: bool,
        /// V15 B3.3: Strict mode — warnings become errors.
        #[arg(long)]
        strict: bool,
    },
    /// Generate Fajar Lang FFI bindings from C/C++/Python/Rust headers.
    Bindgen {
        /// Path to the source header file (.h, .hpp, .pyi, .rs).
        file: PathBuf,
        /// Source language: c, cpp, python, rust (auto-detected from extension if omitted).
        #[arg(long)]
        lang: Option<String>,
        /// Output path for generated .fj bindings (default: `<file>.fj`).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Generate safe wrappers around unsafe FFI calls.
        #[arg(long)]
        safe_wrappers: bool,
    },
    /// Profile a Fajar Lang program (collect call timing data).
    Profile {
        /// Path to the .fj source file.
        file: PathBuf,
        /// Number of top hotspot functions to show (default: 10).
        #[arg(long, default_value = "10")]
        top: usize,
        /// Output format: "text" (default), "chrome" (trace JSON), "speedscope".
        #[arg(long, default_value = "text")]
        format: String,
    },
}

fn main() -> ExitCode {
    // SQ11.7: Increase thread stack size to 16MB for deeply recursive
    // programs (self-hosted compiler tokenizing large files).
    let stack_size = 16 * 1024 * 1024; // 16 MB
    let builder = std::thread::Builder::new().stack_size(stack_size);
    let handler = builder
        .spawn(main_inner)
        .expect("failed to spawn main thread with larger stack");
    match handler.join() {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}

fn main_inner() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
            file,
            vm,
            native,
            llvm,
            profile,
            profile_output,
            strict_ownership,
            jit,
            check_only,
            effect_stats,
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
            if check_only {
                cmd_check(&path)
            } else if llvm {
                cmd_run_llvm(&path)
            } else if native {
                cmd_run_native(&path)
            } else if jit {
                cmd_run_jit(&path)
            } else if vm {
                cmd_run_vm(&path)
            } else if profile {
                let out = profile_output.unwrap_or_else(|| "fj-profile.json".to_string());
                cmd_run_profile(&path, &out)
            } else if strict_ownership {
                cmd_run_strict(&path)
            } else if effect_stats {
                cmd_run_with_effect_stats(&path)
            } else {
                cmd_run(&path)
            }
        }
        Command::Repl => cmd_repl(),
        Command::Check {
            file,
            call_graph,
            strict_ownership,
        } => {
            let result = if strict_ownership {
                cmd_check_strict(&file)
            } else {
                cmd_check(&file)
            };
            if call_graph {
                cmd_call_graph(&file);
            }
            result
        }
        Command::DumpTokens { file } => cmd_dump_tokens(&file),
        Command::DumpAst { file } => cmd_dump_ast(&file),
        Command::Fmt { file, check } => cmd_fmt(&file, check),
        Command::Lsp => cmd_lsp(),
        Command::Pack { output, files } => cmd_pack(&output, &files),
        Command::Playground { output } => cmd_playground(&output),
        Command::Demo { name, list } => {
            if list || name.is_none() {
                cmd_demo_list()
            } else {
                cmd_demo(name.as_deref().unwrap_or(""))
            }
        }
        Command::Deploy {
            target,
            file,
            output,
        } => cmd_deploy(&target, &file, &output),
        Command::New { name } => cmd_new(&name),
        Command::Build {
            file,
            target,
            output,
            no_std,
            linker_script,
            backend,
            opt_level,
            target_cpu,
            target_features,
            reloc,
            code_model,
            lto,
            pgo,
            board,
            linker,
            extra_objects,
            verbose,
            incremental,
            all,
            release,
            security,
            lint,
        } => {
            // --all flag: build all targets from fj.toml
            if all {
                return cmd_build_all(verbose);
            }

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
            // WASI P2 component target
            if target == "wasm32-wasi-p2" || target == "wasm32-wasip2" {
                return cmd_build_wasi_p2(&path, output.as_deref(), verbose);
            }
            // V14 Phase 1: AST-driven GPU codegen for SPIR-V/PTX.
            // Reads .fj source, parses it, finds @gpu fns, and generates shader code.
            // Falls back to hardcoded minimal kernel if no .fj source has @gpu fns.
            // v35.7.1 (Action C): Metal + HLSL targets removed. See
            // docs/decisions/2026-05-12-gpu-codegen-simplification.md.
            if matches!(target.as_str(), "spirv" | "ptx") {
                let ext = match target.as_str() {
                    "spirv" => "spv",
                    "ptx" => "ptx",
                    _ => unreachable!(),
                };
                let default_out = format!("output.{ext}");
                let out_path = output
                    .as_deref()
                    .unwrap_or_else(|| std::path::Path::new(&default_out));

                // Try AST-driven codegen: read .fj source → parse → lower → emit
                let gpu_ir = if path.extension().is_some_and(|e| e == "fj") {
                    let source = match std::fs::read_to_string(&path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("error reading {}: {e}", path.display());
                            return ExitCode::from(EXIT_RUNTIME);
                        }
                    };
                    let tokens = match fajar_lang::lexer::tokenize(&source) {
                        Ok(t) => t,
                        Err(errs) => {
                            for e in &errs {
                                eprintln!("{e}");
                            }
                            return ExitCode::from(EXIT_RUNTIME);
                        }
                    };
                    let program = match fajar_lang::parser::parse(tokens) {
                        Ok(p) => p,
                        Err(errs) => {
                            for e in &errs {
                                eprintln!("{e}");
                            }
                            return ExitCode::from(EXIT_RUNTIME);
                        }
                    };
                    fajar_lang::gpu_codegen::lower_to_gpu_ir(&program).ok()
                } else {
                    None
                };

                let (bytes, label) = if let Some(ir) = gpu_ir {
                    let kernel = &ir.kernels[0];
                    match target.as_str() {
                        "spirv" => (kernel.to_spirv(), "SPIR-V compute shader"),
                        "ptx" => (kernel.to_ptx().into_bytes(), "PTX assembly"),
                        _ => unreachable!(),
                    }
                } else {
                    // Fallback: hardcoded minimal kernels (backwards compat)
                    match target.as_str() {
                        "spirv" => {
                            let mut m = fajar_lang::gpu_codegen::spirv::SpirVModule::new_compute();
                            (
                                m.emit_elementwise_add_shader("main"),
                                "SPIR-V compute shader",
                            )
                        }
                        "ptx" => {
                            let mut m = fajar_lang::gpu_codegen::ptx::PtxModule {
                                ptx_version: 75,
                                sm_version: 80,
                                address_size: 64,
                                kernels: Vec::new(),
                                shared_decls: Vec::new(),
                            };
                            m.add_elementwise_add_kernel("main");
                            (m.emit().into_bytes(), "PTX assembly")
                        }
                        _ => unreachable!(),
                    }
                };

                match std::fs::write(out_path, &bytes) {
                    Ok(()) => {
                        println!(
                            "{label} written to {} ({} bytes)",
                            out_path.display(),
                            bytes.len()
                        );
                        return ExitCode::SUCCESS;
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        return ExitCode::from(EXIT_RUNTIME);
                    }
                }
            }
            // --release flag: auto-select LLVM with O2
            let effective_backend = if release { "llvm" } else { &backend };
            let effective_opt = if release && opt_level == "0" {
                "2".to_string()
            } else {
                opt_level.clone()
            };

            if effective_backend == "llvm" {
                if verbose {
                    eprintln!(
                        "[verbose] Using LLVM backend (O{effective_opt}){}",
                        if release { " [release mode]" } else { "" }
                    );
                }
                // --release with no explicit --lto defaults to thin LTO
                let effective_lto = if release && lto == "none" {
                    "thin".to_string()
                } else {
                    lto.clone()
                };
                cmd_build_llvm(
                    &path,
                    output.as_deref(),
                    &effective_opt,
                    &target_cpu,
                    &target_features,
                    &reloc,
                    &code_model,
                    &effective_lto,
                    &pgo,
                    &target,
                    no_std,
                    ls.as_deref(),
                    linker.as_deref(),
                    &extra_objects,
                    verbose,
                )
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
                    let cranelift_opt: u8 = effective_opt.parse().unwrap_or(0);
                    let r = cmd_build(
                        &path,
                        &target,
                        output.as_deref(),
                        no_std,
                        ls.as_deref(),
                        linker.as_deref(),
                        security,
                        lint,
                        cranelift_opt,
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
        Command::Publish {
            local: _,
            registry: _,
        } => cmd_publish(),
        Command::RegistryInit { path } => cmd_registry_init(&path),
        Command::RegistryServe { port } => cmd_registry_serve(port),
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
        Command::Debug {
            file,
            dap,
            record,
            replay,
        } => {
            if let Some(trace_path) = replay {
                cmd_debug_replay(&trace_path)
            } else if dap {
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
                if let Some(trace_path) = record {
                    cmd_debug_record(&path, &trace_path)
                } else {
                    eprintln!(
                        "Interactive debugger for '{}' not yet implemented.",
                        path.display()
                    );
                    eprintln!("hint: use `fj debug --dap` for IDE integration via DAP protocol");
                    eprintln!(
                        "hint: use `fj debug --record trace.json <file>` to record execution"
                    );
                    ExitCode::from(EXIT_USAGE)
                }
            }
        }
        Command::Plugin { action, path } => cmd_plugin(&action, path.as_deref()),
        Command::Search { query, limit } => cmd_search(&query, limit),
        Command::Login { token, registry } => cmd_login(token.as_deref(), registry.as_deref()),
        Command::Yank { package, version } => cmd_yank(&package, &version),
        Command::Install {
            package,
            version,
            offline,
        } => cmd_install(&package, version.as_deref(), offline),
        // V12 Gap Closure: Package management commands
        Command::Update => cmd_update(),
        Command::Tree => cmd_tree(),
        Command::Audit => cmd_audit(),
        Command::Bootstrap => cmd_bootstrap(),
        Command::Gui { file } => cmd_gui(&file),
        Command::HwInfo => cmd_hw_info(),
        Command::HwJson => cmd_hw_json(),
        Command::Sbom { format, output } => cmd_sbom(&format, output.as_deref()),
        Command::Verify {
            file,
            format,
            verbose,
            strict,
        } => cmd_verify(&file, &format, verbose, strict),
        Command::Bindgen {
            file,
            lang,
            output,
            safe_wrappers,
        } => cmd_bindgen(&file, lang.as_deref(), output.as_deref(), safe_wrappers),
        Command::Profile { file, top, format } => cmd_profile(&file, top, &format),
    }
}

/// Exit code for runtime errors.
pub(crate) const EXIT_RUNTIME: u8 = 1;
/// Exit code for compile errors (lex, parse, semantic).
pub(crate) const EXIT_COMPILE: u8 = 2;
/// Exit code for usage errors (file not found, bad arguments).
pub(crate) const EXIT_USAGE: u8 = 3;

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
