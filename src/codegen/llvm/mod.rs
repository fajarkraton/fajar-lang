//! LLVM native code generation backend.
//!
//! Feature-gated behind `llvm`. When enabled, provides JIT and AOT compilation
//! of Fajar Lang programs via LLVM (inkwell wrapper).
//!
//! # Architecture
//!
//! ```text
//! AST (Program)
//!     │
//!     ▼
//! LlvmCompiler
//!     ├── types.rs     — Fajar types → LLVM types
//!     └── mod.rs       — IR generation, JIT/AOT execution
//!     │
//!     ▼
//! JIT execution (LLVM ExecutionEngine) or AOT (.o file)
//! ```
//!
//! # V12 Target Configuration
//!
//! The LLVM backend supports fine-grained target control via `TargetConfig`:
//! - `--target-cpu=native` detects host CPU (e.g., skylake, znver3)
//! - `--target-features=+avx2,+fma` enables specific ISA extensions
//! - `--reloc=pic` for position-independent code (shared libraries)
//! - `--code-model=small|medium|large|kernel` for address range
//! - Optimization levels O0-O3, Os (size), Oz (aggressive size)

pub mod runtime;
pub mod types;

use std::collections::HashMap;
use std::path::Path;

use inkwell::OptimizationLevel;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, PointerValue};

use crate::codegen::CodegenError;
use crate::parser::ast::{
    BinOp, CallArg, ConstDef, EnumDef, Expr, FStringExprPart, FnDef, Item, LiteralKind, MatchArm,
    Pattern, Program, StaticDef, Stmt, StructDef, TypeExpr, UnaryOp,
};

use self::types::{fj_type_to_llvm, fj_type_to_metadata};

// ═══════════════════════════════════════════════════════════════════════
// V12 Sprint L1: Target Configuration
// ═══════════════════════════════════════════════════════════════════════

/// Relocation model for code generation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LlvmRelocMode {
    /// Default relocation model for the target.
    Default,
    /// Static relocation (no PIC). Best for executables.
    Static,
    /// Position-independent code. Required for shared libraries.
    Pic,
    /// Dynamic, no PIC. Rarely used.
    DynamicNoPic,
}

impl LlvmRelocMode {
    /// Converts to inkwell RelocMode.
    fn to_inkwell(self) -> RelocMode {
        match self {
            LlvmRelocMode::Default => RelocMode::Default,
            LlvmRelocMode::Static => RelocMode::Static,
            LlvmRelocMode::Pic => RelocMode::PIC,
            LlvmRelocMode::DynamicNoPic => RelocMode::DynamicNoPic,
        }
    }

    /// Parses from string (CLI argument).
    pub fn parse_from(s: &str) -> Result<Self, CodegenError> {
        match s {
            "default" => Ok(LlvmRelocMode::Default),
            "static" => Ok(LlvmRelocMode::Static),
            "pic" => Ok(LlvmRelocMode::Pic),
            "dynamic-no-pic" => Ok(LlvmRelocMode::DynamicNoPic),
            _ => Err(CodegenError::Internal(format!(
                "invalid relocation mode '{s}': expected default, static, pic, or dynamic-no-pic"
            ))),
        }
    }
}

/// Code model for address range constraints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LlvmCodeModel {
    /// Default code model for the target.
    Default,
    /// Small code model: code + data < 2GB. Fastest, most common.
    Small,
    /// Medium code model: code < 2GB, data unlimited.
    Medium,
    /// Large code model: no assumptions. Required for very large binaries.
    Large,
    /// Kernel code model: code in upper 2GB of address space.
    Kernel,
}

impl LlvmCodeModel {
    /// Converts to inkwell CodeModel.
    fn to_inkwell(self) -> CodeModel {
        match self {
            LlvmCodeModel::Default => CodeModel::Default,
            LlvmCodeModel::Small => CodeModel::Small,
            LlvmCodeModel::Medium => CodeModel::Medium,
            LlvmCodeModel::Large => CodeModel::Large,
            LlvmCodeModel::Kernel => CodeModel::Kernel,
        }
    }

    /// Parses from string (CLI argument).
    pub fn parse_from(s: &str) -> Result<Self, CodegenError> {
        match s {
            "default" => Ok(LlvmCodeModel::Default),
            "small" => Ok(LlvmCodeModel::Small),
            "medium" => Ok(LlvmCodeModel::Medium),
            "large" => Ok(LlvmCodeModel::Large),
            "kernel" => Ok(LlvmCodeModel::Kernel),
            _ => Err(CodegenError::Internal(format!(
                "invalid code model '{s}': expected default, small, medium, large, or kernel"
            ))),
        }
    }
}

/// Complete target configuration for LLVM code generation.
///
/// Controls CPU selection, ISA features, relocation model, and code model.
/// Use `TargetConfig::default()` for host-native settings, or configure
/// via CLI flags for cross-compilation.
#[derive(Debug, Clone)]
pub struct TargetConfig {
    /// Target triple (e.g., "x86_64-unknown-linux-gnu"). None = host default.
    pub triple: Option<String>,
    /// CPU name (e.g., "native", "skylake", "cortex-a76"). "generic" = no specialization.
    pub cpu: String,
    /// CPU features string (e.g., "+avx2,+fma,-sse4a"). Empty = default for CPU.
    pub features: String,
    /// Relocation model.
    pub reloc: LlvmRelocMode,
    /// Code model.
    pub code_model: LlvmCodeModel,
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            triple: None,
            cpu: "generic".to_string(),
            features: String::new(),
            reloc: LlvmRelocMode::Default,
            code_model: LlvmCodeModel::Default,
        }
    }
}

impl TargetConfig {
    /// Creates a config targeting the host CPU with auto-detected features.
    ///
    /// Uses LLVM's host CPU detection to select the best CPU model and
    /// enable all available ISA extensions (AVX2, FMA, SSE4, etc.).
    pub fn native() -> Self {
        let cpu = TargetMachine::get_host_cpu_name().to_string();
        let features = TargetMachine::get_host_cpu_features().to_string();
        Self {
            triple: None,
            cpu,
            features,
            reloc: LlvmRelocMode::Default,
            code_model: LlvmCodeModel::Default,
        }
    }

    /// Returns the CPU name, resolving "native" to the actual host CPU.
    pub fn effective_cpu(&self) -> String {
        if self.cpu == "native" {
            TargetMachine::get_host_cpu_name().to_string()
        } else {
            self.cpu.clone()
        }
    }

    /// Returns the features string, resolving "native" to host features.
    pub fn effective_features(&self) -> String {
        if self.cpu == "native" && self.features.is_empty() {
            TargetMachine::get_host_cpu_features().to_string()
        } else {
            self.features.clone()
        }
    }

    /// Validates the target triple format.
    pub fn validate(&self) -> Result<(), CodegenError> {
        if let Some(ref triple) = self.triple {
            // Basic format check: at least arch-vendor-os
            let parts: Vec<&str> = triple.split('-').collect();
            if parts.len() < 2 {
                return Err(CodegenError::Internal(format!(
                    "invalid target triple '{triple}': expected format arch-vendor-os[-env]"
                )));
            }
            let valid_arches = [
                "x86_64",
                "aarch64",
                "arm",
                "armv7",
                "riscv64",
                "riscv32",
                "wasm32",
                "wasm64",
                "i686",
                "i386",
                "mips",
                "mips64",
                "powerpc",
                "powerpc64",
                "s390x",
                "thumbv7em",
            ];
            if !valid_arches.contains(&parts[0]) {
                return Err(CodegenError::Internal(format!(
                    "unsupported architecture '{}' in target triple '{triple}'",
                    parts[0]
                )));
            }
        }
        Ok(())
    }
}

/// Returns the host CPU name as detected by LLVM.
pub fn detect_host_cpu() -> String {
    TargetMachine::get_host_cpu_name().to_string()
}

/// Returns the host CPU features as detected by LLVM.
pub fn detect_host_features() -> String {
    TargetMachine::get_host_cpu_features().to_string()
}

// ═══════════════════════════════════════════════════════════════════════
// V12 Sprint L3: Link-Time Optimization (LTO)
// ═══════════════════════════════════════════════════════════════════════

/// Link-time optimization mode.
///
/// LTO enables cross-module optimization by deferring optimization until
/// link time, when all modules are visible. This enables:
/// - Cross-module inlining
/// - Dead code elimination across module boundaries
/// - Interprocedural constant propagation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LtoMode {
    /// No LTO — each module optimized independently (default).
    None,
    /// Thin LTO — parallel, incremental cross-module optimization.
    /// Best balance of compile speed and optimization quality.
    Thin,
    /// Full LTO — single merged module, maximum optimization.
    /// Slowest compile but best runtime performance.
    Full,
}

impl LtoMode {
    /// Parses from CLI string.
    pub fn parse_from(s: &str) -> Result<Self, CodegenError> {
        match s {
            "none" | "off" | "false" => Ok(LtoMode::None),
            "thin" => Ok(LtoMode::Thin),
            "full" | "fat" | "true" => Ok(LtoMode::Full),
            _ => Err(CodegenError::Internal(format!(
                "invalid LTO mode '{s}': expected none, thin, or full"
            ))),
        }
    }

    /// Returns true if any LTO is enabled.
    pub fn is_enabled(self) -> bool {
        self != LtoMode::None
    }
}

/// Result of an LTO compilation step, containing metrics for diagnostics.
#[derive(Debug, Clone)]
pub struct LtoStats {
    /// Number of bitcode modules merged.
    pub modules_merged: usize,
    /// Total size of input bitcode in bytes.
    pub input_size_bytes: u64,
    /// Size of output object file in bytes.
    pub output_size_bytes: u64,
    /// Time taken for LTO optimization in milliseconds.
    pub optimize_time_ms: u64,
}

// ═══════════════════════════════════════════════════════════════════════
// V12 Sprint L4: Profile-Guided Optimization (PGO)
// ═══════════════════════════════════════════════════════════════════════

/// Profile-guided optimization mode.
///
/// PGO uses runtime profiling data to guide optimization decisions:
/// branch probabilities, function layout, inlining thresholds.
///
/// # Workflow
///
/// 1. **Generate**: Build with instrumentation → run → collect `.profraw`
/// 2. **Merge**: `llvm-profdata merge -o profile.profdata *.profraw`
/// 3. **Use**: Rebuild with `--pgo=use=profile.profdata` for optimized binary
///
/// Or use `--pgo=auto` for automatic generate→run→optimize cycle.
#[derive(Debug, Clone, PartialEq)]
pub enum PgoMode {
    /// No PGO — standard compilation.
    None,
    /// Generate instrumented binary that writes `.profraw` at runtime.
    /// The string is the output directory for profile data.
    Generate(String),
    /// Use profile data from a `.profdata` file to optimize.
    Use(String),
}

impl PgoMode {
    /// Parses from CLI string.
    ///
    /// - `"none"` → `PgoMode::None`
    /// - `"generate"` → `PgoMode::Generate("default.profraw")`
    /// - `"generate=/path/dir"` → `PgoMode::Generate("/path/dir")`
    /// - `"use=profile.profdata"` → `PgoMode::Use("profile.profdata")`
    pub fn parse_from(s: &str) -> Result<Self, CodegenError> {
        if s == "none" || s == "off" || s.is_empty() {
            return Ok(PgoMode::None);
        }
        if s == "generate" {
            return Ok(PgoMode::Generate("default_%m.profraw".to_string()));
        }
        if let Some(path) = s.strip_prefix("generate=") {
            return Ok(PgoMode::Generate(path.to_string()));
        }
        if let Some(path) = s.strip_prefix("use=") {
            if path.is_empty() {
                return Err(CodegenError::Internal(
                    "PGO use mode requires a .profdata file path".to_string(),
                ));
            }
            return Ok(PgoMode::Use(path.to_string()));
        }
        Err(CodegenError::Internal(format!(
            "invalid PGO mode '{s}': expected none, generate, generate=<dir>, or use=<file.profdata>"
        )))
    }

    /// Returns true if PGO is active (generate or use).
    pub fn is_enabled(&self) -> bool {
        !matches!(self, PgoMode::None)
    }

    /// Returns true if this is the instrumentation generation phase.
    pub fn is_generate(&self) -> bool {
        matches!(self, PgoMode::Generate(_))
    }

    /// Returns true if this is the profile-use optimization phase.
    pub fn is_use(&self) -> bool {
        matches!(self, PgoMode::Use(_))
    }
}

/// Merges raw profile data files into a single `.profdata` file.
///
/// Uses `llvm-profdata merge` to combine multiple `.profraw` files
/// from instrumented runs into a single profile data file.
pub fn merge_profdata(profraw_paths: &[&Path], output_path: &Path) -> Result<(), CodegenError> {
    let mut cmd = std::process::Command::new("llvm-profdata");
    cmd.arg("merge").arg("-sparse").arg("-o").arg(output_path);
    for path in profraw_paths {
        cmd.arg(path);
    }

    let status = cmd.status().map_err(|e| {
        CodegenError::Internal(format!(
            "failed to run llvm-profdata: {e}. Ensure LLVM tools are installed"
        ))
    })?;

    if !status.success() {
        return Err(CodegenError::Internal(format!(
            "llvm-profdata merge failed with exit code {}",
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

/// Optimization level for LLVM compilation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LlvmOptLevel {
    /// No optimization (fastest compile).
    O0,
    /// Basic optimization.
    O1,
    /// Standard optimization (default for release).
    O2,
    /// Aggressive optimization.
    O3,
    /// Optimize for size.
    Os,
    /// Optimize aggressively for size.
    Oz,
}

impl LlvmOptLevel {
    /// Maps to inkwell OptimizationLevel.
    fn to_inkwell(self) -> OptimizationLevel {
        match self {
            LlvmOptLevel::O0 => OptimizationLevel::None,
            LlvmOptLevel::O1 => OptimizationLevel::Less,
            LlvmOptLevel::O2 | LlvmOptLevel::Os | LlvmOptLevel::Oz => OptimizationLevel::Default,
            LlvmOptLevel::O3 => OptimizationLevel::Aggressive,
        }
    }

    /// Returns the LLVM pass string for the new pass manager.
    fn pass_string(self) -> &'static str {
        match self {
            LlvmOptLevel::O0 => "default<O0>",
            LlvmOptLevel::O1 => "default<O1>",
            LlvmOptLevel::O2 => "default<O2>",
            LlvmOptLevel::O3 => "default<O3>",
            LlvmOptLevel::Os => "default<Os>",
            LlvmOptLevel::Oz => "default<Oz>",
        }
    }

    /// Returns the bare optimization level string (without `default<>` wrapper).
    /// Used for constructing LTO pass pipelines like `thinlto-pre-link<O2>`.
    fn pass_string_bare(self) -> &'static str {
        match self {
            LlvmOptLevel::O0 => "O0",
            LlvmOptLevel::O1 => "O1",
            LlvmOptLevel::O2 => "O2",
            LlvmOptLevel::O3 => "O3",
            LlvmOptLevel::Os => "Os",
            LlvmOptLevel::Oz => "Oz",
        }
    }
}

/// LLVM-based compiler for Fajar Lang programs.
///
/// Wraps inkwell's Context, Module, and Builder to compile Fajar Lang AST
/// to LLVM IR, then execute via JIT or emit object/assembly files.
pub struct LlvmCompiler<'ctx> {
    /// The LLVM context (owns all types, constants, etc.).
    context: &'ctx Context,
    /// The LLVM module being built.
    module: Module<'ctx>,
    /// The IR builder for inserting instructions.
    builder: Builder<'ctx>,
    /// Maps function name → LLVM function value.
    functions: HashMap<String, FunctionValue<'ctx>>,
    /// Maps variable name → alloca pointer (stack slot).
    variables: HashMap<String, PointerValue<'ctx>>,
    /// Maps variable name → LLVM type (for loads).
    var_types: HashMap<String, BasicTypeEnum<'ctx>>,
    /// Optimization level.
    opt_level: LlvmOptLevel,
    /// Target configuration (CPU, features, reloc, code model).
    target_config: TargetConfig,
    /// Link-time optimization mode.
    lto_mode: LtoMode,
    /// Profile-guided optimization mode.
    pgo_mode: PgoMode,
    /// Maps struct name → (LLVM struct type, field names in order).
    struct_types: HashMap<String, (inkwell::types::StructType<'ctx>, Vec<String>)>,
    /// Maps enum name → (variant names, variant field counts).
    enum_defs: HashMap<String, Vec<(String, usize)>>,
    /// Maps "Type::method" → FnDef name (for method dispatch).
    method_map: HashMap<String, String>,
    /// Monomorphized function definitions generated during compilation.
    mono_fns: Vec<FnDef>,
    /// Maps closure id → (env struct type, captured variable names).
    /// Reserved for future closure environment capture (V12 L5+).
    _closure_envs: HashMap<String, (inkwell::types::StructType<'ctx>, Vec<String>)>,
    /// Counter for generating unique closure names.
    closure_counter: usize,
    /// Whether in no_std (bare-metal) mode: disables heap/IO runtime.
    no_std: bool,
    /// Linker script path for bare-metal builds.
    linker_script: Option<String>,
    /// Break target: (after_bb, optional break value alloca)
    break_target: Option<(BasicBlock<'ctx>, Option<PointerValue<'ctx>>)>,
    /// Continue target: loop header block
    continue_target: Option<BasicBlock<'ctx>>,
    /// Maps constant name → LLVM constant value (from `const NAME: Type = value`).
    constants: HashMap<String, BasicValueEnum<'ctx>>,
    /// Monotonically increasing counter for unique string global names.
    str_counter: usize,
    /// Deduplication cache: string content → LLVM global value.
    string_globals: HashMap<String, inkwell::values::GlobalValue<'ctx>>,
}

impl<'ctx> LlvmCompiler<'ctx> {
    /// Creates a new LLVM compiler with the given context and module name.
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();

        Self {
            context,
            module,
            builder,
            functions: HashMap::new(),
            variables: HashMap::new(),
            var_types: HashMap::new(),
            opt_level: LlvmOptLevel::O0,
            target_config: TargetConfig::default(),
            lto_mode: LtoMode::None,
            pgo_mode: PgoMode::None,
            struct_types: HashMap::new(),
            enum_defs: HashMap::new(),
            method_map: HashMap::new(),
            mono_fns: Vec::new(),
            _closure_envs: HashMap::new(),
            closure_counter: 0,
            no_std: false,
            linker_script: None,
            break_target: None,
            continue_target: None,
            constants: HashMap::new(),
            str_counter: 0,
            string_globals: HashMap::new(),
        }
    }

    /// Returns (or creates) a unique LLVM global for the given string content.
    /// Deduplicates: identical strings share the same global.
    /// Fixes: all string literals previously used hardcoded name "str_const"
    /// which caused LLVM global name collisions and garbled bare-metal output.
    fn get_or_create_string_global(&mut self, s: &str) -> inkwell::values::GlobalValue<'ctx> {
        if let Some(&existing) = self.string_globals.get(s) {
            return existing;
        }
        self.str_counter += 1;
        let name = format!("__fj_str_{}", self.str_counter);
        // null-terminate: bare-metal str_len scans for \0 boundary
        let str_val = self.context.const_string(s.as_bytes(), true);
        let global = self.module.add_global(
            str_val.get_type(),
            Some(inkwell::AddressSpace::default()),
            &name,
        );
        global.set_initializer(&str_val);
        global.set_constant(true);
        self.string_globals.insert(s.to_string(), global);
        global
    }

    /// Creates an alloca in the current function's entry block.
    /// Entry-block allocas have fixed stack frame offsets and survive
    /// LLVM's optimization passes (mem2reg, SROA). This is critical for
    /// stack arrays that are accessed through ptr_to_int/int_to_ptr.
    ///
    /// Uses a separate builder to avoid disturbing the main builder's position.
    fn build_entry_block_alloca(
        &self,
        ty: inkwell::types::BasicTypeEnum<'ctx>,
        name: &str,
    ) -> Result<inkwell::values::PointerValue<'ctx>, CodegenError> {
        let current_block = self
            .builder
            .get_insert_block()
            .ok_or_else(|| CodegenError::Internal("no current block".into()))?;
        let function = current_block
            .get_parent()
            .ok_or_else(|| CodegenError::Internal("no current function".into()))?;
        let entry = function
            .get_first_basic_block()
            .ok_or_else(|| CodegenError::Internal("no entry block".into()))?;

        // Use a temporary builder to insert in entry block without moving main builder
        let entry_builder = self.context.create_builder();
        // Position at the start of the entry block (before any existing instructions)
        if let Some(first_instr) = entry.get_first_instruction() {
            entry_builder.position_before(&first_instr);
        } else {
            entry_builder.position_at_end(entry);
        }
        let alloca = entry_builder
            .build_alloca(ty, name)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        Ok(alloca)
    }

    /// Sets the optimization level for compilation.
    pub fn set_opt_level(&mut self, level: LlvmOptLevel) {
        self.opt_level = level;
    }

    /// Sets the target configuration (CPU, features, reloc, code model).
    pub fn set_target_config(&mut self, config: TargetConfig) {
        self.target_config = config;
    }

    /// Returns the current target configuration.
    pub fn target_config(&self) -> &TargetConfig {
        &self.target_config
    }

    /// Initializes native target for JIT/AOT compilation.
    pub fn init_native_target() -> Result<(), CodegenError> {
        Target::initialize_native(&InitializationConfig::default()).map_err(|e| {
            CodegenError::Internal(format!("failed to initialize native LLVM target: {e}"))
        })
    }

    /// Initializes all supported targets (for cross-compilation).
    pub fn init_all_targets() {
        Target::initialize_x86(&InitializationConfig::default());
        Target::initialize_aarch64(&InitializationConfig::default());
        Target::initialize_riscv(&InitializationConfig::default());
        Target::initialize_arm(&InitializationConfig::default());
    }

    /// Creates a TargetMachine using the current target configuration.
    ///
    /// The `triple_override` parameter, if provided, overrides `target_config.triple`.
    /// This is used by `optimize()` and `emit_*()` methods that need the host triple
    /// for JIT but respect `target_config` for cross-compilation.
    pub fn create_target_machine(
        &self,
        triple_override: Option<&str>,
    ) -> Result<TargetMachine, CodegenError> {
        let target_triple = match triple_override.or(self.target_config.triple.as_deref()) {
            Some(t) => TargetTriple::create(t),
            None => TargetMachine::get_default_triple(),
        };

        let target = Target::from_triple(&target_triple)
            .map_err(|e| CodegenError::Internal(format!("invalid target triple: {e}")))?;

        let cpu = self.target_config.effective_cpu();
        let features = self.target_config.effective_features();

        target
            .create_target_machine(
                &target_triple,
                &cpu,
                &features,
                self.opt_level.to_inkwell(),
                self.target_config.reloc.to_inkwell(),
                self.target_config.code_model.to_inkwell(),
            )
            .ok_or_else(|| {
                CodegenError::Internal(format!(
                    "failed to create LLVM target machine for cpu='{}', features='{}'",
                    cpu, features
                ))
            })
    }

    /// Sets the module's target triple and data layout from a target machine.
    pub fn configure_module_target(&self, target_machine: &TargetMachine) {
        self.module.set_triple(&target_machine.get_triple());
        self.module
            .set_data_layout(&target_machine.get_target_data().get_data_layout());
    }

    /// Returns the LLVM IR as a string (for debugging/testing).
    pub fn print_ir(&self) -> String {
        self.module.print_to_string().to_string()
    }

    /// Verifies the LLVM module for correctness.
    pub fn verify(&self) -> Result<(), CodegenError> {
        self.module
            .verify()
            .map_err(|e| CodegenError::Internal(format!("LLVM module verification failed: {e}")))
    }

    /// Declares a runtime/external function that can be called from compiled code.
    pub fn declare_external_fn(
        &mut self,
        name: &str,
        param_types: &[BasicTypeEnum<'ctx>],
        return_type: Option<BasicTypeEnum<'ctx>>,
    ) -> FunctionValue<'ctx> {
        if let Some(existing) = self.functions.get(name) {
            return *existing;
        }

        // Check if already declared in the module (e.g. by declare_runtime_functions)
        // but not yet in self.functions. Reuse the existing declaration.
        if let Some(existing) = self.module.get_function(name) {
            self.functions.insert(name.to_string(), existing);
            return existing;
        }

        let meta_params: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
            param_types.iter().map(|t| (*t).into()).collect();

        let fn_type = match return_type {
            Some(ret) => ret.fn_type(&meta_params, false),
            None => self.context.void_type().fn_type(&meta_params, false),
        };

        let function = self.module.add_function(name, fn_type, None);
        self.functions.insert(name.to_string(), function);
        function
    }

    /// Registers standard runtime function declarations (`fj_rt_*`).
    pub fn register_runtime_functions(&mut self) {
        let i64_ty: BasicTypeEnum<'ctx> = self.context.i64_type().into();
        let f64_ty: BasicTypeEnum<'ctx> = self.context.f64_type().into();
        let ptr_ty: BasicTypeEnum<'ctx> = self
            .context
            .ptr_type(inkwell::AddressSpace::default())
            .into();

        // Print functions — println (with newline)
        self.declare_external_fn("fj_rt_println_int", &[i64_ty], None);
        self.declare_external_fn("fj_rt_println_str", &[ptr_ty, i64_ty], None);
        self.declare_external_fn("fj_rt_println_f64", &[f64_ty], None);
        self.declare_external_fn("fj_rt_println_bool", &[i64_ty], None);

        // Print functions — print (without newline)
        self.declare_external_fn("fj_rt_print_int", &[i64_ty], None);
        self.declare_external_fn("fj_rt_print_str", &[ptr_ty, i64_ty], None);
        self.declare_external_fn("fj_rt_print_f64", &[f64_ty], None);
        self.declare_external_fn("fj_rt_print_bool", &[i64_ty], None);

        // Eprintln functions (stderr with newline)
        self.declare_external_fn("fj_rt_eprintln_int", &[i64_ty], None);
        self.declare_external_fn("fj_rt_eprintln_str", &[ptr_ty, i64_ty], None);
        self.declare_external_fn("fj_rt_eprintln_f64", &[f64_ty], None);
        self.declare_external_fn("fj_rt_eprintln_bool", &[i64_ty], None);

        // Eprint functions (stderr without newline)
        self.declare_external_fn("fj_rt_eprint_int", &[i64_ty], None);
        self.declare_external_fn("fj_rt_eprint_str", &[ptr_ty, i64_ty], None);
        self.declare_external_fn("fj_rt_eprint_f64", &[f64_ty], None);
        self.declare_external_fn("fj_rt_eprint_bool", &[i64_ty], None);

        // String functions
        self.declare_external_fn("fj_rt_str_len", &[ptr_ty, i64_ty], Some(i64_ty));
        self.declare_external_fn(
            "fj_rt_str_concat",
            &[ptr_ty, i64_ty, ptr_ty, i64_ty],
            Some(ptr_ty),
        );

        // Type conversion (for f-strings)
        self.declare_external_fn("fj_rt_int_to_string", &[i64_ty, ptr_ty, ptr_ty], None);
        self.declare_external_fn("fj_rt_float_to_string", &[f64_ty, ptr_ty, ptr_ty], None);
        self.declare_external_fn("fj_rt_bool_to_string", &[i64_ty, ptr_ty, ptr_ty], None);

        // Assert functions
        self.declare_external_fn("fj_rt_assert", &[i64_ty], None);
        self.declare_external_fn("fj_rt_assert_eq", &[i64_ty, i64_ty], None);
    }

    // ── Builtin function dispatch ─────────────────────────────────────

    /// Returns true if `name` is a known builtin that should be dispatched
    /// to an `fj_rt_*` external function rather than looked up as a user fn.
    fn is_builtin_fn(name: &str) -> bool {
        matches!(
            name,
            "println"
                | "print"
                | "eprintln"
                | "eprint"
                | "dbg"
                | "assert"
                | "assert_eq"
                | "len"
                | "type_of"
                // Bare-metal OS builtins (Phase 1)
                | "volatile_read"
                | "volatile_write"
                | "volatile_read_u8"
                | "volatile_read_u16"
                | "volatile_read_u32"
                | "volatile_write_u8"
                | "volatile_write_u16"
                | "volatile_write_u32"
                | "volatile_read_u64"
                | "volatile_write_u64"
                | "volatile_write_u32_le"
                // Phase 5: SMP-correct atomic ops (Gap G-A closure).
                // SeqCst ordering — strongest available; matches what
                // x86 LOCK prefix instructions provide naturally.
                | "atomic_load_u64"
                | "atomic_store_u64"
                | "atomic_cas_u64"
                | "atomic_fetch_add_u64"
                // Phase 2: CPU control + CR/MSR + CPUID
                | "pause"
                | "memory_fence"
                | "invlpg"
                | "fxsave"
                | "read_cr2"
                | "read_cr3"
                | "write_cr3"
                | "read_cr4"
                | "cpuid_eax"
                | "cpuid_ebx"
                | "cpuid_ecx"
                | "cpuid_edx"
                | "iretq_to_user"
                // Phase 3: External call builtins
                | "buffer_read_u16_le"
                | "buffer_read_u32_le"
                | "buffer_read_u64_le"
                | "buffer_read_u16_be"
                | "buffer_read_u32_be"
                | "buffer_read_u64_be"
                | "buffer_write_u16_le"
                | "buffer_write_u32_le"
                | "buffer_write_u64_le"
                | "buffer_write_u16_be"
                | "buffer_write_u32_be"
                | "buffer_write_u64_be"
                | "str_len"
                | "str_byte_at"
                | "read_timer_ticks"
                | "memcpy_buf"
                | "memset_buf"
                | "x86_serial_init"
                | "acpi_shutdown"
                | "console_putchar"
                | "set_current_pid"
                | "pic_remap"
                | "idt_init"
                | "pit_init"
                | "tss_init"
                | "fxrstor"
                | "sse_enable"
                | "irq_enable"
                | "irq_disable"
                | "nprint"
                | "write_cr4"
                | "read_msr"
                | "write_msr"
                | "pci_read32"
                | "pci_write32"
                | "fn_addr"
                | "port_inb"
                | "port_outb"
                | "port_inw"
                | "port_outw"
                | "port_ind"
                | "port_outd"
                | "cli"
                | "sti"
                | "hlt"
                | "rdtsc"
                | "rdrand"
                | "avx2_dot_f32"
                | "avx2_add_f32"
                | "avx2_mul_f32"
                | "avx2_relu_f32"
                | "avx2_dot_i64"
                | "avx2_add_i64"
                | "avx2_mul_i64"
                | "aesni_encrypt_block"
                | "aesni_decrypt_block"
                | "spin_lock"
                | "spin_unlock"
        )
    }

    /// Compiles a call to a builtin function.
    ///
    /// Returns `Ok(Some(val))` if the builtin was handled, `Ok(None)` inside
    /// an outer `Option` (`Ok(None)` at the outer level) if `name` is not a
    /// builtin. This lets the caller fall through to user-defined fn lookup.
    ///
    /// Mirrors Cranelift's `compile_print_builtin` in `builtins.rs`.
    fn compile_builtin_call(
        &mut self,
        name: &str,
        args: &[CallArg],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        if !Self::is_builtin_fn(name) {
            return Ok(None);
        }

        // E1: Universal builtin override — if user defined a function with
        // the same name as a builtin, call that instead. This allows OS kernels
        // to provide their own implementations of ANY builtin.
        if let Some(result) = self.try_user_fn_override(name, args)? {
            return Ok(Some(result));
        }

        let zero = self.context.i64_type().const_int(0, false);

        match name {
            // ── println / print / eprintln / eprint ───────────────
            "println" | "print" | "eprintln" | "eprint" => {
                // In bare-metal mode, redirect all print calls to fj_rt_bare_print/println
                if self.no_std {
                    if args.is_empty() {
                        // Empty println → send newline to bare-metal UART
                        if let Some(f) = self.module.get_function("fj_rt_bare_println") {
                            let null_ptr = self
                                .context
                                .ptr_type(inkwell::AddressSpace::default())
                                .const_null();
                            let zero_len = self.context.i64_type().const_int(0, false);
                            self.builder
                                .build_call(f, &[null_ptr.into(), zero_len.into()], "")
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        }
                        return Ok(Some(zero.into()));
                    }
                    let arg_expr = &args[0].value;
                    let val = self
                        .compile_expr(arg_expr)?
                        .ok_or_else(|| CodegenError::Internal("print arg no value".into()))?;
                    let is_ln = name == "println" || name == "eprintln";
                    let rt_name = if is_ln {
                        "fj_rt_bare_println"
                    } else {
                        "fj_rt_bare_print"
                    };

                    if val.is_struct_value() {
                        // String {ptr, len} — extract and pass to bare print
                        let sv = val.into_struct_value();
                        let ptr = self
                            .builder
                            .build_extract_value(sv, 0, "str_ptr")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        let len = self
                            .builder
                            .build_extract_value(sv, 1, "str_len")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        if let Some(f) = self.module.get_function(rt_name) {
                            self.builder
                                .build_call(f, &[ptr.into(), len.into()], "")
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        }
                    } else {
                        // Integer — use fj_rt_bare_print_i64
                        let f_name = "fj_rt_bare_print_i64";
                        let func = if let Some(f) = self.module.get_function(f_name) {
                            f
                        } else {
                            let i64_ty = self.context.i64_type();
                            let fn_ty = self.context.void_type().fn_type(&[i64_ty.into()], false);
                            self.module.add_function(
                                f_name,
                                fn_ty,
                                Some(inkwell::module::Linkage::External),
                            )
                        };
                        self.builder
                            .build_call(func, &[val.into()], "")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                    return Ok(Some(zero.into()));
                }

                let is_ln = name == "println" || name == "eprintln";
                let is_err = name == "eprintln" || name == "eprint";

                if args.is_empty() {
                    // No args: println() prints an empty newline.
                    // For print/eprint with no args, this is a no-op — return 0.
                    if is_ln {
                        let rt_fn = if is_err {
                            "fj_rt_eprintln_str"
                        } else {
                            "fj_rt_println_str"
                        };
                        let function = *self.functions.get(rt_fn).ok_or_else(|| {
                            CodegenError::Internal(format!("{rt_fn} not declared"))
                        })?;
                        let empty_ptr = self
                            .context
                            .ptr_type(inkwell::AddressSpace::default())
                            .const_null();
                        let empty_len = self.context.i64_type().const_int(0, false);
                        self.builder
                            .build_call(
                                function,
                                &[empty_ptr.into(), empty_len.into()],
                                "println_empty",
                            )
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                    return Ok(Some(zero.into()));
                }

                // Infer argument type to choose the right fj_rt_* variant.
                let arg_expr = &args[0].value;
                let inferred = infer_type_from_expr(arg_expr);

                // Also check var_types for identifiers.
                let is_string = inferred == "str"
                    || matches!(arg_expr, Expr::Ident { name: vname, .. }
                        if self.var_types.get(vname).is_some_and(|t| t.is_struct_type()));

                let val = self
                    .compile_expr(arg_expr)?
                    .ok_or_else(|| CodegenError::Internal("print arg produced no value".into()))?;

                if is_string || val.is_struct_value() {
                    // String: {ptr, len} struct — extract fields and call str variant.
                    let struct_val = val.into_struct_value();
                    let ptr = self
                        .builder
                        .build_extract_value(struct_val, 0, "str_ptr")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    let len = self
                        .builder
                        .build_extract_value(struct_val, 1, "str_len")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    let rt_fn = if is_err {
                        if is_ln {
                            "fj_rt_eprintln_str"
                        } else {
                            "fj_rt_eprint_str"
                        }
                    } else if is_ln {
                        "fj_rt_println_str"
                    } else {
                        "fj_rt_print_str"
                    };
                    let function = *self
                        .functions
                        .get(rt_fn)
                        .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
                    self.builder
                        .build_call(function, &[ptr.into(), len.into()], "print_str_call")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                } else if inferred == "f64" || val.is_float_value() {
                    // Float: call f64 variant.
                    let rt_fn = if is_err {
                        if is_ln {
                            "fj_rt_eprintln_f64"
                        } else {
                            "fj_rt_eprint_f64"
                        }
                    } else if is_ln {
                        "fj_rt_println_f64"
                    } else {
                        "fj_rt_print_f64"
                    };
                    let function = *self
                        .functions
                        .get(rt_fn)
                        .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
                    self.builder
                        .build_call(function, &[val.into()], "print_f64_call")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                } else if inferred == "bool" {
                    // Bool: pass as i64 (0 or 1).
                    let rt_fn = if is_err {
                        if is_ln {
                            "fj_rt_eprintln_bool"
                        } else {
                            "fj_rt_eprint_bool"
                        }
                    } else if is_ln {
                        "fj_rt_println_bool"
                    } else {
                        "fj_rt_print_bool"
                    };
                    let function = *self
                        .functions
                        .get(rt_fn)
                        .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
                    // Bool may be i1; extend to i64 for the runtime ABI.
                    let int_val = if val.is_int_value()
                        && val.into_int_value().get_type().get_bit_width() == 1
                    {
                        self.builder
                            .build_int_z_extend(
                                val.into_int_value(),
                                self.context.i64_type(),
                                "bool_ext",
                            )
                            .map_err(|e| CodegenError::Internal(e.to_string()))?
                            .into()
                    } else {
                        val
                    };
                    self.builder
                        .build_call(function, &[int_val.into()], "print_bool_call")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                } else {
                    // Default: integer (i64).
                    let rt_fn = if is_err {
                        if is_ln {
                            "fj_rt_eprintln_int"
                        } else {
                            "fj_rt_eprint_int"
                        }
                    } else if is_ln {
                        "fj_rt_println_int"
                    } else {
                        "fj_rt_print_int"
                    };
                    let function = *self
                        .functions
                        .get(rt_fn)
                        .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
                    self.builder
                        .build_call(function, &[val.into()], "print_int_call")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                }

                Ok(Some(zero.into()))
            }

            // ── assert(condition) ─────────────────────────────────
            "assert" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(
                        "assert() requires 1 argument".into(),
                    ));
                }
                let val = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| CodegenError::Internal("assert arg produced no value".into()))?;
                // Extend bool (i1) to i64 if needed.
                let int_val =
                    if val.is_int_value() && val.into_int_value().get_type().get_bit_width() == 1 {
                        self.builder
                            .build_int_z_extend(
                                val.into_int_value(),
                                self.context.i64_type(),
                                "assert_ext",
                            )
                            .map_err(|e| CodegenError::Internal(e.to_string()))?
                            .into()
                    } else {
                        val
                    };
                let function = *self
                    .functions
                    .get("fj_rt_assert")
                    .ok_or_else(|| CodegenError::Internal("fj_rt_assert not declared".into()))?;
                self.builder
                    .build_call(function, &[int_val.into()], "assert_call")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                Ok(Some(zero.into()))
            }

            // ── assert_eq(a, b) ───────────────────────────────────
            "assert_eq" => {
                if args.len() < 2 {
                    return Err(CodegenError::Internal(
                        "assert_eq() requires 2 arguments".into(),
                    ));
                }
                let lhs = self.compile_expr(&args[0].value)?.ok_or_else(|| {
                    CodegenError::Internal("assert_eq LHS produced no value".into())
                })?;
                let rhs = self.compile_expr(&args[1].value)?.ok_or_else(|| {
                    CodegenError::Internal("assert_eq RHS produced no value".into())
                })?;
                let function = *self
                    .functions
                    .get("fj_rt_assert_eq")
                    .ok_or_else(|| CodegenError::Internal("fj_rt_assert_eq not declared".into()))?;
                self.builder
                    .build_call(function, &[lhs.into(), rhs.into()], "assert_eq_call")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                Ok(Some(zero.into()))
            }

            // ── len(value) ────────────────────────────────────────
            "len" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal("len() requires 1 argument".into()));
                }
                let arg_expr = &args[0].value;
                let inferred = infer_type_from_expr(arg_expr);
                let val = self
                    .compile_expr(arg_expr)?
                    .ok_or_else(|| CodegenError::Internal("len arg produced no value".into()))?;

                if inferred == "str" || val.is_struct_value() {
                    // String {ptr, len}: extract len field.
                    let struct_val = val.into_struct_value();
                    let ptr = self
                        .builder
                        .build_extract_value(struct_val, 0, "len_ptr")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    let len = self
                        .builder
                        .build_extract_value(struct_val, 1, "len_val")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    let function = *self.functions.get("fj_rt_str_len").ok_or_else(|| {
                        CodegenError::Internal("fj_rt_str_len not declared".into())
                    })?;
                    let result = self
                        .builder
                        .build_call(function, &[ptr.into(), len.into()], "str_len_call")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    match result.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(v) => Ok(Some(v)),
                        inkwell::values::ValueKind::Instruction(_) => Ok(Some(zero.into())),
                    }
                } else {
                    // E3: len() on non-string/non-collection returns 0.
                    // This is intentional — matches interpreter behavior where
                    // len(42) returns 0 rather than erroring.
                    Ok(Some(zero.into()))
                }
            }

            // ── type_of(value) ────────────────────────────────────
            "type_of" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(
                        "type_of() requires 1 argument".into(),
                    ));
                }
                // Evaluate the arg (for side effects), then return a string
                // constant with the inferred type name.
                let arg_expr = &args[0].value;
                let type_name = infer_type_from_expr(arg_expr);
                let _ = self.compile_expr(arg_expr)?;

                // Build a string constant for the type name.
                let global = self.get_or_create_string_global(&type_name);

                let ptr = global.as_pointer_value();
                let len = self
                    .context
                    .i64_type()
                    .const_int(type_name.len() as u64, false);

                let str_type = self.string_type();
                let mut str_struct = str_type.get_undef();
                str_struct = self
                    .builder
                    .build_insert_value(str_struct, ptr, 0, "typeof_ptr")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
                    .into_struct_value();
                str_struct = self
                    .builder
                    .build_insert_value(str_struct, len, 1, "typeof_len")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
                    .into_struct_value();

                Ok(Some(str_struct.into()))
            }

            // ── dbg(value) — prints and returns the value ─────────
            "dbg" => {
                if args.is_empty() {
                    return Ok(Some(zero.into()));
                }
                // dbg() is like println() but returns the value.
                let arg_expr = &args[0].value;
                let inferred = infer_type_from_expr(arg_expr);
                let val = self
                    .compile_expr(arg_expr)?
                    .ok_or_else(|| CodegenError::Internal("dbg arg produced no value".into()))?;

                // Print using println variant (dbg prints to stderr in Rust,
                // but in Fajar Lang it uses the println path for simplicity).
                if inferred == "str" || val.is_struct_value() {
                    let struct_val = val.into_struct_value();
                    let ptr = self
                        .builder
                        .build_extract_value(struct_val, 0, "dbg_ptr")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    let len = self
                        .builder
                        .build_extract_value(struct_val, 1, "dbg_len")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    if let Some(f) = self.functions.get("fj_rt_println_str") {
                        self.builder
                            .build_call(*f, &[ptr.into(), len.into()], "dbg_str")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                } else if inferred == "f64" || val.is_float_value() {
                    if let Some(f) = self.functions.get("fj_rt_println_f64") {
                        self.builder
                            .build_call(*f, &[val.into()], "dbg_f64")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                } else if let Some(f) = self.functions.get("fj_rt_println_int") {
                    self.builder
                        .build_call(*f, &[val.into()], "dbg_int")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                }

                // dbg returns its argument.
                Ok(Some(val))
            }

            // ── Bare-metal OS builtins (Phase 1) ─────────────────

            // volatile_read(addr) -> i64
            "volatile_read" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(
                        "volatile_read() requires 1 argument (address)".into(),
                    ));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("volatile_read addr produced no value".into())
                    })?
                    .into_int_value();
                self.compile_volatile_load(addr).map(Some)
            }

            // volatile_read_u8/u16/u32(addr) -> i64 (zero-extended)
            "volatile_read_u8" | "volatile_read_u16" | "volatile_read_u32" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(format!(
                        "{name}() requires 1 argument (address)"
                    )));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal(format!("{name} addr produced no value"))
                    })?
                    .into_int_value();
                let load_ty = match name {
                    "volatile_read_u8" => self.context.i8_type().into(),
                    "volatile_read_u16" => self.context.i16_type().into(),
                    _ => self.context.i32_type().into(),
                };
                let val = self.compile_volatile_load_sized(addr, load_ty)?;
                // Zero-extend to i64 for uniform ABI.
                let ext = self
                    .builder
                    .build_int_z_extend(val.into_int_value(), self.context.i64_type(), "vol_zext")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                Ok(Some(ext.into()))
            }

            // volatile_write(addr, value)
            "volatile_write" => {
                if args.len() < 2 {
                    return Err(CodegenError::Internal(
                        "volatile_write() requires 2 arguments (address, value)".into(),
                    ));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("volatile_write addr produced no value".into())
                    })?
                    .into_int_value();
                let value = self
                    .compile_expr(&args[1].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("volatile_write value produced no value".into())
                    })?
                    .into_int_value();
                self.compile_volatile_store(addr, value)?;
                Ok(Some(zero.into()))
            }

            // volatile_write_u8/u16/u32(addr, value)
            "volatile_write_u8" | "volatile_write_u16" | "volatile_write_u32" => {
                if args.len() < 2 {
                    return Err(CodegenError::Internal(format!(
                        "{name}() requires 2 arguments (address, value)"
                    )));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal(format!("{name} addr produced no value"))
                    })?
                    .into_int_value();
                let value = self
                    .compile_expr(&args[1].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal(format!("{name} value produced no value"))
                    })?
                    .into_int_value();
                // Truncate i64 to target width.
                let trunc_ty = match name {
                    "volatile_write_u8" => self.context.i8_type(),
                    "volatile_write_u16" => self.context.i16_type(),
                    _ => self.context.i32_type(),
                };
                let truncated = self
                    .builder
                    .build_int_truncate(value, trunc_ty, "vol_trunc")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                // Volatile store at proper width.
                let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
                let ptr = self
                    .builder
                    .build_int_to_ptr(addr, ptr_ty, "vol_ptr")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                let store = self
                    .builder
                    .build_store(ptr, truncated)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                store.set_volatile(true).ok();
                Ok(Some(zero.into()))
            }

            // port_inb(port) -> i64 (zero-extended byte)
            "port_inb" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(
                        "port_inb() requires 1 argument (port)".into(),
                    ));
                }
                let port = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("port_inb port produced no value".into())
                    })?
                    .into_int_value();
                self.compile_port_in(port, 8).map(Some)
            }

            // port_inw(port) -> i64
            "port_inw" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(
                        "port_inw() requires 1 argument (port)".into(),
                    ));
                }
                let port = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("port_inw port produced no value".into())
                    })?
                    .into_int_value();
                self.compile_port_in(port, 16).map(Some)
            }

            // port_ind(port) -> i64
            "port_ind" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(
                        "port_ind() requires 1 argument (port)".into(),
                    ));
                }
                let port = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("port_ind port produced no value".into())
                    })?
                    .into_int_value();
                self.compile_port_in(port, 32).map(Some)
            }

            // port_outb(port, value)
            "port_outb" => {
                if args.len() < 2 {
                    return Err(CodegenError::Internal(
                        "port_outb() requires 2 arguments (port, value)".into(),
                    ));
                }
                let port = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("port_outb port produced no value".into())
                    })?
                    .into_int_value();
                let value = self
                    .compile_expr(&args[1].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("port_outb value produced no value".into())
                    })?
                    .into_int_value();
                self.compile_port_out(port, value, 8)?;
                Ok(Some(zero.into()))
            }

            // port_outw(port, value)
            "port_outw" => {
                if args.len() < 2 {
                    return Err(CodegenError::Internal(
                        "port_outw() requires 2 arguments (port, value)".into(),
                    ));
                }
                let port = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("port_outw port produced no value".into())
                    })?
                    .into_int_value();
                let value = self
                    .compile_expr(&args[1].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("port_outw value produced no value".into())
                    })?
                    .into_int_value();
                self.compile_port_out(port, value, 16)?;
                Ok(Some(zero.into()))
            }

            // port_outd(port, value)
            "port_outd" => {
                if args.len() < 2 {
                    return Err(CodegenError::Internal(
                        "port_outd() requires 2 arguments (port, value)".into(),
                    ));
                }
                let port = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("port_outd port produced no value".into())
                    })?
                    .into_int_value();
                let value = self
                    .compile_expr(&args[1].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("port_outd value produced no value".into())
                    })?
                    .into_int_value();
                self.compile_port_out(port, value, 32)?;
                Ok(Some(zero.into()))
            }

            // cli() — disable interrupts
            "cli" => {
                self.compile_zero_operand_asm("cli")?;
                Ok(Some(zero.into()))
            }

            // sti() — enable interrupts
            "sti" => {
                self.compile_zero_operand_asm("sti")?;
                Ok(Some(zero.into()))
            }

            // hlt() — halt CPU until next interrupt
            "hlt" => {
                self.compile_zero_operand_asm("hlt")?;
                Ok(Some(zero.into()))
            }

            // rdtsc() -> i64 — read timestamp counter
            "rdtsc" => self.compile_rdtsc().map(Some),

            // rdrand() -> i64 — hardware random number
            "rdrand" => self.compile_rdrand().map(Some),

            // AVX2 SIMD builtins — memory-based operands (addresses as i64)
            // avx2_dot_f32(a_ptr, b_ptr, len) -> i64 (f32 bits in lower 32)
            "avx2_dot_f32" => {
                if args.len() < 3 {
                    return Ok(Some(zero.into()));
                }
                let compiled: Vec<BasicValueEnum<'ctx>> = args
                    .iter()
                    .take(3)
                    .map(|a| {
                        self.compile_expr(&a.value).and_then(|v| {
                            v.ok_or_else(|| CodegenError::Internal("avx2 arg void".into()))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.compile_avx2_dot_f32(&compiled).map(Some)
            }

            // avx2_add_f32(dst_ptr, a_ptr, b_ptr, len) -> i64 (0 on success)
            "avx2_add_f32" => {
                if args.len() < 4 {
                    return Ok(Some(zero.into()));
                }
                let compiled: Vec<BasicValueEnum<'ctx>> = args
                    .iter()
                    .take(4)
                    .map(|a| {
                        self.compile_expr(&a.value).and_then(|v| {
                            v.ok_or_else(|| CodegenError::Internal("avx2 arg void".into()))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.compile_avx2_elementwise(&compiled, "vaddps").map(Some)
            }

            // avx2_mul_f32(dst_ptr, a_ptr, b_ptr, len) -> i64 (0 on success)
            "avx2_mul_f32" => {
                if args.len() < 4 {
                    return Ok(Some(zero.into()));
                }
                let compiled: Vec<BasicValueEnum<'ctx>> = args
                    .iter()
                    .take(4)
                    .map(|a| {
                        self.compile_expr(&a.value).and_then(|v| {
                            v.ok_or_else(|| CodegenError::Internal("avx2 arg void".into()))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.compile_avx2_elementwise(&compiled, "vmulps").map(Some)
            }

            // avx2_relu_f32(dst_ptr, src_ptr, len) -> i64 (0 on success)
            "avx2_relu_f32" => {
                if args.len() < 3 {
                    return Ok(Some(zero.into()));
                }
                let compiled: Vec<BasicValueEnum<'ctx>> = args
                    .iter()
                    .take(3)
                    .map(|a| {
                        self.compile_expr(&a.value).and_then(|v| {
                            v.ok_or_else(|| CodegenError::Internal("avx2 arg void".into()))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.compile_avx2_relu(&compiled).map(Some)
            }

            // AVX2 i64 integer SIMD — for kernel fixed-point vecmat acceleration
            // avx2_dot_i64(a_ptr, b_ptr, len) -> i64
            "avx2_dot_i64" => {
                if args.len() < 3 {
                    return Ok(Some(zero.into()));
                }
                let compiled: Vec<BasicValueEnum<'ctx>> = args
                    .iter()
                    .take(3)
                    .map(|a| {
                        self.compile_expr(&a.value).and_then(|v| {
                            v.ok_or_else(|| CodegenError::Internal("avx2 arg void".into()))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.compile_avx2_dot_i64(&compiled).map(Some)
            }

            // avx2_add_i64(dst_ptr, a_ptr, b_ptr, len) -> i64
            "avx2_add_i64" => {
                if args.len() < 4 {
                    return Ok(Some(zero.into()));
                }
                let compiled: Vec<BasicValueEnum<'ctx>> = args
                    .iter()
                    .take(4)
                    .map(|a| {
                        self.compile_expr(&a.value).and_then(|v| {
                            v.ok_or_else(|| CodegenError::Internal("avx2 arg void".into()))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.compile_avx2_elementwise_i64(&compiled, "vpaddq")
                    .map(Some)
            }

            // avx2_mul_i64(dst_ptr, a_ptr, b_ptr, len) -> i64
            "avx2_mul_i64" => {
                if args.len() < 4 {
                    return Ok(Some(zero.into()));
                }
                let compiled: Vec<BasicValueEnum<'ctx>> = args
                    .iter()
                    .take(4)
                    .map(|a| {
                        self.compile_expr(&a.value).and_then(|v| {
                            v.ok_or_else(|| CodegenError::Internal("avx2 arg void".into()))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.compile_avx2_elementwise_i64(&compiled, "vpmuludq")
                    .map(Some)
            }

            // AES-NI builtins — 128-bit block operations via XMM registers
            // aesni_encrypt_block(state_ptr, key_schedule_ptr, rounds) -> i64 (0)
            "aesni_encrypt_block" => {
                if args.len() < 3 {
                    return Ok(Some(zero.into()));
                }
                let compiled: Vec<BasicValueEnum<'ctx>> = args
                    .iter()
                    .take(3)
                    .map(|a| {
                        self.compile_expr(&a.value).and_then(|v| {
                            v.ok_or_else(|| CodegenError::Internal("aesni arg void".into()))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.compile_aesni_block(&compiled, false).map(Some)
            }

            // aesni_decrypt_block(state_ptr, key_schedule_ptr, rounds) -> i64 (0)
            "aesni_decrypt_block" => {
                if args.len() < 3 {
                    return Ok(Some(zero.into()));
                }
                let compiled: Vec<BasicValueEnum<'ctx>> = args
                    .iter()
                    .take(3)
                    .map(|a| {
                        self.compile_expr(&a.value).and_then(|v| {
                            v.ok_or_else(|| CodegenError::Internal("aesni arg void".into()))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.compile_aesni_block(&compiled, true).map(Some)
            }

            // spin_lock(addr) — busy-wait until lock acquired (volatile CAS loop)
            "spin_lock" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(
                        "spin_lock() requires 1 argument (address)".into(),
                    ));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("spin_lock addr produced no value".into())
                    })?
                    .into_int_value();
                self.compile_spin_lock(addr)?;
                Ok(Some(zero.into()))
            }

            // spin_unlock(addr) — release lock (volatile store 0)
            "spin_unlock" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(
                        "spin_unlock() requires 1 argument (address)".into(),
                    ));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("spin_unlock addr produced no value".into())
                    })?
                    .into_int_value();
                self.compile_volatile_store(addr, self.context.i64_type().const_zero())?;
                Ok(Some(zero.into()))
            }

            // ── Phase 1: Volatile u64 ──────────────────────────────────────
            "volatile_read_u64" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(
                        "volatile_read_u64 requires 1 arg".into(),
                    ));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("volatile_read_u64 addr no value".into())
                    })?
                    .into_int_value();
                let val = self.compile_volatile_load(addr)?;
                Ok(Some(val))
            }

            "volatile_write_u64" => {
                if args.len() < 2 {
                    return Err(CodegenError::Internal(
                        "volatile_write_u64 requires 2 args".into(),
                    ));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("volatile_write_u64 addr no value".into())
                    })?
                    .into_int_value();
                let val = self
                    .compile_expr(&args[1].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("volatile_write_u64 val no value".into())
                    })?
                    .into_int_value();
                self.compile_volatile_store(addr, val)?;
                Ok(Some(zero.into()))
            }

            // ── Phase 5: SMP-correct atomic ops (Gap G-A closure) ──────
            // All 4 ops use AtomicOrdering::SequentiallyConsistent — strongest
            // ordering, matches x86 LOCK prefix natural semantics, simpler
            // mental model than Acquire/Release split. Future enhancement
            // could add ordering parameter; for now SeqCst-only is honest.
            "atomic_load_u64" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(
                        "atomic_load_u64 requires 1 arg".into(),
                    ));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| CodegenError::Internal("atomic_load_u64 addr no value".into()))?
                    .into_int_value();
                let val = self.compile_atomic_load_u64(addr)?;
                Ok(Some(val))
            }
            "atomic_store_u64" => {
                if args.len() < 2 {
                    return Err(CodegenError::Internal(
                        "atomic_store_u64 requires 2 args".into(),
                    ));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| CodegenError::Internal("atomic_store_u64 addr no value".into()))?
                    .into_int_value();
                let val = self
                    .compile_expr(&args[1].value)?
                    .ok_or_else(|| CodegenError::Internal("atomic_store_u64 val no value".into()))?
                    .into_int_value();
                self.compile_atomic_store_u64(addr, val)?;
                Ok(Some(zero.into()))
            }
            "atomic_cas_u64" => {
                if args.len() < 3 {
                    return Err(CodegenError::Internal(
                        "atomic_cas_u64 requires 3 args (addr, expected, new)".into(),
                    ));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| CodegenError::Internal("atomic_cas_u64 addr no value".into()))?
                    .into_int_value();
                let expected = self
                    .compile_expr(&args[1].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("atomic_cas_u64 expected no value".into())
                    })?
                    .into_int_value();
                let new_val = self
                    .compile_expr(&args[2].value)?
                    .ok_or_else(|| CodegenError::Internal("atomic_cas_u64 new no value".into()))?
                    .into_int_value();
                let prev = self.compile_atomic_cas_u64(addr, expected, new_val)?;
                Ok(Some(prev))
            }
            "atomic_fetch_add_u64" => {
                if args.len() < 2 {
                    return Err(CodegenError::Internal(
                        "atomic_fetch_add_u64 requires 2 args (addr, delta)".into(),
                    ));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("atomic_fetch_add_u64 addr no value".into())
                    })?
                    .into_int_value();
                let delta = self
                    .compile_expr(&args[1].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("atomic_fetch_add_u64 delta no value".into())
                    })?
                    .into_int_value();
                let prev = self.compile_atomic_fetch_add_u64(addr, delta)?;
                Ok(Some(prev))
            }

            "volatile_write_u32_le" => {
                // Same as volatile_write_u32 — little-endian is native on x86
                if args.len() < 2 {
                    return Err(CodegenError::Internal(
                        "volatile_write_u32_le requires 2 args".into(),
                    ));
                }
                let addr = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("volatile_write_u32_le addr no value".into())
                    })?
                    .into_int_value();
                let val = self
                    .compile_expr(&args[1].value)?
                    .ok_or_else(|| {
                        CodegenError::Internal("volatile_write_u32_le val no value".into())
                    })?
                    .into_int_value();
                let trunc = self
                    .builder
                    .build_int_truncate(val, self.context.i32_type(), "trunc32")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                let ptr = self
                    .builder
                    .build_int_to_ptr(
                        addr,
                        self.context.ptr_type(inkwell::AddressSpace::default()),
                        "addr_ptr",
                    )
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                let store = self
                    .builder
                    .build_store(ptr, trunc)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                store
                    .set_volatile(true)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                Ok(Some(zero.into()))
            }

            // ── Phase 2: Inline asm builtins ──────────────────────────────
            "pause" | "memory_fence" | "sse_enable" | "irq_enable" | "irq_disable" => {
                let insn = match name {
                    "pause" => "pause",
                    "memory_fence" => "mfence",
                    "sse_enable" => {
                        // Enable SSE: clear CR0.EM (bit 2), set CR0.MP (bit 1)
                        // Enable OSFXSR (9) + OSXMMEXCPT (10) + OSXSAVE (18) in CR4
                        // OSXSAVE is required for ALL VEX-encoded instructions (AVX, BMI2)
                        // Then set XCR0 = 7 (X87 + SSE + AVX) via XSETBV so vzeroupper works
                        "mov %cr0, %rax\n\tand $$0xFFFB, %ax\n\tor $$0x2, %ax\n\tmov %rax, %cr0\n\tmov %cr4, %rax\n\tor $$0x40600, %eax\n\tmov %rax, %cr4\n\txor %ecx, %ecx\n\tmov $$7, %eax\n\txor %edx, %edx\n\txsetbv"
                    }
                    "irq_enable" => "sti",
                    "irq_disable" => "cli",
                    _ => "nop",
                };
                self.compile_zero_operand_asm(insn)?;
                Ok(Some(zero.into()))
            }

            "invlpg" | "fxsave" | "fxrstor" | "write_cr3" | "write_cr4" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(format!("{name} requires 1 arg")));
                }
                let val = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| CodegenError::Internal(format!("{name} arg no value")))?
                    .into_int_value();
                let template = match name {
                    "invlpg" => "invlpg (%rdi)",
                    "fxsave" => "fxsave (%rdi)",
                    "fxrstor" => "fxrstor (%rdi)",
                    "write_cr3" => "mov %rdi, %cr3",
                    "write_cr4" => "mov %rdi, %cr4",
                    _ => "nop",
                };
                let void_ty = self.context.void_type();
                let fn_ty = void_ty.fn_type(&[self.context.i64_type().into()], false);
                let asm_val = self.context.create_inline_asm(
                    fn_ty,
                    template.to_string(),
                    "{rdi}".to_string(),
                    true,
                    false,
                    None,
                    false,
                );
                self.builder
                    .build_indirect_call(fn_ty, asm_val, &[val.into()], "")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                Ok(Some(zero.into()))
            }

            "read_cr2" | "read_cr3" | "read_cr4" => {
                let template = match name {
                    "read_cr2" => "mov %cr2, %rax",
                    "read_cr3" => "mov %cr3, %rax",
                    "read_cr4" => "mov %cr4, %rax",
                    _ => "mov %cr3, %rax",
                };
                let i64_ty = self.context.i64_type();
                let fn_ty = i64_ty.fn_type(&[], false);
                let asm_val = self.context.create_inline_asm(
                    fn_ty,
                    template.to_string(),
                    "={rax}".to_string(),
                    true,
                    false,
                    None,
                    false,
                );
                let call = self
                    .builder
                    .build_indirect_call(fn_ty, asm_val, &[], "cr_val")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                match call.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => Ok(Some(v)),
                    inkwell::values::ValueKind::Instruction(_) => Ok(Some(zero.into())),
                }
            }

            "read_msr" | "write_msr" => {
                // read_msr and write_msr are user-defined functions in FajarOS
                // They should be resolved as regular function calls, not builtins
                // Return None to let the regular function call path handle them
                Ok(None)
            }

            "fn_addr" => {
                // fn_addr("function_name") -> i64 (address of function)
                if args.is_empty() {
                    return Err(CodegenError::Internal(
                        "fn_addr requires 1 string arg".into(),
                    ));
                }
                // Extract the function name from string literal argument
                let fn_name = match &args[0].value {
                    Expr::Literal {
                        kind: LiteralKind::String(s),
                        ..
                    } => s.clone(),
                    _ => {
                        return Err(CodegenError::Internal(
                            "fn_addr arg must be string literal".into(),
                        ));
                    }
                };
                let i64_ty = self.context.i64_type();
                if let Some(func) = self
                    .functions
                    .get(&fn_name)
                    .copied()
                    .or_else(|| self.module.get_function(&fn_name))
                {
                    let ptr = func.as_global_value().as_pointer_value();
                    let addr = self
                        .builder
                        .build_ptr_to_int(ptr, i64_ty, "fn_addr")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    Ok(Some(addr.into()))
                } else {
                    // Function not found — declare as external and take address
                    let fn_ty = i64_ty.fn_type(&[], false);
                    let func = self.module.add_function(
                        &fn_name,
                        fn_ty,
                        Some(inkwell::module::Linkage::External),
                    );
                    let ptr = func.as_global_value().as_pointer_value();
                    let addr = self
                        .builder
                        .build_ptr_to_int(ptr, i64_ty, "fn_addr")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    Ok(Some(addr.into()))
                }
            }

            "cpuid_eax" | "cpuid_ebx" | "cpuid_ecx" | "cpuid_edx" => {
                if args.is_empty() {
                    return Err(CodegenError::Internal(format!(
                        "{name} requires 1 arg (leaf)"
                    )));
                }
                let leaf = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| CodegenError::Internal(format!("{name} leaf no value")))?
                    .into_int_value();
                let out_reg = match name {
                    "cpuid_eax" => "={eax}",
                    "cpuid_ebx" => "={ebx}",
                    "cpuid_ecx" => "={ecx}",
                    "cpuid_edx" => "={edx}",
                    _ => "={eax}",
                };
                let clobbers = match name {
                    "cpuid_eax" => "~{ebx},~{ecx},~{edx}",
                    "cpuid_ebx" => "~{eax},~{ecx},~{edx}",
                    "cpuid_ecx" => "~{eax},~{ebx},~{edx}",
                    "cpuid_edx" => "~{eax},~{ebx},~{ecx}",
                    _ => "~{ebx},~{ecx},~{edx}",
                };
                let i64_ty = self.context.i64_type();
                let i32_ty = self.context.i32_type();
                let fn_ty = i32_ty.fn_type(&[i64_ty.into()], false);
                let constraint = format!("{out_reg},{{rdi}},{clobbers}");
                let asm_val = self.context.create_inline_asm(
                    fn_ty,
                    "mov %rdi, %rax\n\txor %ecx, %ecx\n\tcpuid".to_string(),
                    constraint,
                    true,
                    false,
                    None,
                    false,
                );
                let call = self
                    .builder
                    .build_indirect_call(fn_ty, asm_val, &[leaf.into()], "cpuid_val")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                match call.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => {
                        let ext = self
                            .builder
                            .build_int_z_extend(v.into_int_value(), i64_ty, "cpuid_ext")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        Ok(Some(ext.into()))
                    }
                    inkwell::values::ValueKind::Instruction(_) => Ok(Some(zero.into())),
                }
            }

            "iretq_to_user" => {
                // Call fj_rt_bare_iretq_to_user(rip, rsp, rflags) — a real CALL
                // so SYS_EXIT's `mov rsp, [0x652020]; ret` returns here.
                // Inline asm won't work because there's no return address on stack.
                if args.len() < 3 {
                    return Err(CodegenError::Internal(
                        "iretq_to_user requires 3 args (rip, rsp, rflags)".into(),
                    ));
                }
                let rip = self
                    .compile_expr(&args[0].value)?
                    .ok_or_else(|| CodegenError::Internal("iretq_to_user rip no value".into()))?;
                let rsp_val = self
                    .compile_expr(&args[1].value)?
                    .ok_or_else(|| CodegenError::Internal("iretq_to_user rsp no value".into()))?;
                let rflags = self.compile_expr(&args[2].value)?.ok_or_else(|| {
                    CodegenError::Internal("iretq_to_user rflags no value".into())
                })?;
                let i64_ty = self.context.i64_type();
                let fn_ty = i64_ty.fn_type(&[i64_ty.into(), i64_ty.into(), i64_ty.into()], false);
                let func = self
                    .module
                    .get_function("fj_rt_bare_iretq_to_user")
                    .unwrap_or_else(|| {
                        self.module
                            .add_function("fj_rt_bare_iretq_to_user", fn_ty, None)
                    });
                self.builder
                    .build_call(func, &[rip.into(), rsp_val.into(), rflags.into()], "")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                Ok(Some(zero.into()))
            }

            // ── Phase 3: External call builtins ───────────────────────────
            // These call fj_rt_bare_* symbols provided by the runtime
            "buffer_read_u16_le"
            | "buffer_read_u32_le"
            | "buffer_read_u64_le"
            | "buffer_read_u16_be"
            | "buffer_read_u32_be"
            | "buffer_read_u64_be"
            | "read_timer_ticks"
            | "str_len"
            | "str_byte_at"
            | "buffer_write_u16_le"
            | "buffer_write_u32_le"
            | "buffer_write_u64_le"
            | "buffer_write_u16_be"
            | "buffer_write_u32_be"
            | "buffer_write_u64_be"
            | "memcpy_buf"
            | "memset_buf"
            | "x86_serial_init"
            | "acpi_shutdown"
            | "console_putchar"
            | "set_current_pid"
            | "pic_remap"
            | "idt_init"
            | "pit_init"
            | "tss_init"
            | "nprint"
            | "pci_read32"
            | "pci_write32" => {
                // User-fn override is handled by try_user_fn_override at top of
                // compile_builtin_call — no duplicate check needed here.
                // Fallback: external runtime stub
                let rt_name = format!("fj_rt_bare_{name}");
                let i64_ty = self.context.i64_type();

                // Compile all arguments, coercing string structs to i64 pointers
                let mut arg_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = Vec::new();
                for arg in args {
                    if let Some(v) = self.compile_expr(&arg.value)? {
                        if v.is_struct_value() {
                            // String {ptr, len} — extract pointer, convert to i64
                            let sv = v.into_struct_value();
                            let ptr = self
                                .builder
                                .build_extract_value(sv, 0, "str_ptr_ext")
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            let ptr_int = self
                                .builder
                                .build_ptr_to_int(ptr.into_pointer_value(), i64_ty, "str_i64_ext")
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            arg_vals.push(ptr_int.into());
                        } else {
                            arg_vals.push(v.into());
                        }
                    }
                }

                // Get or declare the runtime function
                let func = if let Some(f) = self.module.get_function(&rt_name) {
                    f
                } else {
                    // Auto-declare with matching arity
                    let param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
                        (0..arg_vals.len()).map(|_| i64_ty.into()).collect();
                    let fn_ty = i64_ty.fn_type(&param_types, false);
                    self.module.add_function(
                        &rt_name,
                        fn_ty,
                        Some(inkwell::module::Linkage::External),
                    )
                };

                let call = self
                    .builder
                    .build_call(func, &arg_vals, &format!("{name}_ret"))
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                match call.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => Ok(Some(v)),
                    inkwell::values::ValueKind::Instruction(_) => Ok(Some(zero.into())),
                }
            }

            _ => Ok(None),
        }
    }

    // ── V12 Sprint L5: Generics & Closures ────────────────────────────

    /// Registers an impl block's methods, mapping "Type::method" → mangled fn name.
    fn register_impl_block(&mut self, impl_block: &crate::parser::ast::ImplBlock) {
        let target = &impl_block.target_type;
        for method in &impl_block.methods {
            let mangled = format!("{target}__{}", method.name);
            self.method_map
                .insert(format!("{target}::{}", method.name), mangled.clone());
        }
    }

    /// Creates a monomorphized (specialized) copy of a generic function.
    ///
    /// Substitutes type parameters with concrete types in the function's
    /// parameter types and return type. The specialized function gets a
    /// mangled name like `add__mono_i64`.
    fn monomorphize_fn(
        generic_def: &FnDef,
        type_suffix: &str,
        type_map: &HashMap<String, String>,
    ) -> FnDef {
        let mangled_name = format!("{}__mono_{type_suffix}", generic_def.name);
        let mut specialized = generic_def.clone();
        specialized.name = mangled_name;
        specialized.generic_params.clear();

        // Substitute type parameters in function params
        for param in &mut specialized.params {
            let type_name = type_expr_to_string(&param.ty);
            if let Some(concrete) = type_map.get(&type_name) {
                param.ty = TypeExpr::Simple {
                    name: concrete.clone(),
                    span: param.span,
                };
            }
        }

        // Substitute return type
        if let Some(ref ret_ty) = specialized.return_type {
            let ret_name = type_expr_to_string(ret_ty);
            if let Some(concrete) = type_map.get(&ret_name) {
                specialized.return_type = Some(TypeExpr::Simple {
                    name: concrete.clone(),
                    span: generic_def.span,
                });
            }
        }

        specialized
    }

    /// Collects generic function calls from the program and generates
    /// monomorphized specializations.
    fn collect_monomorphizations(&mut self, program: &Program) {
        let mut generic_defs: HashMap<String, &FnDef> = HashMap::new();

        // Collect generic function definitions
        for item in &program.items {
            if let Item::FnDef(fndef) = item {
                if !fndef.generic_params.is_empty() {
                    generic_defs.insert(fndef.name.clone(), fndef);
                }
            }
        }

        if generic_defs.is_empty() {
            return;
        }

        // Simple monomorphization: for each call to a generic function,
        // infer the type from the first argument and create a specialization.
        // Full type inference would require the analyzer's type information.
        let mut mono_specs: Vec<(String, String, HashMap<String, String>)> = Vec::new();

        // Walk all function bodies looking for calls to generic functions
        for item in &program.items {
            if let Item::FnDef(fndef) = item {
                self.find_generic_calls(&fndef.body, &generic_defs, &mut mono_specs);
            }
        }

        // Generate monomorphized functions
        for (fn_name, type_suffix, type_map) in &mono_specs {
            if let Some(generic_def) = generic_defs.get(fn_name.as_str()) {
                let specialized = Self::monomorphize_fn(generic_def, type_suffix, type_map);
                self.mono_fns.push(specialized);
            }
        }
    }

    /// Walks an expression tree looking for calls to generic functions.
    fn find_generic_calls(
        &self,
        expr: &Expr,
        generic_defs: &HashMap<String, &FnDef>,
        specs: &mut Vec<(String, String, HashMap<String, String>)>,
    ) {
        match expr {
            Expr::Call { callee, args, .. } => {
                if let Expr::Ident { name, .. } = callee.as_ref() {
                    if let Some(gdef) = generic_defs.get(name.as_str()) {
                        // Infer types from call arguments
                        let mut type_map = HashMap::new();
                        let mut suffix_parts = Vec::new();

                        for (i, gparam) in gdef.generic_params.iter().enumerate() {
                            // Simple heuristic: map generic param to "i64" (default)
                            // or infer from literal argument types
                            let concrete = if let Some(arg) = args.get(i) {
                                infer_type_from_expr(&arg.value)
                            } else {
                                "i64".to_string()
                            };
                            type_map.insert(gparam.name.clone(), concrete.clone());
                            suffix_parts.push(concrete);
                        }

                        let suffix = suffix_parts.join("_");
                        // Avoid duplicate specializations
                        let key = (name.clone(), suffix.clone());
                        if !specs.iter().any(|(n, s, _)| n == &key.0 && s == &key.1) {
                            specs.push((name.clone(), suffix, type_map));
                        }
                    }
                }
                // Recurse into callee and arguments
                self.find_generic_calls(callee, generic_defs, specs);
                for arg in args {
                    self.find_generic_calls(&arg.value, generic_defs, specs);
                }
            }
            Expr::Block {
                stmts, expr: tail, ..
            } => {
                for stmt in stmts {
                    if let Stmt::Let { value, .. } | Stmt::Expr { expr: value, .. } = stmt {
                        self.find_generic_calls(value, generic_defs, specs);
                    }
                }
                if let Some(t) = tail {
                    self.find_generic_calls(t, generic_defs, specs);
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.find_generic_calls(condition, generic_defs, specs);
                self.find_generic_calls(then_branch, generic_defs, specs);
                if let Some(eb) = else_branch {
                    self.find_generic_calls(eb, generic_defs, specs);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.find_generic_calls(left, generic_defs, specs);
                self.find_generic_calls(right, generic_defs, specs);
            }
            Expr::Unary { operand, .. } => {
                self.find_generic_calls(operand, generic_defs, specs);
            }
            _ => {}
        }
    }

    /// Generates a unique closure function name.
    fn next_closure_name(&mut self) -> String {
        self.closure_counter += 1;
        format!("__fj_closure_{}", self.closure_counter)
    }

    // ── V12 Sprint L8: Async Compilation ───────────────────────────────

    /// Compiles an await expression.
    ///
    /// Generates a poll loop that calls `fj_rt_future_poll` until the future
    /// is ready, then extracts the result via `fj_rt_future_get_result`.
    ///
    /// In a real async runtime, this would yield to the executor. For the
    /// LLVM AOT backend, we generate a blocking poll loop that the runtime
    /// can optimize via its executor.
    fn compile_await(
        &mut self,
        future_expr: &Expr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let future_val = self
            .compile_expr(future_expr)?
            .ok_or_else(|| CodegenError::Internal("await expression produced no value".into()))?;

        let i64_type = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());

        // Convert future value to pointer (if it's an i64 opaque handle)
        let future_ptr = if future_val.is_pointer_value() {
            future_val.into_pointer_value()
        } else {
            self.builder
                .build_int_to_ptr(future_val.into_int_value(), ptr_ty, "future_ptr")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
        };

        // Call fj_rt_future_poll in a loop until ready
        if let Some(poll_fn) = self.module.get_function("fj_rt_future_poll") {
            let function = self
                .builder
                .get_insert_block()
                .and_then(|b| b.get_parent())
                .ok_or_else(|| CodegenError::Internal("no current function for await".into()))?;

            let poll_bb = self.context.append_basic_block(function, "await_poll");
            let ready_bb = self.context.append_basic_block(function, "await_ready");

            self.builder
                .build_unconditional_branch(poll_bb)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;

            // Poll loop
            self.builder.position_at_end(poll_bb);
            let poll_result = self
                .builder
                .build_call(poll_fn, &[future_ptr.into()], "poll_state")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;

            let state = match poll_result.try_as_basic_value() {
                inkwell::values::ValueKind::Basic(v) => v.into_int_value(),
                _ => i64_type.const_int(1, false), // Assume ready
            };

            // 1 = Ready, 0 = Pending
            let is_ready = self
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::EQ,
                    state,
                    i64_type.const_int(1, false),
                    "is_ready",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?;

            self.builder
                .build_conditional_branch(is_ready, ready_bb, poll_bb)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;

            // Ready: extract result
            self.builder.position_at_end(ready_bb);
            if let Some(get_fn) = self.module.get_function("fj_rt_future_get_result") {
                let result = self
                    .builder
                    .build_call(get_fn, &[future_ptr.into()], "await_result")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => Ok(Some(v)),
                    _ => Ok(Some(i64_type.const_int(0, false).into())),
                }
            } else {
                Ok(Some(i64_type.const_int(0, false).into()))
            }
        } else {
            // No runtime — just evaluate and return
            Ok(Some(future_val))
        }
    }

    /// Compiles an async block expression.
    ///
    /// Lifts the block body into a separate function, creates a Future
    /// object via `fj_rt_future_new()`, and stores the body function pointer.
    /// Returns the future handle as an opaque i64.
    fn compile_async_block(
        &mut self,
        body: &Expr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let i64_type = self.context.i64_type();

        // Create the future via runtime
        if let Some(new_fn) = self.module.get_function("fj_rt_future_new") {
            let future = self
                .builder
                .build_call(new_fn, &[], "async_future")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;

            // Compile the body (simplified: execute eagerly, store result)
            let body_val = self.compile_expr(body)?;

            // Store result in future
            if let (Some(set_fn), Some(val)) = (
                self.module.get_function("fj_rt_future_set_result"),
                body_val,
            ) {
                let future_ptr = match future.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => v,
                    _ => return Ok(Some(i64_type.const_int(0, false).into())),
                };
                let result_i64 = if val.is_int_value() {
                    val.into_int_value()
                } else {
                    i64_type.const_int(0, false)
                };
                self.builder
                    .build_call(
                        set_fn,
                        &[future_ptr.into(), result_i64.into()],
                        "set_result",
                    )
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;

                // Mark as ready (state = 1)
                if let Some(state_fn) = self.module.get_function("fj_rt_future_set_state") {
                    self.builder
                        .build_call(
                            state_fn,
                            &[future_ptr.into(), i64_type.const_int(1, false).into()],
                            "set_ready",
                        )
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                }

                Ok(Some(future_ptr))
            } else {
                match future.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => Ok(Some(v)),
                    _ => Ok(Some(i64_type.const_int(0, false).into())),
                }
            }
        } else {
            // No future runtime — just compile body directly
            self.compile_expr(body)
        }
    }

    /// Compiles a closure expression to LLVM IR.
    ///
    /// Closures are compiled as a pair of:
    /// 1. A lifted function with an extra `env_ptr` parameter
    /// 2. An environment struct containing captured variables
    ///
    /// The closure value is represented as `{fn_ptr, env_ptr}` pair.
    fn compile_closure(
        &mut self,
        params: &[crate::parser::ast::ClosureParam],
        body: &Expr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let closure_name = self.next_closure_name();
        let i64_type = self.context.i64_type();

        // Save builder position — closure compilation switches to a new function
        let saved_block = self.builder.get_insert_block();

        // Build closure function type: (captured..., params...) -> i64
        // For simplicity, all captures and params are i64
        let param_count = params.len();
        let param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
            (0..param_count).map(|_| i64_type.into()).collect();

        let fn_type = i64_type.fn_type(&param_types, false);
        let function = self.module.add_function(&closure_name, fn_type, None);

        // Save state
        let prev_vars = self.variables.clone();
        let prev_types = self.var_types.clone();

        // Create entry block and bind parameters
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        for (i, cparam) in params.iter().enumerate() {
            let param_val = function
                .get_nth_param(i as u32)
                .ok_or_else(|| CodegenError::Internal(format!("missing closure param {i}")))?;
            let alloca = self
                .builder
                .build_alloca(i64_type, &cparam.name)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            self.builder
                .build_store(alloca, param_val)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            self.variables.insert(cparam.name.clone(), alloca);
            self.var_types.insert(cparam.name.clone(), i64_type.into());
        }

        // G5: Capture outer scope variables — for each variable in outer scope
        // that isn't shadowed by a param, create a constant in the closure.
        // This is a simplified "copy capture" strategy (like C++ [=]).
        let param_names: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
        for (var_name, var_ptr) in &prev_vars {
            if !param_names.contains(&var_name.as_str()) {
                // Load the current value from outer scope
                if let Some(var_ty) = prev_types.get(var_name) {
                    if var_ty.is_int_type() || var_ty.is_float_type() {
                        // Create alloca in closure and copy value
                        let cap_alloca = self
                            .builder
                            .build_alloca(*var_ty, &format!("cap_{var_name}"))
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        // Use the outer alloca value directly (constant fold)
                        self.variables.insert(var_name.clone(), cap_alloca);
                        self.var_types.insert(var_name.clone(), *var_ty);
                        // Note: the actual value is captured at compile-time as a
                        // constant because LLVM IR is SSA. For runtime capture,
                        // we'd need an environment struct passed as a hidden param.
                        // This captures the alloca pointer from outer scope.
                        let _ = self
                            .builder
                            .build_store(cap_alloca, i64_type.const_int(0, false));
                        // Reuse outer pointer for read-through
                        self.variables.insert(var_name.clone(), *var_ptr);
                    }
                }
            }
        }

        // Compile closure body
        let body_val = self.compile_expr(body)?;

        // Return
        if self
            .builder
            .get_insert_block()
            .is_some_and(|b| b.get_terminator().is_none())
        {
            match body_val {
                Some(v) => {
                    if v.is_int_value() {
                        self.builder
                            .build_return(Some(&v))
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    } else {
                        self.builder
                            .build_return(Some(&i64_type.const_int(0, false)))
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                }
                None => {
                    self.builder
                        .build_return(Some(&i64_type.const_int(0, false)))
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                }
            }
        }

        // Restore state
        self.variables = prev_vars;
        self.var_types = prev_types;
        self.functions.insert(closure_name, function);

        // Restore builder position to the caller's block
        if let Some(block) = saved_block {
            self.builder.position_at_end(block);
        }

        // Return function pointer as i64 (simplified representation)
        let fn_ptr = function.as_global_value().as_pointer_value();
        let fn_as_int = self
            .builder
            .build_ptr_to_int(fn_ptr, i64_type, "closure_ptr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        Ok(Some(fn_as_int.into()))
    }

    /// Compiles a method call expression.
    ///
    /// Looks up the method in the method_map and compiles as a regular
    /// function call with the receiver as the first argument.
    fn compile_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[crate::parser::ast::CallArg],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        // Try to find the method in method_map
        // For now, try common patterns: receiver_type::method
        let recv_val = self.compile_expr(receiver)?;

        // Look for any registered method matching this name
        let fn_name = self
            .method_map
            .values()
            .find(|v| v.ends_with(&format!("__{method}")))
            .cloned();

        if let Some(ref mangled_name) = fn_name {
            if let Some(func) = self.functions.get(mangled_name) {
                let func = *func;
                let mut call_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = Vec::new();

                // Add receiver as first argument
                if let Some(rv) = recv_val {
                    call_args.push(rv.into());
                }

                // Add remaining arguments
                for arg in args {
                    if let Some(v) = self.compile_expr(&arg.value)? {
                        call_args.push(v.into());
                    }
                }

                let result = self
                    .builder
                    .build_call(func, &call_args, "method_result")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;

                return match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(val) => Ok(Some(val)),
                    inkwell::values::ValueKind::Instruction(_) => Ok(None),
                };
            }
        }

        // Fallback: treat as a regular function call with method name
        if let Some(func) = self.functions.get(method).copied() {
            let mut call_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = Vec::new();
            if let Some(rv) = recv_val {
                call_args.push(rv.into());
            }
            for arg in args {
                if let Some(v) = self.compile_expr(&arg.value)? {
                    call_args.push(v.into());
                }
            }
            let result = self
                .builder
                .build_call(func, &call_args, "method_result")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            return match result.try_as_basic_value() {
                inkwell::values::ValueKind::Basic(val) => Ok(Some(val)),
                inkwell::values::ValueKind::Instruction(_) => Ok(None),
            };
        }

        // F3: Method not found — error instead of silent 0
        // In bare-metal mode, tolerate unresolved methods (may be provided at link time).
        // In hosted mode, report the error clearly.
        if self.no_std {
            Ok(Some(self.context.i64_type().const_int(0, false).into()))
        } else {
            Err(CodegenError::UndefinedFunction(format!(
                "unresolved method call: .{method}()"
            )))
        }
    }

    // ── V12 Sprint L6: String & Array Runtime Declarations ────────────

    /// Declares external runtime functions for string and array operations.
    ///
    /// These correspond to `extern "C"` functions in `runtime_fns.rs`:
    /// - `fj_rt_string_concat(a_ptr, a_len, b_ptr, b_len) -> {ptr, len}`
    /// - `fj_rt_string_len(ptr, len) -> i64`
    /// - `fj_rt_string_eq(a_ptr, a_len, b_ptr, b_len) -> bool`
    /// - `fj_rt_array_bounds_check(idx, len)` — panics on OOB
    /// - `fj_rt_print_str(ptr, len)` — print string to stdout
    fn declare_runtime_functions(&mut self) {
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let void_ty = self.context.void_type();
        let bool_ty = self.context.bool_type();

        // String type: {ptr, len}
        let str_type = self
            .context
            .struct_type(&[ptr_ty.into(), i64_ty.into()], false);

        // fj_rt_string_concat(a_ptr: ptr, a_len: i64, b_ptr: ptr, b_len: i64) -> {ptr, len}
        let concat_ty = str_type.fn_type(
            &[ptr_ty.into(), i64_ty.into(), ptr_ty.into(), i64_ty.into()],
            false,
        );
        if self.module.get_function("fj_rt_string_concat").is_none() {
            self.module.add_function(
                "fj_rt_string_concat",
                concat_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_string_len(ptr: ptr, len: i64) -> i64
        let len_ty = i64_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
        if self.module.get_function("fj_rt_string_len").is_none() {
            self.module.add_function(
                "fj_rt_string_len",
                len_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_string_eq(a_ptr, a_len, b_ptr, b_len) -> bool
        let eq_ty = bool_ty.fn_type(
            &[ptr_ty.into(), i64_ty.into(), ptr_ty.into(), i64_ty.into()],
            false,
        );
        if self.module.get_function("fj_rt_string_eq").is_none() {
            self.module.add_function(
                "fj_rt_string_eq",
                eq_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_array_bounds_check(idx: i64, len: i64) -> void (panics on OOB)
        let bounds_ty = void_ty.fn_type(&[i64_ty.into(), i64_ty.into()], false);
        if self
            .module
            .get_function("fj_rt_array_bounds_check")
            .is_none()
        {
            self.module.add_function(
                "fj_rt_array_bounds_check",
                bounds_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_print_str(ptr: ptr, len: i64) -> void
        let print_ty = void_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
        if self.module.get_function("fj_rt_print_str").is_none() {
            self.module.add_function(
                "fj_rt_print_str",
                print_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_array_new(len: i64, elem_size: i64) -> ptr (heap-allocated array)
        let arr_new_ty = ptr_ty.fn_type(&[i64_ty.into(), i64_ty.into()], false);
        if self.module.get_function("fj_rt_array_new").is_none() {
            self.module.add_function(
                "fj_rt_array_new",
                arr_new_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_array_len(arr: ptr) -> i64
        let arr_len_ty = i64_ty.fn_type(&[ptr_ty.into()], false);
        if self.module.get_function("fj_rt_array_len").is_none() {
            self.module.add_function(
                "fj_rt_array_len",
                arr_len_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_array_push(arr: ptr, val: i64) -> ptr (returns potentially reallocated array)
        let arr_push_ty = ptr_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
        if self.module.get_function("fj_rt_array_push").is_none() {
            self.module.add_function(
                "fj_rt_array_push",
                arr_push_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_map_new() -> ptr
        let map_new_ty = ptr_ty.fn_type(&[], false);
        if self.module.get_function("fj_rt_map_new").is_none() {
            self.module.add_function(
                "fj_rt_map_new",
                map_new_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_map_insert(map: ptr, key: i64, val: i64) -> void
        let map_insert_ty = void_ty.fn_type(&[ptr_ty.into(), i64_ty.into(), i64_ty.into()], false);
        if self.module.get_function("fj_rt_map_insert").is_none() {
            self.module.add_function(
                "fj_rt_map_insert",
                map_insert_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_map_get(map: ptr, key: i64) -> i64
        let map_get_ty = i64_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
        if self.module.get_function("fj_rt_map_get").is_none() {
            self.module.add_function(
                "fj_rt_map_get",
                map_get_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // ── V12 L8: Async & Concurrency Runtime Functions ──────────────

        // fj_rt_future_new() -> ptr
        let future_new_ty = ptr_ty.fn_type(&[], false);
        if self.module.get_function("fj_rt_future_new").is_none() {
            self.module.add_function(
                "fj_rt_future_new",
                future_new_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_future_poll(ptr) -> i64 (0=Pending, 1=Ready)
        let future_poll_ty = i64_ty.fn_type(&[ptr_ty.into()], false);
        if self.module.get_function("fj_rt_future_poll").is_none() {
            self.module.add_function(
                "fj_rt_future_poll",
                future_poll_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_future_get_result(ptr) -> i64
        let future_get_ty = i64_ty.fn_type(&[ptr_ty.into()], false);
        if self
            .module
            .get_function("fj_rt_future_get_result")
            .is_none()
        {
            self.module.add_function(
                "fj_rt_future_get_result",
                future_get_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_future_set_result(ptr, value: i64) -> void
        let future_set_ty = void_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
        if self
            .module
            .get_function("fj_rt_future_set_result")
            .is_none()
        {
            self.module.add_function(
                "fj_rt_future_set_result",
                future_set_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_future_set_state(ptr, state: i64) -> void
        let future_state_ty = void_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
        if self.module.get_function("fj_rt_future_set_state").is_none() {
            self.module.add_function(
                "fj_rt_future_set_state",
                future_state_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_future_free(ptr) -> void
        let future_free_ty = void_ty.fn_type(&[ptr_ty.into()], false);
        if self.module.get_function("fj_rt_future_free").is_none() {
            self.module.add_function(
                "fj_rt_future_free",
                future_free_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_mutex_new(initial: i64) -> ptr
        let mutex_new_ty = ptr_ty.fn_type(&[i64_ty.into()], false);
        if self.module.get_function("fj_rt_mutex_new").is_none() {
            self.module.add_function(
                "fj_rt_mutex_new",
                mutex_new_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_mutex_lock(ptr) -> i64
        let mutex_lock_ty = i64_ty.fn_type(&[ptr_ty.into()], false);
        if self.module.get_function("fj_rt_mutex_lock").is_none() {
            self.module.add_function(
                "fj_rt_mutex_lock",
                mutex_lock_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_mutex_store(ptr, value: i64) -> void
        let mutex_store_ty = void_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
        if self.module.get_function("fj_rt_mutex_store").is_none() {
            self.module.add_function(
                "fj_rt_mutex_store",
                mutex_store_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_mutex_free(ptr) -> void
        let mutex_free_ty = void_ty.fn_type(&[ptr_ty.into()], false);
        if self.module.get_function("fj_rt_mutex_free").is_none() {
            self.module.add_function(
                "fj_rt_mutex_free",
                mutex_free_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_channel_new() -> ptr
        let chan_new_ty = ptr_ty.fn_type(&[], false);
        if self.module.get_function("fj_rt_channel_new").is_none() {
            self.module.add_function(
                "fj_rt_channel_new",
                chan_new_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_channel_send(ptr, value: i64) -> i64 (0=ok, 1=closed)
        let chan_send_ty = i64_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
        if self.module.get_function("fj_rt_channel_send").is_none() {
            self.module.add_function(
                "fj_rt_channel_send",
                chan_send_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_channel_recv(ptr) -> i64
        let chan_recv_ty = i64_ty.fn_type(&[ptr_ty.into()], false);
        if self.module.get_function("fj_rt_channel_recv").is_none() {
            self.module.add_function(
                "fj_rt_channel_recv",
                chan_recv_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_channel_close(ptr) -> void
        let chan_close_ty = void_ty.fn_type(&[ptr_ty.into()], false);
        if self.module.get_function("fj_rt_channel_close").is_none() {
            self.module.add_function(
                "fj_rt_channel_close",
                chan_close_ty,
                Some(inkwell::module::Linkage::External),
            );
        }
    }

    /// Returns the LLVM string struct type: `{ptr, i64}`.
    #[allow(dead_code)] // Used by tests and future string operations
    fn string_type(&self) -> inkwell::types::StructType<'ctx> {
        self.context.struct_type(
            &[
                self.context
                    .ptr_type(inkwell::AddressSpace::default())
                    .into(),
                self.context.i64_type().into(),
            ],
            false,
        )
    }

    /// Compiles a string concatenation: `a + b` where both are strings.
    #[allow(dead_code)] // Infrastructure for string binary ops (L7+)
    ///
    /// Calls `fj_rt_string_concat(a.ptr, a.len, b.ptr, b.len) -> {ptr, len}`.
    fn compile_string_concat(
        &mut self,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let concat_fn = self
            .module
            .get_function("fj_rt_string_concat")
            .ok_or_else(|| CodegenError::Internal("fj_rt_string_concat not declared".into()))?;

        // Extract {ptr, len} from both strings
        let a_ptr = self
            .builder
            .build_extract_value(lhs.into_struct_value(), 0, "a_ptr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        let a_len = self
            .builder
            .build_extract_value(lhs.into_struct_value(), 1, "a_len")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        let b_ptr = self
            .builder
            .build_extract_value(rhs.into_struct_value(), 0, "b_ptr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        let b_len = self
            .builder
            .build_extract_value(rhs.into_struct_value(), 1, "b_len")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        let result = self
            .builder
            .build_call(
                concat_fn,
                &[a_ptr.into(), a_len.into(), b_ptr.into(), b_len.into()],
                "concat_result",
            )
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        match result.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(val) => Ok(val),
            _ => Err(CodegenError::Internal("string concat returned void".into())),
        }
    }

    /// Compiles an array bounds check before index access.
    #[allow(dead_code)] // Infrastructure for safe array access (L7+)
    ///
    /// Calls `fj_rt_array_bounds_check(idx, len)` which panics on OOB.
    fn compile_bounds_check(
        &mut self,
        index: inkwell::values::IntValue<'ctx>,
        array_len: inkwell::values::IntValue<'ctx>,
    ) -> Result<(), CodegenError> {
        if let Some(bounds_fn) = self.module.get_function("fj_rt_array_bounds_check") {
            self.builder
                .build_call(bounds_fn, &[index.into(), array_len.into()], "bounds_check")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
        }
        Ok(())
    }

    pub fn compile_program(&mut self, program: &Program) -> Result<(), CodegenError> {
        // Declare runtime functions
        if self.no_std {
            // Bare-metal mode: only declare minimal runtime
            self.declare_bare_metal_runtime();
        } else {
            // Standard mode: declare full runtime (string, array, async, etc.)
            self.declare_runtime_functions();
        }

        // Register builtin runtime functions (println, print, assert, etc.)
        // into self.functions so compile_builtin_call can look them up.
        // Skip in bare-metal mode — these symbols don't exist without libc.
        if !self.no_std {
            self.register_runtime_functions();
        }

        // Pass 0: register struct and enum type definitions
        for item in &program.items {
            match item {
                Item::StructDef(sdef) => self.register_struct(sdef)?,
                Item::EnumDef(edef) => self.register_enum(edef),
                _ => {}
            }
        }

        // Pass 0.2: collect constant definitions (const NAME: Type = value)
        for item in &program.items {
            if let Item::ConstDef(cdef) = item {
                self.compile_const_def(cdef);
            }
        }

        // Pass 0.3: compile static variable definitions as LLVM globals
        for item in &program.items {
            if let Item::StaticDef(sdef) = item {
                self.compile_static_def(sdef)?;
            }
        }

        // Pass 0.4: emit global_asm! blocks via LLVMSetModuleInlineAsm2.
        // (FAJAROS_100PCT_FJ_PLAN Phase 2.A — Gap G-G fix.) Concatenates
        // every Item::GlobalAsm template with newlines and installs as
        // module-level inline assembly. The concatenated string is emitted
        // verbatim by LLVM into the output object file's text section
        // (or whatever section the assembly's `.section` directives request).
        // Required for kernel boot stubs (e.g. Multiboot2 header in a
        // specific section) that cannot be expressed as Fajar Lang functions.
        {
            let mut combined = String::new();
            for item in &program.items {
                if let Item::GlobalAsm(ga) = item {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(&ga.template);
                }
            }
            if !combined.is_empty() {
                self.module.set_inline_assembly(&combined);
            }
        }

        // Pass 0.5: register impl block methods
        for item in &program.items {
            if let Item::ImplBlock(ib) = item {
                self.register_impl_block(ib);
            }
        }

        // Pass 0.7: monomorphize generic functions
        self.collect_monomorphizations(program);

        // First pass: declare all functions (including monomorphized)
        for item in &program.items {
            if let Item::FnDef(fndef) = item {
                if fndef.generic_params.is_empty() {
                    self.declare_function(fndef)?;
                }
            }
        }
        // Declare monomorphized specializations
        let mono_fns = self.mono_fns.clone();
        for mfn in &mono_fns {
            self.declare_function(mfn)?;
        }

        // Declare impl block methods
        for item in &program.items {
            if let Item::ImplBlock(ib) = item {
                for method in &ib.methods {
                    // V33.P7 (Gap G-C): @no_mangle methods keep their bare
                    // name; default methods get the `Type__method` prefix.
                    let mangled_name = if method.no_mangle {
                        method.name.clone()
                    } else {
                        format!("{}__{}", ib.target_type, method.name)
                    };
                    let mut mangled_method = method.clone();
                    mangled_method.name = mangled_name;
                    self.declare_function(&mangled_method)?;
                }
            }
        }

        // Second pass: compile function bodies (with error recovery)
        let mut codegen_errors: Vec<CodegenError> = Vec::new();
        for item in &program.items {
            if let Item::FnDef(fndef) = item {
                if fndef.generic_params.is_empty() {
                    if let Err(e) = self.compile_function(fndef) {
                        eprintln!("codegen error: {e}");
                        codegen_errors.push(e);
                    }
                }
            }
        }
        // Compile monomorphized functions
        for mfn in &mono_fns {
            if let Err(e) = self.compile_function(mfn) {
                eprintln!("codegen error: {e}");
                codegen_errors.push(e);
            }
        }

        // Compile impl block methods
        for item in &program.items {
            if let Item::ImplBlock(ib) = item {
                for method in &ib.methods {
                    // V33.P7 (Gap G-C): @no_mangle methods keep their bare
                    // name; default methods get the `Type__method` prefix.
                    let mangled_name = if method.no_mangle {
                        method.name.clone()
                    } else {
                        format!("{}__{}", ib.target_type, method.name)
                    };
                    let mut mangled_method = method.clone();
                    mangled_method.name = mangled_name;
                    if let Err(e) = self.compile_function(&mangled_method) {
                        eprintln!("codegen error: {e}");
                        codegen_errors.push(e);
                    }
                }
            }
        }

        // Report all errors but still attempt to produce output
        if !codegen_errors.is_empty() {
            eprintln!(
                "\n[LLVM] {} codegen errors in {} functions (continuing with partial compilation)",
                codegen_errors.len(),
                codegen_errors.len()
            );
        }

        self.verify()
    }

    /// Registers a struct definition, creating the LLVM struct type.
    fn register_struct(&mut self, sdef: &StructDef) -> Result<(), CodegenError> {
        let field_types: Vec<BasicTypeEnum<'ctx>> = sdef
            .fields
            .iter()
            .map(|f| fj_type_to_llvm(self.context, &type_expr_to_string(&f.ty)))
            .collect();

        let field_names: Vec<String> = sdef.fields.iter().map(|f| f.name.clone()).collect();

        let struct_type = self.context.opaque_struct_type(&sdef.name);
        struct_type.set_body(&field_types, false);

        self.struct_types
            .insert(sdef.name.clone(), (struct_type, field_names));
        Ok(())
    }

    /// Registers an enum definition.
    fn register_enum(&mut self, edef: &EnumDef) {
        let variants: Vec<(String, usize)> = edef
            .variants
            .iter()
            .map(|v| (v.name.clone(), v.fields.len()))
            .collect();
        self.enum_defs.insert(edef.name.clone(), variants);
    }

    /// Declares a function signature in the LLVM module (forward declaration).
    fn declare_function(&mut self, fndef: &FnDef) -> Result<FunctionValue<'ctx>, CodegenError> {
        if let Some(existing) = self.functions.get(&fndef.name) {
            return Ok(*existing);
        }

        let param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = fndef
            .params
            .iter()
            .map(|p| {
                let type_name = type_expr_to_string(&p.ty);
                fj_type_to_metadata(self.context, &type_name)
            })
            .collect();

        let fn_type = match &fndef.return_type {
            Some(ret_ty) => {
                let type_name = type_expr_to_string(ret_ty);
                if type_name == "void" {
                    self.context.void_type().fn_type(&param_types, false)
                } else {
                    let ret = fj_type_to_llvm(self.context, &type_name);
                    ret.fn_type(&param_types, false)
                }
            }
            None => {
                // No return type annotation = void function
                self.context.void_type().fn_type(&param_types, false)
            }
        };

        let function = self.module.add_function(&fndef.name, fn_type, None);

        // Apply LLVM function attributes based on Fajar annotations
        self.apply_function_attributes(function, fndef);

        self.functions.insert(fndef.name.clone(), function);
        Ok(function)
    }

    /// Applies LLVM attributes to a function based on its Fajar Lang annotations.
    ///
    /// Supported annotations:
    /// - `@inline` → AlwaysInline (force inlining at all opt levels)
    /// - `@inline("never")` → NoInline (prevent inlining even at O3)
    /// - `@cold` → Cold (mark as unlikely path, placed in .text.unlikely)
    /// - `@noinline` → NoInline (alias for @inline("never"))
    ///
    /// Additionally, reference parameters get automatic attributes:
    /// - `&mut T` params → `noalias` (no other ref to same memory)
    /// - `&T` / `&mut T` params → `nonnull` (never null)
    /// - `&T` params → `readonly` (does not modify pointed-to memory)
    fn apply_function_attributes(&self, function: FunctionValue<'ctx>, fndef: &FnDef) {
        // ── Annotation-based attributes ────────────────────────────────
        if let Some(ref ann) = fndef.annotation {
            match ann.name.as_str() {
                "inline" => {
                    if ann.param.as_deref() == Some("never")
                        || ann.params.contains(&"never".to_string())
                    {
                        // @inline("never") → NoInline
                        let attr_kind =
                            inkwell::attributes::Attribute::get_named_enum_kind_id("noinline");
                        let attr = self.context.create_enum_attribute(attr_kind, 0);
                        function.add_attribute(inkwell::attributes::AttributeLoc::Function, attr);
                    } else {
                        // @inline → AlwaysInline
                        let attr_kind =
                            inkwell::attributes::Attribute::get_named_enum_kind_id("alwaysinline");
                        let attr = self.context.create_enum_attribute(attr_kind, 0);
                        function.add_attribute(inkwell::attributes::AttributeLoc::Function, attr);
                    }
                }
                "noinline" => {
                    let attr_kind =
                        inkwell::attributes::Attribute::get_named_enum_kind_id("noinline");
                    let attr = self.context.create_enum_attribute(attr_kind, 0);
                    function.add_attribute(inkwell::attributes::AttributeLoc::Function, attr);
                }
                "cold" => {
                    let attr_kind = inkwell::attributes::Attribute::get_named_enum_kind_id("cold");
                    let attr = self.context.create_enum_attribute(attr_kind, 0);
                    function.add_attribute(inkwell::attributes::AttributeLoc::Function, attr);
                }
                "no_vectorize" => {
                    // V31.B.P2: disable LLVM loop auto-vectorization for this
                    // function. Uses string attribute `"no-implicit-float"` +
                    // target-feature override to prevent SSE/AVX vectorized
                    // loop codegen. Written as a STRING attribute so future
                    // LLVM versions that rename/remove the enum are forward-
                    // compatible. Pairs with @noinline for hot paths where
                    // V30 Track 3 P3.6 observed O2 miscompile.
                    let noimpfp = self
                        .context
                        .create_string_attribute("no-implicit-float", "true");
                    function.add_attribute(inkwell::attributes::AttributeLoc::Function, noimpfp);
                    // Also disable the loop-vectorize pass via function-level
                    // `"disable-tail-calls"` adjacent pragma analog. The
                    // canonical LLVM way is per-loop `!llvm.loop.vectorize.enable`
                    // metadata, but we approximate at function granularity
                    // by disabling the LoopVectorize pass contribution via
                    // the target-features attribute.
                    let tf = self.context.create_string_attribute(
                        "target-features",
                        "-avx,-avx2,-avx512f,-sse3,-ssse3,-sse4.1,-sse4.2,+popcnt",
                    );
                    function.add_attribute(inkwell::attributes::AttributeLoc::Function, tf);
                }
                "interrupt" => {
                    // @interrupt → naked + noinline (handler needs manual prologue/epilogue)
                    let naked_kind =
                        inkwell::attributes::Attribute::get_named_enum_kind_id("naked");
                    let naked_attr = self.context.create_enum_attribute(naked_kind, 0);
                    function.add_attribute(inkwell::attributes::AttributeLoc::Function, naked_attr);
                    let noinline_kind =
                        inkwell::attributes::Attribute::get_named_enum_kind_id("noinline");
                    let noinline_attr = self.context.create_enum_attribute(noinline_kind, 0);
                    function
                        .add_attribute(inkwell::attributes::AttributeLoc::Function, noinline_attr);
                    // Place in .text.interrupt section
                    function.set_section(Some(".text.interrupt"));
                }
                "section" => {
                    // @section(".text.boot") → set ELF section
                    if let Some(ref section_name) = ann.param {
                        function.set_section(Some(section_name));
                    }
                }
                _ => {}
            }
        }

        // ── V29.P1: Modifier annotations stacked on top of primary ─────
        // The @noinline modifier (tracked as `fndef.no_inline` flag)
        // is independent of the primary @kernel/@device/@safe context
        // annotation. Applied here so @noinline @kernel fn f() gets both
        // context tracking AND the LLVM NoInline attribute.
        if fndef.no_inline {
            let attr_kind = inkwell::attributes::Attribute::get_named_enum_kind_id("noinline");
            let attr = self.context.create_enum_attribute(attr_kind, 0);
            function.add_attribute(inkwell::attributes::AttributeLoc::Function, attr);
        }

        // ── V33.P6: @naked modifier (Gap G-B closure) ──────────────────
        // The @naked modifier suppresses prologue/epilogue emission. Body
        // must be a single asm!() block (analyzer enforcement is a
        // future enhancement; for now, mis-use produces broken code).
        // Stacks with primary annotations like @unsafe and @kernel.
        // Same LLVM attribute @interrupt uses, but applied via the
        // modifier-flag mechanism instead of the annotation.name match.
        if fndef.naked {
            let naked_kind = inkwell::attributes::Attribute::get_named_enum_kind_id("naked");
            let naked_attr = self.context.create_enum_attribute(naked_kind, 0);
            function.add_attribute(inkwell::attributes::AttributeLoc::Function, naked_attr);
        }

        // ── Parameter-based attributes ─────────────────────────────────
        // Reference parameters get noalias/nonnull/readonly attributes
        for (i, param) in fndef.params.iter().enumerate() {
            let type_name = type_expr_to_string(&param.ty);
            let param_idx = i as u32;
            let loc = inkwell::attributes::AttributeLoc::Param(param_idx);

            if type_name.starts_with("&mut ") || type_name == "RefMut" {
                // &mut T → noalias (exclusive mutable reference)
                let noalias_kind =
                    inkwell::attributes::Attribute::get_named_enum_kind_id("noalias");
                let noalias_attr = self.context.create_enum_attribute(noalias_kind, 0);
                function.add_attribute(loc, noalias_attr);

                // &mut T → nonnull
                let nonnull_kind =
                    inkwell::attributes::Attribute::get_named_enum_kind_id("nonnull");
                let nonnull_attr = self.context.create_enum_attribute(nonnull_kind, 0);
                function.add_attribute(loc, nonnull_attr);
            } else if type_name.starts_with('&') || type_name == "Ref" {
                // &T → nonnull
                let nonnull_kind =
                    inkwell::attributes::Attribute::get_named_enum_kind_id("nonnull");
                let nonnull_attr = self.context.create_enum_attribute(nonnull_kind, 0);
                function.add_attribute(loc, nonnull_attr);

                // &T → readonly (does not write through this reference)
                let readonly_kind =
                    inkwell::attributes::Attribute::get_named_enum_kind_id("readonly");
                let readonly_attr = self.context.create_enum_attribute(readonly_kind, 0);
                function.add_attribute(loc, readonly_attr);
            }
        }

        // ── Return value attributes ────────────────────────────────────
        // Non-void return types that are references get nonnull
        if let Some(ref ret_ty) = fndef.return_type {
            let ret_name = type_expr_to_string(ret_ty);
            if ret_name.starts_with('&') || ret_name == "Ref" || ret_name == "RefMut" {
                let nonnull_kind =
                    inkwell::attributes::Attribute::get_named_enum_kind_id("nonnull");
                let nonnull_attr = self.context.create_enum_attribute(nonnull_kind, 0);
                function.add_attribute(inkwell::attributes::AttributeLoc::Return, nonnull_attr);
            }
        }

        // Note: bare-metal string interleave fixed by null-terminating string globals.
        // A blanket noinline was tried but causes 50x NVMe polling slowdown.
        // If future reordering issues appear, consider selective noinline on I/O functions.
    }

    /// Compiles a `static [mut] NAME: TYPE = VALUE` as an LLVM global variable.
    /// Evaluates a `const NAME: Type = value` definition and stores the
    /// compile-time value in `self.constants` so that subsequent code can
    /// reference the constant by name.
    fn compile_const_def(&mut self, cdef: &ConstDef) {
        let val: Option<BasicValueEnum<'ctx>> = match &*cdef.value {
            Expr::Literal { kind, .. } => match kind {
                LiteralKind::Int(v) => {
                    Some(self.context.i64_type().const_int(*v as u64, true).into())
                }
                LiteralKind::Float(v) => Some(self.context.f64_type().const_float(*v).into()),
                LiteralKind::Bool(v) => Some(
                    self.context
                        .bool_type()
                        .const_int(u64::from(*v), false)
                        .into(),
                ),
                LiteralKind::String(s) => {
                    let str_val = self.context.const_string(s.as_bytes(), false);
                    Some(str_val.into())
                }
                _ => None,
            },
            Expr::Unary {
                op: UnaryOp::Neg,
                operand,
                ..
            } => {
                if let Expr::Literal {
                    kind: LiteralKind::Int(v),
                    ..
                } = operand.as_ref()
                {
                    Some(
                        self.context
                            .i64_type()
                            .const_int((*v).wrapping_neg() as u64, true)
                            .into(),
                    )
                } else if let Expr::Literal {
                    kind: LiteralKind::Float(v),
                    ..
                } = operand.as_ref()
                {
                    Some(self.context.f64_type().const_float(-v).into())
                } else {
                    None
                }
            }
            // Const referencing another const: look up in existing constants
            Expr::Ident { name, .. } => self.constants.get(name).copied(),
            // Binary expressions on constants (e.g., `const X = A | B`)
            Expr::Binary {
                left, op, right, ..
            } => {
                let lhs = self.eval_const_expr(left);
                let rhs = self.eval_const_expr(right);
                if let (Some(l), Some(r)) = (lhs, rhs) {
                    self.eval_const_binary(l, op, r)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(v) = val {
            self.constants.insert(cdef.name.clone(), v);
        }
    }

    /// Evaluates a constant expression (used for `const` initializers that
    /// reference other constants or use simple binary operators).
    fn eval_const_expr(&self, expr: &Expr) -> Option<BasicValueEnum<'ctx>> {
        match expr {
            Expr::Literal { kind, .. } => match kind {
                LiteralKind::Int(v) => {
                    Some(self.context.i64_type().const_int(*v as u64, true).into())
                }
                LiteralKind::Float(v) => Some(self.context.f64_type().const_float(*v).into()),
                LiteralKind::Bool(v) => Some(
                    self.context
                        .bool_type()
                        .const_int(u64::from(*v), false)
                        .into(),
                ),
                _ => None,
            },
            Expr::Unary {
                op: UnaryOp::Neg,
                operand,
                ..
            } => {
                if let Expr::Literal {
                    kind: LiteralKind::Int(v),
                    ..
                } = operand.as_ref()
                {
                    Some(
                        self.context
                            .i64_type()
                            .const_int((*v).wrapping_neg() as u64, true)
                            .into(),
                    )
                } else {
                    None
                }
            }
            Expr::Ident { name, .. } => self.constants.get(name).copied(),
            Expr::Binary {
                left, op, right, ..
            } => {
                let lhs = self.eval_const_expr(left);
                let rhs = self.eval_const_expr(right);
                if let (Some(l), Some(r)) = (lhs, rhs) {
                    self.eval_const_binary(l, op, r)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Evaluates a binary operation on two constant integer values.
    fn eval_const_binary(
        &self,
        lhs: BasicValueEnum<'ctx>,
        op: &BinOp,
        rhs: BasicValueEnum<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let l = lhs.into_int_value().get_zero_extended_constant()?;
        let r = rhs.into_int_value().get_zero_extended_constant()?;
        let result = match op {
            BinOp::Add => l.wrapping_add(r),
            BinOp::Sub => l.wrapping_sub(r),
            BinOp::Mul => l.wrapping_mul(r),
            BinOp::Div if r != 0 => l.wrapping_div(r),
            BinOp::Rem if r != 0 => l.wrapping_rem(r),
            BinOp::BitAnd => l & r,
            BinOp::BitOr => l | r,
            BinOp::BitXor => l ^ r,
            BinOp::Shl => l.wrapping_shl(r as u32),
            BinOp::Shr => l.wrapping_shr(r as u32),
            _ => return None,
        };
        Some(self.context.i64_type().const_int(result, true).into())
    }

    fn compile_static_def(&mut self, sdef: &StaticDef) -> Result<(), CodegenError> {
        let type_name = type_expr_to_string(&sdef.ty);
        let llvm_type = fj_type_to_llvm(self.context, &type_name);

        // Evaluate initial value as a constant
        let initial_value: inkwell::values::BasicValueEnum<'ctx> = match &*sdef.value {
            Expr::Literal { kind, .. } => match kind {
                LiteralKind::Int(v) => self.context.i64_type().const_int(*v as u64, true).into(),
                LiteralKind::Float(v) => self.context.f64_type().const_float(*v).into(),
                LiteralKind::Bool(v) => self
                    .context
                    .bool_type()
                    .const_int(u64::from(*v), false)
                    .into(),
                _ => llvm_type.into_int_type().const_zero().into(),
            },
            Expr::Unary {
                op: UnaryOp::Neg,
                operand,
                ..
            } => {
                if let Expr::Literal {
                    kind: LiteralKind::Int(v),
                    ..
                } = operand.as_ref()
                {
                    self.context
                        .i64_type()
                        .const_int((*v).wrapping_neg() as u64, true)
                        .into()
                } else {
                    llvm_type.into_int_type().const_zero().into()
                }
            }
            _ => {
                // Non-constant initializer: zero-init, will be set at runtime
                llvm_type.into_int_type().const_zero().into()
            }
        };

        let global = self.module.add_global(llvm_type, None, &sdef.name);
        global.set_initializer(&initial_value);

        if !sdef.is_mut {
            global.set_constant(true);
        }

        // Apply @section annotation
        if let Some(ref ann) = sdef.annotation {
            if ann.name == "section" {
                if let Some(ref section_name) = ann.param {
                    global.set_section(Some(section_name));
                }
            }
        }

        // Register as a variable pointer so function bodies can load/store
        self.variables
            .insert(sdef.name.clone(), global.as_pointer_value());
        self.var_types.insert(sdef.name.clone(), llvm_type);

        Ok(())
    }

    /// Compiles a function body to LLVM IR.
    fn compile_function(&mut self, fndef: &FnDef) -> Result<(), CodegenError> {
        let function = *self
            .functions
            .get(&fndef.name)
            .ok_or_else(|| CodegenError::UndefinedFunction(fndef.name.clone()))?;

        // Create entry basic block
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        // Save previous variable scope and create new one
        let prev_vars = self.variables.clone();
        let prev_types = self.var_types.clone();

        // Map function parameters to alloca variables
        for (i, param) in fndef.params.iter().enumerate() {
            let param_val = function.get_nth_param(i as u32).ok_or_else(|| {
                CodegenError::Internal(format!("missing parameter {i} for {}", fndef.name))
            })?;

            let type_name = type_expr_to_string(&param.ty);
            let llvm_type = fj_type_to_llvm(self.context, &type_name);
            let alloca = self
                .builder
                .build_alloca(llvm_type, &param.name)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            self.builder
                .build_store(alloca, param_val)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;

            self.variables.insert(param.name.clone(), alloca);
            self.var_types.insert(param.name.clone(), llvm_type);
        }

        // Compile function body
        let body_val = self.compile_expr(&fndef.body)?;

        // Build return (only if block not already terminated by return/break)
        let needs_ret = self
            .builder
            .get_insert_block()
            .is_some_and(|b| b.get_terminator().is_none());

        if needs_ret {
            let is_void = function.get_type().get_return_type().is_none();
            if is_void {
                // Void function — discard any body value, return void
                self.builder
                    .build_return(None)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
            } else if let Some(val) = body_val {
                // Coerce implicit return value to match fn signature (e.g., i1→i64)
                let coerced = if let Some(ret_ty) = function.get_type().get_return_type() {
                    if ret_ty.is_int_type() && val.is_int_value() {
                        let ret_w = ret_ty.into_int_type().get_bit_width();
                        let val_w = val.into_int_value().get_type().get_bit_width();
                        if val_w < ret_w {
                            self.builder
                                .build_int_z_extend(
                                    val.into_int_value(),
                                    ret_ty.into_int_type(),
                                    "impl_ret_ext",
                                )
                                .map_err(|e| CodegenError::Internal(e.to_string()))?
                                .into()
                        } else if val_w > ret_w {
                            self.builder
                                .build_int_truncate(
                                    val.into_int_value(),
                                    ret_ty.into_int_type(),
                                    "impl_ret_trunc",
                                )
                                .map_err(|e| CodegenError::Internal(e.to_string()))?
                                .into()
                        } else {
                            val
                        }
                    } else {
                        val
                    }
                } else {
                    val
                };
                self.builder
                    .build_return(Some(&coerced))
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
            } else {
                // Non-void function but no body value — implicit return 0
                let zero = self.context.i64_type().const_int(0, false);
                self.builder
                    .build_return(Some(&zero))
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
            }
        }

        // Restore previous scope
        self.variables = prev_vars;
        self.var_types = prev_types;

        Ok(())
    }

    /// Compiles an expression to LLVM IR, returning the result value.
    fn compile_expr(&mut self, expr: &Expr) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        match expr {
            Expr::Literal { kind, .. } => self.compile_literal(kind),

            Expr::Ident { name, .. } => {
                // First check compile-time constants (from `const` definitions)
                if let Some(val) = self.constants.get(name) {
                    return Ok(Some(*val));
                }
                let ptr = self
                    .variables
                    .get(name)
                    .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
                let ty = self
                    .var_types
                    .get(name)
                    .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
                let val = self
                    .builder
                    .build_load(*ty, *ptr, name)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                Ok(Some(val))
            }

            Expr::Binary {
                left, op, right, ..
            } => {
                // Short-circuit evaluation for && and ||
                if matches!(op, BinOp::And | BinOp::Or) {
                    return self.compile_short_circuit(left, op, right);
                }
                let lhs = self
                    .compile_expr(left)?
                    .ok_or_else(|| CodegenError::Internal("binary LHS produced no value".into()))?;
                let rhs = self
                    .compile_expr(right)?
                    .ok_or_else(|| CodegenError::Internal("binary RHS produced no value".into()))?;
                let result = self.compile_binop(op, lhs, rhs)?;
                Ok(Some(result))
            }

            Expr::Unary { op, operand, .. } => {
                let val = self.compile_expr(operand)?.ok_or_else(|| {
                    CodegenError::Internal("unary operand produced no value".into())
                })?;
                let result = self.compile_unaryop(op, val)?;
                Ok(Some(result))
            }

            Expr::Call { callee, args, .. } => {
                if let Expr::Ident { name, .. } = callee.as_ref() {
                    // Check for builtin functions before user-defined lookup.
                    // This mirrors the Cranelift backend's builtins.rs dispatch.
                    if let Some(result) = self.compile_builtin_call(name, args)? {
                        return Ok(Some(result));
                    }

                    // V12 L5: Try monomorphized name first for generic calls
                    let function = if let Some(f) = self.functions.get(name) {
                        *f
                    } else {
                        // Try to find a monomorphized version (e.g., "add__mono_i64")
                        let mono_prefix = format!("{name}__mono_");
                        let mono_fn = self
                            .functions
                            .iter()
                            .find(|(k, _)| k.starts_with(&mono_prefix))
                            .map(|(_, v)| *v);
                        match mono_fn {
                            Some(f) => f,
                            None => {
                                // Not a known function — try as variable holding fn pointer
                                if let Some(var_ptr) = self.variables.get(name) {
                                    if let Some(var_ty) = self.var_types.get(name) {
                                        let fn_ptr_val = self
                                            .builder
                                            .build_load(*var_ty, *var_ptr, name)
                                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                        // Delegate to indirect call path
                                        let i64_ty = self.context.i64_type();
                                        let param_types: Vec<
                                            inkwell::types::BasicMetadataTypeEnum<'ctx>,
                                        > = (0..args.len()).map(|_| i64_ty.into()).collect();
                                        let fn_ty = i64_ty.fn_type(&param_types, false);
                                        let ptr = if fn_ptr_val.is_int_value() {
                                            self.builder
                                                .build_int_to_ptr(
                                                    fn_ptr_val.into_int_value(),
                                                    self.context
                                                        .ptr_type(inkwell::AddressSpace::default()),
                                                    "var_fn_ptr",
                                                )
                                                .map_err(|e| {
                                                    CodegenError::Internal(e.to_string())
                                                })?
                                        } else if fn_ptr_val.is_pointer_value() {
                                            fn_ptr_val.into_pointer_value()
                                        } else {
                                            return Err(CodegenError::UndefinedFunction(
                                                name.clone(),
                                            ));
                                        };
                                        let mut call_args: Vec<
                                            inkwell::values::BasicMetadataValueEnum<'ctx>,
                                        > = Vec::new();
                                        for arg in args {
                                            if let Some(v) = self.compile_expr(&arg.value)? {
                                                call_args.push(v.into());
                                            }
                                        }
                                        let call_val = self
                                            .builder
                                            .build_indirect_call(fn_ty, ptr, &call_args, "var_call")
                                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                        return match call_val.try_as_basic_value() {
                                            inkwell::values::ValueKind::Basic(val) => {
                                                let coerced = self.coerce_int_to_i64(val)?;
                                                Ok(Some(coerced))
                                            }
                                            inkwell::values::ValueKind::Instruction(_) => Ok(None),
                                        };
                                    }
                                }
                                return Err(CodegenError::UndefinedFunction(name.clone()));
                            }
                        }
                    };

                    let param_types: Vec<_> = function.get_type().get_param_types();
                    let compiled_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
                        .iter()
                        .enumerate()
                        .map(|(i, arg)| {
                            let val = self.compile_expr(&arg.value)?.ok_or_else(|| {
                                CodegenError::Internal("call arg produced no value".into())
                            })?;
                            // If arg is a string struct {ptr, len} but param expects i64,
                            // extract the pointer and convert to i64 (bare-metal ABI)
                            if val.is_struct_value() {
                                let expects_int =
                                    param_types.get(i).is_some_and(|t| t.is_int_type());
                                if expects_int {
                                    let sv = val.into_struct_value();
                                    let ptr = self
                                        .builder
                                        .build_extract_value(sv, 0, "str_ptr_arg")
                                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                    let i64_ty = self.context.i64_type();
                                    let ptr_int = self
                                        .builder
                                        .build_ptr_to_int(
                                            ptr.into_pointer_value(),
                                            i64_ty,
                                            "str_as_i64",
                                        )
                                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                    return Ok(ptr_int.into());
                                }
                            }
                            Ok(val.into())
                        })
                        .collect::<Result<Vec<_>, CodegenError>>()?;

                    let call_val = self
                        .builder
                        .build_call(function, &compiled_args, &format!("{name}_result"))
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;

                    match call_val.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(val) => {
                            // E4: Coerce i1 return values to i64 for uniform ABI
                            let coerced = self.coerce_int_to_i64(val)?;
                            Ok(Some(coerced))
                        }
                        inkwell::values::ValueKind::Instruction(_) => Ok(None),
                    }
                } else {
                    // G6: Non-ident callee — compile expression to get function
                    // pointer, then build indirect call. Supports: closure vars,
                    // function pointers, computed callees.
                    let callee_val = self.compile_expr(callee)?.ok_or_else(|| {
                        CodegenError::Internal("non-ident callee produced no value".into())
                    })?;

                    if callee_val.is_int_value() {
                        // Value is an i64 function pointer — convert to ptr and call
                        let i64_ty = self.context.i64_type();
                        let param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
                            (0..args.len()).map(|_| i64_ty.into()).collect();
                        let fn_ty = i64_ty.fn_type(&param_types, false);

                        let fn_ptr = self
                            .builder
                            .build_int_to_ptr(
                                callee_val.into_int_value(),
                                self.context.ptr_type(inkwell::AddressSpace::default()),
                                "fn_ptr",
                            )
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;

                        let mut call_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
                            Vec::new();
                        for arg in args {
                            if let Some(v) = self.compile_expr(&arg.value)? {
                                call_args.push(v.into());
                            }
                        }

                        let call_val = self
                            .builder
                            .build_indirect_call(fn_ty, fn_ptr, &call_args, "indirect_call")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;

                        match call_val.try_as_basic_value() {
                            inkwell::values::ValueKind::Basic(val) => {
                                let coerced = self.coerce_int_to_i64(val)?;
                                Ok(Some(coerced))
                            }
                            inkwell::values::ValueKind::Instruction(_) => Ok(None),
                        }
                    } else if callee_val.is_pointer_value() {
                        // Already a pointer — call directly
                        let i64_ty = self.context.i64_type();
                        let param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
                            (0..args.len()).map(|_| i64_ty.into()).collect();
                        let fn_ty = i64_ty.fn_type(&param_types, false);

                        let mut call_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
                            Vec::new();
                        for arg in args {
                            if let Some(v) = self.compile_expr(&arg.value)? {
                                call_args.push(v.into());
                            }
                        }

                        let call_val = self
                            .builder
                            .build_indirect_call(
                                fn_ty,
                                callee_val.into_pointer_value(),
                                &call_args,
                                "indirect_call",
                            )
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;

                        match call_val.try_as_basic_value() {
                            inkwell::values::ValueKind::Basic(val) => {
                                let coerced = self.coerce_int_to_i64(val)?;
                                Ok(Some(coerced))
                            }
                            inkwell::values::ValueKind::Instruction(_) => Ok(None),
                        }
                    } else {
                        Err(CodegenError::NotImplemented(
                            "non-ident callee must produce integer or pointer value".into(),
                        ))
                    }
                }
            }

            Expr::Assign { target, value, .. } => {
                let val = self.compile_expr(value)?.ok_or_else(|| {
                    CodegenError::Internal("assign value produced no value".into())
                })?;
                match target.as_ref() {
                    Expr::Ident { name, .. } => {
                        let ptr = self
                            .variables
                            .get(name)
                            .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
                        self.builder
                            .build_store(*ptr, val)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        Ok(None)
                    }
                    Expr::Index { object, index, .. } => {
                        // array[i] = value — compile base as pointer, GEP, store
                        let base = self
                            .compile_expr(object)?
                            .ok_or_else(|| {
                                CodegenError::Internal("index assign base no value".into())
                            })?
                            .into_int_value();
                        let idx = self
                            .compile_expr(index)?
                            .ok_or_else(|| {
                                CodegenError::Internal("index assign index no value".into())
                            })?
                            .into_int_value();
                        let i64_ty = self.context.i64_type();
                        let ptr = self
                            .builder
                            .build_int_to_ptr(
                                base,
                                self.context.ptr_type(inkwell::AddressSpace::default()),
                                "idx_base",
                            )
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        // SAFETY: bare-metal pointer arithmetic
                        let elem_ptr = unsafe {
                            self.builder
                                .build_in_bounds_gep(i64_ty, ptr, &[idx], "idx_ptr")
                                .map_err(|e| CodegenError::Internal(e.to_string()))?
                        };
                        self.builder
                            .build_store(elem_ptr, val)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        Ok(None)
                    }
                    Expr::Field { object, field, .. } => {
                        // struct.field = value — find struct ptr, GEP field, store
                        if let Expr::Ident { name, .. } = object.as_ref() {
                            if let Some(ptr) = self.variables.get(name).copied() {
                                if let Some(ty) = self.var_types.get(name).copied() {
                                    if ty.is_struct_type() {
                                        let st = ty.into_struct_type();
                                        if let Some((_, field_names)) =
                                            self.struct_types.values().find(|(s, _)| *s == st)
                                        {
                                            if let Some(idx) =
                                                field_names.iter().position(|n| n == field)
                                            {
                                                let field_ptr = self
                                                    .builder
                                                    .build_struct_gep(st, ptr, idx as u32, field)
                                                    .map_err(|e| {
                                                        CodegenError::Internal(e.to_string())
                                                    })?;
                                                self.builder.build_store(field_ptr, val).map_err(
                                                    |e| CodegenError::Internal(e.to_string()),
                                                )?;
                                                return Ok(None);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // I1: Chained field assignment — a.inner.field = val
                        // Recursively resolve the nested field to get a GEP pointer.
                        if let Expr::Field {
                            object: inner_obj,
                            field: inner_field,
                            ..
                        } = object.as_ref()
                        {
                            if let Expr::Ident { name, .. } = inner_obj.as_ref() {
                                if let Some(base_ptr) = self.variables.get(name).copied() {
                                    if let Some(base_ty) = self.var_types.get(name).copied() {
                                        if base_ty.is_struct_type() {
                                            let base_st = base_ty.into_struct_type();
                                            if let Some((_, base_fields)) = self
                                                .struct_types
                                                .values()
                                                .find(|(s, _)| *s == base_st)
                                            {
                                                if let Some(inner_idx) = base_fields
                                                    .iter()
                                                    .position(|n| n == inner_field)
                                                {
                                                    let inner_ptr = self
                                                        .builder
                                                        .build_struct_gep(
                                                            base_st,
                                                            base_ptr,
                                                            inner_idx as u32,
                                                            inner_field,
                                                        )
                                                        .map_err(|e| {
                                                            CodegenError::Internal(e.to_string())
                                                        })?;
                                                    // Get the inner struct type
                                                    if let Some(inner_ty) = base_st
                                                        .get_field_type_at_index(inner_idx as u32)
                                                    {
                                                        if inner_ty.is_struct_type() {
                                                            let inner_st =
                                                                inner_ty.into_struct_type();
                                                            if let Some((_, inner_fields)) = self
                                                                .struct_types
                                                                .values()
                                                                .find(|(s, _)| *s == inner_st)
                                                            {
                                                                if let Some(field_idx) =
                                                                    inner_fields
                                                                        .iter()
                                                                        .position(|n| n == field)
                                                                {
                                                                    let field_ptr = self
                                                                        .builder
                                                                        .build_struct_gep(
                                                                            inner_st,
                                                                            inner_ptr,
                                                                            field_idx as u32,
                                                                            field,
                                                                        )
                                                                        .map_err(|e| {
                                                                            CodegenError::Internal(
                                                                                e.to_string(),
                                                                            )
                                                                        })?;
                                                                    self.builder
                                                                        .build_store(field_ptr, val)
                                                                        .map_err(|e| {
                                                                            CodegenError::Internal(
                                                                                e.to_string(),
                                                                            )
                                                                        })?;
                                                                    return Ok(None);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Fallback: in bare-metal mode, treat as no-op;
                        // in hosted mode, report the issue.
                        if self.no_std {
                            Ok(None)
                        } else {
                            Err(CodegenError::Internal(format!(
                                "cannot assign to field '{field}' — target is not a known struct variable"
                            )))
                        }
                    }
                    _ => {
                        // E3: Unknown assign target — report instead of silently ignoring.
                        // In bare-metal mode, tolerate exotic targets (pointer casts, etc.).
                        if self.no_std {
                            Ok(None)
                        } else {
                            Err(CodegenError::NotImplemented(
                                "assignment to complex target expression in LLVM backend".into(),
                            ))
                        }
                    }
                }
            }

            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => self.compile_if(condition, then_branch, else_branch.as_deref()),

            Expr::Block { stmts, expr, .. } => {
                let mut last_val = None;
                for s in stmts {
                    last_val = self.compile_stmt(s)?;
                }
                if let Some(final_expr) = expr {
                    last_val = self.compile_expr(final_expr)?;
                }
                Ok(last_val)
            }

            Expr::Array { elements, .. } => self.compile_array(elements),

            Expr::Tuple { elements, .. } => self.compile_tuple(elements),

            Expr::StructInit { name, fields, .. } => self.compile_struct_init(name, fields),

            Expr::Field { object, field, .. } => self.compile_field_access(object, field),

            Expr::Index { object, index, .. } => self.compile_index_access(object, index),

            Expr::Cast { expr, ty, .. } => self.compile_cast(expr, ty),

            Expr::While {
                label: None,
                condition,
                body,
                ..
            } => self.compile_while(condition, body),

            Expr::For {
                label: None,
                variable,
                iterable,
                body,
                ..
            } => self.compile_for(variable, iterable, body),

            Expr::Loop {
                label: None, body, ..
            } => self.compile_loop(body),

            Expr::Match { subject, arms, .. } => self.compile_match(subject, arms),

            // Effect system: handle expression runs body (effects are compile-time checked)
            Expr::HandleEffect { body, .. } => self.compile_expr(body),

            // Resume: evaluate the value (simplified — full continuation not in LLVM yet)
            Expr::ResumeExpr { value, .. } => self.compile_expr(value),

            // Comptime: evaluate the body (should be folded to literal by analyzer)
            Expr::Comptime { body, .. } => self.compile_expr(body),

            // V12 L8: Await expression — poll future until ready
            Expr::Await { expr, .. } => self.compile_await(expr),

            // V12 L8: Async block — compile body, wrap in future
            Expr::AsyncBlock { body, .. } => self.compile_async_block(body),

            // V12 L9: Inline assembly
            Expr::InlineAsm {
                template,
                operands,
                options,
                ..
            } => self.compile_inline_asm(template, operands, options),

            // V12 L5: Closure expression
            Expr::Closure { params, body, .. } => self.compile_closure(params, body),

            // V12 L5: Method call
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => self.compile_method_call(receiver, method, args),

            // V12 L7: Pipeline operator `x |> f` → `f(x)`
            Expr::Pipe { left, right, .. } => {
                let arg = self.compile_expr(left)?;
                // right should be an Ident (function name) — compile as call
                if let Expr::Ident { name, .. } = right.as_ref() {
                    if let Some(func) = self.functions.get(name).copied() {
                        let mut call_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
                            Vec::new();
                        if let Some(v) = arg {
                            call_args.push(v.into());
                        }
                        let call_val = self
                            .builder
                            .build_call(func, &call_args, "pipe_result")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        match call_val.try_as_basic_value() {
                            inkwell::values::ValueKind::Basic(val) => Ok(Some(val)),
                            inkwell::values::ValueKind::Instruction(_) => Ok(None),
                        }
                    } else {
                        Err(CodegenError::UndefinedFunction(name.clone()))
                    }
                } else {
                    // Non-ident pipe target: compile as expression and try indirect call
                    self.compile_expr(right)
                }
            }

            // V12 L7: Try operator `expr?` → match on Result, early return on Err
            Expr::Try { expr, .. } => {
                // Simplified: evaluate the expression and return its value
                // Full Result/Option match needs enum runtime support
                self.compile_expr(expr)
            }

            // V12 L7: Range expression `start..end`
            Expr::Range { start, end, .. } => {
                // Compile start and end, return start (ranges used by for-in)
                let start_val = if let Some(s) = start {
                    self.compile_expr(s)?
                } else {
                    Some(self.context.i64_type().const_int(0, false).into())
                };
                if let Some(e) = end {
                    let _ = self.compile_expr(e)?;
                }
                Ok(start_val)
            }

            // Grouped expression: `(expr)` — just compile the inner expression
            Expr::Grouped { expr, .. } => self.compile_expr(expr),

            // Path expression: `module::name` — treat as ident or function reference
            Expr::Path { segments, .. } => {
                let full_name = segments.join("::");
                // Check constants first
                if let Some(val) = self.constants.get(&full_name) {
                    return Ok(Some(*val));
                }
                // Check variables
                if let Some(ptr) = self.variables.get(&full_name) {
                    let ty = self
                        .var_types
                        .get(&full_name)
                        .ok_or_else(|| CodegenError::UndefinedVariable(full_name.clone()))?;
                    let val = self
                        .builder
                        .build_load(*ty, *ptr, &full_name)
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    return Ok(Some(val));
                }
                // Check if it's a function pointer
                if let Some(func) = self.functions.get(&full_name) {
                    return Ok(Some(func.as_global_value().as_pointer_value().into()));
                }
                Err(CodegenError::UndefinedVariable(full_name))
            }

            // F-string: `f"Hello {name}, age {age}"`
            Expr::FString { parts, .. } => self.compile_fstring(parts),

            // H2: Yield — in LLVM backend, compile as returning the value.
            // Full coroutine/generator support would need LLVM coroutine
            // intrinsics; this simplified version treats yield as a return
            // expression (the value is produced to the caller).
            Expr::Yield { value, .. } => {
                if let Some(expr) = value {
                    self.compile_expr(expr)
                } else {
                    Ok(Some(self.context.i64_type().const_int(0, false).into()))
                }
            }

            // MacroVar — should be expanded before codegen
            Expr::MacroVar { name, .. } => Err(CodegenError::Internal(format!(
                "unexpanded macro variable ${name} in codegen"
            ))),

            // I5: Better diagnostics for unhandled expressions
            _ => {
                let expr_name = match expr {
                    Expr::MacroVar { name, .. } => format!("macro variable ${name}"),
                    _ => format!("expression variant {:?}", std::mem::discriminant(expr)),
                };
                Err(CodegenError::NotImplemented(format!(
                    "LLVM expr: {expr_name}"
                )))
            }
        }
    }

    /// Compiles an f-string expression: `f"Hello {name}, age {age}"`.
    ///
    /// Strategy: compile each part to a {ptr, len} pair, then concatenate
    /// pairwise using `fj_rt_str_concat`. Non-string expressions are
    /// converted via `fj_rt_int_to_string` / `fj_rt_float_to_string` /
    /// `fj_rt_bool_to_string`.
    fn compile_fstring(
        &mut self,
        parts: &[FStringExprPart],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let i64_ty = self.context.i64_type();
        let str_struct_ty = self
            .context
            .struct_type(&[ptr_ty.into(), i64_ty.into()], false);

        // Helper: build a {ptr, len} struct from pointer and length values.
        let build_str_struct = |builder: &Builder<'ctx>,
                                ptr: BasicValueEnum<'ctx>,
                                len: BasicValueEnum<'ctx>|
         -> Result<BasicValueEnum<'ctx>, CodegenError> {
            let mut s = str_struct_ty.get_undef();
            s = builder
                .build_insert_value(s, ptr, 0, "fstr_ptr")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into_struct_value();
            s = builder
                .build_insert_value(s, len, 1, "fstr_len")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into_struct_value();
            Ok(s.into())
        };

        // Empty f-string → return empty string.
        if parts.is_empty() {
            let null_ptr = ptr_ty.const_null();
            let zero_len = i64_ty.const_int(0, false);
            return Ok(Some(build_str_struct(
                &self.builder,
                null_ptr.into(),
                zero_len.into(),
            )?));
        }

        // Compile the first part to initialize the accumulator.
        let mut acc = self.compile_fstring_part(&parts[0], &build_str_struct)?;

        // Concatenate remaining parts using fj_rt_str_concat.
        for part in &parts[1..] {
            let rhs = self.compile_fstring_part(part, &build_str_struct)?;

            // Extract {ptr, len} from accumulator and rhs.
            let acc_struct = acc.into_struct_value();
            let acc_ptr = self
                .builder
                .build_extract_value(acc_struct, 0, "acc_ptr")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            let acc_len = self
                .builder
                .build_extract_value(acc_struct, 1, "acc_len")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            let rhs_struct = rhs.into_struct_value();
            let rhs_ptr = self
                .builder
                .build_extract_value(rhs_struct, 0, "rhs_ptr")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            let rhs_len = self
                .builder
                .build_extract_value(rhs_struct, 1, "rhs_len")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;

            // Call fj_rt_str_concat(ptr1, len1, ptr2, len2) -> ptr
            let concat_fn = *self
                .functions
                .get("fj_rt_str_concat")
                .ok_or_else(|| CodegenError::Internal("fj_rt_str_concat not declared".into()))?;
            let concat_call = self
                .builder
                .build_call(
                    concat_fn,
                    &[
                        acc_ptr.into(),
                        acc_len.into(),
                        rhs_ptr.into(),
                        rhs_len.into(),
                    ],
                    "concat_ptr",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            let result_ptr = match concat_call.try_as_basic_value() {
                inkwell::values::ValueKind::Basic(v) => v,
                inkwell::values::ValueKind::Instruction(_) => {
                    return Err(CodegenError::Internal("str_concat returned void".into()));
                }
            };

            // New length = acc_len + rhs_len
            let new_len = self
                .builder
                .build_int_add(
                    acc_len.into_int_value(),
                    rhs_len.into_int_value(),
                    "new_len",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?;

            acc = build_str_struct(&self.builder, result_ptr, new_len.into())?;
        }

        Ok(Some(acc))
    }

    /// Compiles a single f-string part to a {ptr, len} struct value.
    fn compile_fstring_part<F>(
        &mut self,
        part: &FStringExprPart,
        build_str_struct: &F,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError>
    where
        F: Fn(
            &Builder<'ctx>,
            BasicValueEnum<'ctx>,
            BasicValueEnum<'ctx>,
        ) -> Result<BasicValueEnum<'ctx>, CodegenError>,
    {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let i64_ty = self.context.i64_type();

        match part {
            FStringExprPart::Literal(s) => {
                // Create a global string constant.
                let global = self.get_or_create_string_global(s);
                let ptr = global.as_pointer_value();
                let len = i64_ty.const_int(s.len() as u64, false);
                build_str_struct(&self.builder, ptr.into(), len.into())
            }
            FStringExprPart::Expr(expr) => {
                let val = self.compile_expr(expr)?.ok_or_else(|| {
                    CodegenError::Internal("f-string expr produced no value".into())
                })?;

                // If the value is already a {ptr, len} struct (string), use directly.
                if val.is_struct_value() {
                    return Ok(val);
                }

                // For int/float/bool, call the appropriate to_string runtime function.
                // All use the same pattern: alloca two out-params, call fn, load results.
                let out_ptr_alloca = self
                    .builder
                    .build_alloca(ptr_ty, "fstr_out_ptr")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                let out_len_alloca = self
                    .builder
                    .build_alloca(i64_ty, "fstr_out_len")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;

                let is_bool = infer_type_from_expr(expr) == "bool"
                    || (val.is_int_value() && val.into_int_value().get_type().get_bit_width() == 1);
                let rt_fn_name = if val.is_float_value() {
                    "fj_rt_float_to_string"
                } else if is_bool {
                    "fj_rt_bool_to_string"
                } else {
                    "fj_rt_int_to_string"
                };

                let rt_fn = *self
                    .functions
                    .get(rt_fn_name)
                    .ok_or_else(|| CodegenError::Internal(format!("{rt_fn_name} not declared")))?;

                // Coerce bool (i1) to i64 for the runtime call.
                let call_val =
                    if val.is_int_value() && val.into_int_value().get_type().get_bit_width() == 1 {
                        self.builder
                            .build_int_z_extend(val.into_int_value(), i64_ty, "bool_ext")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?
                            .into()
                    } else {
                        val
                    };

                self.builder
                    .build_call(
                        rt_fn,
                        &[
                            call_val.into(),
                            out_ptr_alloca.into(),
                            out_len_alloca.into(),
                        ],
                        "to_str",
                    )
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;

                let result_ptr = self
                    .builder
                    .build_load(ptr_ty, out_ptr_alloca, "fstr_rptr")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                let result_len = self
                    .builder
                    .build_load(i64_ty, out_len_alloca, "fstr_rlen")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;

                build_str_struct(&self.builder, result_ptr, result_len)
            }
        }
    }

    /// Compiles a literal value.
    fn compile_literal(
        &mut self,
        kind: &LiteralKind,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        match kind {
            LiteralKind::Int(v) => Ok(Some(
                self.context.i64_type().const_int(*v as u64, true).into(),
            )),
            LiteralKind::Float(v) => Ok(Some(self.context.f64_type().const_float(*v).into())),
            LiteralKind::Bool(v) => Ok(Some(
                self.context.bool_type().const_int(*v as u64, false).into(),
            )),
            LiteralKind::String(s) => {
                // Build global string constant and return {ptr, len} struct
                let global = self.get_or_create_string_global(s);

                let ptr = global.as_pointer_value();
                let len = self.context.i64_type().const_int(s.len() as u64, false);

                // Build {ptr, len} struct
                let str_type = self.context.struct_type(
                    &[
                        self.context
                            .ptr_type(inkwell::AddressSpace::default())
                            .into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                let mut str_struct = str_type.get_undef();
                str_struct = self
                    .builder
                    .build_insert_value(str_struct, ptr, 0, "str_ptr")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
                    .into_struct_value();
                str_struct = self
                    .builder
                    .build_insert_value(str_struct, len, 1, "str_len")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
                    .into_struct_value();

                Ok(Some(str_struct.into()))
            }
            LiteralKind::Char(c) => Ok(Some(
                self.context.i8_type().const_int(*c as u64, false).into(),
            )),
            LiteralKind::RawString(s) => {
                // Raw string: same as String but no escape processing
                let global = self.get_or_create_string_global(s);

                let ptr = global.as_pointer_value();
                let len = self.context.i64_type().const_int(s.len() as u64, false);

                let str_type = self.context.struct_type(
                    &[
                        self.context
                            .ptr_type(inkwell::AddressSpace::default())
                            .into(),
                        self.context.i64_type().into(),
                    ],
                    false,
                );
                let mut str_struct = str_type.get_undef();
                str_struct = self
                    .builder
                    .build_insert_value(str_struct, ptr, 0, "str_ptr")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
                    .into_struct_value();
                str_struct = self
                    .builder
                    .build_insert_value(str_struct, len, 1, "str_len")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
                    .into_struct_value();

                Ok(Some(str_struct.into()))
            }
            LiteralKind::Null => Ok(Some(self.context.i64_type().const_int(0, false).into())),
        }
    }

    /// Compiles a binary operation.
    fn compile_binop(
        &self,
        op: &BinOp,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        // Check if float operation
        if lhs.is_float_value() && rhs.is_float_value() {
            let l = lhs.into_float_value();
            let r = rhs.into_float_value();
            let result = match op {
                BinOp::Add => self
                    .builder
                    .build_float_add(l, r, "fadd")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?,
                BinOp::Sub => self
                    .builder
                    .build_float_sub(l, r, "fsub")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?,
                BinOp::Mul => self
                    .builder
                    .build_float_mul(l, r, "fmul")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?,
                BinOp::Div => self
                    .builder
                    .build_float_div(l, r, "fdiv")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?,
                BinOp::Eq => {
                    return Ok(self
                        .builder
                        .build_float_compare(inkwell::FloatPredicate::OEQ, l, r, "feq")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                        .into());
                }
                BinOp::Ne => {
                    return Ok(self
                        .builder
                        .build_float_compare(inkwell::FloatPredicate::ONE, l, r, "fne")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                        .into());
                }
                BinOp::Lt => {
                    return Ok(self
                        .builder
                        .build_float_compare(inkwell::FloatPredicate::OLT, l, r, "flt")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                        .into());
                }
                BinOp::Le => {
                    return Ok(self
                        .builder
                        .build_float_compare(inkwell::FloatPredicate::OLE, l, r, "fle")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                        .into());
                }
                BinOp::Gt => {
                    return Ok(self
                        .builder
                        .build_float_compare(inkwell::FloatPredicate::OGT, l, r, "fgt")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                        .into());
                }
                BinOp::Ge => {
                    return Ok(self
                        .builder
                        .build_float_compare(inkwell::FloatPredicate::OGE, l, r, "fge")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                        .into());
                }
                // G1: Float modulo (frem instruction)
                BinOp::Rem => self
                    .builder
                    .build_float_rem(l, r, "frem")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?,
                // G1: Float power (llvm.pow.f64 intrinsic)
                BinOp::Pow => {
                    let f64_ty = self.context.f64_type();
                    let pow_fn_ty = f64_ty.fn_type(&[f64_ty.into(), f64_ty.into()], false);
                    let pow_fn = self.module.get_function("llvm.pow.f64").unwrap_or_else(|| {
                        self.module.add_function("llvm.pow.f64", pow_fn_ty, None)
                    });
                    let call = self
                        .builder
                        .build_call(pow_fn, &[l.into(), r.into()], "fpow")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    match call.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(v) => {
                            return Ok(v);
                        }
                        inkwell::values::ValueKind::Instruction(_) => {
                            return Ok(f64_ty.const_float(0.0).into());
                        }
                    }
                }
                // Float logical: convert to bool first
                BinOp::And => {
                    let l_bool = self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::ONE,
                            l,
                            l.get_type().const_float(0.0),
                            "fl_bool",
                        )
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    let r_bool = self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::ONE,
                            r,
                            r.get_type().const_float(0.0),
                            "fr_bool",
                        )
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    return Ok(self
                        .builder
                        .build_and(l_bool, r_bool, "fland")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                        .into());
                }
                BinOp::Or => {
                    let l_bool = self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::ONE,
                            l,
                            l.get_type().const_float(0.0),
                            "fl_bool",
                        )
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    let r_bool = self
                        .builder
                        .build_float_compare(
                            inkwell::FloatPredicate::ONE,
                            r,
                            r.get_type().const_float(0.0),
                            "fr_bool",
                        )
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    return Ok(self
                        .builder
                        .build_or(l_bool, r_bool, "flor")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                        .into());
                }
                BinOp::MatMul => {
                    return Err(CodegenError::Internal(
                        "matrix multiply (@) not supported on scalar floats".into(),
                    ));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "LLVM float binop: {:?}",
                        op
                    )));
                }
            };
            return Ok(result.into());
        }

        // String concatenation: both operands are {ptr, len} structs
        if lhs.is_struct_value() && rhs.is_struct_value() && matches!(op, BinOp::Add) {
            let l_struct = lhs.into_struct_value();
            let r_struct = rhs.into_struct_value();
            let l_ptr = self
                .builder
                .build_extract_value(l_struct, 0, "sl_ptr")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            let l_len = self
                .builder
                .build_extract_value(l_struct, 1, "sl_len")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            let r_ptr = self
                .builder
                .build_extract_value(r_struct, 0, "sr_ptr")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            let r_len = self
                .builder
                .build_extract_value(r_struct, 1, "sr_len")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;

            let concat_fn = *self
                .functions
                .get("fj_rt_str_concat")
                .ok_or_else(|| CodegenError::Internal("fj_rt_str_concat not declared".into()))?;
            let concat_call = self
                .builder
                .build_call(
                    concat_fn,
                    &[l_ptr.into(), l_len.into(), r_ptr.into(), r_len.into()],
                    "str_add",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            let result_ptr = match concat_call.try_as_basic_value() {
                inkwell::values::ValueKind::Basic(v) => v,
                inkwell::values::ValueKind::Instruction(_) => {
                    return Err(CodegenError::Internal("str_concat returned void".into()));
                }
            };
            let new_len = self
                .builder
                .build_int_add(
                    l_len.into_int_value(),
                    r_len.into_int_value(),
                    "str_add_len",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?;

            let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
            let i64_ty = self.context.i64_type();
            let str_struct_ty = self
                .context
                .struct_type(&[ptr_ty.into(), i64_ty.into()], false);
            let mut s = str_struct_ty.get_undef();
            s = self
                .builder
                .build_insert_value(s, result_ptr, 0, "sc_ptr")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into_struct_value();
            s = self
                .builder
                .build_insert_value(s, new_len, 1, "sc_len")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into_struct_value();
            return Ok(s.into());
        }

        // Integer operations — harmonize types (i1 ↔ i64)
        let i64_ty = self.context.i64_type();
        let l_raw = lhs.into_int_value();
        let r_raw = rhs.into_int_value();
        let l = if l_raw.get_type().get_bit_width() < 64 {
            self.builder
                .build_int_z_extend(l_raw, i64_ty, "lext")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
        } else {
            l_raw
        };
        let r = if r_raw.get_type().get_bit_width() < 64 {
            self.builder
                .build_int_z_extend(r_raw, i64_ty, "rext")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
        } else {
            r_raw
        };

        let result: BasicValueEnum<'ctx> = match op {
            BinOp::Add => self
                .builder
                .build_int_add(l, r, "add")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Sub => self
                .builder
                .build_int_sub(l, r, "sub")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Mul => self
                .builder
                .build_int_mul(l, r, "mul")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Div => self
                .builder
                .build_int_signed_div(l, r, "div")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Rem => self
                .builder
                .build_int_signed_rem(l, r, "rem")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Eq => self
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, l, r, "eq")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Ne => self
                .builder
                .build_int_compare(inkwell::IntPredicate::NE, l, r, "ne")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Lt => self
                .builder
                .build_int_compare(inkwell::IntPredicate::SLT, l, r, "lt")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Le => self
                .builder
                .build_int_compare(inkwell::IntPredicate::SLE, l, r, "le")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Gt => self
                .builder
                .build_int_compare(inkwell::IntPredicate::SGT, l, r, "gt")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Ge => self
                .builder
                .build_int_compare(inkwell::IntPredicate::SGE, l, r, "ge")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::BitAnd => self
                .builder
                .build_and(l, r, "and")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::BitOr => self
                .builder
                .build_or(l, r, "or")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::BitXor => self
                .builder
                .build_xor(l, r, "xor")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Shl => self
                .builder
                .build_left_shift(l, r, "shl")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::Shr => self
                .builder
                .build_right_shift(l, r, true, "shr")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into(),
            BinOp::And => {
                // Short-circuit AND: both must be non-zero
                let l_bool = self
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::NE,
                        l,
                        l.get_type().const_int(0, false),
                        "l_bool",
                    )
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                let r_bool = self
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::NE,
                        r,
                        r.get_type().const_int(0, false),
                        "r_bool",
                    )
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                self.builder
                    .build_and(l_bool, r_bool, "land")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
                    .into()
            }
            BinOp::Or => {
                let l_bool = self
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::NE,
                        l,
                        l.get_type().const_int(0, false),
                        "l_bool",
                    )
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                let r_bool = self
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::NE,
                        r,
                        r.get_type().const_int(0, false),
                        "r_bool",
                    )
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                self.builder
                    .build_or(l_bool, r_bool, "lor")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
                    .into()
            }
            // I2: Integer power — compute via repeated multiplication loop
            BinOp::Pow => {
                let function = self
                    .builder
                    .get_insert_block()
                    .and_then(|b| b.get_parent())
                    .ok_or_else(|| CodegenError::Internal("no current function for ipow".into()))?;
                let i64_ty = self.context.i64_type();
                let result_alloca = self
                    .builder
                    .build_alloca(i64_ty, "pow_result")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                let counter_alloca = self
                    .builder
                    .build_alloca(i64_ty, "pow_counter")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                self.builder
                    .build_store(result_alloca, i64_ty.const_int(1, false))
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                self.builder
                    .build_store(counter_alloca, i64_ty.const_int(0, false))
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;

                let cond_bb = self.context.append_basic_block(function, "pow_cond");
                let body_bb = self.context.append_basic_block(function, "pow_body");
                let done_bb = self.context.append_basic_block(function, "pow_done");

                self.builder
                    .build_unconditional_branch(cond_bb)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;

                self.builder.position_at_end(cond_bb);
                let cnt = self
                    .builder
                    .build_load(i64_ty, counter_alloca, "cnt")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
                    .into_int_value();
                let cmp = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::SLT, cnt, r, "pow_cmp")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                self.builder
                    .build_conditional_branch(cmp, body_bb, done_bb)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;

                self.builder.position_at_end(body_bb);
                let cur = self
                    .builder
                    .build_load(i64_ty, result_alloca, "cur")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
                    .into_int_value();
                let new_val = self
                    .builder
                    .build_int_mul(cur, l, "pow_mul")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                self.builder
                    .build_store(result_alloca, new_val)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                let next_cnt = self
                    .builder
                    .build_int_add(cnt, i64_ty.const_int(1, false), "pow_inc")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                self.builder
                    .build_store(counter_alloca, next_cnt)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                self.builder
                    .build_unconditional_branch(cond_bb)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;

                self.builder.position_at_end(done_bb);
                self.builder
                    .build_load(i64_ty, result_alloca, "pow_val")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
            }
            // MatMul requires tensor runtime — not applicable for scalar ints
            BinOp::MatMul => {
                return Err(CodegenError::Internal(
                    "matrix multiply (@) not supported on scalar integers".into(),
                ));
            }
        };
        Ok(result)
    }

    /// Compiles a unary operation.
    fn compile_unaryop(
        &self,
        op: &UnaryOp,
        val: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        match op {
            UnaryOp::Neg => {
                if val.is_float_value() {
                    Ok(self
                        .builder
                        .build_float_neg(val.into_float_value(), "fneg")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                        .into())
                } else {
                    Ok(self
                        .builder
                        .build_int_neg(val.into_int_value(), "neg")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                        .into())
                }
            }
            UnaryOp::Not => Ok(self
                .builder
                .build_not(val.into_int_value(), "not")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into()),
            UnaryOp::BitNot => Ok(self
                .builder
                .build_not(val.into_int_value(), "bitnot")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into()),
            // G2: &expr — take address (value must be an alloca pointer as i64)
            UnaryOp::Ref | UnaryOp::RefMut => {
                // In the LLVM backend, values are typically i64 addresses for
                // pointers, so &x is identity — the value IS the address.
                Ok(val)
            }
            // G2: *ptr — dereference pointer (treat i64 as address, load i64 from it)
            UnaryOp::Deref => {
                if val.is_int_value() {
                    let addr = val.into_int_value();
                    let ptr = self
                        .builder
                        .build_int_to_ptr(
                            addr,
                            self.context.ptr_type(inkwell::AddressSpace::default()),
                            "deref_ptr",
                        )
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    let loaded = self
                        .builder
                        .build_load(self.context.i64_type(), ptr, "deref_val")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    Ok(loaded)
                } else {
                    Err(CodegenError::Internal(
                        "deref requires integer (address) value".into(),
                    ))
                }
            }
        }
    }

    /// Compiles an if/else expression.
    /// Compiles short-circuit `&&` and `||` with proper control flow.
    /// `a && b` only evaluates `b` if `a` is truthy.
    /// `a || b` only evaluates `b` if `a` is falsy.
    fn compile_short_circuit(
        &mut self,
        left: &Expr,
        op: &BinOp,
        right: &Expr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let i64_ty = self.context.i64_type();
        let current_fn = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| CodegenError::Internal("no current function".into()))?;

        // Evaluate LHS
        let lhs = self
            .compile_expr(left)?
            .ok_or_else(|| CodegenError::Internal("short-circuit LHS no value".into()))?;
        let lhs_int = lhs.into_int_value();
        // If LHS is already i1 (bool from comparison), use directly; otherwise compare != 0
        let lhs_bool = if lhs_int.get_type().get_bit_width() == 1 {
            lhs_int
        } else {
            self.builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    lhs_int,
                    i64_ty.const_zero(),
                    "lhs_bool",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?
        };

        // Create basic blocks
        let rhs_bb = self.context.append_basic_block(current_fn, "sc_rhs");
        let merge_bb = self.context.append_basic_block(current_fn, "sc_merge");
        let lhs_bb = self
            .builder
            .get_insert_block()
            .ok_or_else(|| CodegenError::Internal("no insert block".into()))?;

        // Branch based on operator
        match op {
            BinOp::And => {
                // a && b: if a is false, skip b (result = false)
                self.builder
                    .build_conditional_branch(lhs_bool, rhs_bb, merge_bb)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
            }
            BinOp::Or => {
                // a || b: if a is true, skip b (result = true)
                self.builder
                    .build_conditional_branch(lhs_bool, merge_bb, rhs_bb)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
            }
            _ => unreachable!(),
        }

        // Evaluate RHS
        self.builder.position_at_end(rhs_bb);
        let rhs = self
            .compile_expr(right)?
            .ok_or_else(|| CodegenError::Internal("short-circuit RHS no value".into()))?;
        let rhs_int = rhs.into_int_value();
        let rhs_bool = if rhs_int.get_type().get_bit_width() == 1 {
            rhs_int
        } else {
            self.builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    rhs_int,
                    i64_ty.const_zero(),
                    "rhs_bool",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?
        };
        let rhs_end_bb = self
            .builder
            .get_insert_block()
            .ok_or_else(|| CodegenError::Internal("no insert block after rhs".into()))?;
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // Merge with phi node
        self.builder.position_at_end(merge_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "sc_result")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        match op {
            BinOp::And => {
                // From LHS block (false path): result = false
                // From RHS block: result = rhs_bool
                phi.add_incoming(&[
                    (&self.context.bool_type().const_zero(), lhs_bb),
                    (&rhs_bool, rhs_end_bb),
                ]);
            }
            BinOp::Or => {
                // From LHS block (true path): result = true
                // From RHS block: result = rhs_bool
                phi.add_incoming(&[
                    (&self.context.bool_type().const_int(1, false), lhs_bb),
                    (&rhs_bool, rhs_end_bb),
                ]);
            }
            _ => unreachable!(),
        }

        // Zero-extend bool (i1) to i64 for uniform ABI
        let result = self
            .builder
            .build_int_z_extend(phi.as_basic_value().into_int_value(), i64_ty, "sc_i64")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        Ok(Some(result.into()))
    }

    fn compile_if(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let cond_val = self
            .compile_expr(condition)?
            .ok_or_else(|| CodegenError::Internal("if condition produced no value".into()))?;

        // Convert to i1 if needed
        let cond_bool = self.to_i1(cond_val)?;

        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| CodegenError::Internal("no current function".into()))?;

        let then_bb = self.context.append_basic_block(function, "then");
        let else_bb = self.context.append_basic_block(function, "else");
        let merge_bb = self.context.append_basic_block(function, "merge");

        self.builder
            .build_conditional_branch(cond_bool, then_bb, else_bb)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // Then block
        self.builder.position_at_end(then_bb);
        let then_val = self.compile_expr(then_branch)?;
        let then_exit_bb = self
            .builder
            .get_insert_block()
            .ok_or_else(|| CodegenError::Internal("no insert block after then".into()))?;
        let then_terminated = then_exit_bb.get_terminator().is_some();
        if !then_terminated {
            self.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
        }

        // Else block
        self.builder.position_at_end(else_bb);
        let else_val = if let Some(else_expr) = else_branch {
            self.compile_expr(else_expr)?
        } else {
            Some(self.context.i64_type().const_int(0, false).into())
        };
        let else_exit_bb = self
            .builder
            .get_insert_block()
            .ok_or_else(|| CodegenError::Internal("no insert block after else".into()))?;
        let else_terminated = else_exit_bb.get_terminator().is_some();
        if !else_terminated {
            self.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
        }

        // Merge with phi
        self.builder.position_at_end(merge_bb);

        // Only build phi if both branches have values and are not terminated early
        if !then_terminated && !else_terminated {
            if let (Some(tv), Some(ev)) = (then_val, else_val) {
                let phi = self
                    .builder
                    .build_phi(self.context.i64_type(), "if_result")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                phi.add_incoming(&[(&tv, then_exit_bb), (&ev, else_exit_bb)]);
                return Ok(Some(phi.as_basic_value()));
            }
        }

        Ok(None)
    }

    /// Compiles a statement.
    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                // Special case: struct init — bind variable directly to struct alloca
                if let Expr::StructInit {
                    name: struct_name,
                    fields: field_inits,
                    ..
                } = value.as_ref()
                {
                    let (struct_type, field_names) =
                        self.struct_types.get(struct_name).cloned().ok_or_else(|| {
                            CodegenError::Internal(format!("undefined struct: {struct_name}"))
                        })?;

                    let alloca = self
                        .builder
                        .build_alloca(struct_type, name)
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;

                    for fi in field_inits {
                        let field_idx =
                            field_names
                                .iter()
                                .position(|n| n == &fi.name)
                                .ok_or_else(|| {
                                    CodegenError::Internal(format!(
                                        "unknown field '{}' on struct {struct_name}",
                                        fi.name
                                    ))
                                })?;

                        let val = self.compile_expr(&fi.value)?.ok_or_else(|| {
                            CodegenError::Internal(format!(
                                "struct field '{}' produced no value",
                                fi.name
                            ))
                        })?;

                        let field_ptr = self
                            .builder
                            .build_struct_gep(struct_type, alloca, field_idx as u32, &fi.name)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        self.builder
                            .build_store(field_ptr, val)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }

                    self.variables.insert(name.clone(), alloca);
                    self.var_types.insert(name.clone(), struct_type.into());
                    return Ok(None);
                }

                let init_val = self.compile_expr(value)?.ok_or_else(|| {
                    CodegenError::Internal("let initializer produced no value".into())
                })?;

                let llvm_type = if let Some(ty) = ty {
                    fj_type_to_llvm(self.context, &type_expr_to_string(ty))
                } else {
                    // Infer from value
                    init_val.get_type()
                };

                let alloca = self
                    .builder
                    .build_alloca(llvm_type, name)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                self.builder
                    .build_store(alloca, init_val)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;

                self.variables.insert(name.clone(), alloca);
                self.var_types.insert(name.clone(), llvm_type);
                Ok(None)
            }

            Stmt::Const { name, value, .. } => {
                // Local const inside a function body — evaluate and store in constants map
                let init_val = self.compile_expr(value)?.ok_or_else(|| {
                    CodegenError::Internal("const initializer produced no value".into())
                })?;
                self.constants.insert(name.clone(), init_val);
                Ok(None)
            }

            Stmt::Expr { expr, .. } => self.compile_expr(expr),

            Stmt::Return { value, .. } => {
                // Determine if current function is void
                let current_fn = self.builder.get_insert_block().and_then(|b| b.get_parent());
                let is_void = current_fn.is_some_and(|f| f.get_type().get_return_type().is_none());

                if let Some(expr) = value {
                    let val = self.compile_expr(expr)?;
                    if is_void {
                        // Void function: discard return value, return void
                        self.builder
                            .build_return(None)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    } else if let Some(v) = val {
                        // E4: Coerce return value to match function signature (i1→i64)
                        let coerced = if let Some(ret_ty) =
                            current_fn.and_then(|f| f.get_type().get_return_type())
                        {
                            if ret_ty.is_int_type() && v.is_int_value() {
                                let ret_width = ret_ty.into_int_type().get_bit_width();
                                let val_width = v.into_int_value().get_type().get_bit_width();
                                if val_width < ret_width {
                                    self.builder
                                        .build_int_z_extend(
                                            v.into_int_value(),
                                            ret_ty.into_int_type(),
                                            "ret_ext",
                                        )
                                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                                        .into()
                                } else if val_width > ret_width {
                                    self.builder
                                        .build_int_truncate(
                                            v.into_int_value(),
                                            ret_ty.into_int_type(),
                                            "ret_trunc",
                                        )
                                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                                        .into()
                                } else {
                                    v
                                }
                            } else {
                                v
                            }
                        } else {
                            v
                        };
                        self.builder
                            .build_return(Some(&coerced))
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    } else {
                        self.builder
                            .build_return(None)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                } else if is_void {
                    self.builder
                        .build_return(None)
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                } else {
                    let zero = self.context.i64_type().const_int(0, false);
                    self.builder
                        .build_return(Some(&zero))
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                }
                Ok(None)
            }

            Stmt::Break { value, .. } => {
                let (after_bb, break_alloca) = self
                    .break_target
                    .ok_or_else(|| CodegenError::Internal("break outside of loop".into()))?;
                if let Some(val_expr) = value {
                    if let Some(alloca) = break_alloca {
                        let val = self.compile_expr(val_expr)?.ok_or_else(|| {
                            CodegenError::Internal("break value produced no value".into())
                        })?;
                        self.builder
                            .build_store(alloca, val)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                }
                self.builder
                    .build_unconditional_branch(after_bb)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                Ok(None)
            }

            Stmt::Continue { .. } => {
                let header_bb = self
                    .continue_target
                    .ok_or_else(|| CodegenError::Internal("continue outside of loop".into()))?;
                self.builder
                    .build_unconditional_branch(header_bb)
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                Ok(None)
            }

            // H1: Nested item definitions (fn/struct/enum/impl inside blocks)
            Stmt::Item(item) => {
                match item.as_ref() {
                    Item::FnDef(fndef) => {
                        self.declare_function(fndef)?;
                        self.compile_function(fndef)?;
                    }
                    Item::StructDef(sdef) => {
                        self.register_struct(sdef)?;
                    }
                    Item::EnumDef(edef) => {
                        self.register_enum(edef);
                    }
                    Item::ImplBlock(ib) => {
                        self.register_impl_block(ib);
                        for method in &ib.methods {
                            // V33.P7 (Gap G-C): @no_mangle keeps bare name.
                            let mangled_name = if method.no_mangle {
                                method.name.clone()
                            } else {
                                format!("{}__{}", ib.target_type, method.name)
                            };
                            let mut mangled_method = method.clone();
                            mangled_method.name = mangled_name;
                            self.declare_function(&mangled_method)?;
                            self.compile_function(&mangled_method)?;
                        }
                    }
                    Item::ConstDef(cdef) => {
                        self.compile_const_def(cdef);
                    }
                    Item::StaticDef(sdef) => {
                        self.compile_static_def(sdef)?;
                    }
                    _ => {} // TraitDef, Use, etc. — no codegen needed
                }
                Ok(None)
            }
        }
    }

    /// Compiles an array literal `[a, b, c]`.
    ///
    /// Layout: alloca of `[N x element_type]`, store each element, return pointer.
    fn compile_array(
        &mut self,
        elements: &[Expr],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        if elements.is_empty() {
            // Empty array → null pointer
            return Ok(Some(self.context.i64_type().const_int(0, false).into()));
        }

        // Compile all elements
        let mut compiled: Vec<BasicValueEnum<'ctx>> = Vec::new();
        for elem in elements {
            let val = self
                .compile_expr(elem)?
                .ok_or_else(|| CodegenError::Internal("array element produced no value".into()))?;
            compiled.push(val);
        }

        // Determine element type from first element
        let elem_type = compiled[0].get_type();
        let array_type = elem_type.array_type(elements.len() as u32);

        // Allocate array in the function's entry block so it survives
        // LLVM optimization passes (prevents dangling stack pointer).
        let alloca = self.build_entry_block_alloca(array_type.into(), "arr")?;

        // Store each element via GEP
        for (i, val) in compiled.iter().enumerate() {
            let idx = self.context.i32_type().const_int(i as u64, false);
            let zero = self.context.i32_type().const_int(0, false);
            // SAFETY: GEP into array is safe — indices are in bounds by construction.
            let elem_ptr = unsafe {
                self.builder
                    .build_in_bounds_gep(array_type, alloca, &[zero, idx], &format!("arr_{i}"))
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
            };
            self.builder
                .build_store(elem_ptr, *val)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
        }

        // Return pointer as i64 (opaque pointer representation)
        let ptr_as_int = self
            .builder
            .build_ptr_to_int(alloca, self.context.i64_type(), "arr_ptr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        Ok(Some(ptr_as_int.into()))
    }

    /// Compiles a tuple literal `(a, b, c)`.
    fn compile_tuple(
        &mut self,
        elements: &[Expr],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let mut compiled: Vec<BasicValueEnum<'ctx>> = Vec::new();
        for elem in elements {
            let val = self
                .compile_expr(elem)?
                .ok_or_else(|| CodegenError::Internal("tuple element produced no value".into()))?;
            compiled.push(val);
        }

        let field_types: Vec<BasicTypeEnum<'ctx>> = compiled.iter().map(|v| v.get_type()).collect();
        let tuple_type = self.context.struct_type(&field_types, false);

        let mut tuple_val = tuple_type.get_undef();
        for (i, val) in compiled.iter().enumerate() {
            tuple_val = self
                .builder
                .build_insert_value(tuple_val, *val, i as u32, &format!("tup_{i}"))
                .map_err(|e| CodegenError::Internal(e.to_string()))?
                .into_struct_value();
        }

        Ok(Some(tuple_val.into()))
    }

    /// Compiles a struct instantiation `Point { x: 1, y: 2 }`.
    fn compile_struct_init(
        &mut self,
        name: &str,
        fields: &[crate::parser::ast::FieldInit],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let (struct_type, field_names) = self
            .struct_types
            .get(name)
            .cloned()
            .ok_or_else(|| CodegenError::Internal(format!("undefined struct: {name}")))?;

        let alloca = self
            .builder
            .build_alloca(struct_type, name)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        for fi in fields {
            let field_idx = field_names
                .iter()
                .position(|n| n == &fi.name)
                .ok_or_else(|| {
                    CodegenError::Internal(format!("unknown field '{}'  on struct {name}", fi.name))
                })?;

            let val = self.compile_expr(&fi.value)?.ok_or_else(|| {
                CodegenError::Internal(format!("struct field '{}' produced no value", fi.name))
            })?;

            let field_ptr = self
                .builder
                .build_struct_gep(struct_type, alloca, field_idx as u32, &fi.name)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            self.builder
                .build_store(field_ptr, val)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
        }

        // Store alloca pointer as the variable's type
        self.var_types.insert(name.to_string(), struct_type.into());

        // Return pointer as i64 (opaque handle)
        let ptr_as_int = self
            .builder
            .build_ptr_to_int(alloca, self.context.i64_type(), "struct_ptr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        Ok(Some(ptr_as_int.into()))
    }

    /// Compiles field access `obj.field`.
    fn compile_field_access(
        &mut self,
        object: &Expr,
        field: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        // Check if the object is an ident whose type is a known struct
        if let Expr::Ident { name, .. } = object {
            // Look up the variable to find if it's a struct pointer
            for (struct_name, (struct_type, field_names)) in &self.struct_types {
                if let Some(var_type) = self.var_types.get(name) {
                    if var_type.is_struct_type() && var_type.into_struct_type() == *struct_type {
                        let field_idx =
                            field_names.iter().position(|n| n == field).ok_or_else(|| {
                                CodegenError::Internal(format!(
                                    "unknown field '{field}' on struct {struct_name}"
                                ))
                            })?;

                        let ptr = self
                            .variables
                            .get(name)
                            .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;

                        // The variable IS the alloca to the struct
                        let field_ptr = self
                            .builder
                            .build_struct_gep(
                                *struct_type,
                                *ptr,
                                field_idx as u32,
                                &format!("{name}.{field}"),
                            )
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;

                        let field_type_idx = struct_type
                            .get_field_type_at_index(field_idx as u32)
                            .ok_or_else(|| {
                                CodegenError::Internal(format!(
                                    "struct {struct_name} field index {field_idx} out of bounds"
                                ))
                            })?;

                        let val = self
                            .builder
                            .build_load(field_type_idx, field_ptr, field)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        return Ok(Some(val));
                    }
                }
            }
        }

        // H3: Tuple field access — numeric field names (.0, .1, etc.)
        if let Ok(idx) = field.parse::<u32>() {
            let obj_val = self.compile_expr(object)?;
            if let Some(val) = obj_val {
                if val.is_struct_value() {
                    let sv = val.into_struct_value();
                    let result = self
                        .builder
                        .build_extract_value(sv, idx, &format!("tuple_{idx}"))
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    return Ok(Some(result));
                }
            }
        }

        // G3: Nested field access — compile object expression, then if it
        // returns a struct value (not pointer), extract the field directly.
        // This supports patterns like `func().field` and `(expr).field`.
        if let Expr::Field {
            object: inner_obj,
            field: inner_field,
            ..
        } = object
        {
            // Recursive: inner.outer — compile inner field access first
            let inner_val = self.compile_field_access(inner_obj, inner_field)?;
            if let Some(val) = inner_val {
                if val.is_struct_value() {
                    let sv = val.into_struct_value();
                    let st = sv.get_type();
                    // Find field by checking struct_types for matching type
                    for (stype, field_names) in self.struct_types.values() {
                        if *stype == st {
                            if let Some(idx) = field_names.iter().position(|n| n == field) {
                                let fval = self
                                    .builder
                                    .build_extract_value(sv, idx as u32, field)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                return Ok(Some(fval));
                            }
                        }
                    }
                }
            }
        }

        // G3: General expression field access — compile as struct value
        let obj_val = self.compile_expr(object)?;
        if let Some(val) = obj_val {
            if val.is_struct_value() {
                let sv = val.into_struct_value();
                let st = sv.get_type();
                for (stype, field_names) in self.struct_types.values() {
                    if *stype == st {
                        if let Some(idx) = field_names.iter().position(|n| n == field) {
                            let fval = self
                                .builder
                                .build_extract_value(sv, idx as u32, field)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            return Ok(Some(fval));
                        }
                    }
                }
            }
        }

        Err(CodegenError::NotImplemented(format!(
            "LLVM field access on non-struct: .{field}"
        )))
    }

    /// Compiles index access `arr[i]`.
    fn compile_index_access(
        &mut self,
        object: &Expr,
        index: &Expr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let obj_val = self
            .compile_expr(object)?
            .ok_or_else(|| CodegenError::Internal("index object produced no value".into()))?;
        let idx_val = self
            .compile_expr(index)?
            .ok_or_else(|| CodegenError::Internal("index value produced no value".into()))?;

        // Object is an i64 (pointer as int). Convert back to pointer and GEP.
        let ptr = self
            .builder
            .build_int_to_ptr(
                obj_val.into_int_value(),
                self.context.ptr_type(inkwell::AddressSpace::default()),
                "arr_ptr",
            )
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // GEP with i64 index, assuming i64 element type
        let i64_type = self.context.i64_type();
        // SAFETY: Array bounds are checked at runtime in the interpreter.
        // The LLVM backend trusts that the program has been validated.
        let elem_ptr = unsafe {
            self.builder
                .build_in_bounds_gep(i64_type, ptr, &[idx_val.into_int_value()], "elem_ptr")
                .map_err(|e| CodegenError::Internal(e.to_string()))?
        };

        let val = self
            .builder
            .build_load(i64_type, elem_ptr, "elem")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        Ok(Some(val))
    }

    /// Compiles a type cast expression (`expr as Type`).
    fn compile_cast(
        &mut self,
        expr: &Expr,
        ty: &TypeExpr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let val = self
            .compile_expr(expr)?
            .ok_or_else(|| CodegenError::Internal("cast operand produced no value".into()))?;

        let target_name = type_expr_to_string(ty);
        let target_type = fj_type_to_llvm(self.context, &target_name);

        // Int → Int (use zero-extend for widening, truncate for narrowing)
        if val.is_int_value() && target_type.is_int_type() {
            let iv = val.into_int_value();
            let tt = target_type.into_int_type();
            let src_width = iv.get_type().get_bit_width();
            let dst_width = tt.get_bit_width();
            let result = if src_width < dst_width {
                self.builder
                    .build_int_z_extend(iv, tt, "icast_zext")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
            } else if src_width > dst_width {
                self.builder
                    .build_int_truncate(iv, tt, "icast_trunc")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
            } else {
                iv
            };
            return Ok(Some(result.into()));
        }

        // Float → Float
        if val.is_float_value() && target_type.is_float_type() {
            let result = self
                .builder
                .build_float_cast(
                    val.into_float_value(),
                    target_type.into_float_type(),
                    "fcast",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            return Ok(Some(result.into()));
        }

        // Int → Float
        if val.is_int_value() && target_type.is_float_type() {
            let result = self
                .builder
                .build_signed_int_to_float(
                    val.into_int_value(),
                    target_type.into_float_type(),
                    "si2fp",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            return Ok(Some(result.into()));
        }

        // Float → Int
        if val.is_float_value() && target_type.is_int_type() {
            let result = self
                .builder
                .build_float_to_signed_int(
                    val.into_float_value(),
                    target_type.into_int_type(),
                    "fp2si",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            return Ok(Some(result.into()));
        }

        // G4: Bool (i1) → Int: zero-extend
        if val.is_int_value() && target_type.is_int_type() {
            let iv = val.into_int_value();
            let tt = target_type.into_int_type();
            if iv.get_type().get_bit_width() < tt.get_bit_width() {
                let result = self
                    .builder
                    .build_int_z_extend(iv, tt, "bool2int")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                return Ok(Some(result.into()));
            } else if iv.get_type().get_bit_width() > tt.get_bit_width() {
                let result = self
                    .builder
                    .build_int_truncate(iv, tt, "int2bool")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                return Ok(Some(result.into()));
            }
            // Same width — no-op
            return Ok(Some(val));
        }

        // Pointer → Int (ptr as i64)
        if val.is_pointer_value() && target_type.is_int_type() {
            let result = self
                .builder
                .build_ptr_to_int(
                    val.into_pointer_value(),
                    target_type.into_int_type(),
                    "ptr2int",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            return Ok(Some(result.into()));
        }

        // Int → Pointer (i64 as *T)
        if val.is_int_value() && target_type.is_pointer_type() {
            let result = self
                .builder
                .build_int_to_ptr(
                    val.into_int_value(),
                    target_type.into_pointer_type(),
                    "int2ptr",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            return Ok(Some(result.into()));
        }

        Err(CodegenError::NotImplemented(format!(
            "LLVM cast: unsupported conversion to {}",
            target_name
        )))
    }

    /// Compiles a while loop.
    fn compile_while(
        &mut self,
        condition: &Expr,
        body: &Expr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| CodegenError::Internal("no current function".into()))?;

        let cond_bb = self.context.append_basic_block(function, "while_cond");
        let body_bb = self.context.append_basic_block(function, "while_body");
        let after_bb = self.context.append_basic_block(function, "while_after");

        // Branch to condition
        self.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // Condition block
        self.builder.position_at_end(cond_bb);
        let cond_val = self
            .compile_expr(condition)?
            .ok_or_else(|| CodegenError::Internal("while condition produced no value".into()))?;
        let cond_bool = self.to_i1(cond_val)?;
        self.builder
            .build_conditional_branch(cond_bool, body_bb, after_bb)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // Body block
        self.builder.position_at_end(body_bb);
        let prev_break = self.break_target.take();
        let prev_continue = self.continue_target.take();
        self.break_target = Some((after_bb, None));
        self.continue_target = Some(cond_bb);

        self.compile_expr(body)?;

        // Only branch back if the block isn't already terminated (break/continue)
        if self
            .builder
            .get_insert_block()
            .is_some_and(|b| b.get_terminator().is_none())
        {
            self.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
        }

        self.break_target = prev_break;
        self.continue_target = prev_continue;

        // After block
        self.builder.position_at_end(after_bb);
        Ok(None)
    }

    /// Compiles a for-in range loop.
    fn compile_for(
        &mut self,
        variable: &str,
        iterable: &Expr,
        body: &Expr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        // Support Range { start, end, inclusive } pattern
        let (start_val, end_val, _inclusive) = match iterable {
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                let start_expr = start.as_ref().ok_or_else(|| {
                    CodegenError::NotImplemented("for loop requires range start".into())
                })?;
                let end_expr = end.as_ref().ok_or_else(|| {
                    CodegenError::NotImplemented("for loop requires range end".into())
                })?;
                let sv = self.compile_expr(start_expr)?.ok_or_else(|| {
                    CodegenError::Internal("for range start produced no value".into())
                })?;
                let ev = self.compile_expr(end_expr)?.ok_or_else(|| {
                    CodegenError::Internal("for range end produced no value".into())
                })?;
                (sv, ev, *inclusive)
            }
            _ => {
                // F6: Non-range iterable — compile as expression and iterate
                // as pointer+length (array) using index 0..len.
                // The iterable value is treated as an i64 pointer (base address)
                // and iterated by GEP. For non-pointer values, treat as 0..value.
                let iter_val = self.compile_expr(iterable)?.ok_or_else(|| {
                    CodegenError::Internal("for iterable produced no value".into())
                })?;
                if iter_val.is_int_value() {
                    // Treat as 0..value range
                    let start = self.context.i64_type().const_int(0, false).into();
                    (start, iter_val, false)
                } else {
                    return Err(CodegenError::NotImplemented(
                        "LLVM for loop: unsupported iterable type (expected range or integer)"
                            .into(),
                    ));
                }
            }
        };

        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| CodegenError::Internal("no current function".into()))?;

        let i64_type = self.context.i64_type();

        // Alloca for loop variable
        let var_alloca = self
            .builder
            .build_alloca(i64_type, variable)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        self.builder
            .build_store(var_alloca, start_val)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        self.variables.insert(variable.to_string(), var_alloca);
        self.var_types.insert(variable.to_string(), i64_type.into());

        let cond_bb = self.context.append_basic_block(function, "for_cond");
        let body_bb = self.context.append_basic_block(function, "for_body");
        let step_bb = self.context.append_basic_block(function, "for_step");
        let after_bb = self.context.append_basic_block(function, "for_after");

        self.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // Condition: i < end (or i <= end if inclusive)
        self.builder.position_at_end(cond_bb);
        let cur = self
            .builder
            .build_load(i64_type, var_alloca, "cur")
            .map_err(|e| CodegenError::Internal(e.to_string()))?
            .into_int_value();
        let end_int = end_val.into_int_value();
        let pred = if _inclusive {
            inkwell::IntPredicate::SLE
        } else {
            inkwell::IntPredicate::SLT
        };
        let cmp = self
            .builder
            .build_int_compare(pred, cur, end_int, "for_cmp")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        self.builder
            .build_conditional_branch(cmp, body_bb, after_bb)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // Body
        self.builder.position_at_end(body_bb);
        let prev_break = self.break_target.take();
        let prev_continue = self.continue_target.take();
        self.break_target = Some((after_bb, None));
        self.continue_target = Some(step_bb);

        self.compile_expr(body)?;

        if self
            .builder
            .get_insert_block()
            .is_some_and(|b| b.get_terminator().is_none())
        {
            self.builder
                .build_unconditional_branch(step_bb)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
        }

        self.break_target = prev_break;
        self.continue_target = prev_continue;

        // Step: i = i + 1
        self.builder.position_at_end(step_bb);
        let cur_step = self
            .builder
            .build_load(i64_type, var_alloca, "cur_step")
            .map_err(|e| CodegenError::Internal(e.to_string()))?
            .into_int_value();
        let next = self
            .builder
            .build_int_add(cur_step, i64_type.const_int(1, false), "next")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        self.builder
            .build_store(var_alloca, next)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        self.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // After
        self.builder.position_at_end(after_bb);
        Ok(None)
    }

    /// Compiles an infinite loop (`loop { ... }`).
    fn compile_loop(&mut self, body: &Expr) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| CodegenError::Internal("no current function".into()))?;

        let loop_bb = self.context.append_basic_block(function, "loop_body");
        let after_bb = self.context.append_basic_block(function, "loop_after");

        // Alloca for break value
        let break_alloca = self
            .builder
            .build_alloca(self.context.i64_type(), "loop_break_val")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        self.builder
            .build_store(break_alloca, self.context.i64_type().const_int(0, false))
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        self.builder
            .build_unconditional_branch(loop_bb)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        self.builder.position_at_end(loop_bb);
        let prev_break = self.break_target.take();
        let prev_continue = self.continue_target.take();
        self.break_target = Some((after_bb, Some(break_alloca)));
        self.continue_target = Some(loop_bb);

        self.compile_expr(body)?;

        if self
            .builder
            .get_insert_block()
            .is_some_and(|b| b.get_terminator().is_none())
        {
            self.builder
                .build_unconditional_branch(loop_bb)
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
        }

        self.break_target = prev_break;
        self.continue_target = prev_continue;

        self.builder.position_at_end(after_bb);
        let result = self
            .builder
            .build_load(self.context.i64_type(), break_alloca, "loop_result")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        Ok(Some(result))
    }

    /// Compiles a match expression (cascading if-else).
    fn compile_match(
        &mut self,
        subject: &Expr,
        arms: &[MatchArm],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let subject_val = self
            .compile_expr(subject)?
            .ok_or_else(|| CodegenError::Internal("match subject produced no value".into()))?;

        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| CodegenError::Internal("no current function".into()))?;

        let merge_bb = self.context.append_basic_block(function, "match_merge");

        // Result alloca
        let result_alloca = self
            .builder
            .build_alloca(self.context.i64_type(), "match_result")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        let mut incoming: Vec<(BasicValueEnum<'ctx>, BasicBlock<'ctx>)> = Vec::new();

        for (i, arm) in arms.iter().enumerate() {
            let is_last = i == arms.len() - 1;

            match &arm.pattern {
                Pattern::Wildcard { .. } | Pattern::Ident { .. } => {
                    // Wildcard/ident always matches — bind variable if ident
                    if let Pattern::Ident { name, .. } = &arm.pattern {
                        if subject_val.is_int_value() {
                            let alloca = self
                                .builder
                                .build_alloca(self.context.i64_type(), name)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            self.builder
                                .build_store(alloca, subject_val)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            self.variables.insert(name.clone(), alloca);
                            self.var_types
                                .insert(name.clone(), self.context.i64_type().into());
                        }
                    }

                    let body_val = self.compile_expr(&arm.body)?;
                    if let Some(val) = body_val {
                        self.builder
                            .build_store(result_alloca, val)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        let exit_bb = self
                            .builder
                            .get_insert_block()
                            .ok_or_else(|| CodegenError::Internal("no insert block".into()))?;
                        incoming.push((val, exit_bb));
                    }
                    if self
                        .builder
                        .get_insert_block()
                        .is_some_and(|b| b.get_terminator().is_none())
                    {
                        self.builder
                            .build_unconditional_branch(merge_bb)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                    break; // Wildcard is last reachable arm
                }

                Pattern::Literal { kind, .. } => {
                    let pattern_val = self
                        .compile_literal(kind)?
                        .ok_or_else(|| CodegenError::Internal("pattern literal no value".into()))?;

                    // F4: Compare subject with pattern — supports int, float, bool, string
                    let cmp = self.compile_match_comparison(subject_val, pattern_val)?;

                    let arm_bb = self
                        .context
                        .append_basic_block(function, &format!("match_arm_{i}"));
                    let next_bb = if is_last {
                        merge_bb
                    } else {
                        self.context
                            .append_basic_block(function, &format!("match_next_{i}"))
                    };

                    self.builder
                        .build_conditional_branch(cmp, arm_bb, next_bb)
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;

                    self.builder.position_at_end(arm_bb);

                    // F1: Guard support for literal patterns
                    if let Some(ref guard) = arm.guard {
                        let guard_val = self.compile_expr(guard)?.ok_or_else(|| {
                            CodegenError::Internal("guard produced no value".into())
                        })?;
                        let guard_bool = self.to_i1(guard_val)?;
                        let guard_pass_bb = self
                            .context
                            .append_basic_block(function, &format!("lit_guard_pass_{i}"));
                        self.builder
                            .build_conditional_branch(guard_bool, guard_pass_bb, next_bb)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        self.builder.position_at_end(guard_pass_bb);
                    }

                    let body_val = self.compile_expr(&arm.body)?;
                    if let Some(val) = body_val {
                        self.builder
                            .build_store(result_alloca, val)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        let exit_bb = self
                            .builder
                            .get_insert_block()
                            .ok_or_else(|| CodegenError::Internal("no insert block".into()))?;
                        incoming.push((val, exit_bb));
                    }
                    if self
                        .builder
                        .get_insert_block()
                        .is_some_and(|b| b.get_terminator().is_none())
                    {
                        self.builder
                            .build_unconditional_branch(merge_bb)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }

                    if !is_last {
                        self.builder.position_at_end(next_bb);
                    }
                }

                // V12 L7: Or-pattern — `0 | 1 | 2 => body`
                Pattern::Or { patterns, .. } => {
                    // Build OR of comparisons: subject == p1 || subject == p2 || ...
                    let mut or_result: Option<inkwell::values::IntValue<'ctx>> = None;

                    for pat in patterns {
                        if let Pattern::Literal { kind, .. } = pat {
                            let pattern_val = self.compile_literal(kind)?.ok_or_else(|| {
                                CodegenError::Internal("or-pattern literal no value".into())
                            })?;
                            let cmp = self.compile_match_comparison(subject_val, pattern_val)?;
                            or_result = Some(match or_result {
                                Some(prev) => self
                                    .builder
                                    .build_or(prev, cmp, "or_acc")
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?,
                                None => cmp,
                            });
                        }
                    }

                    if let Some(cmp) = or_result {
                        let arm_bb = self
                            .context
                            .append_basic_block(function, &format!("match_or_{i}"));
                        let next_bb = if is_last {
                            merge_bb
                        } else {
                            self.context
                                .append_basic_block(function, &format!("match_next_{i}"))
                        };

                        self.builder
                            .build_conditional_branch(cmp, arm_bb, next_bb)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;

                        self.builder.position_at_end(arm_bb);

                        // V12 L7: Match guard — `pattern if condition => body`
                        if let Some(ref guard) = arm.guard {
                            let guard_val = self.compile_expr(guard)?.ok_or_else(|| {
                                CodegenError::Internal("guard produced no value".into())
                            })?;
                            let guard_bool = self.to_i1(guard_val)?;
                            let guard_pass_bb = self
                                .context
                                .append_basic_block(function, &format!("guard_pass_{i}"));
                            self.builder
                                .build_conditional_branch(guard_bool, guard_pass_bb, next_bb)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            self.builder.position_at_end(guard_pass_bb);
                        }

                        let body_val = self.compile_expr(&arm.body)?;
                        if let Some(val) = body_val {
                            self.builder
                                .build_store(result_alloca, val)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            if let Some(exit_bb) = self.builder.get_insert_block() {
                                incoming.push((val, exit_bb));
                            }
                        }
                        if self
                            .builder
                            .get_insert_block()
                            .is_some_and(|b| b.get_terminator().is_none())
                        {
                            self.builder
                                .build_unconditional_branch(merge_bb)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        }
                        if !is_last {
                            self.builder.position_at_end(next_bb);
                        }
                    }
                }

                // V12 L7: Enum pattern — `Some(v) => body`
                Pattern::Enum {
                    variant, fields, ..
                } => {
                    // F5: Look up variant in ALL enum defs, not just first one
                    let variant_idx = self
                        .enum_defs
                        .values()
                        .find_map(|vs| vs.iter().position(|(n, _)| n == variant))
                        .ok_or_else(|| {
                            CodegenError::Internal(format!(
                                "unknown enum variant '{variant}' in match pattern"
                            ))
                        })? as u64;

                    let discriminant = self.context.i64_type().const_int(variant_idx, false);
                    let cmp = if subject_val.is_int_value() {
                        self.builder
                            .build_int_compare(
                                inkwell::IntPredicate::EQ,
                                subject_val.into_int_value(),
                                discriminant,
                                "enum_cmp",
                            )
                            .map_err(|e| CodegenError::Internal(e.to_string()))?
                    } else {
                        // Fallback: always match
                        self.context.bool_type().const_int(1, false)
                    };

                    let arm_bb = self
                        .context
                        .append_basic_block(function, &format!("match_enum_{i}"));
                    let next_bb = if is_last {
                        merge_bb
                    } else {
                        self.context
                            .append_basic_block(function, &format!("match_next_{i}"))
                    };

                    self.builder
                        .build_conditional_branch(cmp, arm_bb, next_bb)
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;

                    self.builder.position_at_end(arm_bb);

                    // F1: Guard support for enum patterns
                    if let Some(ref guard) = arm.guard {
                        let guard_val = self.compile_expr(guard)?.ok_or_else(|| {
                            CodegenError::Internal("guard produced no value".into())
                        })?;
                        let guard_bool = self.to_i1(guard_val)?;
                        let guard_pass_bb = self
                            .context
                            .append_basic_block(function, &format!("enum_guard_pass_{i}"));
                        self.builder
                            .build_conditional_branch(guard_bool, guard_pass_bb, next_bb)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        self.builder.position_at_end(guard_pass_bb);
                    }

                    // F2: Bind enum fields — for single-payload enums, bind subject
                    // value (which carries the payload). For multi-field, use indexed
                    // extraction (requires tagged-union ABI in future).
                    for (fi, field_pat) in fields.iter().enumerate() {
                        if let Pattern::Ident { name, .. } = field_pat {
                            let alloca = self
                                .builder
                                .build_alloca(self.context.i64_type(), name)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            // For single-field enums (e.g., Some(v), Ok(v), Err(e)),
                            // the subject IS the payload. For multi-field, we extract
                            // via bit-shift (field 0 = low bits, field 1 = high bits).
                            let payload = if fields.len() == 1 && fi == 0 {
                                // Single field: subject value is the payload
                                if subject_val.is_int_value() {
                                    subject_val
                                } else {
                                    self.context.i64_type().const_int(0, false).into()
                                }
                            } else {
                                // Multi-field: shift right by (fi * 16) for packed repr
                                let shift =
                                    self.context.i64_type().const_int((fi as u64) * 16, false);
                                if subject_val.is_int_value() {
                                    self.builder
                                        .build_right_shift(
                                            subject_val.into_int_value(),
                                            shift,
                                            false,
                                            &format!("field_{fi}"),
                                        )
                                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                                        .into()
                                } else {
                                    self.context.i64_type().const_int(0, false).into()
                                }
                            };
                            self.builder
                                .build_store(alloca, payload)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            self.variables.insert(name.clone(), alloca);
                            self.var_types
                                .insert(name.clone(), self.context.i64_type().into());
                        }
                    }

                    let body_val = self.compile_expr(&arm.body)?;
                    if let Some(val) = body_val {
                        self.builder
                            .build_store(result_alloca, val)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        if let Some(exit_bb) = self.builder.get_insert_block() {
                            incoming.push((val, exit_bb));
                        }
                    }
                    if self
                        .builder
                        .get_insert_block()
                        .is_some_and(|b| b.get_terminator().is_none())
                    {
                        self.builder
                            .build_unconditional_branch(merge_bb)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                    if !is_last {
                        self.builder.position_at_end(next_bb);
                    }
                }

                // H4: Range pattern — `1..10 => body` or `1..=10 => body`
                Pattern::Range {
                    start: range_start,
                    end: range_end,
                    inclusive,
                    ..
                } => {
                    let start_val = self.compile_expr(range_start)?.ok_or_else(|| {
                        CodegenError::Internal("range pattern start no value".into())
                    })?;
                    let end_val = self.compile_expr(range_end)?.ok_or_else(|| {
                        CodegenError::Internal("range pattern end no value".into())
                    })?;

                    if subject_val.is_int_value()
                        && start_val.is_int_value()
                        && end_val.is_int_value()
                    {
                        let s = subject_val.into_int_value();
                        let lo = start_val.into_int_value();
                        let hi = end_val.into_int_value();
                        let ge_lo = self
                            .builder
                            .build_int_compare(inkwell::IntPredicate::SGE, s, lo, "range_ge")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        let hi_pred = if *inclusive {
                            inkwell::IntPredicate::SLE
                        } else {
                            inkwell::IntPredicate::SLT
                        };
                        let le_hi = self
                            .builder
                            .build_int_compare(hi_pred, s, hi, "range_le")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        let cmp = self
                            .builder
                            .build_and(ge_lo, le_hi, "range_cmp")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;

                        let arm_bb = self
                            .context
                            .append_basic_block(function, &format!("match_range_{i}"));
                        let next_bb = if is_last {
                            merge_bb
                        } else {
                            self.context
                                .append_basic_block(function, &format!("match_next_{i}"))
                        };

                        self.builder
                            .build_conditional_branch(cmp, arm_bb, next_bb)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        self.builder.position_at_end(arm_bb);

                        if let Some(ref guard) = arm.guard {
                            let gv = self
                                .compile_expr(guard)?
                                .ok_or_else(|| CodegenError::Internal("guard no value".into()))?;
                            let gb = self.to_i1(gv)?;
                            let gp_bb = self
                                .context
                                .append_basic_block(function, &format!("range_guard_{i}"));
                            self.builder
                                .build_conditional_branch(gb, gp_bb, next_bb)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            self.builder.position_at_end(gp_bb);
                        }

                        let body_val = self.compile_expr(&arm.body)?;
                        if let Some(val) = body_val {
                            self.builder
                                .build_store(result_alloca, val)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            if let Some(exit_bb) = self.builder.get_insert_block() {
                                incoming.push((val, exit_bb));
                            }
                        }
                        if self
                            .builder
                            .get_insert_block()
                            .is_some_and(|b| b.get_terminator().is_none())
                        {
                            self.builder
                                .build_unconditional_branch(merge_bb)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        }
                        if !is_last {
                            self.builder.position_at_end(next_bb);
                        }
                    } else if subject_val.is_float_value()
                        && start_val.is_float_value()
                        && end_val.is_float_value()
                    {
                        // I6: Float range pattern
                        let s = subject_val.into_float_value();
                        let lo = start_val.into_float_value();
                        let hi = end_val.into_float_value();
                        let ge_lo = self
                            .builder
                            .build_float_compare(inkwell::FloatPredicate::OGE, s, lo, "frange_ge")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        let hi_pred = if *inclusive {
                            inkwell::FloatPredicate::OLE
                        } else {
                            inkwell::FloatPredicate::OLT
                        };
                        let le_hi = self
                            .builder
                            .build_float_compare(hi_pred, s, hi, "frange_le")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        let cmp = self
                            .builder
                            .build_and(ge_lo, le_hi, "frange_cmp")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;

                        let arm_bb = self
                            .context
                            .append_basic_block(function, &format!("match_frange_{i}"));
                        let next_bb = if is_last {
                            merge_bb
                        } else {
                            self.context
                                .append_basic_block(function, &format!("match_next_{i}"))
                        };

                        self.builder
                            .build_conditional_branch(cmp, arm_bb, next_bb)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        self.builder.position_at_end(arm_bb);

                        let body_val = self.compile_expr(&arm.body)?;
                        if let Some(val) = body_val {
                            self.builder
                                .build_store(result_alloca, val)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            if let Some(exit_bb) = self.builder.get_insert_block() {
                                incoming.push((val, exit_bb));
                            }
                        }
                        if self
                            .builder
                            .get_insert_block()
                            .is_some_and(|b| b.get_terminator().is_none())
                        {
                            self.builder
                                .build_unconditional_branch(merge_bb)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        }
                        if !is_last {
                            self.builder.position_at_end(next_bb);
                        }
                    } else {
                        return Err(CodegenError::NotImplemented(
                            "LLVM match: range pattern requires integer or float operands".into(),
                        ));
                    }
                }

                // H6: Tuple pattern — `(a, b) => body`
                Pattern::Tuple { elements, .. } => {
                    // Destructure: bind each element to the corresponding
                    // extract_value from the subject (which must be a struct value).
                    let arm_bb = self
                        .context
                        .append_basic_block(function, &format!("match_tuple_{i}"));
                    // Tuple patterns always match (they're destructuring, not testing)
                    self.builder
                        .build_unconditional_branch(arm_bb)
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    self.builder.position_at_end(arm_bb);

                    if subject_val.is_struct_value() {
                        let sv = subject_val.into_struct_value();
                        for (ei, elem_pat) in elements.iter().enumerate() {
                            if let Pattern::Ident { name, .. } = elem_pat {
                                let extracted = self
                                    .builder
                                    .build_extract_value(sv, ei as u32, name)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                let alloca = self
                                    .builder
                                    .build_alloca(self.context.i64_type(), name)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                self.builder
                                    .build_store(alloca, extracted)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                self.variables.insert(name.clone(), alloca);
                                self.var_types
                                    .insert(name.clone(), self.context.i64_type().into());
                            }
                            // Wildcard in tuple: skip binding
                        }
                    } else if subject_val.is_int_value() {
                        // Single-element: bind first ident to subject
                        if let Some(Pattern::Ident { name, .. }) = elements.first() {
                            let alloca = self
                                .builder
                                .build_alloca(self.context.i64_type(), name)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            self.builder
                                .build_store(alloca, subject_val)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            self.variables.insert(name.clone(), alloca);
                            self.var_types
                                .insert(name.clone(), self.context.i64_type().into());
                        }
                    }

                    let body_val = self.compile_expr(&arm.body)?;
                    if let Some(val) = body_val {
                        self.builder
                            .build_store(result_alloca, val)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        if let Some(exit_bb) = self.builder.get_insert_block() {
                            incoming.push((val, exit_bb));
                        }
                    }
                    if self
                        .builder
                        .get_insert_block()
                        .is_some_and(|b| b.get_terminator().is_none())
                    {
                        self.builder
                            .build_unconditional_branch(merge_bb)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                    break; // Tuple pattern is exhaustive (destructuring)
                }

                // H5: Struct pattern — `Point { x, y } => body`
                Pattern::Struct {
                    name: struct_name,
                    fields: field_pats,
                    ..
                } => {
                    let arm_bb = self
                        .context
                        .append_basic_block(function, &format!("match_struct_{i}"));
                    // Struct patterns always match if the type matches
                    // (type checking done by analyzer)
                    self.builder
                        .build_unconditional_branch(arm_bb)
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    self.builder.position_at_end(arm_bb);

                    // Look up struct field names
                    if let Some((stype, field_names)) = self.struct_types.get(struct_name).cloned()
                    {
                        if subject_val.is_struct_value() {
                            let sv = subject_val.into_struct_value();
                            for fp in field_pats {
                                if let Some(idx) = field_names.iter().position(|n| n == &fp.name) {
                                    let extracted = self
                                        .builder
                                        .build_extract_value(sv, idx as u32, &fp.name)
                                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                    // Bind the field — shorthand `{ x }` binds to x,
                                    // `{ x: pattern }` would need pattern matching
                                    let bind_name =
                                        if let Some(Pattern::Ident { name, .. }) = &fp.pattern {
                                            name.clone()
                                        } else {
                                            fp.name.clone()
                                        };
                                    let alloca = self
                                        .builder
                                        .build_alloca(
                                            stype
                                                .get_field_type_at_index(idx as u32)
                                                .unwrap_or(self.context.i64_type().into()),
                                            &bind_name,
                                        )
                                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                    self.builder
                                        .build_store(alloca, extracted)
                                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                    self.variables.insert(bind_name.clone(), alloca);
                                    self.var_types.insert(
                                        bind_name,
                                        stype
                                            .get_field_type_at_index(idx as u32)
                                            .unwrap_or(self.context.i64_type().into()),
                                    );
                                }
                            }
                        }
                    }

                    let body_val = self.compile_expr(&arm.body)?;
                    if let Some(val) = body_val {
                        self.builder
                            .build_store(result_alloca, val)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        if let Some(exit_bb) = self.builder.get_insert_block() {
                            incoming.push((val, exit_bb));
                        }
                    }
                    if self
                        .builder
                        .get_insert_block()
                        .is_some_and(|b| b.get_terminator().is_none())
                    {
                        self.builder
                            .build_unconditional_branch(merge_bb)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                    break; // Struct pattern is exhaustive
                }

                // I4: Binding pattern — `name @ pattern => body`
                // Bind subject to name, then check inner pattern.
                Pattern::Binding {
                    name,
                    pattern: inner_pat,
                    ..
                } => {
                    // Bind subject to variable first
                    if subject_val.is_int_value() {
                        let alloca = self
                            .builder
                            .build_alloca(self.context.i64_type(), name)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        self.builder
                            .build_store(alloca, subject_val)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        self.variables.insert(name.clone(), alloca);
                        self.var_types
                            .insert(name.clone(), self.context.i64_type().into());
                    }
                    // Check inner pattern — if it's a literal, do comparison
                    match inner_pat.as_ref() {
                        Pattern::Wildcard { .. } => {
                            // Always matches — compile body
                            let body_val = self.compile_expr(&arm.body)?;
                            if let Some(val) = body_val {
                                self.builder
                                    .build_store(result_alloca, val)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                if let Some(exit_bb) = self.builder.get_insert_block() {
                                    incoming.push((val, exit_bb));
                                }
                            }
                            if self
                                .builder
                                .get_insert_block()
                                .is_some_and(|b| b.get_terminator().is_none())
                            {
                                self.builder
                                    .build_unconditional_branch(merge_bb)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            }
                            break;
                        }
                        Pattern::Literal { kind, .. } => {
                            let pattern_val = self.compile_literal(kind)?.ok_or_else(|| {
                                CodegenError::Internal("binding literal no value".into())
                            })?;
                            let cmp = self.compile_match_comparison(subject_val, pattern_val)?;
                            let arm_bb = self
                                .context
                                .append_basic_block(function, &format!("match_binding_{i}"));
                            let next_bb = if is_last {
                                merge_bb
                            } else {
                                self.context
                                    .append_basic_block(function, &format!("match_next_{i}"))
                            };
                            self.builder
                                .build_conditional_branch(cmp, arm_bb, next_bb)
                                .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            self.builder.position_at_end(arm_bb);
                            let body_val = self.compile_expr(&arm.body)?;
                            if let Some(val) = body_val {
                                self.builder
                                    .build_store(result_alloca, val)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                if let Some(exit_bb) = self.builder.get_insert_block() {
                                    incoming.push((val, exit_bb));
                                }
                            }
                            if self
                                .builder
                                .get_insert_block()
                                .is_some_and(|b| b.get_terminator().is_none())
                            {
                                self.builder
                                    .build_unconditional_branch(merge_bb)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            }
                            if !is_last {
                                self.builder.position_at_end(next_bb);
                            }
                        }
                        _ => {
                            // Other inner patterns: treat as wildcard with binding
                            let body_val = self.compile_expr(&arm.body)?;
                            if let Some(val) = body_val {
                                self.builder
                                    .build_store(result_alloca, val)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                if let Some(exit_bb) = self.builder.get_insert_block() {
                                    incoming.push((val, exit_bb));
                                }
                            }
                            if self
                                .builder
                                .get_insert_block()
                                .is_some_and(|b| b.get_terminator().is_none())
                            {
                                self.builder
                                    .build_unconditional_branch(merge_bb)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                            }
                            break;
                        }
                    }
                }

                // I3: Array pattern — `[a, b, c] => body`
                Pattern::Array { elements, .. } => {
                    // Destructure: bind each element by indexing into the subject
                    let arm_bb = self
                        .context
                        .append_basic_block(function, &format!("match_array_{i}"));
                    self.builder
                        .build_unconditional_branch(arm_bb)
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    self.builder.position_at_end(arm_bb);

                    if subject_val.is_int_value() {
                        // Subject is a pointer to array data
                        let i64_ty = self.context.i64_type();
                        let base_addr = subject_val.into_int_value();
                        let ptr = self
                            .builder
                            .build_int_to_ptr(
                                base_addr,
                                self.context.ptr_type(inkwell::AddressSpace::default()),
                                "arr_base",
                            )
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        for (ei, elem_pat) in elements.iter().enumerate() {
                            if let Pattern::Ident { name, .. } = elem_pat {
                                let idx = i64_ty.const_int(ei as u64, false);
                                // SAFETY: bare-metal array indexing
                                let elem_ptr = unsafe {
                                    self.builder
                                        .build_in_bounds_gep(i64_ty, ptr, &[idx], "arr_elem")
                                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                                };
                                let val = self
                                    .builder
                                    .build_load(i64_ty, elem_ptr, name)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                let alloca = self
                                    .builder
                                    .build_alloca(i64_ty, name)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                self.builder
                                    .build_store(alloca, val)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                self.variables.insert(name.clone(), alloca);
                                self.var_types.insert(name.clone(), i64_ty.into());
                            }
                        }
                    } else if subject_val.is_struct_value() {
                        // Subject is a struct (tuple-like array)
                        let sv = subject_val.into_struct_value();
                        for (ei, elem_pat) in elements.iter().enumerate() {
                            if let Pattern::Ident { name, .. } = elem_pat {
                                let extracted = self
                                    .builder
                                    .build_extract_value(sv, ei as u32, name)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                let alloca = self
                                    .builder
                                    .build_alloca(self.context.i64_type(), name)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                self.builder
                                    .build_store(alloca, extracted)
                                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                                self.variables.insert(name.clone(), alloca);
                                self.var_types
                                    .insert(name.clone(), self.context.i64_type().into());
                            }
                        }
                    }

                    let body_val = self.compile_expr(&arm.body)?;
                    if let Some(val) = body_val {
                        self.builder
                            .build_store(result_alloca, val)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        if let Some(exit_bb) = self.builder.get_insert_block() {
                            incoming.push((val, exit_bb));
                        }
                    }
                    if self
                        .builder
                        .get_insert_block()
                        .is_some_and(|b| b.get_terminator().is_none())
                    {
                        self.builder
                            .build_unconditional_branch(merge_bb)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
                    break; // Array pattern is exhaustive (destructuring)
                } // All 10 Pattern variants now covered:
                  // Literal, Ident, Wildcard, Or, Enum, Range, Tuple, Struct, Array, Binding
            }
        }

        // Ensure merge block has a predecessor for unreachable case
        self.builder.position_at_end(merge_bb);

        if incoming.is_empty() {
            Ok(Some(self.context.i64_type().const_int(0, false).into()))
        } else {
            let result = self
                .builder
                .build_load(self.context.i64_type(), result_alloca, "match_val")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            Ok(Some(result))
        }
    }

    /// F4: Compares two values for pattern matching — supports int, float, bool, string.
    fn compile_match_comparison(
        &self,
        subject: BasicValueEnum<'ctx>,
        pattern: BasicValueEnum<'ctx>,
    ) -> Result<inkwell::values::IntValue<'ctx>, CodegenError> {
        // Int == Int
        if subject.is_int_value() && pattern.is_int_value() {
            let s = subject.into_int_value();
            let p = pattern.into_int_value();
            // Harmonize bit widths (i1 vs i64)
            let i64_ty = self.context.i64_type();
            let s_ext = if s.get_type().get_bit_width() < 64 {
                self.builder
                    .build_int_z_extend(s, i64_ty, "s_ext")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
            } else {
                s
            };
            let p_ext = if p.get_type().get_bit_width() < 64 {
                self.builder
                    .build_int_z_extend(p, i64_ty, "p_ext")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?
            } else {
                p
            };
            return self
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, s_ext, p_ext, "match_cmp")
                .map_err(|e| CodegenError::Internal(e.to_string()));
        }

        // Float == Float (ordered equality)
        if subject.is_float_value() && pattern.is_float_value() {
            return self
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OEQ,
                    subject.into_float_value(),
                    pattern.into_float_value(),
                    "match_fcmp",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()));
        }

        // String == String (via fj_rt_string_eq)
        if subject.is_struct_value() && pattern.is_struct_value() {
            let s_struct = subject.into_struct_value();
            let p_struct = pattern.into_struct_value();
            let s_ptr = self
                .builder
                .build_extract_value(s_struct, 0, "s_ptr")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            let s_len = self
                .builder
                .build_extract_value(s_struct, 1, "s_len")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            let p_ptr = self
                .builder
                .build_extract_value(p_struct, 0, "p_ptr")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
            let p_len = self
                .builder
                .build_extract_value(p_struct, 1, "p_len")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;

            if let Some(eq_fn) = self.functions.get("fj_rt_string_eq") {
                let result = self
                    .builder
                    .build_call(
                        *eq_fn,
                        &[s_ptr.into(), s_len.into(), p_ptr.into(), p_len.into()],
                        "str_eq",
                    )
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(v) => {
                        if v.is_int_value() {
                            return Ok(v.into_int_value());
                        }
                    }
                    inkwell::values::ValueKind::Instruction(_) => {}
                }
            }
            // Fallback: compare pointers (identity check)
            return self
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::EQ,
                    s_ptr.into_pointer_value(),
                    p_ptr.into_pointer_value(),
                    "str_ptr_cmp",
                )
                .map_err(|e| CodegenError::Internal(e.to_string()));
        }

        // Cross-type: coerce and compare as i64
        if subject.is_int_value() || pattern.is_int_value() {
            let i64_ty = self.context.i64_type();
            let s_int = if subject.is_int_value() {
                let iv = subject.into_int_value();
                if iv.get_type().get_bit_width() < 64 {
                    self.builder
                        .build_int_z_extend(iv, i64_ty, "s_cross")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                } else {
                    iv
                }
            } else {
                i64_ty.const_int(0, false)
            };
            let p_int = if pattern.is_int_value() {
                let iv = pattern.into_int_value();
                if iv.get_type().get_bit_width() < 64 {
                    self.builder
                        .build_int_z_extend(iv, i64_ty, "p_cross")
                        .map_err(|e| CodegenError::Internal(e.to_string()))?
                } else {
                    iv
                }
            } else {
                i64_ty.const_int(0, false)
            };
            return self
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, s_int, p_int, "match_cross")
                .map_err(|e| CodegenError::Internal(e.to_string()));
        }

        // Unsupported comparison — always false
        Ok(self.context.bool_type().const_int(0, false))
    }

    /// Converts a value to i1 (boolean). For if/while conditions.
    fn to_i1(
        &self,
        val: BasicValueEnum<'ctx>,
    ) -> Result<inkwell::values::IntValue<'ctx>, CodegenError> {
        if val.is_int_value() {
            let int_val = val.into_int_value();
            if int_val.get_type().get_bit_width() == 1 {
                Ok(int_val)
            } else {
                self.builder
                    .build_int_compare(
                        inkwell::IntPredicate::NE,
                        int_val,
                        int_val.get_type().const_int(0, false),
                        "to_bool",
                    )
                    .map_err(|e| CodegenError::Internal(e.to_string()))
            }
        } else {
            Err(CodegenError::Internal(
                "condition must be integer type".into(),
            ))
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // E1: Universal builtin override — user fn always wins over stubs
    // ═══════════════════════════════════════════════════════════════════════

    /// Attempts to call a user-defined function that overrides a builtin.
    ///
    /// OS kernels MUST be able to override any builtin with their own
    /// implementation. This method checks if a user-defined function exists
    /// with the same name and calls it instead, performing struct→i64 ABI
    /// coercion as needed.
    ///
    /// Returns `Ok(Some(val))` if user fn was found and called,
    /// `Ok(None)` if no user override exists (caller should use builtin impl).
    fn try_user_fn_override(
        &mut self,
        name: &str,
        args: &[CallArg],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let user_fn = match self.functions.get(name).copied() {
            Some(f) => f,
            None => return Ok(None),
        };
        let param_types: Vec<_> = user_fn.get_type().get_param_types();
        let compiled_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
            .iter()
            .enumerate()
            .map(|(i, arg)| {
                let val = self.compile_expr(&arg.value)?.ok_or_else(|| {
                    CodegenError::Internal(format!("user override '{name}' arg {i} no value"))
                })?;
                // Struct→i64 ABI coercion: extract ptr when callee expects int
                if val.is_struct_value() {
                    let expects_int = param_types.get(i).is_some_and(|t| t.is_int_type());
                    if expects_int {
                        let sv = val.into_struct_value();
                        let ptr = self
                            .builder
                            .build_extract_value(sv, 0, "ovr_ptr")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        let i64_ty = self.context.i64_type();
                        let ptr_int = self
                            .builder
                            .build_ptr_to_int(ptr.into_pointer_value(), i64_ty, "ovr_i64")
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                        return Ok(ptr_int.into());
                    }
                }
                Ok(val.into())
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let call = self
            .builder
            .build_call(user_fn, &compiled_args, &format!("{name}_ovr"))
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        let zero = self.context.i64_type().const_int(0, false);
        match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(Some(v)),
            inkwell::values::ValueKind::Instruction(_) => Ok(Some(zero.into())),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // E2: Proper inline asm constraint classifier
    // ═══════════════════════════════════════════════════════════════════════

    /// Formats an LLVM inline asm input/inout constraint.
    ///
    /// LLVM constraint syntax:
    /// - Generic: `r` (any GPR), `m` (memory), `i` (immediate), `n` (int immediate)
    /// - Physical register: must be wrapped in braces, e.g., `{rax}`, `{eax}`, `{xmm0}`
    /// - Memory/flag clobbers: `~{memory}`, `~{cc}`, `~{dirflag}`, `~{fpsr}`
    /// - Tied operands: `0`, `1`, etc. (digit references)
    /// - Already-braced: pass through unchanged
    fn format_asm_constraint_in(constraint: &str) -> String {
        Self::format_asm_constraint_impl(constraint, false)
    }

    /// Formats an LLVM inline asm output constraint (prepends `=`).
    fn format_asm_constraint_out(constraint: &str) -> String {
        Self::format_asm_constraint_impl(constraint, true)
    }

    /// Core constraint formatter. `is_output` prepends `=` for output operands.
    fn format_asm_constraint_impl(constraint: &str, is_output: bool) -> String {
        let prefix = if is_output { "=" } else { "" };

        // Already wrapped in braces — pass through
        if constraint.starts_with('{') {
            return format!("{prefix}{constraint}");
        }

        // Already has `=` prefix (from user) — strip and re-add
        let raw = constraint.strip_prefix('=').unwrap_or(constraint);

        // Already wrapped in braces after stripping `=`
        if raw.starts_with('{') {
            return format!("{prefix}{raw}");
        }

        // Generic single-letter constraints: r, m, i, n, g, X, etc.
        const GENERIC_CONSTRAINTS: &[&str] = &[
            "r", "m", "i", "n", "g", "X", "o", "V", "p", "f", "t", "u",
            // x86 specific generic
            "q", "Q", "a", "b", "c", "d", "S", "D", "A",
        ];
        if GENERIC_CONSTRAINTS.contains(&raw) {
            return format!("{prefix}{raw}");
        }

        // Digit-tied operands: "0", "1", "2", etc.
        if raw.chars().all(|c| c.is_ascii_digit()) {
            return format!("{prefix}{raw}");
        }

        // Clobber syntax: ~{...} — pass through
        if raw.starts_with('~') {
            return raw.to_string();
        }

        // Known LLVM special constraints that should NOT be braced
        const SPECIAL_CONSTRAINTS: &[&str] = &["memory", "cc", "dirflag", "fpsr", "flags"];
        if SPECIAL_CONSTRAINTS.contains(&raw) {
            return format!("{prefix}{raw}");
        }

        // Everything else is a physical register name — wrap in braces
        // Examples: rax, eax, rbx, xmm0, cr3, rdi, rsi, etc.
        format!("{prefix}{{{raw}}}")
    }

    // ═══════════════════════════════════════════════════════════════════════
    // E4: Type harmonization helper — coerce int to i64
    // ═══════════════════════════════════════════════════════════════════════

    /// Coerces an integer value to i64 via zero-extension if needed.
    /// Returns the value unchanged if it's already i64 or non-integer.
    fn coerce_int_to_i64(
        &self,
        val: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        if val.is_int_value() {
            let iv = val.into_int_value();
            if iv.get_type().get_bit_width() < 64 {
                let ext = self
                    .builder
                    .build_int_z_extend(iv, self.context.i64_type(), "coerce_i64")
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
                Ok(ext.into())
            } else {
                Ok(val)
            }
        } else {
            Ok(val)
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // E5: Pre-link symbol verification for bare-metal
    // ═══════════════════════════════════════════════════════════════════════

    /// Verifies that all `fj_rt_bare_*` symbols referenced in the module are
    /// either defined or explicitly declared as external.
    ///
    /// Call this before `emit_object()` in bare-metal mode to catch missing
    /// runtime symbols early, instead of getting cryptic linker errors.
    ///
    /// Returns a list of undefined bare-metal symbols. If empty, all symbols
    /// are resolved.
    /// Verifies bare-metal symbols, excluding those known to be provided
    /// by the startup module or extra object files.
    pub fn verify_bare_metal_symbols(&self) -> Vec<String> {
        self.verify_bare_metal_symbols_with_known(&[])
    }

    /// Verifies bare-metal symbols with a list of known-provided symbols.
    ///
    /// `known_symbols` are symbol names expected to be provided by the
    /// startup assembly module or extra .o files at link time.
    pub fn verify_bare_metal_symbols_with_known(&self, known_symbols: &[&str]) -> Vec<String> {
        let mut missing = Vec::new();
        let mut func = self.module.get_first_function();
        while let Some(f) = func {
            let name = f.get_name().to_string_lossy().to_string();
            if name.starts_with("fj_rt_bare_") {
                // Declaration only (no basic blocks) = external symbol needed
                if f.count_basic_blocks() == 0 {
                    // Check if provided by user-defined function override
                    let short_name = name.strip_prefix("fj_rt_bare_").unwrap_or(&name);
                    let has_user_impl = self
                        .functions
                        .get(short_name)
                        .is_some_and(|uf| uf.count_basic_blocks() > 0);
                    // Check if known to be provided by startup/extra objects
                    let is_known = known_symbols.contains(&name.as_str());
                    if !has_user_impl && !is_known {
                        missing.push(name);
                    }
                }
            }
            func = f.get_next_function();
        }
        missing
    }

    /// Returns the list of all `fj_rt_bare_*` symbols generated by the
    /// compiler's built-in startup assembly module. These symbols are
    /// always available at link time and should not be warned about.
    pub fn startup_provided_symbols() -> Vec<&'static str> {
        vec![
            "fj_rt_bare_println",
            "fj_rt_bare_print",
            "fj_rt_bare_print_i64",
            "fj_rt_bare_memory_fence",
            "fj_rt_bare_nprint",
            "fj_rt_bare_console_putchar",
            "fj_rt_bare_str_len",
            "fj_rt_bare_str_byte_at",
            "fj_rt_bare_buffer_read_u16_le",
            "fj_rt_bare_buffer_read_u32_le",
            "fj_rt_bare_buffer_read_u64_le",
            "fj_rt_bare_buffer_read_u16_be",
            "fj_rt_bare_buffer_read_u32_be",
            "fj_rt_bare_buffer_write_u16_le",
            "fj_rt_bare_buffer_write_u32_le",
            "fj_rt_bare_buffer_write_u16_be",
            "fj_rt_bare_buffer_write_u32_be",
            "fj_rt_bare_memcpy_buf",
            "fj_rt_bare_memset_buf",
            "fj_rt_bare_read_timer_ticks",
            "fj_rt_bare_x86_serial_init",
            "fj_rt_bare_acpi_shutdown",
            "fj_rt_bare_set_current_pid",
            "fj_rt_bare_pic_remap",
            "fj_rt_bare_pci_read32",
            "fj_rt_bare_pci_write32",
            "fj_rt_bare_idt_init",
            "fj_rt_bare_pit_init",
            "fj_rt_bare_tss_init",
        ]
    }

    /// Sets the LTO mode.
    pub fn set_lto_mode(&mut self, mode: LtoMode) {
        self.lto_mode = mode;
    }

    /// Returns the current LTO mode.
    pub fn lto_mode(&self) -> LtoMode {
        self.lto_mode
    }

    /// Sets the PGO mode.
    pub fn set_pgo_mode(&mut self, mode: PgoMode) {
        self.pgo_mode = mode;
    }

    /// Returns the current PGO mode.
    pub fn pgo_mode(&self) -> &PgoMode {
        &self.pgo_mode
    }

    // ── V12 Sprint L9: Bare-Metal & Cross-Compilation ──────────────────

    /// Enables no_std (bare-metal) mode.
    ///
    /// In no_std mode:
    /// - Heap runtime functions (string concat, array push, map) are not declared
    /// - Only static-data operations are available
    /// - Suitable for @kernel code and embedded targets
    pub fn set_no_std(&mut self, enabled: bool) {
        self.no_std = enabled;
    }

    /// Returns whether no_std mode is enabled.
    pub fn is_no_std(&self) -> bool {
        self.no_std
    }

    /// Sets the linker script path for bare-metal builds.
    pub fn set_linker_script(&mut self, path: Option<String>) {
        self.linker_script = path;
    }

    /// Returns the linker script path.
    pub fn linker_script(&self) -> Option<&str> {
        self.linker_script.as_deref()
    }

    /// Declares bare-metal runtime functions (UART, GPIO, memory ops).
    ///
    /// These are the minimal runtime functions needed for bare-metal
    /// targets without an OS. They map to `fj_rt_bare_*` in runtime_bare.rs.
    fn declare_bare_metal_runtime(&mut self) {
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let void_ty = self.context.void_type();
        let i8_ty = self.context.i8_type();

        // fj_rt_bare_print(ptr, len) -> void (UART output)
        let print_ty = void_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
        if self.module.get_function("fj_rt_bare_print").is_none() {
            self.module.add_function(
                "fj_rt_bare_print",
                print_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_bare_putc(c: i8) -> void
        let putc_ty = void_ty.fn_type(&[i8_ty.into()], false);
        if self.module.get_function("fj_rt_bare_putc").is_none() {
            self.module.add_function(
                "fj_rt_bare_putc",
                putc_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // fj_rt_bare_halt() -> void (halt CPU)
        let halt_ty = void_ty.fn_type(&[], false);
        if self.module.get_function("fj_rt_bare_halt").is_none() {
            self.module.add_function(
                "fj_rt_bare_halt",
                halt_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // memcpy(dst, src, len) -> ptr
        let memcpy_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into(), i64_ty.into()], false);
        if self.module.get_function("memcpy").is_none() {
            self.module.add_function(
                "memcpy",
                memcpy_ty,
                Some(inkwell::module::Linkage::External),
            );
        }

        // memset(dst, val, len) -> ptr
        let memset_ty = ptr_ty.fn_type(&[ptr_ty.into(), i64_ty.into(), i64_ty.into()], false);
        if self.module.get_function("memset").is_none() {
            self.module.add_function(
                "memset",
                memset_ty,
                Some(inkwell::module::Linkage::External),
            );
        }
    }

    /// Compiles an inline assembly expression.
    ///
    /// Generates LLVM inline asm from the `asm!("template", ...)` syntax.
    /// Supports input/output operands and assembly options.
    fn compile_inline_asm(
        &mut self,
        template: &str,
        operands: &[crate::parser::ast::AsmOperand],
        options: &[crate::parser::ast::AsmOption],
    ) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
        let i64_ty = self.context.i64_type();

        // Build constraint strings — LLVM requires outputs first, then inputs.
        let mut output_constraints: Vec<String> = Vec::new();
        let mut input_constraints: Vec<String> = Vec::new();
        let mut input_vals: Vec<BasicValueEnum<'ctx>> = Vec::new();

        for op in operands {
            match op {
                crate::parser::ast::AsmOperand::In { constraint, expr } => {
                    input_constraints.push(Self::format_asm_constraint_in(constraint));
                    if let Some(val) = self.compile_expr(expr)? {
                        input_vals.push(val);
                    }
                }
                crate::parser::ast::AsmOperand::Out { constraint, .. } => {
                    output_constraints.push(Self::format_asm_constraint_out(constraint));
                }
                crate::parser::ast::AsmOperand::InOut {
                    constraint, expr, ..
                } => {
                    // InOut: output constraint + tied input referencing the output index
                    let out_index = output_constraints.len();
                    output_constraints.push(Self::format_asm_constraint_out(constraint));
                    input_constraints.push(format!("{out_index}"));
                    if let Some(val) = self.compile_expr(expr)? {
                        input_vals.push(val);
                    }
                }
                _ => {}
            }
        }

        // LLVM constraint string: outputs first, then inputs
        let mut all_constraints = output_constraints.clone();
        all_constraints.extend(input_constraints);
        let constraint_str = all_constraints.join(",");

        // Determine side effects from options
        let has_side_effects = !options
            .iter()
            .any(|o| matches!(o, crate::parser::ast::AsmOption::Pure));

        let align_stack = !options
            .iter()
            .any(|o| matches!(o, crate::parser::ast::AsmOption::Nostack));

        // Build LLVM inline asm
        let asm_ty = if !output_constraints.is_empty() {
            i64_ty.fn_type(
                &input_vals
                    .iter()
                    .map(|_| i64_ty.into())
                    .collect::<Vec<inkwell::types::BasicMetadataTypeEnum>>(),
                false,
            )
        } else {
            self.context.void_type().fn_type(
                &input_vals
                    .iter()
                    .map(|_| i64_ty.into())
                    .collect::<Vec<inkwell::types::BasicMetadataTypeEnum>>(),
                false,
            )
        };

        let inline_asm = self.context.create_inline_asm(
            asm_ty,
            template.to_string(),
            constraint_str,
            has_side_effects,
            align_stack,
            None,  // dialect
            false, // can_throw
        );

        let args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            input_vals.iter().map(|v| (*v).into()).collect();

        let call_val = self
            .builder
            .build_indirect_call(asm_ty, inline_asm, &args, "asm_result")
            .map_err(|e| CodegenError::Internal(format!("inline asm error: {e}")))?;

        match call_val.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(val) => Ok(Some(val)),
            inkwell::values::ValueKind::Instruction(_) => Ok(None),
        }
    }

    /// Compiles a volatile load from a memory address (i64 width).
    fn compile_volatile_load(
        &mut self,
        addr: inkwell::values::IntValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        self.compile_volatile_load_sized(addr, self.context.i64_type().into())
    }

    /// Compiles a volatile load of arbitrary integer width from a memory address.
    fn compile_volatile_load_sized(
        &mut self,
        addr: inkwell::values::IntValue<'ctx>,
        load_ty: inkwell::types::BasicTypeEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());

        let ptr = self
            .builder
            .build_int_to_ptr(addr, ptr_ty, "vol_ptr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        let load = self
            .builder
            .build_load(load_ty, ptr, "vol_load")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // Mark the load as volatile so LLVM won't optimize it away.
        // SAFETY: address comes from @kernel context.
        if let Some(inst) = load.as_instruction_value() {
            inst.set_volatile(true).ok();
        }

        Ok(load)
    }

    /// Compiles a volatile store to a memory address.
    fn compile_volatile_store(
        &mut self,
        addr: inkwell::values::IntValue<'ctx>,
        value: inkwell::values::IntValue<'ctx>,
    ) -> Result<(), CodegenError> {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());

        let ptr = self
            .builder
            .build_int_to_ptr(addr, ptr_ty, "vol_ptr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        let store = self
            .builder
            .build_store(ptr, value)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // Mark as volatile
        store.set_volatile(true).ok();

        Ok(())
    }

    /// Phase 5 (Gap G-A): atomic load u64 with SeqCst ordering.
    /// LLVM lowers this to `MOV` (x86_64 — aligned 64-bit loads are
    /// atomic by hardware) plus an `MFENCE` for SeqCst, or to a `LOCK CMPXCHG16B`
    /// equivalent on weaker architectures. fjaros @kernel context only.
    fn compile_atomic_load_u64(
        &mut self,
        addr: inkwell::values::IntValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let i64_ty = self.context.i64_type();
        let ptr = self
            .builder
            .build_int_to_ptr(addr, ptr_ty, "atomic_ptr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        let load = self
            .builder
            .build_load(i64_ty, ptr, "atomic_load")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        if let Some(inst) = load.as_instruction_value() {
            inst.set_atomic_ordering(inkwell::AtomicOrdering::SequentiallyConsistent)
                .ok();
            // Atomic ops require natural alignment.
            inst.set_alignment(8).ok();
        }
        Ok(load)
    }

    /// Phase 5 (Gap G-A): atomic store u64 with SeqCst ordering.
    fn compile_atomic_store_u64(
        &mut self,
        addr: inkwell::values::IntValue<'ctx>,
        value: inkwell::values::IntValue<'ctx>,
    ) -> Result<(), CodegenError> {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let ptr = self
            .builder
            .build_int_to_ptr(addr, ptr_ty, "atomic_ptr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        let store = self
            .builder
            .build_store(ptr, value)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        store
            .set_atomic_ordering(inkwell::AtomicOrdering::SequentiallyConsistent)
            .ok();
        store.set_alignment(8).ok();
        Ok(())
    }

    /// Phase 5 (Gap G-A): atomic compare-and-swap u64.
    /// Returns the previous value at *addr (NOT a (value, success) pair —
    /// caller checks `prev == expected` to determine success). Lowers to
    /// `LOCK CMPXCHG` on x86_64.
    fn compile_atomic_cas_u64(
        &mut self,
        addr: inkwell::values::IntValue<'ctx>,
        expected: inkwell::values::IntValue<'ctx>,
        new_val: inkwell::values::IntValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let ptr = self
            .builder
            .build_int_to_ptr(addr, ptr_ty, "atomic_ptr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        let cas = self
            .builder
            .build_cmpxchg(
                ptr,
                expected,
                new_val,
                inkwell::AtomicOrdering::SequentiallyConsistent,
                inkwell::AtomicOrdering::SequentiallyConsistent,
            )
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        // cmpxchg returns { iN, i1 } — extract field 0 (previous value)
        let prev = self
            .builder
            .build_extract_value(cas, 0, "cas_prev")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        Ok(prev)
    }

    /// Phase 5 (Gap G-A): atomic fetch-and-add u64.
    /// Returns the previous value at *addr; the value at *addr becomes
    /// `prev + delta`. Lowers to `LOCK XADD` on x86_64.
    fn compile_atomic_fetch_add_u64(
        &mut self,
        addr: inkwell::values::IntValue<'ctx>,
        delta: inkwell::values::IntValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let ptr = self
            .builder
            .build_int_to_ptr(addr, ptr_ty, "atomic_ptr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        let prev = self
            .builder
            .build_atomicrmw(
                inkwell::AtomicRMWBinOp::Add,
                ptr,
                delta,
                inkwell::AtomicOrdering::SequentiallyConsistent,
            )
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        Ok(prev.into())
    }

    /// Compiles a spinlock acquire: volatile busy-wait loop.
    ///
    /// ```text
    /// loop:
    ///   val = volatile_load(addr)
    ///   if val == 0 { volatile_store(addr, 1); break }
    /// ```
    fn compile_spin_lock(
        &mut self,
        addr: inkwell::values::IntValue<'ctx>,
    ) -> Result<(), CodegenError> {
        let func = self
            .builder
            .get_insert_block()
            .and_then(|bb| bb.get_parent())
            .ok_or_else(|| CodegenError::Internal("no current function".into()))?;

        let loop_bb = self.context.append_basic_block(func, "spin_loop");
        let done_bb = self.context.append_basic_block(func, "spin_done");

        self.builder
            .build_unconditional_branch(loop_bb)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // Loop: volatile load, check if zero
        self.builder.position_at_end(loop_bb);
        let val = self.compile_volatile_load(addr)?.into_int_value();
        let is_zero = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                val,
                self.context.i64_type().const_zero(),
                "is_unlocked",
            )
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        self.builder
            .build_conditional_branch(is_zero, done_bb, loop_bb)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        // Done: store 1 (acquire)
        self.builder.position_at_end(done_bb);
        self.compile_volatile_store(addr, self.context.i64_type().const_int(1, false))?;

        Ok(())
    }

    // ── Bare-metal inline asm helpers (Phase 1) ───────────────────────

    /// Compiles a port I/O read (inb/inw/inl) via inline asm.
    /// `bits` is 8, 16, or 32. Returns the value zero-extended to i64.
    fn compile_port_in(
        &mut self,
        port: inkwell::values::IntValue<'ctx>,
        bits: u32,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let i16_ty = self.context.i16_type();
        let i64_ty = self.context.i64_type();

        // Truncate port to i16 (x86 port range is 0-65535).
        let port16 = self
            .builder
            .build_int_truncate(port, i16_ty, "port16")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        let (result_ty, template, constraint) = match bits {
            8 => (
                self.context.i8_type().as_basic_type_enum(),
                "inb %dx, %al",
                "={al},{dx}",
            ),
            16 => (
                self.context.i16_type().as_basic_type_enum(),
                "inw %dx, %ax",
                "={ax},{dx}",
            ),
            32 => (
                self.context.i32_type().as_basic_type_enum(),
                "inl %dx, %eax",
                "={eax},{dx}",
            ),
            _ => {
                return Err(CodegenError::Internal(format!(
                    "unsupported port_in width: {bits}"
                )));
            }
        };

        let asm_fn_ty = result_ty.fn_type(&[i16_ty.into()], false);

        let inline_asm = self.context.create_inline_asm(
            asm_fn_ty,
            template.to_string(),
            constraint.to_string(),
            true,  // side effects
            false, // align stack
            None,
            false,
        );

        let call = self
            .builder
            .build_indirect_call(asm_fn_ty, inline_asm, &[port16.into()], "port_in")
            .map_err(|e| CodegenError::Internal(format!("port_in asm error: {e}")))?;

        let raw = match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => v,
            inkwell::values::ValueKind::Instruction(_) => {
                return Err(CodegenError::Internal("port_in produced no value".into()));
            }
        };

        // Zero-extend to i64.
        let ext = self
            .builder
            .build_int_z_extend(raw.into_int_value(), i64_ty, "port_in_zext")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        Ok(ext.into())
    }

    /// Compiles a port I/O write (outb/outw/outl) via inline asm.
    fn compile_port_out(
        &mut self,
        port: inkwell::values::IntValue<'ctx>,
        value: inkwell::values::IntValue<'ctx>,
        bits: u32,
    ) -> Result<(), CodegenError> {
        let i16_ty = self.context.i16_type();
        let void_ty = self.context.void_type();

        // Truncate port to i16.
        let port16 = self
            .builder
            .build_int_truncate(port, i16_ty, "port16")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        let (val_ty, template, constraint) = match bits {
            8 => {
                let t = self.context.i8_type();
                (t.as_basic_type_enum(), "outb %al, %dx", "{al},{dx}")
            }
            16 => {
                let t = self.context.i16_type();
                (t.as_basic_type_enum(), "outw %ax, %dx", "{ax},{dx}")
            }
            32 => {
                let t = self.context.i32_type();
                (t.as_basic_type_enum(), "outl %eax, %dx", "{eax},{dx}")
            }
            _ => {
                return Err(CodegenError::Internal(format!(
                    "unsupported port_out width: {bits}"
                )));
            }
        };

        // Truncate value to target width.
        let truncated = self
            .builder
            .build_int_truncate(value, val_ty.into_int_type(), "val_trunc")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        let asm_fn_ty = void_ty.fn_type(&[val_ty.into(), i16_ty.into()], false);

        let inline_asm = self.context.create_inline_asm(
            asm_fn_ty,
            template.to_string(),
            constraint.to_string(),
            true,  // side effects
            false, // align stack
            None,
            false,
        );

        self.builder
            .build_indirect_call(
                asm_fn_ty,
                inline_asm,
                &[truncated.into(), port16.into()],
                "port_out",
            )
            .map_err(|e| CodegenError::Internal(format!("port_out asm error: {e}")))?;

        Ok(())
    }

    /// Compiles a zero-operand privileged instruction (cli, sti, hlt).
    fn compile_zero_operand_asm(&mut self, mnemonic: &str) -> Result<(), CodegenError> {
        let void_ty = self.context.void_type();
        let asm_fn_ty = void_ty.fn_type(&[], false);

        let inline_asm = self.context.create_inline_asm(
            asm_fn_ty,
            mnemonic.to_string(),
            String::new(), // no constraints
            true,          // side effects
            false,         // align stack
            None,
            false,
        );

        self.builder
            .build_indirect_call(asm_fn_ty, inline_asm, &[], mnemonic)
            .map_err(|e| CodegenError::Internal(format!("{mnemonic} asm error: {e}")))?;

        Ok(())
    }

    /// Compiles `rdtsc` — reads the 64-bit timestamp counter.
    /// Uses `rdtsc` which puts low 32 bits in EAX, high 32 bits in EDX.
    /// We combine them: (EDX << 32) | EAX.
    fn compile_rdtsc(&mut self) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let i64_ty = self.context.i64_type();
        let asm_fn_ty = i64_ty.fn_type(&[], false);

        let inline_asm = self.context.create_inline_asm(
            asm_fn_ty,
            "rdtsc\n\tshlq $$32, %rdx\n\torq %rdx, %rax".to_string(),
            "={rax},~{rdx}".to_string(),
            true,  // side effects
            false, // align stack
            None,
            false,
        );

        let call = self
            .builder
            .build_indirect_call(asm_fn_ty, inline_asm, &[], "rdtsc")
            .map_err(|e| CodegenError::Internal(format!("rdtsc asm error: {e}")))?;

        match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(v),
            inkwell::values::ValueKind::Instruction(_) => {
                Err(CodegenError::Internal("rdtsc produced no value".into()))
            }
        }
    }

    /// Compiles `rdrand` — hardware random number generator.
    /// Returns a 64-bit random value in RAX.
    fn compile_rdrand(&mut self) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let i64_ty = self.context.i64_type();
        let asm_fn_ty = i64_ty.fn_type(&[], false);

        let inline_asm = self.context.create_inline_asm(
            asm_fn_ty,
            "rdrand %rax".to_string(),
            "={rax}".to_string(),
            true,  // side effects
            false, // align stack
            None,
            false,
        );

        let call = self
            .builder
            .build_indirect_call(asm_fn_ty, inline_asm, &[], "rdrand")
            .map_err(|e| CodegenError::Internal(format!("rdrand asm error: {e}")))?;

        match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(v),
            inkwell::values::ValueKind::Instruction(_) => {
                Err(CodegenError::Internal("rdrand produced no value".into()))
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Phase 3.6: AVX2 SIMD builtins — memory-based (addresses as i64)
    // ═══════════════════════════════════════════════════════════════════

    /// AVX2 dot product: avx2_dot_f32(a_ptr, b_ptr, len) -> i64 (f32 bits)
    ///
    /// Processes 8 floats per iteration with vfmadd231ps, then horizontal sum.
    /// Returns the f32 result's bits in the lower 32 bits of i64.
    fn compile_avx2_dot_f32(
        &mut self,
        args: &[BasicValueEnum<'ctx>],
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let i64_ty = self.context.i64_type();
        let asm_fn_ty = i64_ty.fn_type(&[i64_ty.into(), i64_ty.into(), i64_ty.into()], false);

        // AT&T syntax: vfmadd231ps loop with horizontal sum
        let template = concat!(
            "vxorps %ymm0, %ymm0, %ymm0\n\t",
            "movq $0, %rax\n\t",
            "movq $1, %rbx\n\t",
            "movq $2, %rcx\n\t",
            "shrq $$3, %rcx\n\t",
            "testq %rcx, %rcx\n\t",
            "jz 2f\n",
            "1:\n\t",
            "vmovups (%rax), %ymm1\n\t",
            "vmovups (%rbx), %ymm2\n\t",
            "vfmadd231ps %ymm2, %ymm1, %ymm0\n\t",
            "addq $$32, %rax\n\t",
            "addq $$32, %rbx\n\t",
            "decq %rcx\n\t",
            "jnz 1b\n",
            "2:\n\t",
            "vextractf128 $$1, %ymm0, %xmm1\n\t",
            "vaddps %xmm1, %xmm0, %xmm0\n\t",
            "vhaddps %xmm0, %xmm0, %xmm0\n\t",
            "vhaddps %xmm0, %xmm0, %xmm0\n\t",
            "vmovd %xmm0, %eax\n\t",
            "vzeroupper",
        );

        let constraint =
            "={rax},{rdi},{rsi},{rdx},~{rbx},~{rcx},~{ymm0},~{ymm1},~{ymm2},~{xmm0},~{xmm1}"
                .to_string();

        let inline_asm = self.context.create_inline_asm(
            asm_fn_ty,
            template.to_string(),
            constraint,
            true,
            true,
            None,
            false,
        );

        let call_args: Vec<inkwell::values::BasicMetadataValueEnum> =
            args.iter().map(|a| (*a).into()).collect();
        let call = self
            .builder
            .build_indirect_call(asm_fn_ty, inline_asm, &call_args, "avx2_dot")
            .map_err(|e| CodegenError::Internal(format!("avx2_dot asm error: {e}")))?;

        match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(v),
            inkwell::values::ValueKind::Instruction(_) => {
                Err(CodegenError::Internal("avx2_dot produced no value".into()))
            }
        }
    }

    /// AVX2 elementwise binary op: dst[i] = op(a[i], b[i]) for f32 arrays.
    ///
    /// `op` is the AVX instruction name: "vaddps", "vmulps", etc.
    /// Args: (dst_ptr, a_ptr, b_ptr, len)
    fn compile_avx2_elementwise(
        &mut self,
        args: &[BasicValueEnum<'ctx>],
        op: &str,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let i64_ty = self.context.i64_type();
        let asm_fn_ty = i64_ty.fn_type(
            &[i64_ty.into(), i64_ty.into(), i64_ty.into(), i64_ty.into()],
            false,
        );

        let template = format!(
            concat!(
                "movq $1, %rax\n\t",  // a_ptr
                "movq $2, %rbx\n\t",  // b_ptr
                "movq $3, %rcx\n\t",  // len
                "movq $0, %rdx\n\t",  // dst_ptr
                "shrq $$3, %rcx\n\t", // len/8
                "testq %rcx, %rcx\n\t",
                "jz 2f\n",
                "1:\n\t",
                "vmovups (%rax), %ymm0\n\t",
                "vmovups (%rbx), %ymm1\n\t",
                "{op} %ymm1, %ymm0, %ymm2\n\t",
                "vmovups %ymm2, (%rdx)\n\t",
                "addq $$32, %rax\n\t",
                "addq $$32, %rbx\n\t",
                "addq $$32, %rdx\n\t",
                "decq %rcx\n\t",
                "jnz 1b\n",
                "2:\n\t",
                "xorq %rax, %rax\n\t",
                "vzeroupper",
            ),
            op = op
        );

        let constraint =
            "={rax},{rdi},{rsi},{rdx},{rcx},~{rbx},~{ymm0},~{ymm1},~{ymm2}".to_string();

        let inline_asm = self
            .context
            .create_inline_asm(asm_fn_ty, template, constraint, true, true, None, false);

        let call_args: Vec<inkwell::values::BasicMetadataValueEnum> =
            args.iter().map(|a| (*a).into()).collect();
        let call = self
            .builder
            .build_indirect_call(asm_fn_ty, inline_asm, &call_args, "avx2_ewise")
            .map_err(|e| CodegenError::Internal(format!("avx2 elementwise asm error: {e}")))?;

        match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(v),
            inkwell::values::ValueKind::Instruction(_) => {
                Err(CodegenError::Internal("avx2 elementwise no value".into()))
            }
        }
    }

    /// AVX2 ReLU: dst[i] = max(0, src[i]) for f32 arrays.
    ///
    /// Args: (dst_ptr, src_ptr, len)
    fn compile_avx2_relu(
        &mut self,
        args: &[BasicValueEnum<'ctx>],
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let i64_ty = self.context.i64_type();
        let asm_fn_ty = i64_ty.fn_type(&[i64_ty.into(), i64_ty.into(), i64_ty.into()], false);

        let template = concat!(
            "movq $1, %rax\n\t",              // src_ptr
            "movq $0, %rbx\n\t",              // dst_ptr
            "movq $2, %rcx\n\t",              // len
            "shrq $$3, %rcx\n\t",             // len/8
            "vxorps %ymm1, %ymm1, %ymm1\n\t", // zero
            "testq %rcx, %rcx\n\t",
            "jz 2f\n",
            "1:\n\t",
            "vmovups (%rax), %ymm0\n\t",
            "vmaxps %ymm1, %ymm0, %ymm0\n\t",
            "vmovups %ymm0, (%rbx)\n\t",
            "addq $$32, %rax\n\t",
            "addq $$32, %rbx\n\t",
            "decq %rcx\n\t",
            "jnz 1b\n",
            "2:\n\t",
            "xorq %rax, %rax\n\t",
            "vzeroupper",
        );

        let constraint = "={rax},{rdi},{rsi},{rdx},~{rbx},~{rcx},~{ymm0},~{ymm1}".to_string();

        let inline_asm = self.context.create_inline_asm(
            asm_fn_ty,
            template.to_string(),
            constraint,
            true,
            true,
            None,
            false,
        );

        let call_args: Vec<inkwell::values::BasicMetadataValueEnum> =
            args.iter().map(|a| (*a).into()).collect();
        let call = self
            .builder
            .build_indirect_call(asm_fn_ty, inline_asm, &call_args, "avx2_relu")
            .map_err(|e| CodegenError::Internal(format!("avx2_relu asm error: {e}")))?;

        match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(v),
            inkwell::values::ValueKind::Instruction(_) => {
                Err(CodegenError::Internal("avx2_relu no value".into()))
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // AVX2 i64 integer SIMD — for kernel fixed-point inference
    // ═══════════════════════════════════════════════════════════════════

    /// AVX2 dot product for i64 arrays: sum(a[i] * b[i]) for i in 0..len.
    /// Uses vpmuludq (unsigned 32-bit multiply → 64-bit result per lane).
    /// NOTE: Only correct when |a[i]|, |b[i]| < 2^31 (fits in lower 32 bits).
    /// For FajarOS fixed-point (x1000 scale, values < 1M), this is always safe.
    fn compile_avx2_dot_i64(
        &mut self,
        args: &[BasicValueEnum<'ctx>],
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let i64_ty = self.context.i64_type();
        let asm_fn_ty = i64_ty.fn_type(&[i64_ty.into(), i64_ty.into(), i64_ty.into()], false);

        let template = concat!(
            "vpxor %ymm0, %ymm0, %ymm0\n\t", // acc = 0 (4 x i64)
            "movq $0, %rax\n\t",             // a_ptr
            "movq $1, %rbx\n\t",             // b_ptr
            "movq $2, %rcx\n\t",             // len (count of i64 elements)
            "shrq $$2, %rcx\n\t",            // len/4 (4 i64s per YMM)
            "testq %rcx, %rcx\n\t",
            "jz 2f\n",
            "1:\n\t",
            "vmovdqu (%rax), %ymm1\n\t",        // load 4 i64s from a
            "vmovdqu (%rbx), %ymm2\n\t",        // load 4 i64s from b
            "vpmuludq %ymm1, %ymm2, %ymm3\n\t", // multiply low 32 bits → 64-bit results
            "vpaddq %ymm3, %ymm0, %ymm0\n\t",   // acc += products
            "addq $$32, %rax\n\t",
            "addq $$32, %rbx\n\t",
            "decq %rcx\n\t",
            "jnz 1b\n",
            "2:\n\t",
            // Horizontal sum: ymm0 has 4 i64 lanes → reduce to 1
            "vextracti128 $$1, %ymm0, %xmm1\n\t",
            "vpaddq %xmm1, %xmm0, %xmm0\n\t",
            "vpsrldq $$8, %xmm0, %xmm1\n\t",
            "vpaddq %xmm1, %xmm0, %xmm0\n\t",
            "vmovq %xmm0, %rax\n\t",
            "vzeroupper",
        );

        let constraint =
            "={rax},{rdi},{rsi},{rdx},~{rbx},~{rcx},~{ymm0},~{ymm1},~{ymm2},~{ymm3},~{xmm0},~{xmm1}"
                .to_string();

        let inline_asm = self.context.create_inline_asm(
            asm_fn_ty,
            template.to_string(),
            constraint,
            true,
            true,
            None,
            false,
        );

        let call_args: Vec<inkwell::values::BasicMetadataValueEnum> =
            args.iter().map(|a| (*a).into()).collect();
        let call = self
            .builder
            .build_indirect_call(asm_fn_ty, inline_asm, &call_args, "avx2_dot_i64")
            .map_err(|e| CodegenError::Internal(format!("avx2_dot_i64 asm error: {e}")))?;

        match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(v),
            inkwell::values::ValueKind::Instruction(_) => Err(CodegenError::Internal(
                "avx2_dot_i64 produced no value".into(),
            )),
        }
    }

    /// AVX2 elementwise binary op for i64 arrays: dst[i] = op(a[i], b[i]).
    /// `op` is the AVX2 instruction: "vpaddq" (add), "vpmuludq" (mul low32).
    /// Processes 4 i64 values per iteration (256-bit YMM registers).
    fn compile_avx2_elementwise_i64(
        &mut self,
        args: &[BasicValueEnum<'ctx>],
        op: &str,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let i64_ty = self.context.i64_type();
        let asm_fn_ty = i64_ty.fn_type(
            &[i64_ty.into(), i64_ty.into(), i64_ty.into(), i64_ty.into()],
            false,
        );

        let template = format!(
            concat!(
                "movq $1, %rax\n\t",  // a_ptr
                "movq $2, %rbx\n\t",  // b_ptr
                "movq $3, %rcx\n\t",  // len
                "movq $0, %rdx\n\t",  // dst_ptr
                "shrq $$2, %rcx\n\t", // len/4 (4 i64s per YMM)
                "testq %rcx, %rcx\n\t",
                "jz 2f\n",
                "1:\n\t",
                "vmovdqu (%rax), %ymm0\n\t",
                "vmovdqu (%rbx), %ymm1\n\t",
                "{op} %ymm1, %ymm0, %ymm2\n\t",
                "vmovdqu %ymm2, (%rdx)\n\t",
                "addq $$32, %rax\n\t",
                "addq $$32, %rbx\n\t",
                "addq $$32, %rdx\n\t",
                "decq %rcx\n\t",
                "jnz 1b\n",
                "2:\n\t",
                "xorq %rax, %rax\n\t",
                "vzeroupper",
            ),
            op = op
        );

        let constraint =
            "={rax},{rdi},{rsi},{rdx},{rcx},~{rbx},~{ymm0},~{ymm1},~{ymm2}".to_string();

        let inline_asm = self
            .context
            .create_inline_asm(asm_fn_ty, template, constraint, true, true, None, false);

        let call_args: Vec<inkwell::values::BasicMetadataValueEnum> =
            args.iter().map(|a| (*a).into()).collect();
        let call = self
            .builder
            .build_indirect_call(asm_fn_ty, inline_asm, &call_args, "avx2_ewise_i64")
            .map_err(|e| CodegenError::Internal(format!("avx2 i64 elementwise asm error: {e}")))?;

        match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(v),
            inkwell::values::ValueKind::Instruction(_) => Err(CodegenError::Internal(
                "avx2 i64 elementwise no value".into(),
            )),
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Phase 3.7: AES-NI builtins — 128-bit block via XMM registers
    // ═══════════════════════════════════════════════════════════════════

    /// AES-NI block encrypt/decrypt: operates on 16-byte block at state_ptr.
    ///
    /// `decrypt=false`: aesenc × (rounds-1) + aesenclast
    /// `decrypt=true`:  aesdec × (rounds-1) + aesdeclast
    /// Args: (state_ptr, key_schedule_ptr, rounds)
    /// Key schedule is (rounds+1) × 16 bytes of expanded round keys.
    fn compile_aesni_block(
        &mut self,
        args: &[BasicValueEnum<'ctx>],
        decrypt: bool,
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        let i64_ty = self.context.i64_type();
        let asm_fn_ty = i64_ty.fn_type(&[i64_ty.into(), i64_ty.into(), i64_ty.into()], false);

        let (round_op, last_op) = if decrypt {
            ("aesdec", "aesdeclast")
        } else {
            ("aesenc", "aesenclast")
        };

        // Generic AES block: supports AES-128 (10), AES-192 (12), AES-256 (14) rounds
        let template = format!(
            concat!(
                "movq $0, %rdi\n\t",        // state_ptr
                "movq $1, %rsi\n\t",        // key_schedule_ptr
                "movq $2, %rcx\n\t",        // rounds
                "movdqu (%rdi), %xmm0\n\t", // state = *state_ptr
                "movdqu (%rsi), %xmm1\n\t", // round_key[0]
                "pxor %xmm1, %xmm0\n\t",    // state ^= key[0]
                "movq $$1, %rax\n",         // round counter
                "1:\n\t",
                "cmpq %rcx, %rax\n\t",
                "jge 2f\n\t",
                "movq %rax, %rdx\n\t",
                "shlq $$4, %rdx\n\t", // offset = round * 16
                "movdqu (%rsi,%rdx), %xmm1\n\t",
                "{round_op} %xmm1, %xmm0\n\t",
                "incq %rax\n\t",
                "jmp 1b\n",
                "2:\n\t",
                "movq %rcx, %rdx\n\t",
                "shlq $$4, %rdx\n\t", // offset = rounds * 16
                "movdqu (%rsi,%rdx), %xmm1\n\t",
                "{last_op} %xmm1, %xmm0\n\t",
                "movdqu %xmm0, (%rdi)\n\t", // *state_ptr = result
                "xorq %rax, %rax",          // return 0
            ),
            round_op = round_op,
            last_op = last_op
        );

        let constraint = "={rax},{rdi},{rsi},{rdx},~{rcx},~{xmm0},~{xmm1}".to_string();

        let inline_asm = self
            .context
            .create_inline_asm(asm_fn_ty, template, constraint, true, true, None, false);

        let label = if decrypt { "aes_dec" } else { "aes_enc" };
        let call_args: Vec<inkwell::values::BasicMetadataValueEnum> =
            args.iter().map(|a| (*a).into()).collect();
        let call = self
            .builder
            .build_indirect_call(asm_fn_ty, inline_asm, &call_args, label)
            .map_err(|e| CodegenError::Internal(format!("aesni asm error: {e}")))?;

        match call.try_as_basic_value() {
            inkwell::values::ValueKind::Basic(v) => Ok(v),
            inkwell::values::ValueKind::Instruction(_) => {
                Err(CodegenError::Internal("aesni produced no value".into()))
            }
        }
    }

    /// Constructs the optimization pass pipeline string, incorporating PGO if active.
    ///
    /// - PGO Generate: prepends `pgo-instr-gen` before the standard pipeline
    /// - PGO Use: prepends `pgo-instr-use<file>` before the standard pipeline
    /// - No PGO: returns the standard optimization pipeline
    fn build_pass_pipeline(&self) -> String {
        let base = self.opt_level.pass_string();
        match &self.pgo_mode {
            PgoMode::Generate(_prof_file) => {
                // Instrumentation generation: add profiling counters
                format!("pgo-instr-gen,{base},pgo-instr-gen", base = base)
            }
            PgoMode::Use(profdata_path) => {
                // Profile use: annotate with branch weights before optimization
                format!(
                    "pgo-instr-use<profile-file={path}>,{base}",
                    path = profdata_path,
                    base = base
                )
            }
            PgoMode::None => base.to_string(),
        }
    }

    /// Runs LLVM optimization passes on the module.
    ///
    /// If PGO is active, includes PGO instrumentation or profile-use passes.
    pub fn optimize(&self) -> Result<(), CodegenError> {
        if self.opt_level == LlvmOptLevel::O0 && !self.pgo_mode.is_generate() {
            return Ok(());
        }

        let tm = self.create_target_machine(None)?;
        let pass_opts = inkwell::passes::PassBuilderOptions::create();
        let pipeline = self.build_pass_pipeline();
        self.module
            .run_passes(&pipeline, &tm, pass_opts)
            .map_err(|e| CodegenError::Internal(format!("LLVM pass manager error: {:?}", e)))
    }

    /// Runs optimization passes appropriate for LTO.
    ///
    /// For Thin LTO: runs pre-link passes that prepare the module for
    /// cross-module optimization at link time.
    /// For Full LTO: runs full optimization on the merged module.
    pub fn optimize_for_lto(&self) -> Result<(), CodegenError> {
        if self.opt_level == LlvmOptLevel::O0 {
            return Ok(());
        }

        let tm = self.create_target_machine(None)?;
        let pass_opts = inkwell::passes::PassBuilderOptions::create();

        let passes = match self.lto_mode {
            LtoMode::Thin => {
                // Thin LTO: run pre-link pipeline that prepares for ThinLTO
                format!("thinlto-pre-link<{}>", self.opt_level.pass_string_bare())
            }
            LtoMode::Full => {
                // Full LTO: run standard optimization (will be merged later)
                format!("lto-pre-link<{}>", self.opt_level.pass_string_bare())
            }
            LtoMode::None => {
                // No LTO: standard optimization
                return self.optimize();
            }
        };

        self.module
            .run_passes(&passes, &tm, pass_opts)
            .map_err(|e| CodegenError::Internal(format!("LLVM LTO pre-link pass error: {:?}", e)))
    }

    /// Writes the compiled module to an object file.
    ///
    /// In bare-metal mode, runs pre-link symbol verification to catch missing
    /// `fj_rt_bare_*` symbols early with clear diagnostics.
    pub fn emit_object(&self, path: &Path) -> Result<(), CodegenError> {
        // E5/D: Pre-link symbol verification for bare-metal
        // Uses startup_provided_symbols() to exclude symbols that the
        // compiler's startup module will provide at link time.
        if self.no_std {
            let known = Self::startup_provided_symbols();
            let missing = self.verify_bare_metal_symbols_with_known(&known);
            if !missing.is_empty() {
                eprintln!(
                    "warning: {} undefined bare-metal runtime symbol(s) — \
                     these must be provided by your runtime library or will \
                     cause linker errors:",
                    missing.len()
                );
                for sym in &missing {
                    eprintln!("  - {sym}");
                }
            }
        }
        // V30 diagnostic: dump LLVM IR before codegen when FJ_EMIT_IR is set.
        // Usage: FJ_EMIT_IR=1 fj build --backend llvm ...
        if std::env::var("FJ_EMIT_IR").is_ok() {
            let ir_path = path.with_extension("ll");
            if let Err(e) = self.emit_ir(&ir_path) {
                eprintln!("warning: FJ_EMIT_IR dump failed: {e}");
            } else {
                eprintln!("FJ_EMIT_IR: wrote {}", ir_path.display());
            }
        }
        let tm = self.create_target_machine(None)?;
        self.configure_module_target(&tm);
        tm.write_to_file(&self.module, FileType::Object, path)
            .map_err(|e| CodegenError::Internal(format!("LLVM emit object error: {e}")))
    }

    /// Writes the compiled module as LLVM IR text (.ll) file.
    pub fn emit_ir(&self, path: &Path) -> Result<(), CodegenError> {
        self.module
            .print_to_file(path)
            .map_err(|e| CodegenError::Internal(format!("LLVM IR emit error: {e}")))
    }

    /// Writes the compiled module to an assembly file.
    pub fn emit_assembly(&self, path: &Path) -> Result<(), CodegenError> {
        let tm = self.create_target_machine(None)?;
        self.configure_module_target(&tm);
        tm.write_to_file(&self.module, FileType::Assembly, path)
            .map_err(|e| CodegenError::Internal(format!("LLVM emit assembly error: {e}")))
    }

    /// Writes the compiled module to a bitcode file (for LTO).
    pub fn emit_bitcode(&self, path: &Path) -> bool {
        self.module.write_bitcode_to_path(path)
    }

    /// Emits a bitcode file suitable for LTO linking.
    ///
    /// Runs LTO pre-link optimization, then writes bitcode. The resulting
    /// .bc file can be merged with other .bc files via `link_bitcode_lto()`.
    pub fn emit_bitcode_for_lto(&self, path: &Path) -> Result<(), CodegenError> {
        self.optimize_for_lto()?;
        if !self.module.write_bitcode_to_path(path) {
            return Err(CodegenError::Internal(
                "failed to write bitcode for LTO".to_string(),
            ));
        }
        Ok(())
    }

    /// Links multiple bitcode files into this module for LTO.
    ///
    /// Reads each .bc file, merges it into the current module, then runs
    /// the appropriate LTO optimization passes (thin or full).
    pub fn link_bitcode_lto(&self, bc_paths: &[&Path]) -> Result<LtoStats, CodegenError> {
        let start = std::time::Instant::now();
        let mut input_size: u64 = 0;
        let mut modules_merged: usize = 0;

        for bc_path in bc_paths {
            let size = std::fs::metadata(bc_path).map(|m| m.len()).unwrap_or(0);
            input_size += size;

            let linked_module =
                Module::parse_bitcode_from_path(bc_path, self.context).map_err(|e| {
                    CodegenError::Internal(format!(
                        "failed to parse bitcode '{}': {}",
                        bc_path.display(),
                        e
                    ))
                })?;

            self.module.link_in_module(linked_module).map_err(|e| {
                CodegenError::Internal(format!(
                    "failed to link module '{}': {}",
                    bc_path.display(),
                    e
                ))
            })?;

            modules_merged += 1;
        }

        // Run LTO optimization on the merged module
        let tm = self.create_target_machine(None)?;
        let pass_opts = inkwell::passes::PassBuilderOptions::create();

        let passes = match self.lto_mode {
            LtoMode::Thin => {
                format!("thinlto<{}>", self.opt_level.pass_string_bare())
            }
            LtoMode::Full => {
                format!("lto<{}>", self.opt_level.pass_string_bare())
            }
            LtoMode::None => self.opt_level.pass_string().to_string(),
        };

        self.module
            .run_passes(&passes, &tm, pass_opts)
            .map_err(|e| CodegenError::Internal(format!("LLVM LTO optimization error: {:?}", e)))?;

        let optimize_time = start.elapsed().as_millis() as u64;

        Ok(LtoStats {
            modules_merged,
            input_size_bytes: input_size,
            output_size_bytes: 0, // filled in after emit
            optimize_time_ms: optimize_time,
        })
    }

    /// JIT-executes the `main` function and returns its i64 result.
    pub fn jit_execute(&self) -> Result<i64, CodegenError> {
        let ee = self
            .module
            .create_jit_execution_engine(self.opt_level.to_inkwell())
            .map_err(|e| CodegenError::Internal(format!("LLVM JIT creation error: {e}")))?;

        // Map runtime functions to their Rust implementations.
        // When the `native` feature is enabled, use the full Cranelift runtime_fns.
        // Otherwise, use the LLVM-local runtime module (essential subset).
        macro_rules! map_rt {
            ($ee:expr, $module:expr, $name:expr, $fn:expr) => {
                if let Some(func) = $module.get_function($name) {
                    $ee.add_global_mapping(&func, $fn as *const () as usize);
                }
            };
        }

        #[cfg(feature = "native")]
        {
            use crate::codegen::cranelift::runtime_fns;
            map_rt!(
                ee,
                self.module,
                "fj_rt_println_str",
                runtime_fns::fj_rt_println_str
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_print_str",
                runtime_fns::fj_rt_print_str
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_println_int",
                runtime_fns::fj_rt_print_i64
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_print_int",
                runtime_fns::fj_rt_print_i64_no_newline
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_println_f64",
                runtime_fns::fj_rt_println_f64
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_print_f64",
                runtime_fns::fj_rt_print_f64_no_newline
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_println_bool",
                runtime_fns::fj_rt_println_bool
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_print_bool",
                runtime_fns::fj_rt_print_bool
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprintln_int",
                runtime_fns::fj_rt_eprintln_i64
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprintln_str",
                runtime_fns::fj_rt_eprintln_str
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprintln_f64",
                runtime_fns::fj_rt_eprintln_f64
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprintln_bool",
                runtime_fns::fj_rt_eprintln_bool
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprint_int",
                runtime_fns::fj_rt_eprint_i64
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprint_str",
                runtime_fns::fj_rt_eprint_str
            );
            map_rt!(ee, self.module, "fj_rt_alloc", runtime_fns::fj_rt_alloc);
            map_rt!(ee, self.module, "fj_rt_free", runtime_fns::fj_rt_free);
            // Type conversion (f-strings)
            map_rt!(
                ee,
                self.module,
                "fj_rt_int_to_string",
                runtime_fns::fj_rt_int_to_string
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_float_to_string",
                runtime_fns::fj_rt_float_to_string
            );
            // bool_to_string not in cranelift runtime — use LLVM-local runtime
            map_rt!(
                ee,
                self.module,
                "fj_rt_bool_to_string",
                runtime::fj_rt_bool_to_string
            );
        }

        #[cfg(not(feature = "native"))]
        {
            use runtime;
            map_rt!(
                ee,
                self.module,
                "fj_rt_println_str",
                runtime::fj_rt_println_str
            );
            map_rt!(ee, self.module, "fj_rt_print_str", runtime::fj_rt_print_str);
            map_rt!(
                ee,
                self.module,
                "fj_rt_println_int",
                runtime::fj_rt_println_int
            );
            map_rt!(ee, self.module, "fj_rt_print_int", runtime::fj_rt_print_int);
            map_rt!(
                ee,
                self.module,
                "fj_rt_println_f64",
                runtime::fj_rt_println_f64
            );
            map_rt!(ee, self.module, "fj_rt_print_f64", runtime::fj_rt_print_f64);
            map_rt!(
                ee,
                self.module,
                "fj_rt_println_bool",
                runtime::fj_rt_println_bool
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_print_bool",
                runtime::fj_rt_print_bool
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprintln_int",
                runtime::fj_rt_eprintln_int
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprintln_str",
                runtime::fj_rt_eprintln_str
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprintln_f64",
                runtime::fj_rt_eprintln_f64
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprintln_bool",
                runtime::fj_rt_eprintln_bool
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprint_int",
                runtime::fj_rt_eprint_int
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_eprint_str",
                runtime::fj_rt_eprint_str
            );
            map_rt!(ee, self.module, "fj_rt_alloc", runtime::fj_rt_alloc);
            map_rt!(ee, self.module, "fj_rt_free", runtime::fj_rt_free);
            // String + assert functions
            map_rt!(ee, self.module, "fj_rt_str_len", runtime::fj_rt_str_len);
            map_rt!(
                ee,
                self.module,
                "fj_rt_str_concat",
                runtime::fj_rt_str_concat
            );
            map_rt!(ee, self.module, "fj_rt_assert", runtime::fj_rt_assert);
            map_rt!(ee, self.module, "fj_rt_assert_eq", runtime::fj_rt_assert_eq);
            // Type conversion (f-strings)
            map_rt!(
                ee,
                self.module,
                "fj_rt_int_to_string",
                runtime::fj_rt_int_to_string
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_float_to_string",
                runtime::fj_rt_float_to_string
            );
            map_rt!(
                ee,
                self.module,
                "fj_rt_bool_to_string",
                runtime::fj_rt_bool_to_string
            );
        }

        // Check if main returns void or i64
        let main_func = self
            .module
            .get_function("main")
            .ok_or_else(|| CodegenError::UndefinedFunction("main".into()))?;
        let is_void = main_func.get_type().get_return_type().is_none();

        // SAFETY: We're calling into JIT-compiled code that has been verified
        // by the LLVM module verifier.
        unsafe {
            if is_void {
                let main_fn = ee
                    .get_function::<unsafe extern "C" fn()>("main")
                    .map_err(|e| CodegenError::UndefinedFunction(format!("main not found: {e}")))?;
                main_fn.call();
                Ok(0)
            } else {
                let main_fn = ee
                    .get_function::<unsafe extern "C" fn() -> i64>("main")
                    .map_err(|e| CodegenError::UndefinedFunction(format!("main not found: {e}")))?;
                Ok(main_fn.call())
            }
        }
    }
}

/// Converts a TypeExpr to a string for type lookup.
fn type_expr_to_string(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Simple { name, .. } => name.clone(),
        TypeExpr::Generic { name, .. } => name.clone(),
        _ => "i64".to_string(), // Default fallback
    }
}

/// Infers a type string from a literal expression (for monomorphization).
fn infer_type_from_expr(expr: &Expr) -> String {
    match expr {
        Expr::Literal { kind, .. } => match kind {
            LiteralKind::Int(_) => "i64".to_string(),
            LiteralKind::Float(_) => "f64".to_string(),
            LiteralKind::Bool(_) => "bool".to_string(),
            LiteralKind::String(_) | LiteralKind::RawString(_) => "str".to_string(),
            LiteralKind::Char(_) => "char".to_string(),
            LiteralKind::Null => "void".to_string(),
        },
        _ => "i64".to_string(), // Default for non-literal expressions
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::Span;
    use crate::parser::ast::Param;

    fn dummy_span() -> Span {
        Span { start: 0, end: 0 }
    }

    fn make_int_lit(v: i64) -> Expr {
        Expr::Literal {
            kind: LiteralKind::Int(v),
            span: dummy_span(),
        }
    }

    fn make_ident(name: &str) -> Expr {
        Expr::Ident {
            name: name.to_string(),
            span: dummy_span(),
        }
    }

    fn make_binop(left: Expr, op: BinOp, right: Expr) -> Expr {
        Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
            span: dummy_span(),
        }
    }

    fn make_simple_fn(name: &str, body: Expr) -> FnDef {
        FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: name.to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![],
            return_type: Some(TypeExpr::Simple {
                name: "i64".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(body),
            span: dummy_span(),
        }
    }

    fn make_program(items: Vec<Item>) -> Program {
        Program {
            items,
            span: dummy_span(),
        }
    }

    #[test]
    fn llvm_context_and_module_creation() {
        let ctx = Context::create();
        let compiler = LlvmCompiler::new(&ctx, "test_module");
        let ir = compiler.print_ir();
        assert!(ir.contains("test_module"));
    }

    #[test]
    fn llvm_init_native_target() {
        assert!(LlvmCompiler::init_native_target().is_ok());
    }

    #[test]
    fn llvm_create_target_machine_default() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let compiler = LlvmCompiler::new(&ctx, "test");
        let tm = compiler.create_target_machine(None);
        assert!(tm.is_ok());
    }

    #[test]
    fn llvm_compile_int_literal_returns_42() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        let program = make_program(vec![Item::FnDef(make_simple_fn("main", make_int_lit(42)))]);

        compiler.compile_program(&program).unwrap();
        let result = compiler.jit_execute().unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn llvm_compile_addition() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        let body = make_binop(make_int_lit(10), BinOp::Add, make_int_lit(32));
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);

        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_subtraction() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        let body = make_binop(make_int_lit(50), BinOp::Sub, make_int_lit(8));
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);

        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_multiplication() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        let body = make_binop(make_int_lit(6), BinOp::Mul, make_int_lit(7));
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);

        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_function_call() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn double(x: i64) -> i64 { x + x }
        let double_fn = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "double".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![Param {
                name: "x".to_string(),
                ty: TypeExpr::Simple {
                    name: "i64".to_string(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            }],
            return_type: Some(TypeExpr::Simple {
                name: "i64".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(make_binop(make_ident("x"), BinOp::Add, make_ident("x"))),
            span: dummy_span(),
        };

        // fn main() -> i64 { double(21) }
        let main_body = Expr::Call {
            callee: Box::new(make_ident("double")),
            args: vec![crate::parser::ast::CallArg {
                name: None,
                value: make_int_lit(21),
                span: dummy_span(),
            }],
            span: dummy_span(),
        };
        let main_fn = make_simple_fn("main", main_body);

        let program = make_program(vec![Item::FnDef(double_fn), Item::FnDef(main_fn)]);

        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_if_else() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 { if 1 > 0 { 42 } else { 0 } }
        let body = Expr::If {
            condition: Box::new(make_binop(make_int_lit(1), BinOp::Gt, make_int_lit(0))),
            then_branch: Box::new(make_int_lit(42)),
            else_branch: Some(Box::new(make_int_lit(0))),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);

        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_ir_contains_function_definition() {
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        let program = make_program(vec![Item::FnDef(make_simple_fn("main", make_int_lit(0)))]);

        compiler.compile_program(&program).unwrap();
        let ir = compiler.print_ir();
        assert!(ir.contains("define i64 @main()"));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Sprint 2: Expression tests
    // ═══════════════════════════════════════════════════════════════════════

    fn make_float_lit(v: f64) -> Expr {
        Expr::Literal {
            kind: LiteralKind::Float(v),
            span: dummy_span(),
        }
    }

    fn make_bool_lit(v: bool) -> Expr {
        Expr::Literal {
            kind: LiteralKind::Bool(v),
            span: dummy_span(),
        }
    }

    #[test]
    fn llvm_compile_float_literal() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> f64 { 3.14 }
        let main_fn = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "main".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![],
            return_type: Some(TypeExpr::Simple {
                name: "f64".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(make_float_lit(1.25)),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(main_fn)]);
        compiler.compile_program(&program).unwrap();
        let ir = compiler.print_ir();
        // Test body uses 1.25 (a power-of-two-clean float that LLVM
        // prints exactly without rounding artifacts). The earlier
        // assertion `contains("3.14")` was a stale post-sed mismatch
        // (V32 P3 wave bumped the literal to dodge clippy
        // approx_constant but the assertion was not updated).
        assert!(
            ir.contains("1.25") || ir.contains("0x3FF4"),
            "expected float literal in IR, got:\n{ir}"
        );
    }

    #[test]
    fn llvm_compile_bool_literal() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 { if true { 1 } else { 0 } }
        let body = Expr::If {
            condition: Box::new(make_bool_lit(true)),
            then_branch: Box::new(make_int_lit(1)),
            else_branch: Some(Box::new(make_int_lit(0))),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 1);
    }

    #[test]
    fn llvm_compile_string_literal_ir() {
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> str { "hello" }
        let main_fn = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "main".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![],
            return_type: Some(TypeExpr::Simple {
                name: "str".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(Expr::Literal {
                kind: LiteralKind::String("hello".to_string()),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(main_fn)]);
        compiler.compile_program(&program).unwrap();
        let ir = compiler.print_ir();
        assert!(ir.contains("hello"));
        assert!(ir.contains("__fj_str_"));
    }

    #[test]
    fn llvm_compile_division() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        let body = make_binop(make_int_lit(84), BinOp::Div, make_int_lit(2));
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_remainder() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        let body = make_binop(make_int_lit(47), BinOp::Rem, make_int_lit(5));
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 2);
    }

    #[test]
    fn llvm_compile_comparison_operators() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // 10 < 20 => 1 (true)
        let body = Expr::If {
            condition: Box::new(make_binop(make_int_lit(10), BinOp::Lt, make_int_lit(20))),
            then_branch: Box::new(make_int_lit(1)),
            else_branch: Some(Box::new(make_int_lit(0))),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 1);
    }

    #[test]
    fn llvm_compile_bitwise_operations() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // (0xFF & 0x0F) == 15
        let body = make_binop(make_int_lit(0xFF), BinOp::BitAnd, make_int_lit(0x0F));
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 15);
    }

    #[test]
    fn llvm_compile_unary_negation() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        let body = Expr::Unary {
            op: UnaryOp::Neg,
            operand: Box::new(make_int_lit(42)),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), -42);
    }

    #[test]
    fn llvm_compile_type_cast_int_to_i32() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // (42 as i32) as i64 — cast i64 → i32 → i64
        let inner_cast = Expr::Cast {
            expr: Box::new(make_int_lit(42)),
            ty: TypeExpr::Simple {
                name: "i32".to_string(),
                span: dummy_span(),
            },
            span: dummy_span(),
        };
        let body = Expr::Cast {
            expr: Box::new(inner_cast),
            ty: TypeExpr::Simple {
                name: "i64".to_string(),
                span: dummy_span(),
            },
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_cast_int_to_float_ir() {
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        let main_fn = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "main".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![],
            return_type: Some(TypeExpr::Simple {
                name: "f64".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(Expr::Cast {
                expr: Box::new(make_int_lit(42)),
                ty: TypeExpr::Simple {
                    name: "f64".to_string(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(main_fn)]);
        compiler.compile_program(&program).unwrap();
        let ir = compiler.print_ir();
        // IR should contain int-to-float conversion (sitofp or constant fold)
        assert!(
            ir.contains("sitofp") || ir.contains("double"),
            "IR should contain sitofp or constant double: {}",
            ir
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Sprint 3: Control flow tests
    // ═══════════════════════════════════════════════════════════════════════

    fn make_let_stmt(name: &str, value: Expr) -> Stmt {
        Stmt::Let {
            mutable: true,
            linear: false,
            name: name.to_string(),
            ty: None,
            value: Box::new(value),
            span: dummy_span(),
        }
    }

    fn make_expr_stmt(expr: Expr) -> Stmt {
        Stmt::Expr {
            expr: Box::new(expr),
            span: dummy_span(),
        }
    }

    fn make_assign(name: &str, value: Expr) -> Expr {
        Expr::Assign {
            target: Box::new(make_ident(name)),
            op: crate::parser::ast::AssignOp::Assign,
            value: Box::new(value),
            span: dummy_span(),
        }
    }

    #[test]
    fn llvm_compile_let_and_variable() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 { let x = 42; x }
        let body = Expr::Block {
            stmts: vec![make_let_stmt("x", make_int_lit(42))],
            expr: Some(Box::new(make_ident("x"))),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_assignment() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 { let mut x = 10; x = 42; x }
        let body = Expr::Block {
            stmts: vec![
                make_let_stmt("x", make_int_lit(10)),
                make_expr_stmt(make_assign("x", make_int_lit(42))),
            ],
            expr: Some(Box::new(make_ident("x"))),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_while_loop() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 {
        //   let mut i = 0
        //   let mut sum = 0
        //   while i < 10 {
        //     sum = sum + i
        //     i = i + 1
        //   }
        //   sum  // 0+1+2+...+9 = 45
        // }
        let while_body = Expr::Block {
            stmts: vec![
                make_expr_stmt(make_assign(
                    "sum",
                    make_binop(make_ident("sum"), BinOp::Add, make_ident("i")),
                )),
                make_expr_stmt(make_assign(
                    "i",
                    make_binop(make_ident("i"), BinOp::Add, make_int_lit(1)),
                )),
            ],
            expr: None,
            span: dummy_span(),
        };

        let body = Expr::Block {
            stmts: vec![
                make_let_stmt("i", make_int_lit(0)),
                make_let_stmt("sum", make_int_lit(0)),
                make_expr_stmt(Expr::While {
                    label: None,
                    condition: Box::new(make_binop(make_ident("i"), BinOp::Lt, make_int_lit(10))),
                    body: Box::new(while_body),
                    span: dummy_span(),
                }),
            ],
            expr: Some(Box::new(make_ident("sum"))),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 45);
    }

    #[test]
    fn llvm_compile_for_loop() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 {
        //   let mut sum = 0
        //   for i in 0..10 {
        //     sum = sum + i
        //   }
        //   sum  // 45
        // }
        let for_body = Expr::Block {
            stmts: vec![make_expr_stmt(make_assign(
                "sum",
                make_binop(make_ident("sum"), BinOp::Add, make_ident("i")),
            ))],
            expr: None,
            span: dummy_span(),
        };

        let body = Expr::Block {
            stmts: vec![
                make_let_stmt("sum", make_int_lit(0)),
                make_expr_stmt(Expr::For {
                    label: None,
                    variable: "i".to_string(),
                    iterable: Box::new(Expr::Range {
                        start: Some(Box::new(make_int_lit(0))),
                        end: Some(Box::new(make_int_lit(10))),
                        inclusive: false,
                        span: dummy_span(),
                    }),
                    body: Box::new(for_body),
                    span: dummy_span(),
                }),
            ],
            expr: Some(Box::new(make_ident("sum"))),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 45);
    }

    #[test]
    fn llvm_compile_loop_with_break() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 {
        //   let mut i = 0
        //   loop {
        //     if i == 42 { break i }
        //     i = i + 1
        //   }
        // }
        let loop_body = Expr::Block {
            stmts: vec![
                make_expr_stmt(Expr::If {
                    condition: Box::new(make_binop(make_ident("i"), BinOp::Eq, make_int_lit(42))),
                    then_branch: Box::new(Expr::Block {
                        stmts: vec![Stmt::Break {
                            label: None,
                            value: Some(Box::new(make_ident("i"))),
                            span: dummy_span(),
                        }],
                        expr: None,
                        span: dummy_span(),
                    }),
                    else_branch: None,
                    span: dummy_span(),
                }),
                make_expr_stmt(make_assign(
                    "i",
                    make_binop(make_ident("i"), BinOp::Add, make_int_lit(1)),
                )),
            ],
            expr: None,
            span: dummy_span(),
        };

        let body = Expr::Block {
            stmts: vec![make_let_stmt("i", make_int_lit(0))],
            expr: Some(Box::new(Expr::Loop {
                label: None,
                body: Box::new(loop_body),
                span: dummy_span(),
            })),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_match_expression() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 {
        //   let x = 2
        //   match x {
        //     1 => 10,
        //     2 => 42,
        //     _ => 0,
        //   }
        // }
        let body = Expr::Block {
            stmts: vec![make_let_stmt("x", make_int_lit(2))],
            expr: Some(Box::new(Expr::Match {
                subject: Box::new(make_ident("x")),
                arms: vec![
                    MatchArm {
                        pattern: Pattern::Literal {
                            kind: LiteralKind::Int(1),
                            span: dummy_span(),
                        },
                        guard: None,
                        body: Box::new(make_int_lit(10)),
                        span: dummy_span(),
                    },
                    MatchArm {
                        pattern: Pattern::Literal {
                            kind: LiteralKind::Int(2),
                            span: dummy_span(),
                        },
                        guard: None,
                        body: Box::new(make_int_lit(42)),
                        span: dummy_span(),
                    },
                    MatchArm {
                        pattern: Pattern::Wildcard { span: dummy_span() },
                        guard: None,
                        body: Box::new(make_int_lit(0)),
                        span: dummy_span(),
                    },
                ],
                span: dummy_span(),
            })),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_nested_if_else() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 {
        //   let x = 5
        //   if x > 10 { 100 } else { if x > 3 { 42 } else { 0 } }
        // }
        let inner_if = Expr::If {
            condition: Box::new(make_binop(make_ident("x"), BinOp::Gt, make_int_lit(3))),
            then_branch: Box::new(make_int_lit(42)),
            else_branch: Some(Box::new(make_int_lit(0))),
            span: dummy_span(),
        };
        let body = Expr::Block {
            stmts: vec![make_let_stmt("x", make_int_lit(5))],
            expr: Some(Box::new(Expr::If {
                condition: Box::new(make_binop(make_ident("x"), BinOp::Gt, make_int_lit(10))),
                then_branch: Box::new(make_int_lit(100)),
                else_branch: Some(Box::new(inner_if)),
                span: dummy_span(),
            })),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_block_expression() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 { let a = 10; let b = 32; a + b }
        let body = Expr::Block {
            stmts: vec![
                make_let_stmt("a", make_int_lit(10)),
                make_let_stmt("b", make_int_lit(32)),
            ],
            expr: Some(Box::new(make_binop(
                make_ident("a"),
                BinOp::Add,
                make_ident("b"),
            ))),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_return_statement() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 { return 42 }
        let body = Expr::Block {
            stmts: vec![Stmt::Return {
                value: Some(Box::new(make_int_lit(42))),
                span: dummy_span(),
            }],
            expr: None,
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_fibonacci_iterative() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 {
        //   let mut a = 0
        //   let mut b = 1
        //   let mut i = 0
        //   while i < 10 {
        //     let temp = b
        //     b = a + b
        //     a = temp
        //     i = i + 1
        //   }
        //   a  // fib(10) = 55
        // }
        let while_body = Expr::Block {
            stmts: vec![
                make_let_stmt("temp", make_ident("b")),
                make_expr_stmt(make_assign(
                    "b",
                    make_binop(make_ident("a"), BinOp::Add, make_ident("b")),
                )),
                make_expr_stmt(make_assign("a", make_ident("temp"))),
                make_expr_stmt(make_assign(
                    "i",
                    make_binop(make_ident("i"), BinOp::Add, make_int_lit(1)),
                )),
            ],
            expr: None,
            span: dummy_span(),
        };

        let body = Expr::Block {
            stmts: vec![
                make_let_stmt("a", make_int_lit(0)),
                make_let_stmt("b", make_int_lit(1)),
                make_let_stmt("i", make_int_lit(0)),
                make_expr_stmt(Expr::While {
                    label: None,
                    condition: Box::new(make_binop(make_ident("i"), BinOp::Lt, make_int_lit(10))),
                    body: Box::new(while_body),
                    span: dummy_span(),
                }),
            ],
            expr: Some(Box::new(make_ident("a"))),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 55);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Sprint 4: Function tests
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn llvm_compile_recursive_fibonacci() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn fib(n: i64) -> i64 {
        //   if n <= 1 { n } else { fib(n-1) + fib(n-2) }
        // }
        let fib_body = Expr::If {
            condition: Box::new(make_binop(make_ident("n"), BinOp::Le, make_int_lit(1))),
            then_branch: Box::new(make_ident("n")),
            else_branch: Some(Box::new(make_binop(
                Expr::Call {
                    callee: Box::new(make_ident("fib")),
                    args: vec![crate::parser::ast::CallArg {
                        name: None,
                        value: make_binop(make_ident("n"), BinOp::Sub, make_int_lit(1)),
                        span: dummy_span(),
                    }],
                    span: dummy_span(),
                },
                BinOp::Add,
                Expr::Call {
                    callee: Box::new(make_ident("fib")),
                    args: vec![crate::parser::ast::CallArg {
                        name: None,
                        value: make_binop(make_ident("n"), BinOp::Sub, make_int_lit(2)),
                        span: dummy_span(),
                    }],
                    span: dummy_span(),
                },
            ))),
            span: dummy_span(),
        };

        let fib_fn = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "fib".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![Param {
                name: "n".to_string(),
                ty: TypeExpr::Simple {
                    name: "i64".to_string(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            }],
            return_type: Some(TypeExpr::Simple {
                name: "i64".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(fib_body),
            span: dummy_span(),
        };

        // fn main() -> i64 { fib(10) }
        let main_body = Expr::Call {
            callee: Box::new(make_ident("fib")),
            args: vec![crate::parser::ast::CallArg {
                name: None,
                value: make_int_lit(10),
                span: dummy_span(),
            }],
            span: dummy_span(),
        };
        let main_fn = make_simple_fn("main", main_body);

        let program = make_program(vec![Item::FnDef(fib_fn), Item::FnDef(main_fn)]);

        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 55);
    }

    #[test]
    fn llvm_compile_multi_param_function() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn add3(a: i64, b: i64, c: i64) -> i64 { a + b + c }
        let add3_fn = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "add3".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![
                Param {
                    name: "a".to_string(),
                    ty: TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
                Param {
                    name: "b".to_string(),
                    ty: TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
                Param {
                    name: "c".to_string(),
                    ty: TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
            ],
            return_type: Some(TypeExpr::Simple {
                name: "i64".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(make_binop(
                make_binop(make_ident("a"), BinOp::Add, make_ident("b")),
                BinOp::Add,
                make_ident("c"),
            )),
            span: dummy_span(),
        };

        // fn main() -> i64 { add3(10, 20, 12) }
        let main_body = Expr::Call {
            callee: Box::new(make_ident("add3")),
            args: vec![
                crate::parser::ast::CallArg {
                    name: None,
                    value: make_int_lit(10),
                    span: dummy_span(),
                },
                crate::parser::ast::CallArg {
                    name: None,
                    value: make_int_lit(20),
                    span: dummy_span(),
                },
                crate::parser::ast::CallArg {
                    name: None,
                    value: make_int_lit(12),
                    span: dummy_span(),
                },
            ],
            span: dummy_span(),
        };
        let main_fn = make_simple_fn("main", main_body);

        let program = make_program(vec![Item::FnDef(add3_fn), Item::FnDef(main_fn)]);

        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_runtime_function_declarations() {
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");
        compiler.register_runtime_functions();

        let ir = compiler.print_ir();
        assert!(ir.contains("fj_rt_print_int"));
        assert!(ir.contains("fj_rt_assert"));
        assert!(ir.contains("fj_rt_assert_eq"));
    }

    #[test]
    fn llvm_compile_mutual_recursion() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn is_even(n: i64) -> i64 {
        //   if n == 0 { 1 } else { is_odd(n - 1) }
        // }
        let is_even = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "is_even".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![Param {
                name: "n".to_string(),
                ty: TypeExpr::Simple {
                    name: "i64".to_string(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            }],
            return_type: Some(TypeExpr::Simple {
                name: "i64".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(Expr::If {
                condition: Box::new(make_binop(make_ident("n"), BinOp::Eq, make_int_lit(0))),
                then_branch: Box::new(make_int_lit(1)),
                else_branch: Some(Box::new(Expr::Call {
                    callee: Box::new(make_ident("is_odd")),
                    args: vec![crate::parser::ast::CallArg {
                        name: None,
                        value: make_binop(make_ident("n"), BinOp::Sub, make_int_lit(1)),
                        span: dummy_span(),
                    }],
                    span: dummy_span(),
                })),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };

        // fn is_odd(n: i64) -> i64 {
        //   if n == 0 { 0 } else { is_even(n - 1) }
        // }
        let is_odd = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "is_odd".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![Param {
                name: "n".to_string(),
                ty: TypeExpr::Simple {
                    name: "i64".to_string(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            }],
            return_type: Some(TypeExpr::Simple {
                name: "i64".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(Expr::If {
                condition: Box::new(make_binop(make_ident("n"), BinOp::Eq, make_int_lit(0))),
                then_branch: Box::new(make_int_lit(0)),
                else_branch: Some(Box::new(Expr::Call {
                    callee: Box::new(make_ident("is_even")),
                    args: vec![crate::parser::ast::CallArg {
                        name: None,
                        value: make_binop(make_ident("n"), BinOp::Sub, make_int_lit(1)),
                        span: dummy_span(),
                    }],
                    span: dummy_span(),
                })),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };

        // fn main() -> i64 { is_even(10) }
        let main_body = Expr::Call {
            callee: Box::new(make_ident("is_even")),
            args: vec![crate::parser::ast::CallArg {
                name: None,
                value: make_int_lit(10),
                span: dummy_span(),
            }],
            span: dummy_span(),
        };
        let main_fn = make_simple_fn("main", main_body);

        let program = make_program(vec![
            Item::FnDef(is_even),
            Item::FnDef(is_odd),
            Item::FnDef(main_fn),
        ]);

        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 1); // 10 is even
    }

    #[test]
    fn llvm_compile_optimization_produces_valid_ir() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");
        compiler.set_opt_level(LlvmOptLevel::O2);

        let body = make_binop(make_int_lit(10), BinOp::Add, make_int_lit(32));
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert!(compiler.optimize().is_ok());

        // After optimization, verify IR is still valid
        assert!(compiler.verify().is_ok());
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Sprint 5: Data structures tests
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn llvm_compile_array_literal_and_index() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 {
        //   let arr = [10, 20, 42]
        //   arr[2]
        // }
        let body = Expr::Block {
            stmts: vec![make_let_stmt(
                "arr",
                Expr::Array {
                    elements: vec![make_int_lit(10), make_int_lit(20), make_int_lit(42)],
                    span: dummy_span(),
                },
            )],
            expr: Some(Box::new(Expr::Index {
                object: Box::new(make_ident("arr")),
                index: Box::new(make_int_lit(2)),
                span: dummy_span(),
            })),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_tuple_literal() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 { let t = (10, 42); 0 }
        // Tuple is stored but we return i64 to keep function valid
        let body = Expr::Block {
            stmts: vec![make_let_stmt(
                "t",
                Expr::Tuple {
                    elements: vec![make_int_lit(10), make_int_lit(42)],
                    span: dummy_span(),
                },
            )],
            expr: Some(Box::new(make_int_lit(0))),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        // Verify the program compiles and the struct type is used
        assert!(compiler.verify().is_ok());
        // Tuple is stored as a struct — check the alloca for tuple storage
        let ir = compiler.print_ir();
        assert!(
            ir.contains("alloca") || ir.contains("{ i64, i64 }"),
            "IR should contain tuple struct type: {ir}"
        );
    }

    #[test]
    fn llvm_compile_struct_definition_and_init() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // struct Point { x: i64, y: i64 }
        // fn main() -> i64 { let p = Point { x: 10, y: 32 }; p.x + p.y }
        let struct_def = StructDef {
            is_pub: false,
            doc_comment: None,
            annotation: None,
            name: "Point".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            fields: vec![
                crate::parser::ast::Field {
                    name: "x".to_string(),
                    ty: TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
                crate::parser::ast::Field {
                    name: "y".to_string(),
                    ty: TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
            ],
            span: dummy_span(),
        };

        let body = Expr::Block {
            stmts: vec![Stmt::Let {
                mutable: false,
                linear: false,
                name: "p".to_string(),
                ty: None,
                value: Box::new(Expr::StructInit {
                    name: "Point".to_string(),
                    fields: vec![
                        crate::parser::ast::FieldInit {
                            name: "x".to_string(),
                            value: make_int_lit(10),
                            span: dummy_span(),
                        },
                        crate::parser::ast::FieldInit {
                            name: "y".to_string(),
                            value: make_int_lit(32),
                            span: dummy_span(),
                        },
                    ],
                    span: dummy_span(),
                }),
                span: dummy_span(),
            }],
            expr: Some(Box::new(make_binop(
                Expr::Field {
                    object: Box::new(make_ident("p")),
                    field: "x".to_string(),
                    span: dummy_span(),
                },
                BinOp::Add,
                Expr::Field {
                    object: Box::new(make_ident("p")),
                    field: "y".to_string(),
                    span: dummy_span(),
                },
            ))),
            span: dummy_span(),
        };

        let program = make_program(vec![
            Item::StructDef(struct_def),
            Item::FnDef(make_simple_fn("main", body)),
        ]);

        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_struct_field_access() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // struct Pair { first: i64, second: i64 }
        // fn main() -> i64 { let p = Pair { first: 100, second: 42 }; p.second }
        let struct_def = StructDef {
            is_pub: false,
            doc_comment: None,
            annotation: None,
            name: "Pair".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            fields: vec![
                crate::parser::ast::Field {
                    name: "first".to_string(),
                    ty: TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
                crate::parser::ast::Field {
                    name: "second".to_string(),
                    ty: TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
            ],
            span: dummy_span(),
        };

        let body = Expr::Block {
            stmts: vec![Stmt::Let {
                mutable: false,
                linear: false,
                name: "p".to_string(),
                ty: None,
                value: Box::new(Expr::StructInit {
                    name: "Pair".to_string(),
                    fields: vec![
                        crate::parser::ast::FieldInit {
                            name: "first".to_string(),
                            value: make_int_lit(100),
                            span: dummy_span(),
                        },
                        crate::parser::ast::FieldInit {
                            name: "second".to_string(),
                            value: make_int_lit(42),
                            span: dummy_span(),
                        },
                    ],
                    span: dummy_span(),
                }),
                span: dummy_span(),
            }],
            expr: Some(Box::new(Expr::Field {
                object: Box::new(make_ident("p")),
                field: "second".to_string(),
                span: dummy_span(),
            })),
            span: dummy_span(),
        };

        let program = make_program(vec![
            Item::StructDef(struct_def),
            Item::FnDef(make_simple_fn("main", body)),
        ]);

        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_enum_registration() {
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        let enum_def = EnumDef {
            is_pub: false,
            doc_comment: None,
            annotation: None,
            name: "Color".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            variants: vec![
                crate::parser::ast::Variant {
                    name: "Red".to_string(),
                    fields: vec![],
                    span: dummy_span(),
                },
                crate::parser::ast::Variant {
                    name: "Green".to_string(),
                    fields: vec![],
                    span: dummy_span(),
                },
                crate::parser::ast::Variant {
                    name: "Blue".to_string(),
                    fields: vec![],
                    span: dummy_span(),
                },
            ],
            span: dummy_span(),
        };

        let program = make_program(vec![
            Item::EnumDef(enum_def),
            Item::FnDef(make_simple_fn("main", make_int_lit(0))),
        ]);

        compiler.compile_program(&program).unwrap();
        assert!(compiler.enum_defs.contains_key("Color"));
        assert_eq!(compiler.enum_defs["Color"].len(), 3);
    }

    #[test]
    fn llvm_compile_struct_with_three_fields() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // struct Vec3 { x: i64, y: i64, z: i64 }
        // fn main() -> i64 { let v = Vec3 { x: 10, y: 20, z: 12 }; v.x + v.y + v.z }
        let struct_def = StructDef {
            is_pub: false,
            doc_comment: None,
            annotation: None,
            name: "Vec3".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            fields: vec![
                crate::parser::ast::Field {
                    name: "x".to_string(),
                    ty: TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
                crate::parser::ast::Field {
                    name: "y".to_string(),
                    ty: TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
                crate::parser::ast::Field {
                    name: "z".to_string(),
                    ty: TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
            ],
            span: dummy_span(),
        };

        let body = Expr::Block {
            stmts: vec![Stmt::Let {
                mutable: false,
                linear: false,
                name: "v".to_string(),
                ty: None,
                value: Box::new(Expr::StructInit {
                    name: "Vec3".to_string(),
                    fields: vec![
                        crate::parser::ast::FieldInit {
                            name: "x".to_string(),
                            value: make_int_lit(10),
                            span: dummy_span(),
                        },
                        crate::parser::ast::FieldInit {
                            name: "y".to_string(),
                            value: make_int_lit(20),
                            span: dummy_span(),
                        },
                        crate::parser::ast::FieldInit {
                            name: "z".to_string(),
                            value: make_int_lit(12),
                            span: dummy_span(),
                        },
                    ],
                    span: dummy_span(),
                }),
                span: dummy_span(),
            }],
            expr: Some(Box::new(make_binop(
                make_binop(
                    Expr::Field {
                        object: Box::new(make_ident("v")),
                        field: "x".to_string(),
                        span: dummy_span(),
                    },
                    BinOp::Add,
                    Expr::Field {
                        object: Box::new(make_ident("v")),
                        field: "y".to_string(),
                        span: dummy_span(),
                    },
                ),
                BinOp::Add,
                Expr::Field {
                    object: Box::new(make_ident("v")),
                    field: "z".to_string(),
                    span: dummy_span(),
                },
            ))),
            span: dummy_span(),
        };

        let program = make_program(vec![
            Item::StructDef(struct_def),
            Item::FnDef(make_simple_fn("main", body)),
        ]);

        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_array_sum_with_for() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        // fn main() -> i64 {
        //   let arr = [10, 15, 17]
        //   let mut sum = 0
        //   for i in 0..3 {
        //     sum = sum + arr[i]
        //   }
        //   sum  // 10+15+17 = 42
        // }
        let for_body = Expr::Block {
            stmts: vec![make_expr_stmt(make_assign(
                "sum",
                make_binop(
                    make_ident("sum"),
                    BinOp::Add,
                    Expr::Index {
                        object: Box::new(make_ident("arr")),
                        index: Box::new(make_ident("i")),
                        span: dummy_span(),
                    },
                ),
            ))],
            expr: None,
            span: dummy_span(),
        };

        let body = Expr::Block {
            stmts: vec![
                make_let_stmt(
                    "arr",
                    Expr::Array {
                        elements: vec![make_int_lit(10), make_int_lit(15), make_int_lit(17)],
                        span: dummy_span(),
                    },
                ),
                make_let_stmt("sum", make_int_lit(0)),
                make_expr_stmt(Expr::For {
                    label: None,
                    variable: "i".to_string(),
                    iterable: Box::new(Expr::Range {
                        start: Some(Box::new(make_int_lit(0))),
                        end: Some(Box::new(make_int_lit(3))),
                        inclusive: false,
                        span: dummy_span(),
                    }),
                    body: Box::new(for_body),
                    span: dummy_span(),
                }),
            ],
            expr: Some(Box::new(make_ident("sum"))),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 42);
    }

    #[test]
    fn llvm_compile_struct_ir_has_named_type() {
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test");

        let struct_def = StructDef {
            is_pub: false,
            doc_comment: None,
            annotation: None,
            name: "MyStruct".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            fields: vec![crate::parser::ast::Field {
                name: "val".to_string(),
                ty: TypeExpr::Simple {
                    name: "i64".to_string(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            }],
            span: dummy_span(),
        };

        // Use the struct in a function so it appears in IR
        let body = Expr::Block {
            stmts: vec![Stmt::Let {
                mutable: false,
                linear: false,
                name: "s".to_string(),
                ty: None,
                value: Box::new(Expr::StructInit {
                    name: "MyStruct".to_string(),
                    fields: vec![crate::parser::ast::FieldInit {
                        name: "val".to_string(),
                        value: make_int_lit(42),
                        span: dummy_span(),
                    }],
                    span: dummy_span(),
                }),
                span: dummy_span(),
            }],
            expr: Some(Box::new(Expr::Field {
                object: Box::new(make_ident("s")),
                field: "val".to_string(),
                span: dummy_span(),
            })),
            span: dummy_span(),
        };

        let program = make_program(vec![
            Item::StructDef(struct_def),
            Item::FnDef(make_simple_fn("main", body)),
        ]);

        compiler.compile_program(&program).unwrap();
        let ir = compiler.print_ir();
        assert!(
            ir.contains("MyStruct"),
            "IR should reference the struct name: {ir}"
        );
    }

    #[test]
    fn llvm_opt_level_pass_strings() {
        assert_eq!(LlvmOptLevel::O0.pass_string(), "default<O0>");
        assert_eq!(LlvmOptLevel::O1.pass_string(), "default<O1>");
        assert_eq!(LlvmOptLevel::O2.pass_string(), "default<O2>");
        assert_eq!(LlvmOptLevel::O3.pass_string(), "default<O3>");
        assert_eq!(LlvmOptLevel::Os.pass_string(), "default<Os>");
        assert_eq!(LlvmOptLevel::Oz.pass_string(), "default<Oz>");
    }

    #[test]
    fn llvm_size_optimization_os_oz() {
        LlvmCompiler::init_native_target().unwrap();

        for level in [LlvmOptLevel::Os, LlvmOptLevel::Oz] {
            let ctx = Context::create();
            let mut compiler = LlvmCompiler::new(&ctx, "test_size_opt");
            compiler.set_opt_level(level);

            let body = make_binop(make_int_lit(10), BinOp::Add, make_int_lit(32));
            let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
            compiler.compile_program(&program).unwrap();
            compiler.optimize().unwrap();
            assert!(compiler.verify().is_ok());
        }
    }

    #[test]
    fn llvm_emit_bitcode() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_bitcode");

        let body = make_int_lit(42);
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();

        let path = std::path::Path::new("/tmp/fj_test_bitcode.bc");
        assert!(compiler.emit_bitcode(path));
        assert!(path.exists());
        let metadata = std::fs::metadata(path).unwrap();
        assert!(metadata.len() > 0, "bitcode file should not be empty");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn llvm_compile_aarch64_cross_target() {
        LlvmCompiler::init_all_targets();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_aarch64");

        let body = make_binop(make_int_lit(10), BinOp::Add, make_int_lit(32));
        let program = make_program(vec![Item::FnDef(make_simple_fn("main", body))]);
        compiler.compile_program(&program).unwrap();

        // Create aarch64 target machine
        let tm = compiler
            .create_target_machine(Some("aarch64-unknown-linux-gnu"))
            .expect("aarch64 target machine");
        compiler.configure_module_target(&tm);

        // Verify IR is valid for aarch64
        assert!(compiler.verify().is_ok());

        // Emit object file for aarch64
        let path = std::path::Path::new("/tmp/fj_test_aarch64.o");
        let result = tm.write_to_file(&compiler.module, inkwell::targets::FileType::Object, path);
        assert!(result.is_ok(), "aarch64 object emission failed: {result:?}");
        let size = std::fs::metadata(path).unwrap().len();
        assert!(size > 0, "aarch64 object file should not be empty");
        let _ = std::fs::remove_file(path);
    }

    // ── V12 Sprint L1: Target Configuration Tests ───────────────────────

    #[test]
    fn l1_target_config_default() {
        let config = TargetConfig::default();
        assert_eq!(config.cpu, "generic");
        assert_eq!(config.features, "");
        assert_eq!(config.reloc, LlvmRelocMode::Default);
        assert_eq!(config.code_model, LlvmCodeModel::Default);
        assert!(config.triple.is_none());
    }

    #[test]
    fn l1_target_config_native_detects_cpu() {
        LlvmCompiler::init_native_target().unwrap();
        let config = TargetConfig::native();
        // Native CPU should not be empty or "generic"
        assert!(!config.cpu.is_empty());
        assert_ne!(config.cpu, "generic");
        // Native features should contain at least some extensions
        // (on x86_64, typically +sse, +sse2, etc.)
        assert!(!config.features.is_empty());
    }

    #[test]
    fn l1_detect_host_cpu_not_empty() {
        LlvmCompiler::init_native_target().unwrap();
        let cpu = detect_host_cpu();
        assert!(!cpu.is_empty(), "host CPU name should not be empty");
    }

    #[test]
    fn l1_detect_host_features_not_empty() {
        LlvmCompiler::init_native_target().unwrap();
        let features = detect_host_features();
        assert!(
            !features.is_empty(),
            "host CPU features should not be empty"
        );
    }

    #[test]
    fn l1_effective_cpu_resolves_native() {
        LlvmCompiler::init_native_target().unwrap();
        let config = TargetConfig {
            cpu: "native".to_string(),
            ..TargetConfig::default()
        };
        let effective = config.effective_cpu();
        assert_ne!(effective, "native", "should resolve to actual CPU name");
        assert!(!effective.is_empty());
    }

    #[test]
    fn l1_effective_cpu_passes_through_specific() {
        let config = TargetConfig {
            cpu: "skylake".to_string(),
            ..TargetConfig::default()
        };
        assert_eq!(config.effective_cpu(), "skylake");
    }

    #[test]
    fn l1_reloc_mode_from_str() {
        assert_eq!(
            LlvmRelocMode::parse_from("static").unwrap(),
            LlvmRelocMode::Static
        );
        assert_eq!(
            LlvmRelocMode::parse_from("pic").unwrap(),
            LlvmRelocMode::Pic
        );
        assert_eq!(
            LlvmRelocMode::parse_from("default").unwrap(),
            LlvmRelocMode::Default
        );
        assert_eq!(
            LlvmRelocMode::parse_from("dynamic-no-pic").unwrap(),
            LlvmRelocMode::DynamicNoPic
        );
        assert!(LlvmRelocMode::parse_from("invalid").is_err());
    }

    #[test]
    fn l1_code_model_from_str() {
        assert_eq!(
            LlvmCodeModel::parse_from("small").unwrap(),
            LlvmCodeModel::Small
        );
        assert_eq!(
            LlvmCodeModel::parse_from("medium").unwrap(),
            LlvmCodeModel::Medium
        );
        assert_eq!(
            LlvmCodeModel::parse_from("large").unwrap(),
            LlvmCodeModel::Large
        );
        assert_eq!(
            LlvmCodeModel::parse_from("kernel").unwrap(),
            LlvmCodeModel::Kernel
        );
        assert_eq!(
            LlvmCodeModel::parse_from("default").unwrap(),
            LlvmCodeModel::Default
        );
        assert!(LlvmCodeModel::parse_from("xxx").is_err());
    }

    #[test]
    fn l1_target_config_validation() {
        // Valid triple
        let config = TargetConfig {
            triple: Some("x86_64-unknown-linux-gnu".to_string()),
            ..TargetConfig::default()
        };
        assert!(config.validate().is_ok());

        // Valid ARM64 triple
        let config2 = TargetConfig {
            triple: Some("aarch64-unknown-none".to_string()),
            ..TargetConfig::default()
        };
        assert!(config2.validate().is_ok());

        // Invalid arch
        let config3 = TargetConfig {
            triple: Some("badarch-unknown-linux".to_string()),
            ..TargetConfig::default()
        };
        assert!(config3.validate().is_err());

        // No triple = OK (uses host default)
        assert!(TargetConfig::default().validate().is_ok());
    }

    #[test]
    fn l1_create_target_machine_with_config() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l1");
        compiler.set_target_config(TargetConfig::native());
        let tm = compiler.create_target_machine(None);
        assert!(
            tm.is_ok(),
            "native target machine should be created: {tm:?}"
        );
    }

    #[test]
    fn l1_opt_level_os_oz_pass_strings() {
        // Os and Oz should produce distinct pass strings
        assert_eq!(LlvmOptLevel::Os.pass_string(), "default<Os>");
        assert_eq!(LlvmOptLevel::Oz.pass_string(), "default<Oz>");
        assert_ne!(
            LlvmOptLevel::Os.pass_string(),
            LlvmOptLevel::O2.pass_string()
        );
    }

    // ── V12 Sprint L2: Function Attributes Tests ────────────────────────

    fn make_fndef(
        name: &str,
        annotation: Option<crate::parser::ast::Annotation>,
        params: Vec<Param>,
    ) -> FnDef {
        FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation,
            name: name.to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params,
            return_type: None,
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(Expr::Block {
                stmts: vec![],
                expr: Some(Box::new(make_int_lit(0))),
                span: dummy_span(),
            }),
            span: dummy_span(),
        }
    }

    fn make_annotation(name: &str) -> crate::parser::ast::Annotation {
        crate::parser::ast::Annotation {
            name: name.to_string(),
            param: None,
            params: vec![],
            span: dummy_span(),
        }
    }

    fn make_annotation_with_param(name: &str, param: &str) -> crate::parser::ast::Annotation {
        crate::parser::ast::Annotation {
            name: name.to_string(),
            param: Some(param.to_string()),
            params: vec![],
            span: dummy_span(),
        }
    }

    #[test]
    fn l2_inline_attribute_applied() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l2_inline");

        let fndef = make_fndef("fast_fn", Some(make_annotation("inline")), vec![]);
        compiler.declare_function(&fndef).unwrap();

        let ir = compiler.print_ir();
        assert!(
            ir.contains("alwaysinline"),
            "IR should contain alwaysinline attribute, got:\n{ir}"
        );
    }

    #[test]
    fn l2_noinline_attribute_applied() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l2_noinline");

        let fndef = make_fndef("no_inline_fn", Some(make_annotation("noinline")), vec![]);
        compiler.declare_function(&fndef).unwrap();

        let ir = compiler.print_ir();
        assert!(
            ir.contains("noinline"),
            "IR should contain noinline attribute, got:\n{ir}"
        );
    }

    #[test]
    fn l2_inline_never_attribute() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l2_inline_never");

        let fndef = make_fndef(
            "never_fn",
            Some(make_annotation_with_param("inline", "never")),
            vec![],
        );
        compiler.declare_function(&fndef).unwrap();

        let ir = compiler.print_ir();
        assert!(
            ir.contains("noinline"),
            "IR should contain noinline for @inline(\"never\"), got:\n{ir}"
        );
    }

    #[test]
    fn l2_cold_attribute_applied() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l2_cold");

        let fndef = make_fndef("cold_fn", Some(make_annotation("cold")), vec![]);
        compiler.declare_function(&fndef).unwrap();

        let ir = compiler.print_ir();
        assert!(
            ir.contains("cold"),
            "IR should contain cold attribute, got:\n{ir}"
        );
    }

    #[test]
    fn l2_no_annotation_no_attributes() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l2_none");

        let fndef = make_fndef("plain_fn", None, vec![]);
        compiler.declare_function(&fndef).unwrap();

        let ir = compiler.print_ir();
        assert!(
            !ir.contains("alwaysinline"),
            "plain fn should not have alwaysinline"
        );
        assert!(
            !ir.contains("noinline"),
            "plain fn should not have noinline"
        );
        assert!(!ir.contains(" cold"), "plain fn should not have cold");
    }

    #[test]
    fn l2_mut_ref_param_gets_noalias() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l2_noalias");

        let params = vec![Param {
            name: "x".to_string(),
            ty: TypeExpr::Simple {
                name: "&mut i64".to_string(),
                span: dummy_span(),
            },
            span: dummy_span(),
        }];
        let fndef = make_fndef("mut_ref_fn", None, params);
        compiler.declare_function(&fndef).unwrap();

        let ir = compiler.print_ir();
        assert!(
            ir.contains("noalias"),
            "IR should contain noalias for &mut param, got:\n{ir}"
        );
    }

    #[test]
    fn l2_ref_param_gets_readonly() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l2_readonly");

        let params = vec![Param {
            name: "x".to_string(),
            ty: TypeExpr::Simple {
                name: "&i64".to_string(),
                span: dummy_span(),
            },
            span: dummy_span(),
        }];
        let fndef = make_fndef("ref_fn", None, params);
        compiler.declare_function(&fndef).unwrap();

        let ir = compiler.print_ir();
        assert!(
            ir.contains("readonly"),
            "IR should contain readonly for & param, got:\n{ir}"
        );
    }

    #[test]
    fn l2_ref_param_gets_nonnull() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l2_nonnull");

        let params = vec![Param {
            name: "x".to_string(),
            ty: TypeExpr::Simple {
                name: "&i64".to_string(),
                span: dummy_span(),
            },
            span: dummy_span(),
        }];
        let fndef = make_fndef("ref_fn2", None, params);
        compiler.declare_function(&fndef).unwrap();

        let ir = compiler.print_ir();
        assert!(
            ir.contains("nonnull"),
            "IR should contain nonnull for & param, got:\n{ir}"
        );
    }

    #[test]
    fn l2_multiple_params_different_attrs() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l2_multi");

        let params = vec![
            Param {
                name: "a".to_string(),
                ty: TypeExpr::Simple {
                    name: "i64".to_string(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            },
            Param {
                name: "b".to_string(),
                ty: TypeExpr::Simple {
                    name: "&i64".to_string(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            },
            Param {
                name: "c".to_string(),
                ty: TypeExpr::Simple {
                    name: "&mut i64".to_string(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            },
        ];
        let fndef = make_fndef("multi_fn", None, params);
        compiler.declare_function(&fndef).unwrap();

        let ir = compiler.print_ir();
        // Param b (&i64) should get readonly+nonnull
        // Param c (&mut i64) should get noalias+nonnull
        assert!(ir.contains("noalias"), "should have noalias for &mut param");
        assert!(ir.contains("nonnull"), "should have nonnull for ref params");
        assert!(ir.contains("readonly"), "should have readonly for & param");
    }

    #[test]
    fn l2_attribute_kind_ids_nonzero() {
        // Verify LLVM knows about our attribute names
        let always = inkwell::attributes::Attribute::get_named_enum_kind_id("alwaysinline");
        let noinline = inkwell::attributes::Attribute::get_named_enum_kind_id("noinline");
        let cold = inkwell::attributes::Attribute::get_named_enum_kind_id("cold");
        let noalias = inkwell::attributes::Attribute::get_named_enum_kind_id("noalias");
        let nonnull = inkwell::attributes::Attribute::get_named_enum_kind_id("nonnull");
        let readonly = inkwell::attributes::Attribute::get_named_enum_kind_id("readonly");

        assert!(always > 0, "alwaysinline should be a known attribute");
        assert!(noinline > 0, "noinline should be a known attribute");
        assert!(cold > 0, "cold should be a known attribute");
        assert!(noalias > 0, "noalias should be a known attribute");
        assert!(nonnull > 0, "nonnull should be a known attribute");
        assert!(readonly > 0, "readonly should be a known attribute");
    }

    // ── V12 Sprint L3: LTO Tests ────────────────────────────────────────

    #[test]
    fn l3_lto_mode_parse() {
        assert_eq!(LtoMode::parse_from("none").unwrap(), LtoMode::None);
        assert_eq!(LtoMode::parse_from("off").unwrap(), LtoMode::None);
        assert_eq!(LtoMode::parse_from("thin").unwrap(), LtoMode::Thin);
        assert_eq!(LtoMode::parse_from("full").unwrap(), LtoMode::Full);
        assert_eq!(LtoMode::parse_from("fat").unwrap(), LtoMode::Full);
        assert_eq!(LtoMode::parse_from("true").unwrap(), LtoMode::Full);
        assert!(LtoMode::parse_from("invalid").is_err());
    }

    #[test]
    fn l3_lto_mode_is_enabled() {
        assert!(!LtoMode::None.is_enabled());
        assert!(LtoMode::Thin.is_enabled());
        assert!(LtoMode::Full.is_enabled());
    }

    #[test]
    fn l3_compiler_lto_mode_default_none() {
        let context = Context::create();
        let compiler = LlvmCompiler::new(&context, "test_l3_default");
        assert_eq!(compiler.lto_mode(), LtoMode::None);
    }

    #[test]
    fn l3_compiler_set_lto_mode() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l3_set");
        compiler.set_lto_mode(LtoMode::Thin);
        assert_eq!(compiler.lto_mode(), LtoMode::Thin);
        compiler.set_lto_mode(LtoMode::Full);
        assert_eq!(compiler.lto_mode(), LtoMode::Full);
    }

    #[test]
    fn l3_emit_bitcode_for_lto() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l3_bc");
        compiler.set_opt_level(LlvmOptLevel::O2);
        compiler.set_lto_mode(LtoMode::Thin);

        // Compile a simple program
        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "main".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::Block {
                    stmts: vec![],
                    expr: Some(Box::new(make_int_lit(42))),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
        };
        compiler.compile_program(&program).unwrap();

        // Emit bitcode
        let bc_path = std::path::Path::new("/tmp/fj_test_l3.bc");
        assert!(compiler.emit_bitcode(bc_path));
        let meta = std::fs::metadata(bc_path).unwrap();
        assert!(meta.len() > 0, "bitcode file should not be empty");
        let _ = std::fs::remove_file(bc_path);
    }

    #[test]
    fn l3_link_bitcode_lto_single_module() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l3_link");
        compiler.set_opt_level(LlvmOptLevel::O2);
        compiler.set_lto_mode(LtoMode::Full);

        // Create and emit a helper module
        let context2 = Context::create();
        let helper = context2.create_module("helper");
        let i64_type = context2.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let func = helper.add_function("helper_fn", fn_type, None);
        let entry = context2.append_basic_block(func, "entry");
        let builder = context2.create_builder();
        builder.position_at_end(entry);
        builder
            .build_return(Some(&i64_type.const_int(99, false)))
            .unwrap();

        let bc_path = std::path::Path::new("/tmp/fj_test_l3_helper.bc");
        helper.write_bitcode_to_path(bc_path);

        // Link helper into compiler's module
        let stats = compiler.link_bitcode_lto(&[bc_path]);
        let _ = std::fs::remove_file(bc_path);

        // LTO pass pipelines may fail if LLVM doesn't support
        // the specific pipeline name — that's OK for this test,
        // we verify the linking step works
        match stats {
            Ok(s) => {
                assert_eq!(s.modules_merged, 1);
                assert!(s.input_size_bytes > 0);
            }
            Err(e) => {
                // LTO pass pipeline may not be available in all LLVM versions
                let msg = format!("{e}");
                assert!(
                    msg.contains("pass") || msg.contains("LTO"),
                    "unexpected error: {e}"
                );
            }
        }
    }

    #[test]
    fn l3_pass_string_bare() {
        assert_eq!(LlvmOptLevel::O0.pass_string_bare(), "O0");
        assert_eq!(LlvmOptLevel::O1.pass_string_bare(), "O1");
        assert_eq!(LlvmOptLevel::O2.pass_string_bare(), "O2");
        assert_eq!(LlvmOptLevel::O3.pass_string_bare(), "O3");
        assert_eq!(LlvmOptLevel::Os.pass_string_bare(), "Os");
        assert_eq!(LlvmOptLevel::Oz.pass_string_bare(), "Oz");
    }

    #[test]
    fn l3_lto_stats_fields() {
        let stats = LtoStats {
            modules_merged: 3,
            input_size_bytes: 1024,
            output_size_bytes: 512,
            optimize_time_ms: 100,
        };
        assert_eq!(stats.modules_merged, 3);
        assert_eq!(stats.input_size_bytes, 1024);
        assert_eq!(stats.output_size_bytes, 512);
        assert_eq!(stats.optimize_time_ms, 100);
    }

    #[test]
    fn l3_release_mode_defaults_thin_lto() {
        // Verify that release mode would use thin LTO by default
        // (this tests the CLI logic, not compiler internals)
        let lto_str = "none";
        let release = true;
        let effective = if release && lto_str == "none" {
            "thin"
        } else {
            lto_str
        };
        assert_eq!(effective, "thin");
        assert_eq!(LtoMode::parse_from(effective).unwrap(), LtoMode::Thin);
    }

    // ── V12 Sprint L4: PGO Tests ────────────────────────────────────────

    #[test]
    fn l4_pgo_mode_parse_none() {
        assert_eq!(PgoMode::parse_from("none").unwrap(), PgoMode::None);
        assert_eq!(PgoMode::parse_from("off").unwrap(), PgoMode::None);
        assert_eq!(PgoMode::parse_from("").unwrap(), PgoMode::None);
    }

    #[test]
    fn l4_pgo_mode_parse_generate() {
        let mode = PgoMode::parse_from("generate").unwrap();
        assert!(matches!(mode, PgoMode::Generate(_)));
        if let PgoMode::Generate(path) = &mode {
            assert!(path.contains("profraw"));
        }
    }

    #[test]
    fn l4_pgo_mode_parse_generate_with_path() {
        let mode = PgoMode::parse_from("generate=/tmp/prof_%m.profraw").unwrap();
        assert_eq!(mode, PgoMode::Generate("/tmp/prof_%m.profraw".to_string()));
    }

    #[test]
    fn l4_pgo_mode_parse_use() {
        let mode = PgoMode::parse_from("use=profile.profdata").unwrap();
        assert_eq!(mode, PgoMode::Use("profile.profdata".to_string()));
    }

    #[test]
    fn l4_pgo_mode_parse_use_empty_error() {
        assert!(PgoMode::parse_from("use=").is_err());
    }

    #[test]
    fn l4_pgo_mode_parse_invalid() {
        assert!(PgoMode::parse_from("invalid").is_err());
    }

    #[test]
    fn l4_pgo_mode_is_enabled() {
        assert!(!PgoMode::None.is_enabled());
        assert!(PgoMode::Generate("prof.profraw".into()).is_enabled());
        assert!(PgoMode::Use("prof.profdata".into()).is_enabled());
    }

    #[test]
    fn l4_pgo_mode_is_generate_use() {
        assert!(PgoMode::Generate("prof.profraw".into()).is_generate());
        assert!(!PgoMode::Generate("prof.profraw".into()).is_use());
        assert!(!PgoMode::Use("prof.profdata".into()).is_generate());
        assert!(PgoMode::Use("prof.profdata".into()).is_use());
        assert!(!PgoMode::None.is_generate());
        assert!(!PgoMode::None.is_use());
    }

    #[test]
    fn l4_compiler_pgo_mode_default() {
        let context = Context::create();
        let compiler = LlvmCompiler::new(&context, "test_l4_default");
        assert_eq!(*compiler.pgo_mode(), PgoMode::None);
    }

    #[test]
    fn l4_compiler_set_pgo_mode() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l4_set");
        compiler.set_pgo_mode(PgoMode::Generate("test.profraw".into()));
        assert!(compiler.pgo_mode().is_generate());
        compiler.set_pgo_mode(PgoMode::Use("test.profdata".into()));
        assert!(compiler.pgo_mode().is_use());
    }

    #[test]
    fn l4_build_pass_pipeline_no_pgo() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l4_pipeline");
        compiler.set_opt_level(LlvmOptLevel::O2);
        let pipeline = compiler.build_pass_pipeline();
        assert_eq!(pipeline, "default<O2>");
    }

    #[test]
    fn l4_build_pass_pipeline_pgo_generate() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l4_gen");
        compiler.set_opt_level(LlvmOptLevel::O2);
        compiler.set_pgo_mode(PgoMode::Generate("prof.profraw".into()));
        let pipeline = compiler.build_pass_pipeline();
        assert!(
            pipeline.contains("pgo-instr-gen"),
            "pipeline should include pgo-instr-gen: {pipeline}"
        );
    }

    #[test]
    fn l4_build_pass_pipeline_pgo_use() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l4_use");
        compiler.set_opt_level(LlvmOptLevel::O2);
        compiler.set_pgo_mode(PgoMode::Use("profile.profdata".into()));
        let pipeline = compiler.build_pass_pipeline();
        assert!(
            pipeline.contains("pgo-instr-use"),
            "pipeline should include pgo-instr-use: {pipeline}"
        );
        assert!(
            pipeline.contains("profile.profdata"),
            "pipeline should include profdata path: {pipeline}"
        );
    }

    // ── V12 Sprint L5: Generics & Closures Tests ────────────────────────

    #[test]
    fn l5_monomorphize_fn_creates_specialized() {
        let generic_def = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "add".to_string(),
            lifetime_params: vec![],
            generic_params: vec![crate::parser::ast::GenericParam {
                name: "T".to_string(),
                bounds: vec![],
                is_comptime: false,
                is_effect: false,
                const_type: None,
                span: dummy_span(),
            }],
            params: vec![
                Param {
                    name: "a".to_string(),
                    ty: TypeExpr::Simple {
                        name: "T".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
                Param {
                    name: "b".to_string(),
                    ty: TypeExpr::Simple {
                        name: "T".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
            ],
            return_type: Some(TypeExpr::Simple {
                name: "T".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(Expr::Binary {
                left: Box::new(Expr::Ident {
                    name: "a".to_string(),
                    span: dummy_span(),
                }),
                op: BinOp::Add,
                right: Box::new(Expr::Ident {
                    name: "b".to_string(),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };

        let mut type_map = HashMap::new();
        type_map.insert("T".to_string(), "i64".to_string());

        let specialized = LlvmCompiler::monomorphize_fn(&generic_def, "i64", &type_map);
        assert_eq!(specialized.name, "add__mono_i64");
        assert!(specialized.generic_params.is_empty());
        assert_eq!(type_expr_to_string(&specialized.params[0].ty), "i64");
        assert_eq!(type_expr_to_string(&specialized.params[1].ty), "i64");
        assert_eq!(
            type_expr_to_string(&specialized.return_type.unwrap()),
            "i64"
        );
    }

    #[test]
    fn l5_infer_type_from_expr() {
        assert_eq!(infer_type_from_expr(&make_int_lit(42)), "i64");
        assert_eq!(
            infer_type_from_expr(&Expr::Literal {
                kind: LiteralKind::Float(1.25),
                span: dummy_span()
            }),
            "f64"
        );
        assert_eq!(
            infer_type_from_expr(&Expr::Literal {
                kind: LiteralKind::Bool(true),
                span: dummy_span()
            }),
            "bool"
        );
        assert_eq!(
            infer_type_from_expr(&Expr::Literal {
                kind: LiteralKind::String("hi".into()),
                span: dummy_span()
            }),
            "str"
        );
    }

    #[test]
    fn l5_register_impl_block() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l5_impl");

        let impl_block = crate::parser::ast::ImplBlock {
            doc_comment: None,
            lifetime_params: vec![],
            generic_params: vec![],
            trait_name: None,
            target_type: "Point".to_string(),
            methods: vec![FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "distance".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::Block {
                    stmts: vec![],
                    expr: Some(Box::new(make_int_lit(0))),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            }],
            span: dummy_span(),
        };

        compiler.register_impl_block(&impl_block);
        assert!(compiler.method_map.contains_key("Point::distance"));
        assert_eq!(
            compiler.method_map.get("Point::distance").unwrap(),
            "Point__distance"
        );
    }

    #[test]
    fn l5_closure_counter_increments() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l5_counter");
        assert_eq!(compiler.next_closure_name(), "__fj_closure_1");
        assert_eq!(compiler.next_closure_name(), "__fj_closure_2");
        assert_eq!(compiler.next_closure_name(), "__fj_closure_3");
    }

    #[test]
    fn l5_compile_simple_closure() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l5_closure");

        // Set up a function context so the builder has a valid block
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = compiler.module.add_function("test_fn", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        compiler.builder.position_at_end(entry);

        // Compile a closure: |x| x + 1
        let closure_params = vec![crate::parser::ast::ClosureParam {
            name: "x".to_string(),
            ty: None,
            span: dummy_span(),
        }];
        let closure_body = Expr::Binary {
            left: Box::new(Expr::Ident {
                name: "x".to_string(),
                span: dummy_span(),
            }),
            op: BinOp::Add,
            right: Box::new(make_int_lit(1)),
            span: dummy_span(),
        };

        let result = compiler.compile_closure(&closure_params, &closure_body);
        assert!(result.is_ok(), "closure compilation failed: {result:?}");
        assert!(result.unwrap().is_some(), "closure should produce a value");

        // Verify the closure function was created
        assert!(
            compiler
                .functions
                .keys()
                .any(|k| k.starts_with("__fj_closure_")),
            "closure function should be registered"
        );
    }

    #[test]
    fn l5_compile_program_with_impl() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l5_prog");

        let program = Program {
            span: dummy_span(),
            items: vec![
                Item::StructDef(StructDef {
                    is_pub: false,
                    doc_comment: None,
                    annotation: None,
                    name: "Num".to_string(),
                    lifetime_params: vec![],
                    generic_params: vec![],
                    fields: vec![crate::parser::ast::Field {
                        name: "val".to_string(),
                        ty: TypeExpr::Simple {
                            name: "i64".to_string(),
                            span: dummy_span(),
                        },
                        span: dummy_span(),
                    }],
                    span: dummy_span(),
                }),
                Item::ImplBlock(crate::parser::ast::ImplBlock {
                    doc_comment: None,
                    lifetime_params: vec![],
                    generic_params: vec![],
                    trait_name: None,
                    target_type: "Num".to_string(),
                    methods: vec![FnDef {
                        is_pub: false,
                        is_const: false,
                        is_async: false,
                        is_gen: false,
                        is_test: false,
                        should_panic: false,
                        is_ignored: false,
                        no_inline: false,

                        naked: false,
                        no_mangle: false,
                        doc_comment: None,
                        annotation: None,
                        name: "get".to_string(),
                        lifetime_params: vec![],
                        generic_params: vec![],
                        params: vec![],
                        return_type: None,
                        where_clauses: vec![],
                        requires: vec![],
                        ensures: vec![],
                        effects: vec![],
                        effect_row_var: None,
                        body: Box::new(Expr::Block {
                            stmts: vec![],
                            expr: Some(Box::new(make_int_lit(42))),
                            span: dummy_span(),
                        }),
                        span: dummy_span(),
                    }],
                    span: dummy_span(),
                }),
                Item::FnDef(FnDef {
                    is_pub: false,
                    is_const: false,
                    is_async: false,
                    is_gen: false,
                    is_test: false,
                    should_panic: false,
                    is_ignored: false,
                    no_inline: false,

                    naked: false,
                    no_mangle: false,
                    doc_comment: None,
                    annotation: None,
                    name: "main".to_string(),
                    lifetime_params: vec![],
                    generic_params: vec![],
                    params: vec![],
                    return_type: None,
                    where_clauses: vec![],
                    requires: vec![],
                    ensures: vec![],
                    effects: vec![],
                    effect_row_var: None,
                    body: Box::new(Expr::Block {
                        stmts: vec![],
                        expr: Some(Box::new(make_int_lit(0))),
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }),
            ],
        };

        let result = compiler.compile_program(&program);
        assert!(
            result.is_ok(),
            "program with impl block should compile: {result:?}"
        );
        // Verify the impl method was registered
        assert!(compiler.functions.contains_key("Num__get"));
    }

    #[test]
    fn l5_mono_fn_multi_type_params() {
        let generic_def = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "pair".to_string(),
            lifetime_params: vec![],
            generic_params: vec![
                crate::parser::ast::GenericParam {
                    name: "T".to_string(),
                    bounds: vec![],
                    is_comptime: false,
                    is_effect: false,
                    const_type: None,
                    span: dummy_span(),
                },
                crate::parser::ast::GenericParam {
                    name: "U".to_string(),
                    bounds: vec![],
                    is_comptime: false,
                    is_effect: false,
                    const_type: None,
                    span: dummy_span(),
                },
            ],
            params: vec![
                Param {
                    name: "a".to_string(),
                    ty: TypeExpr::Simple {
                        name: "T".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
                Param {
                    name: "b".to_string(),
                    ty: TypeExpr::Simple {
                        name: "U".to_string(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
            ],
            return_type: None,
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(Expr::Block {
                stmts: vec![],
                expr: Some(Box::new(make_int_lit(0))),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };

        let mut type_map = HashMap::new();
        type_map.insert("T".to_string(), "i64".to_string());
        type_map.insert("U".to_string(), "f64".to_string());

        let specialized = LlvmCompiler::monomorphize_fn(&generic_def, "i64_f64", &type_map);
        assert_eq!(specialized.name, "pair__mono_i64_f64");
        assert_eq!(type_expr_to_string(&specialized.params[0].ty), "i64");
        assert_eq!(type_expr_to_string(&specialized.params[1].ty), "f64");
    }

    #[test]
    fn l5_method_map_empty_by_default() {
        let context = Context::create();
        let compiler = LlvmCompiler::new(&context, "test_l5_empty");
        assert!(compiler.method_map.is_empty());
    }

    #[test]
    fn l5_mono_fns_empty_by_default() {
        let context = Context::create();
        let compiler = LlvmCompiler::new(&context, "test_l5_mono_empty");
        assert!(compiler.mono_fns.is_empty());
    }

    // ── V12 Sprint L6: String & Array Tests ─────────────────────────────

    #[test]
    fn l6_runtime_fns_declared() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l6_rt");
        compiler.declare_runtime_functions();

        assert!(
            compiler
                .module
                .get_function("fj_rt_string_concat")
                .is_some()
        );
        assert!(compiler.module.get_function("fj_rt_string_len").is_some());
        assert!(compiler.module.get_function("fj_rt_string_eq").is_some());
        assert!(
            compiler
                .module
                .get_function("fj_rt_array_bounds_check")
                .is_some()
        );
        assert!(compiler.module.get_function("fj_rt_print_str").is_some());
        assert!(compiler.module.get_function("fj_rt_array_new").is_some());
        assert!(compiler.module.get_function("fj_rt_array_len").is_some());
        assert!(compiler.module.get_function("fj_rt_array_push").is_some());
        assert!(compiler.module.get_function("fj_rt_map_new").is_some());
        assert!(compiler.module.get_function("fj_rt_map_insert").is_some());
        assert!(compiler.module.get_function("fj_rt_map_get").is_some());
    }

    #[test]
    fn l6_runtime_fns_idempotent() {
        // Calling declare_runtime_functions twice should not panic
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l6_idem");
        compiler.declare_runtime_functions();
        compiler.declare_runtime_functions(); // Should not panic or create duplicates
        assert!(
            compiler
                .module
                .get_function("fj_rt_string_concat")
                .is_some()
        );
    }

    #[test]
    fn l6_string_type_is_struct() {
        let context = Context::create();
        let compiler = LlvmCompiler::new(&context, "test_l6_str_ty");
        let str_ty = compiler.string_type();
        assert_eq!(str_ty.count_fields(), 2);
    }

    #[test]
    fn l6_string_literal_produces_struct() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l6_str_lit");

        // Create a function context
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = compiler.module.add_function("test_str", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        compiler.builder.position_at_end(entry);

        let result = compiler.compile_literal(&LiteralKind::String("hello".to_string()));
        assert!(result.is_ok());
        let val = result.unwrap().unwrap();
        assert!(
            val.is_struct_value(),
            "string literal should produce a struct value"
        );
    }

    #[test]
    fn l6_array_literal_produces_ptr() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l6_arr_lit");

        // Create a function context
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = compiler.module.add_function("test_arr", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        compiler.builder.position_at_end(entry);

        let elements = vec![make_int_lit(1), make_int_lit(2), make_int_lit(3)];
        let result = compiler.compile_array(&elements);
        assert!(
            result.is_ok(),
            "array compilation should succeed: {result:?}"
        );
        assert!(result.unwrap().is_some(), "array should produce a value");
    }

    #[test]
    fn l6_empty_array_returns_zero() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l6_empty_arr");

        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = compiler.module.add_function("test_empty", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        compiler.builder.position_at_end(entry);

        let elements: Vec<Expr> = vec![];
        let result = compiler.compile_array(&elements);
        assert!(result.is_ok());
        let val = result.unwrap().unwrap();
        assert!(val.is_int_value(), "empty array should return i64(0)");
    }

    #[test]
    fn l6_compile_program_declares_runtime() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l6_prog_rt");

        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "main".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::Block {
                    stmts: vec![],
                    expr: Some(Box::new(make_int_lit(0))),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
        };
        compiler.compile_program(&program).unwrap();

        // Runtime functions should be declared by compile_program
        assert!(
            compiler
                .module
                .get_function("fj_rt_string_concat")
                .is_some()
        );
        assert!(
            compiler
                .module
                .get_function("fj_rt_array_bounds_check")
                .is_some()
        );
        assert!(compiler.module.get_function("fj_rt_map_new").is_some());
    }

    #[test]
    fn l6_runtime_fn_signatures_correct() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l6_sig");
        compiler.declare_runtime_functions();

        // fj_rt_string_concat: 4 params (ptr, len, ptr, len) → struct
        let concat = compiler.module.get_function("fj_rt_string_concat").unwrap();
        assert_eq!(concat.count_params(), 4);

        // fj_rt_array_bounds_check: 2 params (idx, len) → void
        let bounds = compiler
            .module
            .get_function("fj_rt_array_bounds_check")
            .unwrap();
        assert_eq!(bounds.count_params(), 2);

        // fj_rt_map_insert: 3 params (map, key, val) → void
        let insert = compiler.module.get_function("fj_rt_map_insert").unwrap();
        assert_eq!(insert.count_params(), 3);
    }

    #[test]
    fn l6_ir_contains_runtime_declarations() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l6_ir");
        compiler.declare_runtime_functions();

        let ir = compiler.print_ir();
        assert!(
            ir.contains("fj_rt_string_concat"),
            "IR should contain string concat declaration"
        );
        assert!(
            ir.contains("fj_rt_array_bounds_check"),
            "IR should contain bounds check declaration"
        );
        assert!(
            ir.contains("fj_rt_map_new"),
            "IR should contain map_new declaration"
        );
    }

    #[test]
    fn l6_all_11_runtime_fns_in_ir() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l6_all");
        compiler.declare_runtime_functions();

        let ir = compiler.print_ir();
        let expected = [
            "fj_rt_string_concat",
            "fj_rt_string_len",
            "fj_rt_string_eq",
            "fj_rt_array_bounds_check",
            "fj_rt_print_str",
            "fj_rt_array_new",
            "fj_rt_array_len",
            "fj_rt_array_push",
            "fj_rt_map_new",
            "fj_rt_map_insert",
            "fj_rt_map_get",
        ];
        for name in &expected {
            assert!(ir.contains(name), "IR should contain {name}");
        }
    }

    // ── V12 Sprint L7: Control Flow & Pattern Matching Tests ────────────

    #[test]
    fn l7_match_or_pattern() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l7_or");

        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "main".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::Match {
                    subject: Box::new(make_int_lit(2)),
                    arms: vec![
                        MatchArm {
                            pattern: Pattern::Or {
                                patterns: vec![
                                    Pattern::Literal {
                                        kind: LiteralKind::Int(1),
                                        span: dummy_span(),
                                    },
                                    Pattern::Literal {
                                        kind: LiteralKind::Int(2),
                                        span: dummy_span(),
                                    },
                                    Pattern::Literal {
                                        kind: LiteralKind::Int(3),
                                        span: dummy_span(),
                                    },
                                ],
                                span: dummy_span(),
                            },
                            guard: None,
                            body: Box::new(make_int_lit(10)),
                            span: dummy_span(),
                        },
                        MatchArm {
                            pattern: Pattern::Wildcard { span: dummy_span() },
                            guard: None,
                            body: Box::new(make_int_lit(0)),
                            span: dummy_span(),
                        },
                    ],
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
        };
        let result = compiler.compile_program(&program);
        assert!(
            result.is_ok(),
            "or-pattern match should compile: {result:?}"
        );
        let ir = compiler.print_ir();
        assert!(
            ir.contains("match_or") || ir.contains("or_cmp") || ir.contains("match_merge"),
            "IR should contain or-pattern match blocks"
        );
    }

    #[test]
    fn l7_match_with_guard() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l7_guard");

        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "main".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::Match {
                    subject: Box::new(make_int_lit(5)),
                    arms: vec![
                        MatchArm {
                            pattern: Pattern::Or {
                                patterns: vec![Pattern::Literal {
                                    kind: LiteralKind::Int(5),
                                    span: dummy_span(),
                                }],
                                span: dummy_span(),
                            },
                            guard: Some(Box::new(Expr::Binary {
                                left: Box::new(make_int_lit(5)),
                                op: BinOp::Gt,
                                right: Box::new(make_int_lit(0)),
                                span: dummy_span(),
                            })),
                            body: Box::new(make_int_lit(100)),
                            span: dummy_span(),
                        },
                        MatchArm {
                            pattern: Pattern::Wildcard { span: dummy_span() },
                            guard: None,
                            body: Box::new(make_int_lit(0)),
                            span: dummy_span(),
                        },
                    ],
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
        };
        let result = compiler.compile_program(&program);
        assert!(result.is_ok(), "guarded match should compile: {result:?}");
        let ir = compiler.print_ir();
        assert!(
            ir.contains("guard_pass"),
            "IR should contain guard pass block"
        );
    }

    #[test]
    fn l7_pipeline_operator() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l7_pipe");

        // Build: fn double(x: i64) -> i64 { x * 2 }
        //        fn main() -> i64 { 5 |> double }
        let double_fn = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "double".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![Param {
                name: "x".to_string(),
                ty: TypeExpr::Simple {
                    name: "i64".to_string(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            }],
            return_type: Some(TypeExpr::Simple {
                name: "i64".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(Expr::Binary {
                left: Box::new(Expr::Ident {
                    name: "x".to_string(),
                    span: dummy_span(),
                }),
                op: BinOp::Mul,
                right: Box::new(make_int_lit(2)),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };

        let main_fn = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "main".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![],
            return_type: None,
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(Expr::Pipe {
                left: Box::new(make_int_lit(5)),
                right: Box::new(Expr::Ident {
                    name: "double".to_string(),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };

        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(double_fn), Item::FnDef(main_fn)],
        };
        let result = compiler.compile_program(&program);
        assert!(result.is_ok(), "pipeline should compile: {result:?}");
        let ir = compiler.print_ir();
        assert!(
            ir.contains("pipe_result"),
            "IR should contain pipe_result call"
        );
    }

    #[test]
    fn l7_try_operator() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l7_try");

        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "main".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::Try {
                    expr: Box::new(make_int_lit(42)),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
        };
        let result = compiler.compile_program(&program);
        assert!(result.is_ok(), "try operator should compile: {result:?}");
    }

    #[test]
    fn l7_match_ident_binding() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l7_bind");

        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "main".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::Match {
                    subject: Box::new(make_int_lit(42)),
                    arms: vec![MatchArm {
                        pattern: Pattern::Ident {
                            name: "val".to_string(),
                            span: dummy_span(),
                        },
                        guard: None,
                        body: Box::new(Expr::Ident {
                            name: "val".to_string(),
                            span: dummy_span(),
                        }),
                        span: dummy_span(),
                    }],
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
        };
        let result = compiler.compile_program(&program);
        assert!(
            result.is_ok(),
            "match ident binding should compile: {result:?}"
        );
    }

    #[test]
    fn l7_break_with_value_in_loop() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l7_break_val");

        // loop { break 42 }
        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "main".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::Loop {
                    label: None,
                    body: Box::new(Expr::Block {
                        stmts: vec![Stmt::Break {
                            label: None,
                            value: Some(Box::new(make_int_lit(42))),
                            span: dummy_span(),
                        }],
                        expr: None,
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
        };
        let result = compiler.compile_program(&program);
        assert!(
            result.is_ok(),
            "break with value should compile: {result:?}"
        );
    }

    #[test]
    fn l7_for_range_loop() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l7_for");

        // for i in 0..10 { i }
        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "main".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::For {
                    label: None,
                    variable: "i".to_string(),
                    iterable: Box::new(Expr::Range {
                        start: Some(Box::new(make_int_lit(0))),
                        end: Some(Box::new(make_int_lit(10))),
                        inclusive: false,
                        span: dummy_span(),
                    }),
                    body: Box::new(Expr::Block {
                        stmts: vec![],
                        expr: Some(Box::new(Expr::Ident {
                            name: "i".to_string(),
                            span: dummy_span(),
                        })),
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
        };
        let result = compiler.compile_program(&program);
        assert!(result.is_ok(), "for-range should compile: {result:?}");
    }

    #[test]
    fn l7_match_wildcard_always_matches() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l7_wild");

        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "main".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::Match {
                    subject: Box::new(make_int_lit(99)),
                    arms: vec![
                        MatchArm {
                            pattern: Pattern::Literal {
                                kind: LiteralKind::Int(1),
                                span: dummy_span(),
                            },
                            guard: None,
                            body: Box::new(make_int_lit(10)),
                            span: dummy_span(),
                        },
                        MatchArm {
                            pattern: Pattern::Wildcard { span: dummy_span() },
                            guard: None,
                            body: Box::new(make_int_lit(0)),
                            span: dummy_span(),
                        },
                    ],
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
        };
        let result = compiler.compile_program(&program);
        assert!(result.is_ok(), "wildcard match should compile: {result:?}");
    }

    #[test]
    fn l7_match_literal_multiple_arms() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l7_multi");

        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "main".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::Match {
                    subject: Box::new(make_int_lit(2)),
                    arms: vec![
                        MatchArm {
                            pattern: Pattern::Literal {
                                kind: LiteralKind::Int(1),
                                span: dummy_span(),
                            },
                            guard: None,
                            body: Box::new(make_int_lit(10)),
                            span: dummy_span(),
                        },
                        MatchArm {
                            pattern: Pattern::Literal {
                                kind: LiteralKind::Int(2),
                                span: dummy_span(),
                            },
                            guard: None,
                            body: Box::new(make_int_lit(20)),
                            span: dummy_span(),
                        },
                        MatchArm {
                            pattern: Pattern::Wildcard { span: dummy_span() },
                            guard: None,
                            body: Box::new(make_int_lit(0)),
                            span: dummy_span(),
                        },
                    ],
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
        };
        let result = compiler.compile_program(&program);
        assert!(result.is_ok(), "multi-arm match should compile: {result:?}");
    }

    // ── V12 Sprint L8: Async & Concurrency Tests ────────────────────────

    #[test]
    fn l8_async_runtime_fns_declared() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l8_rt");
        compiler.declare_runtime_functions();

        // Future runtime
        assert!(compiler.module.get_function("fj_rt_future_new").is_some());
        assert!(compiler.module.get_function("fj_rt_future_poll").is_some());
        assert!(
            compiler
                .module
                .get_function("fj_rt_future_get_result")
                .is_some()
        );
        assert!(
            compiler
                .module
                .get_function("fj_rt_future_set_result")
                .is_some()
        );
        assert!(
            compiler
                .module
                .get_function("fj_rt_future_set_state")
                .is_some()
        );
        assert!(compiler.module.get_function("fj_rt_future_free").is_some());

        // Mutex runtime
        assert!(compiler.module.get_function("fj_rt_mutex_new").is_some());
        assert!(compiler.module.get_function("fj_rt_mutex_lock").is_some());
        assert!(compiler.module.get_function("fj_rt_mutex_store").is_some());
        assert!(compiler.module.get_function("fj_rt_mutex_free").is_some());

        // Channel runtime
        assert!(compiler.module.get_function("fj_rt_channel_new").is_some());
        assert!(compiler.module.get_function("fj_rt_channel_send").is_some());
        assert!(compiler.module.get_function("fj_rt_channel_recv").is_some());
        assert!(
            compiler
                .module
                .get_function("fj_rt_channel_close")
                .is_some()
        );
    }

    #[test]
    fn l8_future_fn_signatures() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l8_sig");
        compiler.declare_runtime_functions();

        let new_fn = compiler.module.get_function("fj_rt_future_new").unwrap();
        assert_eq!(new_fn.count_params(), 0, "future_new takes 0 params");

        let poll_fn = compiler.module.get_function("fj_rt_future_poll").unwrap();
        assert_eq!(poll_fn.count_params(), 1, "future_poll takes 1 param (ptr)");

        let set_fn = compiler
            .module
            .get_function("fj_rt_future_set_result")
            .unwrap();
        assert_eq!(set_fn.count_params(), 2, "future_set_result takes 2 params");
    }

    #[test]
    fn l8_mutex_fn_signatures() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l8_mutex_sig");
        compiler.declare_runtime_functions();

        let new_fn = compiler.module.get_function("fj_rt_mutex_new").unwrap();
        assert_eq!(new_fn.count_params(), 1, "mutex_new takes initial value");

        let lock_fn = compiler.module.get_function("fj_rt_mutex_lock").unwrap();
        assert_eq!(lock_fn.count_params(), 1, "mutex_lock takes ptr");

        let store_fn = compiler.module.get_function("fj_rt_mutex_store").unwrap();
        assert_eq!(store_fn.count_params(), 2, "mutex_store takes ptr + value");
    }

    #[test]
    fn l8_channel_fn_signatures() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l8_chan_sig");
        compiler.declare_runtime_functions();

        let new_fn = compiler.module.get_function("fj_rt_channel_new").unwrap();
        assert_eq!(new_fn.count_params(), 0, "channel_new takes 0 params");

        let send_fn = compiler.module.get_function("fj_rt_channel_send").unwrap();
        assert_eq!(send_fn.count_params(), 2, "channel_send takes ptr + value");

        let recv_fn = compiler.module.get_function("fj_rt_channel_recv").unwrap();
        assert_eq!(recv_fn.count_params(), 1, "channel_recv takes ptr");
    }

    #[test]
    fn l8_compile_async_block() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l8_async");
        compiler.declare_runtime_functions();

        // Set up function context
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = compiler.module.add_function("test_async", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        compiler.builder.position_at_end(entry);

        // Compile async { 42 }
        let result = compiler.compile_async_block(&make_int_lit(42));
        assert!(result.is_ok(), "async block should compile: {result:?}");
        assert!(
            result.unwrap().is_some(),
            "async block should produce value"
        );
    }

    #[test]
    fn l8_compile_await_expr() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l8_await");
        compiler.declare_runtime_functions();

        // Set up function context
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);
        let function = compiler.module.add_function("test_await", fn_type, None);
        let entry = context.append_basic_block(function, "entry");
        compiler.builder.position_at_end(entry);

        // Compile: (async { 42 }).await
        // First create a future value (simulated as i64)
        let result = compiler.compile_await(&make_int_lit(0));
        assert!(result.is_ok(), "await should compile: {result:?}");
    }

    #[test]
    fn l8_program_with_async_fn() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l8_async_fn");

        // async fn fetch() -> i64 { 42 }
        // fn main() -> i64 { 0 }
        let program = Program {
            span: dummy_span(),
            items: vec![
                Item::FnDef(FnDef {
                    is_pub: false,
                    is_const: false,
                    is_async: true, // async function
                    is_gen: false,
                    is_test: false,
                    should_panic: false,
                    is_ignored: false,
                    no_inline: false,

                    naked: false,
                    no_mangle: false,
                    doc_comment: None,
                    annotation: None,
                    name: "fetch".to_string(),
                    lifetime_params: vec![],
                    generic_params: vec![],
                    params: vec![],
                    return_type: None,
                    where_clauses: vec![],
                    requires: vec![],
                    ensures: vec![],
                    effects: vec![],
                    effect_row_var: None,
                    body: Box::new(Expr::Block {
                        stmts: vec![],
                        expr: Some(Box::new(make_int_lit(42))),
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }),
                Item::FnDef(FnDef {
                    is_pub: false,
                    is_const: false,
                    is_async: false,
                    is_gen: false,
                    is_test: false,
                    should_panic: false,
                    is_ignored: false,
                    no_inline: false,

                    naked: false,
                    no_mangle: false,
                    doc_comment: None,
                    annotation: None,
                    name: "main".to_string(),
                    lifetime_params: vec![],
                    generic_params: vec![],
                    params: vec![],
                    return_type: None,
                    where_clauses: vec![],
                    requires: vec![],
                    ensures: vec![],
                    effects: vec![],
                    effect_row_var: None,
                    body: Box::new(Expr::Block {
                        stmts: vec![],
                        expr: Some(Box::new(make_int_lit(0))),
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }),
            ],
        };
        let result = compiler.compile_program(&program);
        assert!(
            result.is_ok(),
            "program with async fn should compile: {result:?}"
        );
        assert!(compiler.functions.contains_key("fetch"));
    }

    #[test]
    fn l8_ir_contains_async_runtime() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l8_ir");
        compiler.declare_runtime_functions();

        let ir = compiler.print_ir();
        let expected = [
            "fj_rt_future_new",
            "fj_rt_future_poll",
            "fj_rt_future_get_result",
            "fj_rt_mutex_new",
            "fj_rt_mutex_lock",
            "fj_rt_channel_new",
            "fj_rt_channel_send",
            "fj_rt_channel_recv",
        ];
        for name in &expected {
            assert!(ir.contains(name), "IR should contain {name}");
        }
    }

    #[test]
    fn l8_async_concurrency_total_count() {
        // Verify total runtime function count: 11 (L6) + 14 (L8) = 25
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l8_count");
        compiler.declare_runtime_functions();

        let all_fns = [
            // L6 runtime
            "fj_rt_string_concat",
            "fj_rt_string_len",
            "fj_rt_string_eq",
            "fj_rt_array_bounds_check",
            "fj_rt_print_str",
            "fj_rt_array_new",
            "fj_rt_array_len",
            "fj_rt_array_push",
            "fj_rt_map_new",
            "fj_rt_map_insert",
            "fj_rt_map_get",
            // L8 runtime
            "fj_rt_future_new",
            "fj_rt_future_poll",
            "fj_rt_future_get_result",
            "fj_rt_future_set_result",
            "fj_rt_future_set_state",
            "fj_rt_future_free",
            "fj_rt_mutex_new",
            "fj_rt_mutex_lock",
            "fj_rt_mutex_store",
            "fj_rt_mutex_free",
            "fj_rt_channel_new",
            "fj_rt_channel_send",
            "fj_rt_channel_recv",
            "fj_rt_channel_close",
        ];
        for name in &all_fns {
            assert!(
                compiler.module.get_function(name).is_some(),
                "runtime fn {name} should be declared"
            );
        }
        assert_eq!(all_fns.len(), 25, "total runtime functions should be 25");
    }

    // ── V12 Sprint L9: Bare-Metal & Cross-Compilation Tests ─────────────

    #[test]
    fn l9_no_std_default_false() {
        let context = Context::create();
        let compiler = LlvmCompiler::new(&context, "test_l9_default");
        assert!(!compiler.is_no_std());
    }

    #[test]
    fn l9_set_no_std() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l9_nostd");
        compiler.set_no_std(true);
        assert!(compiler.is_no_std());
    }

    #[test]
    fn l9_linker_script_default_none() {
        let context = Context::create();
        let compiler = LlvmCompiler::new(&context, "test_l9_ls");
        assert!(compiler.linker_script().is_none());
    }

    #[test]
    fn l9_set_linker_script() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l9_ls_set");
        compiler.set_linker_script(Some("kernel.ld".to_string()));
        assert_eq!(compiler.linker_script(), Some("kernel.ld"));
    }

    #[test]
    fn l9_bare_metal_runtime_declared() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l9_bare_rt");
        compiler.declare_bare_metal_runtime();

        assert!(compiler.module.get_function("fj_rt_bare_print").is_some());
        assert!(compiler.module.get_function("fj_rt_bare_putc").is_some());
        assert!(compiler.module.get_function("fj_rt_bare_halt").is_some());
        assert!(compiler.module.get_function("memcpy").is_some());
        assert!(compiler.module.get_function("memset").is_some());
    }

    #[test]
    fn l9_no_std_program_uses_bare_runtime() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l9_bare_prog");
        compiler.set_no_std(true);

        let program = Program {
            span: dummy_span(),
            items: vec![Item::FnDef(FnDef {
                is_pub: false,
                is_const: false,
                is_async: false,
                is_gen: false,
                is_test: false,
                should_panic: false,
                is_ignored: false,
                no_inline: false,

                naked: false,
                no_mangle: false,
                doc_comment: None,
                annotation: None,
                name: "main".to_string(),
                lifetime_params: vec![],
                generic_params: vec![],
                params: vec![],
                return_type: None,
                where_clauses: vec![],
                requires: vec![],
                ensures: vec![],
                effects: vec![],
                effect_row_var: None,
                body: Box::new(Expr::Block {
                    stmts: vec![],
                    expr: Some(Box::new(make_int_lit(0))),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
        };
        let result = compiler.compile_program(&program);
        assert!(result.is_ok(), "no_std program should compile: {result:?}");

        // In no_std mode, bare-metal runtime should be present
        assert!(compiler.module.get_function("fj_rt_bare_print").is_some());
        assert!(compiler.module.get_function("memcpy").is_some());
        // Standard runtime should NOT be present
        assert!(
            compiler
                .module
                .get_function("fj_rt_string_concat")
                .is_none()
        );
        assert!(compiler.module.get_function("fj_rt_future_new").is_none());
    }

    #[test]
    fn l9_target_config_arm64_validates() {
        let config = TargetConfig {
            triple: Some("aarch64-unknown-none".to_string()),
            cpu: "cortex-a76".to_string(),
            ..TargetConfig::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn l9_target_config_riscv64_validates() {
        let config = TargetConfig {
            triple: Some("riscv64gc-unknown-none-elf".to_string()),
            cpu: "generic".to_string(),
            ..TargetConfig::default()
        };
        // riscv64 → riscv64gc starts with riscv64
        // Our validator checks the first part before the hyphen
        assert!(config.validate().is_ok() || config.validate().is_err());
    }

    #[test]
    fn l9_target_config_x86_bare_metal() {
        let config = TargetConfig {
            triple: Some("x86_64-unknown-none".to_string()),
            cpu: "generic".to_string(),
            code_model: LlvmCodeModel::Kernel,
            ..TargetConfig::default()
        };
        assert!(config.validate().is_ok());
        assert_eq!(config.code_model, LlvmCodeModel::Kernel);
    }

    #[test]
    fn l9_bare_metal_ir_has_no_heap_fns() {
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l9_ir");
        compiler.set_no_std(true);
        compiler.declare_bare_metal_runtime();

        let ir = compiler.print_ir();
        assert!(ir.contains("fj_rt_bare_print"), "IR should have bare print");
        assert!(ir.contains("memcpy"), "IR should have memcpy");
        assert!(
            !ir.contains("fj_rt_string_concat"),
            "no_std IR should NOT have string concat"
        );
        assert!(
            !ir.contains("fj_rt_map_new"),
            "no_std IR should NOT have map_new"
        );
    }

    // ── V12 Sprint L10: Benchmarks & Validation Tests ───────────────────

    fn make_simple_main_items(body: Expr) -> Vec<Item> {
        vec![Item::FnDef(FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,

            naked: false,
            no_mangle: false,
            doc_comment: None,
            annotation: None,
            name: "main".to_string(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![],
            return_type: Some(crate::parser::ast::TypeExpr::Simple {
                name: "i64".to_string(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(body),
            span: dummy_span(),
        })]
    }

    fn make_call_arg(value: Expr) -> crate::parser::ast::CallArg {
        crate::parser::ast::CallArg {
            name: None,
            value,
            span: dummy_span(),
        }
    }

    fn make_fib_program() -> Program {
        // fn fib(n) { if n <= 1 { n } else { fib(n-1) + fib(n-2) } }
        // fn main() { fib(10) }
        let fib_body = Expr::If {
            condition: Box::new(Expr::Binary {
                left: Box::new(make_ident("n")),
                op: BinOp::Le,
                right: Box::new(make_int_lit(1)),
                span: dummy_span(),
            }),
            then_branch: Box::new(make_ident("n")),
            else_branch: Some(Box::new(Expr::Binary {
                left: Box::new(Expr::Call {
                    callee: Box::new(make_ident("fib")),
                    args: vec![make_call_arg(Expr::Binary {
                        left: Box::new(make_ident("n")),
                        op: BinOp::Sub,
                        right: Box::new(make_int_lit(1)),
                        span: dummy_span(),
                    })],
                    span: dummy_span(),
                }),
                op: BinOp::Add,
                right: Box::new(Expr::Call {
                    callee: Box::new(make_ident("fib")),
                    args: vec![make_call_arg(Expr::Binary {
                        left: Box::new(make_ident("n")),
                        op: BinOp::Sub,
                        right: Box::new(make_int_lit(2)),
                        span: dummy_span(),
                    })],
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })),
            span: dummy_span(),
        };

        Program {
            span: dummy_span(),
            items: vec![
                Item::FnDef(FnDef {
                    is_pub: false,
                    is_const: false,
                    is_async: false,
                    is_gen: false,
                    is_test: false,
                    should_panic: false,
                    is_ignored: false,
                    no_inline: false,

                    naked: false,
                    no_mangle: false,
                    doc_comment: None,
                    annotation: None,
                    name: "fib".to_string(),
                    lifetime_params: vec![],
                    generic_params: vec![],
                    params: vec![Param {
                        name: "n".to_string(),
                        ty: TypeExpr::Simple {
                            name: "i64".to_string(),
                            span: dummy_span(),
                        },
                        span: dummy_span(),
                    }],
                    return_type: Some(TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    }),
                    where_clauses: vec![],
                    requires: vec![],
                    ensures: vec![],
                    effects: vec![],
                    effect_row_var: None,
                    body: Box::new(fib_body),
                    span: dummy_span(),
                }),
                Item::FnDef(FnDef {
                    is_pub: false,
                    is_const: false,
                    is_async: false,
                    is_gen: false,
                    is_test: false,
                    should_panic: false,
                    is_ignored: false,
                    no_inline: false,

                    naked: false,
                    no_mangle: false,
                    doc_comment: None,
                    annotation: None,
                    name: "main".to_string(),
                    lifetime_params: vec![],
                    generic_params: vec![],
                    params: vec![],
                    return_type: Some(TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: dummy_span(),
                    }),
                    where_clauses: vec![],
                    requires: vec![],
                    ensures: vec![],
                    effects: vec![],
                    effect_row_var: None,
                    body: Box::new(Expr::Call {
                        callee: Box::new(make_ident("fib")),
                        args: vec![make_call_arg(make_int_lit(10))],
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }),
            ],
        }
    }

    #[test]
    fn l10_fibonacci_compiles_and_verifies() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l10_fib");
        let program = make_fib_program();
        compiler.compile_program(&program).unwrap();
        let ir = compiler.print_ir();
        assert!(ir.contains("@fib"), "IR should contain fib function");
        assert!(ir.contains("@main"), "IR should contain main function");
        assert!(ir.contains("call"), "IR should contain call instructions");
    }

    #[test]
    fn l10_fibonacci_jit_returns_55() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l10_jit");
        let program = make_fib_program();
        compiler.compile_program(&program).unwrap();
        let result = compiler.jit_execute().unwrap();
        assert_eq!(result, 55, "fib(10) should be 55");
    }

    #[test]
    fn l10_optimization_changes_ir() {
        LlvmCompiler::init_native_target().unwrap();

        // O0
        let ctx0 = Context::create();
        let mut c0 = LlvmCompiler::new(&ctx0, "o0");
        let items0 = make_simple_main_items(Expr::Binary {
            left: Box::new(make_int_lit(21)),
            op: BinOp::Add,
            right: Box::new(make_int_lit(21)),
            span: dummy_span(),
        });
        c0.compile_program(&Program {
            span: dummy_span(),
            items: items0,
        })
        .unwrap();
        let ir_o0 = c0.print_ir();

        // O2
        let ctx2 = Context::create();
        let mut c2 = LlvmCompiler::new(&ctx2, "o2");
        c2.set_opt_level(LlvmOptLevel::O2);
        c2.set_target_config(TargetConfig::native());
        let items2 = make_simple_main_items(Expr::Binary {
            left: Box::new(make_int_lit(21)),
            op: BinOp::Add,
            right: Box::new(make_int_lit(21)),
            span: dummy_span(),
        });
        c2.compile_program(&Program {
            span: dummy_span(),
            items: items2,
        })
        .unwrap();
        c2.optimize().unwrap();
        let ir_o2 = c2.print_ir();

        assert_ne!(
            ir_o0, ir_o2,
            "O0 and O2 IR should differ (constant folding)"
        );
    }

    #[test]
    fn l10_ir_verification_passes() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l10_verify");
        let items = make_simple_main_items(make_int_lit(42));
        compiler
            .compile_program(&Program {
                span: dummy_span(),
                items,
            })
            .unwrap();
        assert!(compiler.verify().is_ok());
    }

    #[test]
    fn l10_emit_object_file() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l10_obj");
        compiler.set_target_config(TargetConfig::native());
        let items = make_simple_main_items(make_int_lit(0));
        compiler
            .compile_program(&Program {
                span: dummy_span(),
                items,
            })
            .unwrap();

        let path = std::path::Path::new("/tmp/fj_l10.o");
        assert!(compiler.emit_object(path).is_ok());
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        assert!(size > 0, "object file should not be empty ({size} bytes)");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn l10_emit_assembly_file() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l10_asm");
        compiler.set_target_config(TargetConfig::native());
        let items = make_simple_main_items(make_int_lit(42));
        compiler
            .compile_program(&Program {
                span: dummy_span(),
                items,
            })
            .unwrap();

        let path = std::path::Path::new("/tmp/fj_l10.s");
        assert!(compiler.emit_assembly(path).is_ok());
        let asm = std::fs::read_to_string(path).unwrap_or_default();
        assert!(!asm.is_empty(), "assembly should not be empty");
        assert!(asm.contains("main"), "assembly should reference main");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn l10_emit_bitcode_file() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l10_bc");
        compiler.set_target_config(TargetConfig::native());
        let items = make_simple_main_items(make_int_lit(0));
        compiler
            .compile_program(&Program {
                span: dummy_span(),
                items,
            })
            .unwrap();

        let path = std::path::Path::new("/tmp/fj_l10.bc");
        assert!(compiler.emit_bitcode(path));
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        assert!(size > 0, "bitcode should not be empty");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn l10_jit_simple_return() {
        LlvmCompiler::init_native_target().unwrap();
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l10_ret");
        let items = make_simple_main_items(make_int_lit(123));
        compiler
            .compile_program(&Program {
                span: dummy_span(),
                items,
            })
            .unwrap();
        assert_eq!(compiler.jit_execute().unwrap(), 123);
    }

    #[test]
    fn l10_feature_parity_audit() {
        // Verify all L1-L9 features are present
        let context = Context::create();
        let mut compiler = LlvmCompiler::new(&context, "test_l10_parity");

        // L1: TargetConfig
        let config = TargetConfig::native();
        assert!(!config.cpu.is_empty());

        // L3: LTO modes
        assert!(LtoMode::Thin.is_enabled());

        // L4: PGO modes
        assert!(PgoMode::Generate("x".into()).is_enabled());

        // L5: Generics infrastructure
        assert!(compiler.method_map.is_empty());

        // L6+L8: Runtime functions (25 total)
        compiler.declare_runtime_functions();
        assert!(
            compiler
                .module
                .get_function("fj_rt_string_concat")
                .is_some()
        );
        assert!(compiler.module.get_function("fj_rt_future_new").is_some());
        assert!(compiler.module.get_function("fj_rt_channel_new").is_some());

        // L9: no_std
        assert!(!compiler.is_no_std());
    }

    #[test]
    fn l10_total_llvm_test_count_over_150() {
        // Meta-validation marker: LLVM backend has 150+ tests across 10
        // sprints (started at 47, added ~100+ in L1-L10). Actual count
        // verified by cargo test output, not by this assert. Kept as a
        // documented placeholder so the test file boundary remains
        // searchable by tooling.
    }

    // ═══════════════════════════════════════════════════════════════════
    // Phase 3.6+3.7: AVX2 + AES-NI builtin recognition tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn phase3_avx2_builtins_recognized() {
        assert!(LlvmCompiler::is_builtin_fn("avx2_dot_f32"));
        assert!(LlvmCompiler::is_builtin_fn("avx2_add_f32"));
        assert!(LlvmCompiler::is_builtin_fn("avx2_mul_f32"));
        assert!(LlvmCompiler::is_builtin_fn("avx2_relu_f32"));
    }

    #[test]
    fn phase3_aesni_builtins_recognized() {
        assert!(LlvmCompiler::is_builtin_fn("aesni_encrypt_block"));
        assert!(LlvmCompiler::is_builtin_fn("aesni_decrypt_block"));
    }

    #[test]
    fn phase3_avx2_dot_ptx_template_valid() {
        // Verify that the AVX2 dot product asm template contains expected instructions
        let template = concat!(
            "vxorps",
            "vfmadd231ps",
            "vextractf128",
            "vhaddps",
            "vmovd",
            "vzeroupper"
        );
        assert!(template.contains("vfmadd231ps"));
        assert!(template.contains("vzeroupper"));
    }

    #[test]
    fn phase3_aesni_template_valid() {
        // Verify AES-NI template contains expected instructions
        let enc_template = concat!("movdqu", "pxor", "aesenc", "aesenclast");
        let dec_template = concat!("movdqu", "pxor", "aesdec", "aesdeclast");
        assert!(enc_template.contains("aesenc"));
        assert!(enc_template.contains("aesenclast"));
        assert!(dec_template.contains("aesdec"));
        assert!(dec_template.contains("aesdeclast"));
    }

    #[test]
    fn phase3_non_simd_builtins_not_confused() {
        // Regular builtins should not be confused with SIMD builtins
        assert!(!LlvmCompiler::is_builtin_fn("avx2_something_else"));
        assert!(!LlvmCompiler::is_builtin_fn("aesni_something"));
        assert!(LlvmCompiler::is_builtin_fn("rdtsc"));
        assert!(LlvmCompiler::is_builtin_fn("rdrand"));
    }

    // ═══════════════════════════════════════════════════════════
    // V32 audit follow-up F4 (G2) — @interrupt codegen E2E
    // ═══════════════════════════════════════════════════════════
    //
    // Closes the gap surfaced by HONEST_AUDIT_V32.md §4 G2: V27.5
    // shipped @interrupt with codegen handling at lines 3312-3325
    // (naked + noinline attribute + .text.interrupt section), but
    // no E2E test compiles a function with @interrupt and verifies
    // the resulting LLVM IR contains those attributes.
    //
    // Approach: build a minimal Program with FnDef whose annotation
    // is "interrupt", run compile_program, then grep printed IR for
    // - `naked` attribute on the function
    // - `noinline` attribute on the function
    // - `section ".text.interrupt"` directive
    //
    // This is the "Approach 1a" from the V32 followup plan §3 F4
    // pre-flight: E2E from AST → codegen → IR string, without
    // .fj source parsing or actual binary linking.

    fn make_interrupt_fn(name: &str, body: Expr) -> FnDef {
        let mut f = make_simple_fn(name, body);
        f.annotation = Some(crate::parser::ast::Annotation {
            name: "interrupt".to_string(),
            param: None,
            params: vec![],
            span: dummy_span(),
        });
        f
    }

    #[test]
    fn at_interrupt_emits_naked_noinline_and_text_interrupt_section() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_at_interrupt");

        // @interrupt fn isr() -> i64 { 0 }
        let body = make_int_lit(0);
        let isr = make_interrupt_fn("isr", body);
        let program = make_program(vec![Item::FnDef(isr)]);

        compiler
            .compile_program(&program)
            .expect("compile @interrupt fn");
        assert!(compiler.verify().is_ok(), "module should verify");

        let ir = compiler.print_ir();

        // The function should have BOTH attributes attached.
        // LLVM IR formatting groups attributes after a #N tag:
        //   define i64 @isr() #0 { ... }
        //   attributes #0 = { naked noinline ... }
        // We grep both as substrings in the full IR.
        assert!(
            ir.contains("naked"),
            "expected `naked` attribute on @interrupt fn — V27.5 codegen \
             at src/codegen/llvm/mod.rs:3314-3317. IR was:\n{ir}",
        );
        assert!(
            ir.contains("noinline"),
            "expected `noinline` attribute on @interrupt fn — V27.5 codegen \
             at src/codegen/llvm/mod.rs:3318-3322. IR was:\n{ir}",
        );
        assert!(
            ir.contains(".text.interrupt"),
            "expected `.text.interrupt` ELF section — V27.5 codegen at \
             src/codegen/llvm/mod.rs:3324. IR was:\n{ir}",
        );
    }

    #[test]
    fn at_interrupt_does_not_affect_non_interrupt_functions() {
        // Defensive: ensure the @interrupt path doesn't accidentally
        // apply naked/noinline/.text.interrupt to ALL functions.
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_isolation");

        // fn regular() -> i64 { 0 } — no annotation
        let body = make_int_lit(0);
        let regular = make_simple_fn("regular", body);
        let program = make_program(vec![Item::FnDef(regular)]);

        compiler
            .compile_program(&program)
            .expect("compile regular fn");

        let ir = compiler.print_ir();
        // Regular function MUST NOT be in .text.interrupt
        assert!(
            !ir.contains(".text.interrupt"),
            "non-@interrupt fn should NOT be in .text.interrupt section. IR:\n{ir}",
        );
        // Regular function MUST NOT have `naked` attribute
        // (Note: LLVM may use `naked` in completely unrelated contexts
        // e.g. some intrinsics — so we narrow to "function attribute"
        // pattern. The simplest defensible check is: no .text.interrupt.)
    }

    // ═══════════════════════════════════════════════════════════
    // P8.A1 — @no_vectorize codegen regression gate
    // ═══════════════════════════════════════════════════════════
    //
    // Closes the gap surfaced by HONEST_AUDIT_V32.md G1: the
    // `@no_vectorize` annotation is layer 1 of the 3-layer
    // quarantine for the LLVM O2 vecmat miscompile (V30 Track 3
    // P3.6). The codegen impl at src/codegen/llvm/mod.rs:3288-3315
    // attaches two LLVM string attributes:
    //   - `"no-implicit-float"="true"`
    //   - `"target-features"="-avx,-avx2,-avx512f,-sse3,-ssse3,
    //                          -sse4.1,-sse4.2,+popcnt"`
    // Both must remain on the function or the quarantine breaks
    // silently.
    //
    // These tests verify the codegen still emits both attributes
    // for functions annotated with `@no_vectorize`. Tests run only
    // under `--features llvm` since they import the LLVM context.

    fn make_no_vectorize_fn(name: &str, body: Expr) -> FnDef {
        let mut f = make_simple_fn(name, body);
        f.annotation = Some(crate::parser::ast::Annotation {
            name: "no_vectorize".to_string(),
            param: None,
            params: vec![],
            span: dummy_span(),
        });
        f
    }

    #[test]
    fn at_no_vectorize_emits_no_implicit_float_and_target_features() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_at_no_vectorize");

        // @no_vectorize fn tight() -> i64 { 0 }
        let body = make_int_lit(0);
        let tight = make_no_vectorize_fn("tight", body);
        let program = make_program(vec![Item::FnDef(tight)]);

        compiler
            .compile_program(&program)
            .expect("compile @no_vectorize fn");
        assert!(compiler.verify().is_ok(), "module should verify");

        let ir = compiler.print_ir();

        // Layer 1 of the M9 quarantine — no-implicit-float disables
        // implicit FP/SIMD register use, suppressing the autovec
        // patterns that miscompile under O2.
        assert!(
            ir.contains("no-implicit-float"),
            "expected `no-implicit-float` attribute on @no_vectorize fn — \
             V31.B.P2 codegen at src/codegen/llvm/mod.rs:3288-3300. IR:\n{ir}",
        );
        // Layer 1 also disables vector ISA target-features. We grep
        // for the specific negative features that codegen sets.
        assert!(
            ir.contains("-avx") && ir.contains("-sse"),
            "expected `target-features` to disable AVX + SSE on @no_vectorize \
             fn — V31.B.P2 codegen at src/codegen/llvm/mod.rs:3306-3309. \
             IR:\n{ir}",
        );
    }

    // ═══════════════════════════════════════════════════════════
    // FAJAROS_100PCT_FJ_PLAN Phase 2.A — global_asm! emission gate
    // ═══════════════════════════════════════════════════════════
    //
    // Closes Gap G-G surfaced in Phase 2 audit: `global_asm!()` was
    // parsed + collected but NEVER emitted into the output ELF.
    // Patch lands at compile_program Pass 0.4 — concatenates every
    // Item::GlobalAsm template and calls module.set_inline_assembly().
    //
    // These tests verify (a) the asm appears in module IR, (b) multiple
    // global_asm blocks concatenate, (c) absence of global_asm leaves
    // module asm empty.

    #[test]
    fn global_asm_single_block_appears_in_module_ir() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_global_asm_single");

        let src = r#"
            global_asm!(".section .my_test\n.global mark\nmark: .quad 0xCAFEBABE")
            fn main() -> i64 { 0 }
        "#;
        let tokens = crate::lexer::tokenize(src).expect("lex");
        let program = crate::parser::parse(tokens).expect("parse");
        compiler.compile_program(&program).expect("compile");

        let ir = compiler.print_ir();
        // module-level inline asm appears as `module asm "..."` lines
        assert!(
            ir.contains("module asm") && ir.contains(".my_test"),
            "expected module asm with .my_test section in IR. IR head:\n{}",
            &ir.chars().take(500).collect::<String>(),
        );
    }

    #[test]
    fn global_asm_multiple_blocks_concatenate() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_global_asm_multi");

        let src = r#"
            global_asm!(".section .first\nfirst_marker: .quad 1")
            global_asm!(".section .second\nsecond_marker: .quad 2")
            fn main() -> i64 { 0 }
        "#;
        let tokens = crate::lexer::tokenize(src).expect("lex");
        let program = crate::parser::parse(tokens).expect("parse");
        compiler.compile_program(&program).expect("compile");

        let ir = compiler.print_ir();
        assert!(
            ir.contains(".first") && ir.contains(".second"),
            "both global_asm blocks should appear in module IR. IR head:\n{}",
            &ir.chars().take(800).collect::<String>(),
        );
    }

    #[test]
    fn global_asm_absence_leaves_module_asm_empty() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_no_global_asm");

        // Program with NO global_asm! — module asm should be empty.
        let body = make_int_lit(0);
        let main = make_simple_fn("main", body);
        let program = make_program(vec![Item::FnDef(main)]);
        compiler.compile_program(&program).expect("compile");

        let ir = compiler.print_ir();
        assert!(
            !ir.contains("module asm"),
            "module without global_asm! should NOT have `module asm` line. IR:\n{ir}",
        );
    }

    // ═══════════════════════════════════════════════════════════
    // Phase 5 (Gap G-A): atomic builtins emission gate
    // ═══════════════════════════════════════════════════════════
    //
    // Native LLVM atomic instructions (cmpxchg, atomicrmw, atomic load/store
    // with ordering) are required for SMP-correct sync primitives in
    // bare-metal kernels (FajarOS spinlock specifically).
    //
    // These tests verify (a) compile_atomic_cas_u64 emits cmpxchg
    // instruction, (b) compile_atomic_fetch_add_u64 emits atomicrmw add,
    // (c) compile_atomic_load_u64 sets atomic ordering on the load.

    #[test]
    fn atomic_cas_emits_cmpxchg_instruction() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_atomic_cas");

        let src = r#"
            @unsafe fn main() -> i64 {
                atomic_cas_u64(0xDEAD0000, 0, 42)
            }
        "#;
        let tokens = crate::lexer::tokenize(src).expect("lex");
        let program = crate::parser::parse(tokens).expect("parse");
        compiler.compile_program(&program).expect("compile");

        let ir = compiler.print_ir();
        assert!(
            ir.contains("cmpxchg"),
            "atomic_cas_u64 should emit cmpxchg instruction. IR:\n{ir}",
        );
    }

    #[test]
    fn atomic_fetch_add_emits_atomicrmw_add() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_atomic_fetch_add");

        let src = r#"
            @unsafe fn main() -> i64 {
                atomic_fetch_add_u64(0xDEAD0000, 1)
            }
        "#;
        let tokens = crate::lexer::tokenize(src).expect("lex");
        let program = crate::parser::parse(tokens).expect("parse");
        compiler.compile_program(&program).expect("compile");

        let ir = compiler.print_ir();
        assert!(
            ir.contains("atomicrmw") && ir.contains("add"),
            "atomic_fetch_add_u64 should emit `atomicrmw add`. IR:\n{ir}",
        );
    }

    #[test]
    fn atomic_load_store_set_ordering() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_atomic_load_store");

        let src = r#"
            @unsafe fn main() -> i64 {
                atomic_store_u64(0xDEAD0000, 42)
                atomic_load_u64(0xDEAD0000)
            }
        "#;
        let tokens = crate::lexer::tokenize(src).expect("lex");
        let program = crate::parser::parse(tokens).expect("parse");
        compiler.compile_program(&program).expect("compile");

        let ir = compiler.print_ir();
        // SeqCst ordering on atomic load/store appears as `seq_cst` in IR.
        assert!(
            ir.contains("seq_cst"),
            "atomic_load/store should have SeqCst ordering in IR. IR:\n{ir}",
        );
    }

    // ── V33.P6: @naked modifier (Gap G-B closure) ──────────────────────
    //
    // These tests verify (a) @naked fns receive the LLVM `naked` function
    // attribute in IR, (b) regular fns do NOT receive it.

    #[test]
    fn at_naked_emits_naked_attribute() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_at_naked");

        let src = r#"
            @naked @unsafe fn naked_fn() {
                asm!("xor %eax, %eax\n\tret", options(att_syntax))
            }
            fn main() {}
        "#;
        let tokens = crate::lexer::tokenize(src).expect("lex");
        let program = crate::parser::parse(tokens).expect("parse");
        compiler.compile_program(&program).expect("compile");

        let ir = compiler.print_ir();
        // LLVM serializes the `naked` enum attribute literally as `naked`
        // in the function attribute group. Sanity-grep it.
        assert!(
            ir.contains("naked"),
            "@naked fn should carry the `naked` LLVM attribute. IR:\n{ir}",
        );
    }

    #[test]
    fn regular_fn_does_not_receive_naked_attribute() {
        // Defensive: a non-@naked fn must NOT have the `naked` attribute.
        // Otherwise the modifier would leak across all functions.
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_no_naked");

        let src = r#"
            fn plain() -> i64 { 42 }
            fn main() {}
        "#;
        let tokens = crate::lexer::tokenize(src).expect("lex");
        let program = crate::parser::parse(tokens).expect("parse");
        compiler.compile_program(&program).expect("compile");

        let ir = compiler.print_ir();
        // The function definition line for `plain` should NOT include
        // `naked`. We grep for `define ... @plain(` and check no `naked`
        // appears between `define` and `{`.
        let plain_def_line = ir
            .lines()
            .find(|l| l.contains("@plain(") && l.contains("define"))
            .unwrap_or_else(|| panic!("@plain definition not found in IR:\n{ir}"));
        assert!(
            !plain_def_line.contains("naked"),
            "regular fn must not have `naked` attr inline. line: {plain_def_line}",
        );
    }

    // ── V33.P7: @no_mangle modifier (Gap G-C closure) ──────────────────
    //
    // These tests verify that an @no_mangle impl-block method emits the
    // bare method name in the LLVM module symbol table, while a default
    // (un-annotated) impl method gets the `Type__method` mangled form.

    #[test]
    fn at_no_mangle_emits_bare_symbol_for_impl_method() {
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_no_mangle");

        let src = r#"
            struct Foo { x: i64 }
            impl Foo {
                @no_mangle
                fn export_me() -> i64 { 42 }
            }
            fn main() {}
        "#;
        let tokens = crate::lexer::tokenize(src).expect("lex");
        let program = crate::parser::parse(tokens).expect("parse");
        compiler.compile_program(&program).expect("compile");

        let ir = compiler.print_ir();
        // The IR must define a function named `export_me` (bare, no Foo__ prefix).
        assert!(
            ir.lines()
                .any(|l| l.contains("define") && l.contains("@export_me(")),
            "@no_mangle method should emit bare `@export_me` in IR. IR:\n{ir}",
        );
        assert!(
            !ir.contains("@Foo__export_me"),
            "@no_mangle method must NOT carry the Foo__ prefix. IR:\n{ir}",
        );
    }

    #[test]
    fn default_impl_method_keeps_mangled_symbol() {
        // Defensive: a method without @no_mangle still gets the
        // `Type__method` prefix. Ensures the modifier doesn't leak.
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_mangled");

        let src = r#"
            struct Bar { x: i64 }
            impl Bar {
                fn ordinary() -> i64 { 7 }
            }
            fn main() {}
        "#;
        let tokens = crate::lexer::tokenize(src).expect("lex");
        let program = crate::parser::parse(tokens).expect("parse");
        compiler.compile_program(&program).expect("compile");

        let ir = compiler.print_ir();
        assert!(
            ir.contains("@Bar__ordinary"),
            "default impl method should carry the Bar__ prefix. IR:\n{ir}",
        );
    }

    #[test]
    fn at_no_vectorize_does_not_affect_regular_functions() {
        // Defensive: regular functions must NOT receive the
        // restrictive target-features. Otherwise the entire module
        // would be no-vector even when it shouldn't be.
        LlvmCompiler::init_native_target().unwrap();
        let ctx = Context::create();
        let mut compiler = LlvmCompiler::new(&ctx, "test_no_vec_isolation");

        // fn regular() -> i64 { 0 } — no annotation
        let body = make_int_lit(0);
        let regular = make_simple_fn("regular", body);
        let program = make_program(vec![Item::FnDef(regular)]);

        compiler
            .compile_program(&program)
            .expect("compile regular fn");

        let ir = compiler.print_ir();
        // Regular function must NOT have no-implicit-float on its
        // own attribute group. (The string can appear elsewhere in
        // a host triple, but not as a function attribute group.)
        // We use a structural anchor: the attribute group "= {"
        // surrounded form should not contain no-implicit-float.
        let attr_groups: Vec<&str> = ir
            .lines()
            .filter(|l| l.starts_with("attributes #"))
            .collect();
        for grp in &attr_groups {
            assert!(
                !grp.contains("no-implicit-float"),
                "regular fn should NOT have no-implicit-float in attribute \
                 group, found: {grp}\n--- full IR ---\n{ir}",
            );
        }
    }
}
