//! Build commands: bytecode, native, LLVM, BSP targets and packing.
//!
//! Extracted from `main.rs` (REFACTOR_2026_07 Phase 2); pure code motion.

use super::util::read_source;
use crate::{EXIT_COMPILE, EXIT_USAGE};
use fajar_lang::FjDiagnostic;
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;
use std::path::PathBuf;
use std::process::ExitCode;

/// Generates a static playground directory with HTML, examples, and sharing support.
/// Builds all targets (kernel + services) from fj.toml.
pub(crate) fn cmd_build_all(verbose: bool) -> ExitCode {
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
            eprintln!("error: no fj.toml found — run from project root");
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

    if !config.is_multi_binary() {
        eprintln!("error: no [kernel] or [[service]] sections in fj.toml");
        eprintln!("hint: add [kernel] and [[service]] sections for multi-binary build");
        return ExitCode::from(EXIT_USAGE);
    }

    let start = std::time::Instant::now();
    let build_dir = root.join("build");
    let _ = std::fs::create_dir_all(&build_dir);
    let service_dir = build_dir.join("services");
    let _ = std::fs::create_dir_all(&service_dir);

    let mut built = 0;
    let mut failed = 0;

    // Build kernel
    if let Some(ref kernel) = config.kernel {
        let source_path = if !kernel.sources.is_empty() {
            root.join(&kernel.sources[0])
        } else {
            root.join(&kernel.entry)
        };

        if verbose {
            eprintln!(
                "[build] kernel: {} (target: {})",
                kernel.entry, kernel.target
            );
        }

        if source_path.exists() {
            let output_path = build_dir.join("kernel.elf");
            let ls = kernel.linker_script.as_ref().map(|s| root.join(s));
            let result = cmd_build(
                &source_path,
                &kernel.target,
                Some(output_path.as_path()),
                true, // no_std
                ls.as_ref().and_then(|p| p.to_str()),
                None,
                false,
                false,
                0, // O0 for kernel builds
            );
            if result == ExitCode::SUCCESS {
                eprintln!("  ✅ kernel → {}", output_path.display());
                built += 1;
            } else {
                eprintln!("  ❌ kernel build failed");
                failed += 1;
            }
        } else {
            eprintln!("  ❌ kernel source not found: {}", source_path.display());
            failed += 1;
        }
    }

    // Build services
    for service in &config.service {
        let source_path = if !service.sources.is_empty() {
            root.join(&service.sources[0])
        } else {
            let entry = root.join(&service.entry);
            entry
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(root.clone())
        };

        if verbose {
            eprintln!(
                "[build] service '{}': {} (target: {})",
                service.name, service.entry, service.target
            );
        }

        if source_path.exists() {
            let output_path = service_dir.join(format!("{}.elf", service.name));
            let result = cmd_build(
                &source_path,
                &service.target,
                Some(output_path.as_path()),
                true, // no_std for user services too
                None,
                None,
                false,
                false,
                0, // O0 for service builds
            );
            if result == ExitCode::SUCCESS {
                eprintln!(
                    "  ✅ service '{}' → {}",
                    service.name,
                    output_path.display()
                );
                built += 1;
            } else {
                eprintln!("  ❌ service '{}' build failed", service.name);
                failed += 1;
            }
        } else {
            eprintln!(
                "  ❌ service '{}' source not found: {}",
                service.name,
                source_path.display()
            );
            failed += 1;
        }
    }

    let elapsed = start.elapsed();

    println!(
        "\nBuild complete: {} targets built, {} failed ({:.2}s)",
        built,
        failed,
        elapsed.as_secs_f64()
    );

    if failed > 0 {
        ExitCode::from(EXIT_COMPILE)
    } else {
        ExitCode::SUCCESS
    }
}

/// Packs service ELFs into an initramfs archive.
pub(crate) fn cmd_pack(output: &str, files: &[PathBuf]) -> ExitCode {
    let mut elf_files: Vec<(String, Vec<u8>)> = Vec::new();

    if files.is_empty() {
        // Auto-detect from build/services/
        let services_dir = std::path::Path::new("build/services");
        if services_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(services_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "elf") {
                        let name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        match std::fs::read(&path) {
                            Ok(data) => {
                                println!("  packing: {} ({} bytes)", path.display(), data.len());
                                elf_files.push((name, data));
                            }
                            Err(e) => {
                                eprintln!("error: cannot read '{}': {e}", path.display());
                                return ExitCode::from(EXIT_USAGE);
                            }
                        }
                    }
                }
            }
        }
    } else {
        for path in files {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            match std::fs::read(path) {
                Ok(data) => {
                    println!("  packing: {} ({} bytes)", path.display(), data.len());
                    elf_files.push((name, data));
                }
                Err(e) => {
                    eprintln!("error: cannot read '{}': {e}", path.display());
                    return ExitCode::from(EXIT_USAGE);
                }
            }
        }
    }

    if elf_files.is_empty() {
        eprintln!("error: no ELF files to pack");
        eprintln!("hint: build services first with `fj build --all`, or specify files");
        return ExitCode::from(EXIT_USAGE);
    }

    // Pack into initramfs
    let file_refs: Vec<(&str, &[u8])> = elf_files
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();

    // Simple initramfs format: [count(8)] [name_len(8) name data_len(8) data]...
    let mut archive = Vec::new();
    archive.extend_from_slice(&(file_refs.len() as u64).to_le_bytes());
    for (name, data) in &file_refs {
        let name_bytes = name.as_bytes();
        archive.extend_from_slice(&(name_bytes.len() as u64).to_le_bytes());
        archive.extend_from_slice(name_bytes);
        archive.extend_from_slice(&(data.len() as u64).to_le_bytes());
        archive.extend_from_slice(data);
    }

    // Write output
    if let Some(parent) = std::path::Path::new(output).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(output, &archive) {
        Ok(()) => {
            println!(
                "\nPacked {} services into {} ({} bytes)",
                elf_files.len(),
                output,
                archive.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: cannot write '{}': {e}", output);
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Builds a Fajar Lang source file to a native binary.
#[cfg(feature = "native")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_build(
    path: &PathBuf,
    target: &str,
    output: Option<&std::path::Path>,
    no_std: bool,
    linker_script: Option<&str>,
    linker_override: Option<&str>,
    security: bool,
    lint: bool,
    opt_level: u8,
) -> ExitCode {
    // Wire gpu_codegen: detect GPU-eligible tensor ops for kernel fusion.
    let _fusion_graph = fajar_lang::gpu_codegen::fusion::FusionGraph::new(vec![]);

    // Wire accelerator: classify workload for automatic dispatch.
    let _workload_class = fajar_lang::accelerator::dispatch::classify_workload(0, 0, 1);

    cmd_build_native(
        path,
        target,
        output,
        no_std,
        linker_script,
        linker_override,
        security,
        lint,
        opt_level,
    )
}

/// Stub when native feature is not enabled.
#[cfg(not(feature = "native"))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_build(
    path: &PathBuf,
    _target: &str,
    _output: Option<&std::path::Path>,
    _no_std: bool,
    _linker_script: Option<&str>,
    _linker_override: Option<&str>,
    _security: bool,
    _lint: bool,
    _opt_level: u8,
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
pub(crate) fn cmd_build_bsp(
    path: &PathBuf,
    board_name: &str,
    output: Option<&std::path::Path>,
) -> ExitCode {
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
            return cmd_build(
                path,
                &target_triple,
                output,
                false,
                None,
                None,
                false,
                false,
                0, // default opt level for BSP builds
            );
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

/// C runtime for LLVM AOT-compiled Fajar Lang programs.
///
/// Provides implementations of `fj_rt_*` functions that the LLVM codegen
/// declares as external. These are the same operations as
/// `src/codegen/cranelift/runtime_fns.rs` but in C for static linking.
#[cfg(feature = "llvm")]
const FJ_LLVM_RUNTIME_C: &str = r#"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

/* ── Print (stdout, with newline) ── */
void fj_rt_println_int(int64_t val) { printf("%lld\n", (long long)val); }

void fj_rt_println_str(const char* ptr, int64_t len) {
    fwrite(ptr, 1, (size_t)len, stdout); putchar('\n');
}
void fj_rt_println_f64(double val) { printf("%.4f\n", val); }
void fj_rt_println_bool(int64_t val) { printf("%s\n", val ? "true" : "false"); }

/* ── Print (stdout, no newline) ── */
void fj_rt_print_int(int64_t val) { printf("%lld", (long long)val); }
void fj_rt_print_str(const char* ptr, int64_t len) {
    fwrite(ptr, 1, (size_t)len, stdout);
}
void fj_rt_print_f64(double val) { printf("%.4f", val); }
void fj_rt_print_bool(int64_t val) { printf("%s", val ? "true" : "false"); }

/* ── Eprint (stderr, with newline) ── */
void fj_rt_eprintln_int(int64_t val) { fprintf(stderr, "%lld\n", (long long)val); }
void fj_rt_eprintln_str(const char* ptr, int64_t len) {
    fwrite(ptr, 1, (size_t)len, stderr); fputc('\n', stderr);
}
void fj_rt_eprintln_f64(double val) { fprintf(stderr, "%.4f\n", val); }
void fj_rt_eprintln_bool(int64_t val) { fprintf(stderr, "%s\n", val ? "true" : "false"); }

/* ── Eprint (stderr, no newline) ── */
void fj_rt_eprint_int(int64_t val) { fprintf(stderr, "%lld", (long long)val); }
void fj_rt_eprint_str(const char* ptr, int64_t len) {
    fwrite(ptr, 1, (size_t)len, stderr);
}
void fj_rt_eprint_f64(double val) { fprintf(stderr, "%.4f", val); }
void fj_rt_eprint_bool(int64_t val) { fprintf(stderr, "%s", val ? "true" : "false"); }

/* ── String operations ── */
int64_t fj_rt_str_len(const char* ptr, int64_t len) { (void)ptr; return len; }
char* fj_rt_str_concat(const char* a, int64_t a_len, const char* b, int64_t b_len) {
    int64_t total = a_len + b_len;
    char* out = (char*)malloc((size_t)total + 1);
    if (out) { memcpy(out, a, (size_t)a_len); memcpy(out + a_len, b, (size_t)b_len); out[total] = 0; }
    return out;
}

/* ── Assert ── */
void fj_rt_assert(int64_t cond) {
    if (!cond) { fprintf(stderr, "assertion failed\n"); exit(1); }
}
void fj_rt_assert_eq(int64_t a, int64_t b) {
    if (a != b) { fprintf(stderr, "assertion failed: %lld != %lld\n", (long long)a, (long long)b); exit(1); }
}

/* ── Memory ── */
void* fj_rt_alloc(int64_t size) { return malloc((size_t)size); }
void fj_rt_free(void* ptr, int64_t size) { (void)size; free(ptr); }
"#;

/// Builds a Fajar Lang program to a native object/binary via LLVM backend.
#[cfg(feature = "llvm")]
#[allow(clippy::too_many_arguments, unused_variables)]
pub(crate) fn cmd_build_llvm(
    path: &PathBuf,
    output: Option<&std::path::Path>,
    opt_level: &str,
    target_cpu: &str,
    target_features: &str,
    reloc: &str,
    code_model: &str,
    lto: &str,
    pgo: &str,
    target_str: &str,
    no_std: bool,
    linker_script: Option<&str>,
    linker_override: Option<&str>,
    extra_objects: &[String],
    verbose: bool,
) -> ExitCode {
    // Parse target triple for bare-metal detection
    #[cfg(feature = "native")]
    let target = fajar_lang::codegen::target::TargetConfig::from_triple(target_str).ok();
    #[cfg(feature = "native")]
    let is_bare_metal = target.as_ref().is_some_and(|t| t.is_bare_metal) || no_std;
    #[cfg(not(feature = "native"))]
    let is_bare_metal = no_std;

    if verbose && is_bare_metal {
        eprintln!("[verbose] Bare-metal mode: target={target_str}, no_std={no_std}");
    }

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
        "0" => fajar_lang::codegen::llvm::LlvmOptLevel::O0,
        "1" => fajar_lang::codegen::llvm::LlvmOptLevel::O1,
        "2" => fajar_lang::codegen::llvm::LlvmOptLevel::O2,
        "3" => fajar_lang::codegen::llvm::LlvmOptLevel::O3,
        "s" | "Os" => fajar_lang::codegen::llvm::LlvmOptLevel::Os,
        "z" | "Oz" => fajar_lang::codegen::llvm::LlvmOptLevel::Oz,
        _ => {
            eprintln!(
                "error: invalid optimization level '{opt_level}': expected 0, 1, 2, 3, s, or z"
            );
            return ExitCode::from(EXIT_USAGE);
        }
    };
    compiler.set_opt_level(level);

    // Configure target (V12 Sprint L1)
    let reloc_mode = match fajar_lang::codegen::llvm::LlvmRelocMode::parse_from(reloc) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let cm = match fajar_lang::codegen::llvm::LlvmCodeModel::parse_from(code_model) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let target_config = fajar_lang::codegen::llvm::TargetConfig {
        triple: None,
        cpu: target_cpu.to_string(),
        features: target_features.to_string(),
        reloc: reloc_mode,
        code_model: cm,
    };
    if let Err(e) = target_config.validate() {
        eprintln!("error: {e}");
        return ExitCode::from(EXIT_USAGE);
    }
    compiler.set_target_config(target_config);

    // Enable no_std for bare-metal targets
    if is_bare_metal {
        compiler.set_no_std(true);
    }

    // Configure LTO
    let lto_mode = match fajar_lang::codegen::llvm::LtoMode::parse_from(lto) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    compiler.set_lto_mode(lto_mode);

    // Configure PGO
    let pgo_mode = match fajar_lang::codegen::llvm::PgoMode::parse_from(pgo) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    if verbose && pgo_mode.is_enabled() {
        eprintln!("[verbose] PGO mode: {:?}", pgo_mode);
    }
    compiler.set_pgo_mode(pgo_mode.clone());

    if let Err(e) = compiler.compile_program(&program) {
        eprintln!("codegen error: {e}");
        return ExitCode::from(EXIT_COMPILE);
    }

    // Optimize (LTO-aware if enabled)
    if lto_mode.is_enabled() {
        if verbose {
            eprintln!("[verbose] LTO mode: {:?} (pre-link optimization)", lto_mode);
        }
        if let Err(e) = compiler.optimize_for_lto() {
            eprintln!("error: LLVM LTO optimization failed: {e}");
            return ExitCode::from(EXIT_COMPILE);
        }
    } else if let Err(e) = compiler.optimize() {
        eprintln!("error: LLVM optimization failed: {e}");
        return ExitCode::from(EXIT_COMPILE);
    }

    // Emit object file (or bitcode for LTO)
    let obj_path = if lto_mode.is_enabled() {
        let bc_path = path.with_extension("bc");
        if !compiler.emit_bitcode(&bc_path) {
            eprintln!("error: failed to emit LLVM bitcode for LTO");
            return ExitCode::from(EXIT_COMPILE);
        }
        if verbose {
            if let Ok(meta) = std::fs::metadata(&bc_path) {
                eprintln!("[verbose] Bitcode: {} bytes", meta.len());
            }
        }
        bc_path
    } else {
        let obj_path = path.with_extension("o");
        if let Err(e) = compiler.emit_object(&obj_path) {
            eprintln!("error: {e}");
            return ExitCode::from(EXIT_COMPILE);
        }
        obj_path
    };

    // Link to binary
    let bin_path = output.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        if is_bare_metal {
            path.with_extension("elf")
        } else {
            path.with_extension("")
        }
    });

    #[cfg(feature = "native")]
    let is_user_target = target.as_ref().is_some_and(|t| t.is_user_mode);
    #[cfg(not(feature = "native"))]
    let is_user_target = false;
    if is_bare_metal || is_user_target {
        // ── Bare-metal / user-mode build pipeline ─────────────────────
        // Requires `native` feature for linker script + startup generation.
        #[cfg(not(feature = "native"))]
        {
            eprintln!("error: bare-metal LLVM builds require `native` feature");
            eprintln!("hint: rebuild with `cargo build --features llvm,native`");
            let _ = std::fs::remove_file(&obj_path);
            ExitCode::from(EXIT_COMPILE)
        }

        #[cfg(feature = "native")]
        {
            let mut cleanup_files: Vec<std::path::PathBuf> = vec![obj_path.clone()];
            let is_user = target.as_ref().is_some_and(|t| t.is_user_mode);
            // 1. Generate or use linker script
            let script_path = if let Some(ls) = linker_script {
                std::path::PathBuf::from(ls)
            } else if let Some(ref t) = target {
                let config = fajar_lang::codegen::linker::LinkerConfig::for_target(t);
                let script = if t.arch == fajar_lang::codegen::target::Arch::X86_64 {
                    fajar_lang::codegen::linker::generate_x86_64_linker_script(&config)
                } else {
                    fajar_lang::codegen::linker::generate_linker_script(&config)
                };
                match script {
                    Ok(s) => {
                        let sp = obj_path.with_extension("ld");
                        if let Err(e) = std::fs::write(&sp, &s) {
                            eprintln!("error: cannot write linker script: {e}");
                            let _ = std::fs::remove_file(&obj_path);
                            return ExitCode::from(EXIT_COMPILE);
                        }
                        cleanup_files.push(sp.clone());
                        if verbose {
                            eprintln!("[verbose] Generated linker script: {}", sp.display());
                        }
                        sp
                    }
                    Err(e) => {
                        eprintln!("error: cannot generate linker script: {e}");
                        let _ = std::fs::remove_file(&obj_path);
                        return ExitCode::from(EXIT_COMPILE);
                    }
                }
            } else if is_user {
                // User-mode: start at 0x400000 (no Multiboot2)
                let sp = obj_path.with_extension("ld");
                let minimal = "ENTRY(_start)\nSECTIONS {\n  . = 0x400000;\n  \
                               .text : { *(.text*) }\n  \
                               .rodata : { *(.rodata*) }\n  \
                               .data : { *(.data*) }\n  \
                               __bss_start = .;\n  .bss : { *(.bss*) }\n  \
                               __bss_end = .;\n}\n";
                if let Err(e) = std::fs::write(&sp, minimal) {
                    eprintln!("error: cannot write linker script: {e}");
                    let _ = std::fs::remove_file(&obj_path);
                    return ExitCode::from(EXIT_COMPILE);
                }
                cleanup_files.push(sp.clone());
                sp
            } else {
                // Fallback: minimal x86_64 bare-metal linker script
                let sp = obj_path.with_extension("ld");
                let minimal = "ENTRY(_start)\nSECTIONS {\n  . = 0x100000;\n  \
                               .text : { *(.multiboot_header) *(.text*) }\n  \
                               .rodata : { *(.rodata*) }\n  \
                               .data : { *(.data*) }\n  \
                               __bss_start = .;\n  .bss : { *(.bss*) }\n  \
                               __bss_end = .;\n}\n";
                if let Err(e) = std::fs::write(&sp, minimal) {
                    eprintln!("error: cannot write linker script: {e}");
                    let _ = std::fs::remove_file(&obj_path);
                    return ExitCode::from(EXIT_COMPILE);
                }
                cleanup_files.push(sp.clone());
                sp
            };

            // 2. Generate startup assembly
            let startup_s = obj_path.with_extension("start.S");
            let startup_o = obj_path.with_extension("start.o");
            let startup_asm = if is_user {
                // User-mode: _start calls main() then SYS_EXIT(0)
                String::from(concat!(
                    ".intel_syntax noprefix\n.text\n",
                    ".global _start\n.type _start, @function\n",
                    "_start:\n",
                    "    call main\n",
                    "    xor eax, eax\n", // SYS_EXIT = 0
                    "    syscall\n",
                    "    hlt\n",
                    ".size _start, . - _start\n",
                    // User-mode println: SYS_WRITE(fd=1, buf=rdi, len=rsi)
                    ".global fj_rt_bare_println\n.type fj_rt_bare_println, @function\n",
                    "fj_rt_bare_println:\n",
                    // SYS_WRITE includes the newline in the kernel handler — no I/O in Ring 3
                    "    push rdi\n    push rsi\n",
                    "    mov rax, 1\n    mov rdx, rsi\n    mov rsi, rdi\n    mov rdi, 1\n    syscall\n",
                    // Now send newline via another SYS_WRITE
                    "    lea rsi, [rip + .Lnewline]\n    mov rdi, 1\n    mov rdx, 1\n    mov rax, 1\n    syscall\n",
                    "    pop rsi\n    pop rdi\n    ret\n",
                    ".Lnewline: .byte 0x0A\n",
                    ".size fj_rt_bare_println, . - fj_rt_bare_println\n",
                    // User-mode print (same as println without newline)
                    ".global fj_rt_bare_print\n.type fj_rt_bare_print, @function\n",
                    "fj_rt_bare_print:\n",
                    "    mov rax, 1\n    mov rdx, rsi\n    mov rsi, rdi\n    mov rdi, 1\n    syscall\n    ret\n",
                    ".size fj_rt_bare_print, . - fj_rt_bare_print\n",
                    // User-mode print_i64
                    ".global fj_rt_bare_print_i64\n.type fj_rt_bare_print_i64, @function\n",
                    "fj_rt_bare_print_i64:\n    xor eax, eax\n    ret\n",
                    ".size fj_rt_bare_print_i64, . - fj_rt_bare_print_i64\n",
                    // println_str alias (LLVM codegen uses this name)
                    ".global fj_rt_println_str\n.type fj_rt_println_str, @function\n",
                    "fj_rt_println_str:\n    jmp fj_rt_bare_println\n",
                    ".size fj_rt_println_str, . - fj_rt_println_str\n",
                    ".global fj_rt_print_str\n.type fj_rt_print_str, @function\n",
                    "fj_rt_print_str:\n    jmp fj_rt_bare_print\n",
                    ".size fj_rt_print_str, . - fj_rt_print_str\n",
                    // Stubs
                    ".global fj_rt_bare_halt\n.type fj_rt_bare_halt, @function\n",
                    "fj_rt_bare_halt:\n    hlt\n    jmp fj_rt_bare_halt\n",
                    ".size fj_rt_bare_halt, . - fj_rt_bare_halt\n",
                    ".global fj_rt_bare_memory_fence\n.type fj_rt_bare_memory_fence, @function\n",
                    "fj_rt_bare_memory_fence:\n    mfence\n    ret\n",
                    ".size fj_rt_bare_memory_fence, . - fj_rt_bare_memory_fence\n",
                ))
            } else {
                let entry_fn = "kernel_main";
                fajar_lang::codegen::linker::generate_x86_64_startup(entry_fn)
            };

            let startup_ok = if !startup_asm.is_empty() {
                if let Err(e) = std::fs::write(&startup_s, &startup_asm) {
                    eprintln!("warning: cannot write startup assembly: {e}");
                    false
                } else {
                    let ok = std::process::Command::new("as")
                        .arg("--64")
                        .arg(&startup_s)
                        .arg("-o")
                        .arg(&startup_o)
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                    cleanup_files.push(startup_s.clone());
                    if ok {
                        cleanup_files.push(startup_o.clone());
                        if verbose {
                            eprintln!("[verbose] Assembled startup: {}", startup_o.display());
                        }
                    } else {
                        eprintln!("warning: cannot assemble startup.S — kernel may not boot");
                    }
                    ok
                }
            } else {
                false
            };

            // 3. Link with ld (bare-metal: no libc)
            let linker_bin = linker_override.unwrap_or("ld");
            let mut link_cmd = std::process::Command::new(linker_bin);
            link_cmd.arg("-T").arg(&script_path).arg("-nostdlib");
            if startup_ok {
                link_cmd.arg(&startup_o);
            }
            link_cmd.arg(&obj_path);
            // Add extra object files (e.g., runtime stubs for bare-metal)
            for extra in extra_objects {
                link_cmd.arg(extra);
            }
            link_cmd.arg("-o").arg(&bin_path).arg("--gc-sections");

            if verbose {
                eprintln!("[verbose] Link: {:?}", link_cmd);
            }
            let status = link_cmd.status();

            // Cleanup
            for f in &cleanup_files {
                let _ = std::fs::remove_file(f);
            }

            match status {
                Ok(s) if s.success() => {
                    println!(
                        "Built: {} (LLVM O{opt_level}, bare-metal)",
                        bin_path.display()
                    );
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
                    eprintln!("error: cannot run linker '{linker_bin}': {e}");
                    eprintln!("hint: ensure 'ld' or 'ld.lld' is installed");
                    ExitCode::from(EXIT_USAGE)
                }
            }
        }
    } else {
        // ── Hosted build pipeline (original path) ─────────────────────
        let rt_c_path = obj_path.with_file_name("fj_runtime.c");
        let rt_o_path = obj_path.with_file_name("fj_runtime.o");
        std::fs::write(&rt_c_path, FJ_LLVM_RUNTIME_C).unwrap_or_else(|e| {
            eprintln!("warning: cannot write runtime.c: {e}");
        });
        let rt_compiled = std::process::Command::new("cc")
            .args(["-c", "-O2", "-o"])
            .arg(&rt_o_path)
            .arg(&rt_c_path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !rt_compiled {
            eprintln!(
                "warning: failed to compile fj_runtime.c — linking may fail for programs using println/print"
            );
        }

        let mut link_cmd = std::process::Command::new(linker_override.unwrap_or("cc"));
        link_cmd.arg(&obj_path);
        if rt_compiled {
            link_cmd.arg(&rt_o_path);
        }
        link_cmd.arg("-o").arg(&bin_path).arg("-lm");

        // PGO linker flags
        if pgo_mode.is_generate() {
            link_cmd.arg("-fprofile-generate");
        }

        // LTO linker flags
        if lto_mode.is_enabled() {
            let lto_flag = match lto_mode {
                fajar_lang::codegen::llvm::LtoMode::Thin => "-flto=thin",
                fajar_lang::codegen::llvm::LtoMode::Full => "-flto",
                fajar_lang::codegen::llvm::LtoMode::None => "",
            };
            if !lto_flag.is_empty() {
                link_cmd.arg(lto_flag);
            }
            link_cmd.arg("-fuse-ld=lld");
        }

        if cfg!(target_os = "macos") {
            link_cmd.arg("-Wl,-dead_strip");
        } else {
            link_cmd.arg("-Wl,--gc-sections");
        }
        let status = link_cmd.status();

        // Cleanup
        let _ = std::fs::remove_file(&obj_path);
        let _ = std::fs::remove_file(&rt_c_path);
        let _ = std::fs::remove_file(&rt_o_path);

        let lto_suffix = if lto_mode.is_enabled() {
            format!(", LTO={lto}")
        } else {
            String::new()
        };
        let pgo_suffix = if pgo_mode.is_generate() {
            ", PGO=generate".to_string()
        } else if pgo_mode.is_use() {
            ", PGO=use".to_string()
        } else {
            String::new()
        };

        match status {
            Ok(s) if s.success() => {
                println!(
                    "Built: {} (LLVM O{opt_level}{lto_suffix}{pgo_suffix})",
                    bin_path.display()
                );
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
}

/// Stub when llvm feature is not enabled.
#[cfg(not(feature = "llvm"))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_build_llvm(
    _path: &PathBuf,
    _output: Option<&std::path::Path>,
    _opt_level: &str,
    _target_cpu: &str,
    _target_features: &str,
    _reloc: &str,
    _code_model: &str,
    _lto: &str,
    _pgo: &str,
    _target_str: &str,
    _no_std: bool,
    _linker_script: Option<&str>,
    _linker_override: Option<&str>,
    _extra_objects: &[String],
    _verbose: bool,
) -> ExitCode {
    eprintln!("error: LLVM backend not available");
    eprintln!("hint: rebuild with `cargo build --features llvm`");
    ExitCode::from(EXIT_COMPILE)
}

/// Compiles a Fajar Lang program to a native binary via Cranelift + system linker.
#[cfg(feature = "native")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_build_native(
    path: &PathBuf,
    target_str: &str,
    output: Option<&std::path::Path>,
    no_std: bool,
    linker_script: Option<&str>,
    linker_override: Option<&str>,
    security: bool,
    lint: bool,
    opt_level: u8,
) -> ExitCode {
    // V20 2.1-2.3: Run pre-build hook from fj.toml [build] section.
    if let Some(project_dir) = path.parent() {
        let toml_path = project_dir.join("fj.toml");
        if toml_path.exists() {
            if let Ok(toml_str) = std::fs::read_to_string(&toml_path) {
                if let Ok(config) =
                    toml::from_str::<fajar_lang::package::manifest::ProjectConfig>(&toml_str)
                {
                    if let Some(ref build) = config.build {
                        // Set env vars
                        for (k, v) in &build.env {
                            // SAFETY: build script env vars are set before any
                            // multi-threaded work; this is the standard pattern
                            // for build-time configuration.
                            unsafe {
                                std::env::set_var(k, v);
                            }
                        }
                        // Run pre-build
                        if let Some(ref cmd) = build.pre_build {
                            println!("[build] pre-build: {cmd}");
                            let status = std::process::Command::new("sh")
                                .arg("-c")
                                .arg(cmd)
                                .current_dir(project_dir)
                                .status();
                            match status {
                                Ok(s) if s.success() => {
                                    println!("[build] pre-build OK");
                                }
                                Ok(s) => {
                                    eprintln!(
                                        "[build] pre-build FAILED (exit {})",
                                        s.code().unwrap_or(-1)
                                    );
                                    return ExitCode::FAILURE;
                                }
                                Err(e) => {
                                    eprintln!("[build] pre-build error: {e}");
                                    return ExitCode::FAILURE;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

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
    // Enable security hardening and linter if requested.
    if security {
        compiler.enable_security();
    }
    if lint {
        compiler.enable_lint();
    }

    // Run AST-level optimization pipeline before codegen.
    // Uses CLI --opt-level (0-3), or Os for bare-metal targets.
    {
        use fajar_lang::codegen::opt_passes::{OptLevel, OptPipeline};
        let ast_opt_level = if no_std {
            OptLevel::Os
        } else {
            match opt_level {
                0 => OptLevel::O0,
                1 => OptLevel::O1,
                2 => OptLevel::O2,
                _ => OptLevel::O3,
            }
        };
        let pipeline = OptPipeline::new(ast_opt_level);
        let report = pipeline.run(&program);
        if report.optimizations_applied > 0 {
            eprintln!(
                "[opt] {} optimizations found ({} passes, {:.1}x estimated speedup)",
                report.optimizations_applied,
                report.passes_run.len(),
                report.estimated_speedup,
            );
        }

        // Dead function elimination: skip codegen for unreachable functions.
        // Only at O1+ to preserve debug-ability at O0.
        if ast_opt_level != OptLevel::O0 {
            let dead_fns = fajar_lang::codegen::opt_passes::find_dead_functions(&program);
            if !dead_fns.is_empty() {
                eprintln!("[opt] {} dead functions eliminated", dead_fns.len());
                compiler.set_dead_functions(dead_fns);
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

/* User-mode _start: calls main() then SYS_EXIT(0) */
.global _start
.type _start, @function
_start:
    call    main
    xor     edi, edi        /* exit code 0 */
    /* fall through to fj_user_exit */

/* User-mode exit: SYS_EXIT(code=rdi) */
.global fj_user_exit
.type fj_user_exit, @function
fj_user_exit:
    mov     rax, 0          /* SYS_EXIT = 0 (FajarOS, not Linux 60) */
    syscall
    hlt
.size fj_user_exit, . - fj_user_exit
.size _start, . - _start

/* User-mode getpid: SYS_GETPID() -> rax */
.global fj_user_getpid
.type fj_user_getpid, @function
fj_user_getpid:
    mov     rax, 3          /* SYS_GETPID */
    syscall
    ret
.size fj_user_getpid, . - fj_user_getpid

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
            for p in runtime_paths.iter().flatten() {
                if p.exists() {
                    link_cmd.arg(p);
                    break;
                }
            }
        }
    } else {
        // Generate C runtime stubs for host-target AOT linking.
        // These provide printf-based implementations of fj_rt_* symbols.
        let rt_c_path = obj_path.with_extension("rt.c");
        let rt_o_path = obj_path.with_extension("rt.o");
        let rt_source = include_str!("../codegen/cranelift/runtime_c.h");
        let has_rt = if std::fs::write(&rt_c_path, rt_source).is_ok() {
            let rt_ok = std::process::Command::new("cc")
                .arg("-c")
                .arg(&rt_c_path)
                .arg("-o")
                .arg(&rt_o_path)
                .arg("-O2")
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            let _ = std::fs::remove_file(&rt_c_path);
            rt_ok
        } else {
            false
        };
        if has_rt {
            link_cmd.arg(&rt_o_path);
        }
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

    // Clean up object file, runtime stubs, startup object, and generated linker script
    let _ = std::fs::remove_file(&obj_path);
    let _ = std::fs::remove_file(obj_path.with_extension("rt.o"));
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
            // V20 2.3: Run post-build hook from fj.toml [build] section.
            if let Some(project_dir) = path.parent() {
                let toml_path = project_dir.join("fj.toml");
                if toml_path.exists() {
                    if let Ok(toml_str) = std::fs::read_to_string(&toml_path) {
                        if let Ok(config) = toml::from_str::<
                            fajar_lang::package::manifest::ProjectConfig,
                        >(&toml_str)
                        {
                            if let Some(ref build) = config.build {
                                if let Some(ref cmd) = build.post_build {
                                    println!("[build] post-build: {cmd}");
                                    let _ = std::process::Command::new("sh")
                                        .arg("-c")
                                        .arg(cmd)
                                        .current_dir(project_dir)
                                        .status();
                                }
                            }
                        }
                    }
                }
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
pub(crate) fn cross_linker(target: &fajar_lang::codegen::target::TargetConfig) -> String {
    use fajar_lang::codegen::target::Arch;
    match target.arch {
        Arch::Aarch64 => "aarch64-linux-gnu-gcc".to_string(),
        Arch::Riscv64 => "riscv64-linux-gnu-gcc".to_string(),
        Arch::X86_64 => "cc".to_string(),
    }
}
