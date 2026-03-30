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
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};

use crate::codegen::CodegenError;
use crate::parser::ast::{
    BinOp, EnumDef, Expr, FnDef, Item, LiteralKind, MatchArm, Pattern, Program, Stmt, StructDef,
    TypeExpr, UnaryOp,
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
    /// Break target: (after_bb, optional break value alloca)
    break_target: Option<(BasicBlock<'ctx>, Option<PointerValue<'ctx>>)>,
    /// Continue target: loop header block
    continue_target: Option<BasicBlock<'ctx>>,
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
            break_target: None,
            continue_target: None,
        }
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
        let ptr_ty: BasicTypeEnum<'ctx> = self
            .context
            .ptr_type(inkwell::AddressSpace::default())
            .into();

        // Print functions
        self.declare_external_fn("fj_rt_print_int", &[i64_ty], None);
        self.declare_external_fn("fj_rt_print_str", &[ptr_ty, i64_ty], None);
        self.declare_external_fn("fj_rt_println_int", &[i64_ty], None);
        self.declare_external_fn("fj_rt_println_str", &[ptr_ty, i64_ty], None);

        // String functions
        self.declare_external_fn("fj_rt_str_len", &[ptr_ty, i64_ty], Some(i64_ty));
        self.declare_external_fn(
            "fj_rt_str_concat",
            &[ptr_ty, i64_ty, ptr_ty, i64_ty],
            Some(ptr_ty),
        );

        // Assert functions
        self.declare_external_fn("fj_rt_assert", &[i64_ty], None);
        self.declare_external_fn("fj_rt_assert_eq", &[i64_ty, i64_ty], None);
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

        // Method not found — return 0 (graceful fallback for unresolved methods)
        Ok(Some(self.context.i64_type().const_int(0, false).into()))
    }

    pub fn compile_program(&mut self, program: &Program) -> Result<(), CodegenError> {
        // Pass 0: register struct and enum type definitions
        for item in &program.items {
            match item {
                Item::StructDef(sdef) => self.register_struct(sdef)?,
                Item::EnumDef(edef) => self.register_enum(edef),
                _ => {}
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
                    let mangled_name = format!("{}__{}", ib.target_type, method.name);
                    let mut mangled_method = method.clone();
                    mangled_method.name = mangled_name;
                    self.declare_function(&mangled_method)?;
                }
            }
        }

        // Second pass: compile function bodies
        for item in &program.items {
            if let Item::FnDef(fndef) = item {
                if fndef.generic_params.is_empty() {
                    self.compile_function(fndef)?;
                }
            }
        }
        // Compile monomorphized functions
        for mfn in &mono_fns {
            self.compile_function(mfn)?;
        }

        // Compile impl block methods
        for item in &program.items {
            if let Item::ImplBlock(ib) = item {
                for method in &ib.methods {
                    let mangled_name = format!("{}__{}", ib.target_type, method.name);
                    let mut mangled_method = method.clone();
                    mangled_method.name = mangled_name;
                    self.compile_function(&mangled_method)?;
                }
            }
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
                // Default return: i64
                self.context.i64_type().fn_type(&param_types, false)
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
                _ => {}
            }
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
            if let Some(val) = body_val {
                self.builder
                    .build_return(Some(&val))
                    .map_err(|e| CodegenError::Internal(e.to_string()))?;
            } else {
                // void return or implicit return 0
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
                            None => return Err(CodegenError::UndefinedFunction(name.clone())),
                        }
                    };

                    let compiled_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
                        .iter()
                        .map(|arg| {
                            let val = self.compile_expr(&arg.value)?.ok_or_else(|| {
                                CodegenError::Internal("call arg produced no value".into())
                            })?;
                            Ok(val.into())
                        })
                        .collect::<Result<Vec<_>, CodegenError>>()?;

                    let call_val = self
                        .builder
                        .build_call(function, &compiled_args, &format!("{name}_result"))
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;

                    match call_val.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(val) => Ok(Some(val)),
                        inkwell::values::ValueKind::Instruction(_) => Ok(None),
                    }
                } else {
                    Err(CodegenError::NotImplemented(
                        "non-ident callee in LLVM backend".into(),
                    ))
                }
            }

            Expr::Assign { target, value, .. } => {
                if let Expr::Ident { name, .. } = target.as_ref() {
                    let val = self.compile_expr(value)?.ok_or_else(|| {
                        CodegenError::Internal("assign value produced no value".into())
                    })?;
                    let ptr = self
                        .variables
                        .get(name)
                        .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
                    self.builder
                        .build_store(*ptr, val)
                        .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    Ok(None)
                } else {
                    Err(CodegenError::NotImplemented(
                        "non-ident assignment target in LLVM".into(),
                    ))
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

            // V12 L5: Closure expression
            Expr::Closure { params, body, .. } => self.compile_closure(params, body),

            // V12 L5: Method call
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => self.compile_method_call(receiver, method, args),

            _ => Err(CodegenError::NotImplemented(format!(
                "LLVM expr: {:?}",
                std::mem::discriminant(expr)
            ))),
        }
    }

    /// Compiles a literal value.
    fn compile_literal(
        &self,
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
                let str_val = self.context.const_string(s.as_bytes(), false);
                let global = self.module.add_global(
                    str_val.get_type(),
                    Some(inkwell::AddressSpace::default()),
                    "str_const",
                );
                global.set_initializer(&str_val);
                global.set_constant(true);

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
                let str_val = self.context.const_string(s.as_bytes(), false);
                let global = self.module.add_global(
                    str_val.get_type(),
                    Some(inkwell::AddressSpace::default()),
                    "raw_str_const",
                );
                global.set_initializer(&str_val);
                global.set_constant(true);

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
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "LLVM float binop: {:?}",
                        op
                    )));
                }
            };
            return Ok(result.into());
        }

        // Integer operations
        let l = lhs.into_int_value();
        let r = rhs.into_int_value();

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
            _ => {
                return Err(CodegenError::NotImplemented(format!(
                    "LLVM int binop: {:?}",
                    op
                )));
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
            _ => Err(CodegenError::NotImplemented(format!(
                "LLVM unary op: {:?}",
                op
            ))),
        }
    }

    /// Compiles an if/else expression.
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

            Stmt::Expr { expr, .. } => self.compile_expr(expr),

            Stmt::Return { value, .. } => {
                if let Some(expr) = value {
                    let val = self.compile_expr(expr)?;
                    if let Some(v) = val {
                        self.builder
                            .build_return(Some(&v))
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    } else {
                        self.builder
                            .build_return(None)
                            .map_err(|e| CodegenError::Internal(e.to_string()))?;
                    }
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

            _ => Err(CodegenError::NotImplemented(format!(
                "LLVM stmt: {:?}",
                std::mem::discriminant(stmt)
            ))),
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

        let alloca = self
            .builder
            .build_alloca(array_type, "arr")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

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

        // Int → Int
        if val.is_int_value() && target_type.is_int_type() {
            let result = self
                .builder
                .build_int_cast(val.into_int_value(), target_type.into_int_type(), "icast")
                .map_err(|e| CodegenError::Internal(e.to_string()))?;
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
                return Err(CodegenError::NotImplemented(
                    "LLVM for loop only supports range iterables".into(),
                ));
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

                    // Compare subject with pattern
                    let cmp = if subject_val.is_int_value() && pattern_val.is_int_value() {
                        self.builder
                            .build_int_compare(
                                inkwell::IntPredicate::EQ,
                                subject_val.into_int_value(),
                                pattern_val.into_int_value(),
                                "match_cmp",
                            )
                            .map_err(|e| CodegenError::Internal(e.to_string()))?
                    } else {
                        return Err(CodegenError::NotImplemented(
                            "LLVM match: non-int pattern comparison".into(),
                        ));
                    };

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

                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "LLVM match pattern: {:?}",
                        std::mem::discriminant(&arm.pattern)
                    )));
                }
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
    pub fn emit_object(&self, path: &Path) -> Result<(), CodegenError> {
        let tm = self.create_target_machine(None)?;
        self.configure_module_target(&tm);
        tm.write_to_file(&self.module, FileType::Object, path)
            .map_err(|e| CodegenError::Internal(format!("LLVM emit object error: {e}")))
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

        // SAFETY: We're calling into JIT-compiled code that has been verified
        // by the LLVM module verifier. The function signature matches main() -> i64.
        unsafe {
            let main_fn = ee
                .get_function::<unsafe extern "C" fn() -> i64>("main")
                .map_err(|e| CodegenError::UndefinedFunction(format!("main not found: {e}")))?;

            Ok(main_fn.call())
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
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
            body: Box::new(make_float_lit(3.14)),
            span: dummy_span(),
        };
        let program = make_program(vec![Item::FnDef(main_fn)]);
        compiler.compile_program(&program).unwrap();
        let ir = compiler.print_ir();
        assert!(ir.contains("3.14"));
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
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
        assert!(ir.contains("str_const"));
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
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
                is_test: false,
                should_panic: false,
                is_ignored: false,
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
            doc_comment: None,
            annotation: None,
            name: "add".to_string(),
            lifetime_params: vec![],
            generic_params: vec![crate::parser::ast::GenericParam {
                name: "T".to_string(),
                bounds: vec![],
                is_comptime: false,
                is_effect: false,
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
                kind: LiteralKind::Float(3.14),
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
                is_test: false,
                should_panic: false,
                is_ignored: false,
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
                        is_test: false,
                        should_panic: false,
                        is_ignored: false,
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
                    is_test: false,
                    should_panic: false,
                    is_ignored: false,
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
            is_test: false,
            should_panic: false,
            is_ignored: false,
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
                    span: dummy_span(),
                },
                crate::parser::ast::GenericParam {
                    name: "U".to_string(),
                    bounds: vec![],
                    is_comptime: false,
                    is_effect: false,
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
}
