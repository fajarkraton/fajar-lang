//! Cranelift-based native code compiler for Fajar Lang.
//!
//! Compiles a Fajar Lang AST to native machine code via Cranelift JIT.
//! Currently supports: i64 arithmetic, comparisons, booleans, function
//! definitions, function calls, if/else, local variables, and return.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicU64;

use cranelift_codegen::Context;
use cranelift_codegen::ir::InstBuilder;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule, ObjectProduct};

mod closures;
mod compile;
mod context;
mod generics;
pub mod runtime_bare;
pub mod runtime_fns;
mod runtime_hosted;
pub mod runtime_user;
#[cfg(test)]
mod tests;

use closures::scan_closures_in_body;
use compile::*;
use context::{CodegenCtx, emit_owned_cleanup};
use generics::{collect_called_fns, collect_generic_calls, compute_reachable, specialize_fndef};

use super::CodegenError;
use super::types as clif_types;
use crate::parser::ast::{Expr, ExternFn, FnDef, Item, LiteralKind, Program, Stmt, TypeExpr};

// ═══════════════════════════════════════════════════════════════════════
// Shared helpers for JIT + AOT compilers
// ═══════════════════════════════════════════════════════════════════════

/// Collects trait definitions and trait impl mappings from a program.
///
/// Returns `(trait_defs, trait_impls)` where:
/// - `trait_defs`: trait name → list of method names
/// - `trait_impls`: (trait_name, type_name) → list of method names
#[allow(clippy::type_complexity)]
fn collect_trait_info(
    program: &Program,
) -> (
    HashMap<String, Vec<String>>,
    HashMap<(String, String), Vec<String>>,
) {
    let mut trait_defs = HashMap::new();
    let mut trait_impls = HashMap::new();

    for item in &program.items {
        if let Item::TraitDef(tdef) = item {
            let methods: Vec<String> = tdef.methods.iter().map(|m| m.name.clone()).collect();
            trait_defs.insert(tdef.name.clone(), methods);
        }
    }

    for item in &program.items {
        if let Item::ImplBlock(impl_block) = item {
            if let Some(ref trait_name) = impl_block.trait_name {
                let method_names: Vec<String> =
                    impl_block.methods.iter().map(|m| m.name.clone()).collect();
                trait_impls.insert(
                    (trait_name.clone(), impl_block.target_type.clone()),
                    method_names,
                );
            }
        }
    }

    (trait_defs, trait_impls)
}

// Runtime functions are resolved lazily via lookup_runtime_symbol

// ═══════════════════════════════════════════════════════════════════════
// Closure & Generics helpers
// ═══════════════════════════════════════════════════════════════════════

/// Global counter for unique data section names.
static DATA_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Global counter for generating unique closure function names.
static CLOSURE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// JIT-based native compiler for Fajar Lang programs.
pub struct CraneliftCompiler {
    /// The JIT module that holds compiled functions.
    module: JITModule,
    /// Cranelift codegen context (reused per function).
    ctx: Context,
    /// Function builder context (reused per function).
    builder_ctx: FunctionBuilderContext,
    /// Map of function names to their Cranelift FuncIds.
    functions: HashMap<String, FuncId>,
    /// Map of string literal contents to their DataIds in the data section.
    string_data: HashMap<String, DataId>,
    /// Generic function definitions (stored for monomorphization).
    generic_fns: HashMap<String, FnDef>,
    /// Generic fn param mapping: fn_name → Vec of (param_index, generic_param_name).
    generic_fn_params: HashMap<String, Vec<(usize, String)>>,
    /// Monomorphization map: generic fn name → mangled specialized name.
    mono_map: HashMap<String, String>,
    /// Tracks the return type of each function for type-aware dispatch.
    fn_return_types: HashMap<String, cranelift_codegen::ir::Type>,
    /// Enum definitions: enum name → list of variant names (index = tag).
    enum_defs: HashMap<String, Vec<String>>,
    /// Enum variant payload types: (enum_name, variant_name) → list of Cranelift types.
    enum_variant_types: HashMap<(String, String), Vec<cranelift_codegen::ir::Type>>,
    /// Generic enum definitions: enum_name → list of generic param names.
    generic_enum_defs: HashMap<String, Vec<String>>,
    /// Struct definitions: struct name → ordered list of (field_name, clif_type).
    struct_defs: HashMap<String, Vec<(String, cranelift_codegen::ir::Type)>>,
    /// Set of type names that are unions (all fields at offset 0).
    union_names: HashSet<String>,
    /// Bitfield layouts: struct_name → vec of (field_name, bit_offset, bit_width).
    bitfield_layouts: HashMap<String, Vec<(String, u8, u8)>>,
    /// Impl methods: (type_name, method_name) → mangled function name.
    impl_methods: HashMap<(String, String), String>,
    /// Trait definitions: trait name → list of required method names.
    trait_defs: HashMap<String, Vec<String>>,
    /// Trait impls: (trait_name, type_name) → list of method names implemented.
    trait_impls: HashMap<(String, String), Vec<String>>,
    /// Top-level const definitions: (name, value expr, type).
    const_defs: Vec<(String, Expr, TypeExpr)>,
    /// Const fn definitions: fn_name → FnDef (for compile-time evaluation).
    const_fn_defs: HashMap<String, FnDef>,
    /// Functions that return fixed-size arrays: fn_name → (array_len, elem_type).
    fn_array_returns: HashMap<String, (usize, cranelift_codegen::ir::Type)>,
    /// Functions that return strings (two return values: ptr, len).
    fn_returns_string: HashSet<String>,
    /// Functions that return a heap-allocated dynamic array (Slice type like `[i64]`).
    fn_returns_heap_array: HashSet<String>,
    /// Functions that return a closure handle (closure with captures).
    fn_returns_closure_handle: HashSet<String>,
    /// Functions that return a struct type: fn_name → struct_name.
    fn_returns_struct: HashMap<String, String>,
    /// Functions that return an enum type (two return values: tag, payload).
    fn_returns_enum: HashSet<String>,
    /// Maps closure variable names to their generated function names.
    closure_fn_map: HashMap<String, String>,
    /// Maps closure function names to their list of captured variable names.
    closure_captures: HashMap<String, Vec<String>>,
    /// Maps closure source span → function name for inline closure lookup.
    closure_span_to_fn: HashMap<(usize, usize), String>,
    /// Maps mangled function name → module prefix for intra-module resolution.
    module_fns: HashMap<String, String>,
    /// When true, disables standard library (IO, heap) runtime declarations.
    no_std: bool,
    /// When true, generates user-mode (Ring 3) ELF with SYSCALL-based runtime.
    #[allow(dead_code)]
    user_mode: bool,
    /// User-defined panic handler function name.
    panic_handler_fn: Option<String>,
    /// Set of async function names (their return is wrapped in a future handle).
    async_fns: HashSet<String>,
    /// Global assembly sections collected from `global_asm!()` items.
    global_asm_sections: Vec<String>,
    /// Function section annotations: fn_name → section name (from @section("name")).
    fn_sections: HashMap<String, String>,
    /// Data section annotations: const name → section name (from @section on ConstDef).
    data_sections: HashMap<String, String>,
    /// Global data objects for section-annotated consts: name → DataId.
    global_data: HashMap<String, DataId>,
    /// Functions annotated with `@interrupt` — need assembly wrapper with
    /// register save/restore and `eret` instead of `ret`.
    interrupt_fns: Vec<String>,
    /// Security configuration for codegen hardening.
    security_enabled: bool,
    /// Run security linter before compilation.
    lint_on_compile: bool,
    /// Dead functions to skip during codegen (set by optimizer).
    dead_functions: HashSet<String>,
    /// When true, inject profiling instrumentation into compiled functions.
    profiling_enabled: bool,
}

/// Coerces a return value to match the declared function return type.
fn coerce_ret(
    builder: &mut FunctionBuilder,
    val: cranelift_codegen::ir::Value,
    expected: Option<cranelift_codegen::ir::Type>,
) -> cranelift_codegen::ir::Value {
    let Some(expected_ty) = expected else {
        return val;
    };
    let actual_ty = builder.func.dfg.value_type(val);
    if actual_ty == expected_ty
        || clif_types::is_float(actual_ty)
        || clif_types::is_float(expected_ty)
    {
        return val;
    }
    if actual_ty.bits() > expected_ty.bits() {
        builder.ins().ireduce(expected_ty, val)
    } else {
        builder.ins().uextend(expected_ty, val)
    }
}

/// H4: Scans an expression tree for function calls forbidden in the given context.
///
/// Extracts nested `FnDef` items from a function body expression tree.
///
/// Nested functions (e.g., `fn inner(x: i64) -> i64 { ... }` inside another fn)
/// are extracted and added to the concrete_fns list so they get declared and compiled
/// at the module level. The body compilation skips `Expr::Item(FnDef)` nodes.
fn extract_nested_fns(body: &Expr, out: &mut Vec<FnDef>) {
    if let Expr::Block { stmts, expr, .. } = body {
        for stmt in stmts {
            extract_nested_fns_stmt(stmt, out);
        }
        if let Some(tail_expr) = expr {
            extract_nested_fns_expr(tail_expr, out);
        }
    }
}

fn extract_nested_fns_stmt(stmt: &Stmt, out: &mut Vec<FnDef>) {
    match stmt {
        Stmt::Item(item) => {
            if let Item::FnDef(fndef) = item.as_ref() {
                if fndef.generic_params.is_empty() {
                    out.push(fndef.clone());
                    extract_nested_fns(&fndef.body, out);
                }
            }
        }
        Stmt::Expr { expr, .. } => extract_nested_fns_expr(expr, out),
        _ => {}
    }
}

fn extract_nested_fns_expr(expr: &Expr, out: &mut Vec<FnDef>) {
    if let Expr::Block { stmts, expr, .. } = expr {
        for stmt in stmts {
            extract_nested_fns_stmt(stmt, out);
        }
        if let Some(tail_expr) = expr {
            extract_nested_fns_expr(tail_expr, out);
        }
    }
}

/// Returns a list of violation descriptions (empty = no violations).
/// - `"kernel"`: forbids tensor ops, heap allocation, string ops
/// - `"device"`: forbids raw pointer ops, IRQ ops
fn check_context_violations(body: &Expr, context: &str) -> Vec<String> {
    let mut violations = Vec::new();
    collect_violations(body, context, &mut violations);
    violations
}

/// Recursively collects context violations from an expression tree.
fn collect_violations(expr: &Expr, context: &str, out: &mut Vec<String>) {
    match expr {
        Expr::Call { callee, args, .. } => {
            // Check the callee name
            if let Expr::Ident { name, .. } = callee.as_ref() {
                check_call_name(name, context, out);
            }
            if let Expr::Path { segments, .. } = callee.as_ref() {
                if let Some(name) = segments.last() {
                    check_call_name(name, context, out);
                }
            }
            // Recurse into args
            for arg in args {
                collect_violations(&arg.value, context, out);
            }
        }
        Expr::Block { stmts, .. } => {
            for stmt in stmts {
                collect_stmt_violations(stmt, context, out);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_violations(condition, context, out);
            collect_violations(then_branch, context, out);
            if let Some(e) = else_branch {
                collect_violations(e, context, out);
            }
        }
        Expr::While {
            label: _,
            condition,
            body,
            ..
        } => {
            collect_violations(condition, context, out);
            collect_violations(body, context, out);
        }
        Expr::Loop { label: _, body, .. } => {
            collect_violations(body, context, out);
        }
        Expr::For {
            label: _,
            iterable,
            body,
            ..
        } => {
            collect_violations(iterable, context, out);
            collect_violations(body, context, out);
        }
        Expr::Binary { left, right, .. } => {
            collect_violations(left, context, out);
            collect_violations(right, context, out);
        }
        Expr::Unary { operand, .. } => {
            collect_violations(operand, context, out);
        }
        Expr::Assign { value, .. } => {
            collect_violations(value, context, out);
        }
        _ => {}
    }
}

/// Collects violations from a statement.
fn collect_stmt_violations(stmt: &Stmt, context: &str, out: &mut Vec<String>) {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Const { value, .. } => {
            collect_violations(value, context, out);
        }
        Stmt::Expr { expr, .. } => {
            collect_violations(expr, context, out);
        }
        Stmt::Return { value: Some(v), .. } => {
            collect_violations(v, context, out);
        }
        _ => {}
    }
}

/// Checks whether a function call name is forbidden in the given context.
fn check_call_name(name: &str, context: &str, out: &mut Vec<String>) {
    match context {
        "kernel" => {
            // KE002: tensor ops forbidden in @kernel
            const TENSOR_OPS: &[&str] = &[
                "tensor_zeros",
                "tensor_ones",
                "tensor_rand",
                "tensor_xavier",
                "tensor_matmul",
                "tensor_relu",
                "tensor_sigmoid",
                "tensor_softmax",
                "zeros",
                "ones",
                "randn",
                "xavier",
                "matmul",
                "relu",
                "sigmoid",
                "softmax",
                "backward",
                "tensor_grad",
                "cross_entropy_loss",
            ];
            if TENSOR_OPS.contains(&name) {
                out.push(format!("[KE002] tensor op '{}' forbidden in @kernel", name));
            }
            // KE001: heap allocation forbidden in @kernel
            const HEAP_OPS: &[&str] = &[
                "String_new",
                "Vec_new",
                "read_file",
                "write_file",
                "append_file",
            ];
            if HEAP_OPS.contains(&name) {
                out.push(format!("[KE001] heap op '{}' forbidden in @kernel", name));
            }
        }
        "device" => {
            // DE001: raw pointer ops forbidden in @device
            const PTR_OPS: &[&str] = &[
                "mem_alloc",
                "mem_free",
                "mem_read",
                "mem_write",
                "mem_read_u8",
                "mem_read_u16",
                "mem_read_u32",
                "mem_write_u8",
                "mem_write_u16",
                "mem_write_u32",
            ];
            if PTR_OPS.contains(&name) {
                out.push(format!(
                    "[DE001] raw pointer op '{}' forbidden in @device",
                    name
                ));
            }
            // DE002: IRQ/hardware ops forbidden in @device
            const IRQ_OPS: &[&str] = &[
                "irq_register",
                "irq_unregister",
                "irq_enable",
                "irq_disable",
                "port_read",
                "port_write",
            ];
            if IRQ_OPS.contains(&name) {
                out.push(format!(
                    "[DE002] hardware op '{}' forbidden in @device",
                    name
                ));
            }
        }
        _ => {}
    }
}

impl CraneliftCompiler {
    /// Creates a new Cranelift JIT compiler for the host target.
    pub fn new() -> Result<Self, CodegenError> {
        Self::with_opt_level("none")
    }

    /// Creates a new Cranelift JIT compiler with specified optimization level.
    /// Valid levels: "none", "speed", "speed_and_size"
    pub fn with_opt_level(opt_level: &str) -> Result<Self, CodegenError> {
        let mut flag_builder = settings::builder();
        flag_builder
            .set("use_colocated_libcalls", "false")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        flag_builder
            .set("is_pic", "false")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;
        flag_builder
            .set("opt_level", opt_level)
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        let isa_builder = cranelift_native::builder()
            .map_err(|e| CodegenError::Internal(format!("host ISA: {e}")))?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e: cranelift_codegen::CodegenError| CodegenError::Internal(e.to_string()))?;

        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        // Lazy runtime symbol resolution: instead of pre-registering ~300 symbols,
        // use a lookup function that resolves fj_rt_* names on demand.
        builder.symbol_lookup_fn(Box::new(runtime_fns::lookup_runtime_symbol));

        let module = JITModule::new(builder);
        let ctx = module.make_context();

        Ok(CraneliftCompiler {
            module,
            ctx,
            builder_ctx: FunctionBuilderContext::new(),
            functions: HashMap::new(),
            string_data: HashMap::new(),
            generic_fns: HashMap::new(),
            generic_fn_params: HashMap::new(),
            mono_map: HashMap::new(),
            fn_return_types: HashMap::new(),
            enum_defs: HashMap::new(),
            enum_variant_types: HashMap::new(),
            generic_enum_defs: HashMap::new(),
            struct_defs: HashMap::new(),
            union_names: HashSet::new(),
            bitfield_layouts: HashMap::new(),
            impl_methods: HashMap::new(),
            trait_defs: HashMap::new(),
            trait_impls: HashMap::new(),
            const_defs: Vec::new(),
            const_fn_defs: HashMap::new(),
            fn_array_returns: HashMap::new(),
            fn_returns_string: HashSet::new(),
            fn_returns_heap_array: HashSet::new(),
            fn_returns_closure_handle: HashSet::new(),
            fn_returns_struct: HashMap::new(),
            fn_returns_enum: HashSet::new(),
            closure_fn_map: HashMap::new(),
            closure_captures: HashMap::new(),
            closure_span_to_fn: HashMap::new(),
            module_fns: HashMap::new(),
            no_std: false,
            user_mode: false,
            panic_handler_fn: None,
            async_fns: HashSet::new(),
            global_asm_sections: Vec::new(),
            fn_sections: HashMap::new(),
            data_sections: HashMap::new(),
            global_data: HashMap::new(),
            interrupt_fns: Vec::new(),
            security_enabled: false,
            lint_on_compile: false,
            dead_functions: HashSet::new(),
            profiling_enabled: false,
        })
    }

    /// Returns the list of `@interrupt`-annotated function names.
    ///
    /// Use with `linker::generate_interrupt_wrapper()` to create
    /// assembly wrappers that save/restore registers and use `eret`.
    pub fn interrupt_functions(&self) -> &[String] {
        &self.interrupt_fns
    }

    /// Enables no_std mode: disables IO/heap runtime declarations.
    pub fn set_no_std(&mut self, enabled: bool) {
        self.no_std = enabled;
    }

    /// Enables runtime security hardening (bounds checks, overflow checks).
    pub fn enable_security(&mut self) {
        self.security_enabled = true;
    }

    /// Enables the security linter pre-pass before compilation.
    pub fn enable_lint(&mut self) {
        self.lint_on_compile = true;
    }

    /// Sets the dead function list from the optimizer's dead function elimination pass.
    /// Functions in this set will be skipped during codegen.
    pub fn set_dead_functions(&mut self, dead: Vec<String>) {
        self.dead_functions = dead.into_iter().collect();
    }

    /// Enables profiling instrumentation in compiled code.
    pub fn enable_profiling(&mut self) {
        self.profiling_enabled = true;
    }

    /// Declares built-in runtime functions (println, print) in the module.
    fn declare_runtime_functions(&mut self) -> Result<(), CodegenError> {
        // In no_std mode, declare bare-metal HAL builtins alongside standard ones.
        // The JIT linker resolves them to the hosted simulation functions in runtime_bare.rs.
        if self.no_std {
            self.declare_bare_metal_jit_builtins()?;
        }

        runtime_hosted::declare_hosted_runtime(
            &mut self.module,
            &mut self.functions,
            /* jit_extras */ true,
            self.profiling_enabled,
        )
    }

    /// Compiles all functions in a program.
    pub fn compile_program(&mut self, program: &Program) -> Result<(), Vec<CodegenError>> {
        let mut errors = Vec::new();

        // Security linter pre-pass: emit warnings for known unsafe patterns.
        if self.lint_on_compile {
            let source_repr = format!("{program:?}");
            let linter = crate::codegen::security::SecurityLinter::new();
            let violations = linter.lint(&source_repr);
            for v in &violations {
                eprintln!("security: [{}] line {}: {}", v.rule_id, v.line, v.message);
            }
        }

        // H1: Enforce no_std compliance when enabled
        if self.no_std {
            let config = crate::codegen::nostd::NoStdConfig::bare_metal();
            let violations = crate::codegen::nostd::check_nostd_compliance(program, &config);
            for v in violations {
                errors.push(CodegenError::NoStdViolation(v.to_string()));
            }
            if !errors.is_empty() {
                return Err(errors);
            }
        }

        // Declare runtime built-in functions
        if let Err(e) = self.declare_runtime_functions() {
            errors.push(e);
            return Err(errors);
        }

        // Register built-in Poll<T> enum: Ready(T)=0, Pending=1 (S4.1)
        self.enum_defs.insert(
            "Poll".to_string(),
            vec!["Ready".to_string(), "Pending".to_string()],
        );
        self.enum_variant_types.insert(
            ("Poll".to_string(), "Ready".to_string()),
            vec![clif_types::default_int_type()],
        );
        self.generic_enum_defs
            .insert("Poll".to_string(), vec!["T".to_string()]);

        // Collect enum and struct definitions
        for item in &program.items {
            match item {
                Item::EnumDef(edef) => {
                    let variants: Vec<String> =
                        edef.variants.iter().map(|v| v.name.clone()).collect();
                    // Track payload types for each variant
                    for v in &edef.variants {
                        let payload_types: Vec<cranelift_codegen::ir::Type> = v
                            .fields
                            .iter()
                            .map(|f| {
                                clif_types::lower_type(f).unwrap_or(clif_types::default_int_type())
                            })
                            .collect();
                        self.enum_variant_types
                            .insert((edef.name.clone(), v.name.clone()), payload_types);
                    }
                    // Track generic enum definitions (S1.2)
                    if !edef.generic_params.is_empty() {
                        let param_names: Vec<String> = edef
                            .generic_params
                            .iter()
                            .map(|gp| gp.name.clone())
                            .collect();
                        self.generic_enum_defs
                            .insert(edef.name.clone(), param_names);
                    }
                    self.enum_defs.insert(edef.name.clone(), variants);
                }
                Item::StructDef(sdef) => {
                    let fields: Vec<(String, cranelift_codegen::ir::Type)> = sdef
                        .fields
                        .iter()
                        .map(|f| {
                            let ty = clif_types::lower_type(&f.ty)
                                .unwrap_or(clif_types::default_int_type());
                            (f.name.clone(), ty)
                        })
                        .collect();
                    // Check for bitfield fields (u1-u7 types) and compute layout
                    let mut bit_offset: u8 = 0;
                    let mut bf_layout = Vec::new();
                    for f in &sdef.fields {
                        if let TypeExpr::Simple { name: tname, .. } = &f.ty {
                            if let Some(width) = clif_types::bitfield_width(tname) {
                                bf_layout.push((f.name.clone(), bit_offset, width));
                                bit_offset += width;
                            }
                        }
                    }
                    if !bf_layout.is_empty() {
                        self.bitfield_layouts.insert(sdef.name.clone(), bf_layout);
                    }
                    self.struct_defs.insert(sdef.name.clone(), fields);
                }
                Item::UnionDef(udef) => {
                    let fields: Vec<(String, cranelift_codegen::ir::Type)> = udef
                        .fields
                        .iter()
                        .map(|f| {
                            let ty = clif_types::lower_type(&f.ty)
                                .unwrap_or(clif_types::default_int_type());
                            (f.name.clone(), ty)
                        })
                        .collect();
                    self.struct_defs.insert(udef.name.clone(), fields);
                    self.union_names.insert(udef.name.clone());
                }
                Item::ConstDef(cdef) => {
                    self.const_defs
                        .push((cdef.name.clone(), *cdef.value.clone(), cdef.ty.clone()));
                    if let Some(ref ann) = cdef.annotation {
                        if ann.name == "section" {
                            if let Some(ref sec) = ann.param {
                                self.data_sections.insert(cdef.name.clone(), sec.clone());
                            }
                        }
                    }
                }
                Item::StaticDef(sdef) => {
                    // Treat static mut like const for codegen
                    self.const_defs
                        .push((sdef.name.clone(), *sdef.value.clone(), sdef.ty.clone()));
                }
                Item::ServiceDef(_svc) => {
                    // Service handlers are processed when encountered as FnDef
                    // (ServiceDef expands to FnDef items in the analyzer)
                }
                Item::ModDecl(mdecl) => {
                    if let Some(ref body) = mdecl.body {
                        for mod_item in body {
                            match mod_item {
                                Item::EnumDef(edef) => {
                                    let variants: Vec<String> =
                                        edef.variants.iter().map(|v| v.name.clone()).collect();
                                    self.enum_defs.insert(edef.name.clone(), variants);
                                }
                                Item::StructDef(sdef) => {
                                    let fields: Vec<(String, cranelift_codegen::ir::Type)> = sdef
                                        .fields
                                        .iter()
                                        .map(|f| {
                                            let ty = clif_types::lower_type(&f.ty)
                                                .unwrap_or(clif_types::default_int_type());
                                            (f.name.clone(), ty)
                                        })
                                        .collect();
                                    self.struct_defs.insert(sdef.name.clone(), fields);
                                }
                                Item::ConstDef(cdef) => {
                                    self.const_defs.push((
                                        cdef.name.clone(),
                                        *cdef.value.clone(),
                                        cdef.ty.clone(),
                                    ));
                                    if let Some(ref ann) = cdef.annotation {
                                        if ann.name == "section" {
                                            if let Some(ref sec) = ann.param {
                                                self.data_sections
                                                    .insert(cdef.name.clone(), sec.clone());
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Collect global_asm sections
        for item in &program.items {
            if let Item::GlobalAsm(ga) = item {
                self.global_asm_sections.push(ga.template.clone());
            }
        }

        // Collect trait definitions and impls (shared helper)
        let (td, ti) = collect_trait_info(program);
        self.trait_defs = td;
        self.trait_impls = ti;

        // Declare extern functions (imported C symbols)
        for item in &program.items {
            if let Item::ExternFn(efn) = item {
                if let Err(e) = self.declare_extern_fn(efn) {
                    errors.push(e);
                }
            }
        }

        // Separate generic from concrete functions (including module functions)
        let mut concrete_fns = Vec::new();
        for item in &program.items {
            if let Item::FnDef(fndef) = item {
                if fndef.generic_params.is_empty() {
                    concrete_fns.push(fndef.clone());
                    // Extract nested function definitions from body
                    extract_nested_fns(&fndef.body, &mut concrete_fns);
                } else {
                    self.generic_fns.insert(fndef.name.clone(), fndef.clone());
                }
            }
            // Flatten module functions with mangled names: modname_fnname
            if let Item::ModDecl(mdecl) = item {
                if let Some(ref body) = mdecl.body {
                    for mod_item in body {
                        if let Item::FnDef(fndef) = mod_item {
                            let mut mangled_fn = fndef.clone();
                            let mangled_name = format!("{}_{}", mdecl.name, fndef.name);
                            mangled_fn.name = mangled_name.clone();
                            self.module_fns.insert(mangled_name, mdecl.name.clone());
                            if mangled_fn.generic_params.is_empty() {
                                concrete_fns.push(mangled_fn);
                            } else {
                                self.generic_fns.insert(mangled_fn.name.clone(), mangled_fn);
                            }
                        }
                    }
                }
            }
        }

        // Collect const fn definitions for compile-time evaluation
        for fndef in &concrete_fns {
            if fndef.is_const {
                self.const_fn_defs.insert(fndef.name.clone(), fndef.clone());
            }
        }

        // Collect impl blocks: mangle methods as TypeName_method_name
        for item in &program.items {
            if let Item::ImplBlock(impl_block) = item {
                for method in &impl_block.methods {
                    let mangled = format!("{}_{}", impl_block.target_type, method.name);
                    self.impl_methods.insert(
                        (impl_block.target_type.clone(), method.name.clone()),
                        mangled.clone(),
                    );
                    let mut mangled_fn = method.clone();
                    mangled_fn.name = mangled;
                    concrete_fns.push(mangled_fn);
                }
            }
        }

        // Dead function elimination: compute reachable set from entry points
        // This must happen BEFORE declarations so we only declare reachable functions.
        let mut fn_bodies_for_dce: HashMap<String, &Expr> = HashMap::new();
        let mut dce_entry_points = Vec::new();
        for fndef in &concrete_fns {
            fn_bodies_for_dce.insert(fndef.name.clone(), &fndef.body);
            if fndef.name == "main" {
                dce_entry_points.push(fndef.name.clone());
            }
            if let Some(ref ann) = fndef.annotation {
                if ann.name == "entry"
                    || ann.name == "panic_handler"
                    || ann.name == "kernel"
                    || ann.name == "unsafe"
                {
                    dce_entry_points.push(fndef.name.clone());
                }
            }
            // K1: Bare-metal entry points — these are called from assembly/linker,
            // not from .fj code, so they appear dead to call-graph analysis.
            if self.no_std
                && (fndef.name == "kernel_main"
                    || fndef.name == "_start"
                    || fndef.name == "fj_exception_sync"
                    || fndef.name == "fj_exception_irq")
            {
                dce_entry_points.push(fndef.name.clone());
            }
            // Public functions are always reachable (library exports)
            if fndef.is_pub {
                dce_entry_points.push(fndef.name.clone());
            }
        }
        // K1: Scan for fn_addr("name") calls — these reference functions by string,
        // invisible to normal call-graph DCE.
        for fndef in &concrete_fns {
            crate::codegen::cranelift::scan_fn_addr_targets(&fndef.body, &mut dce_entry_points);
        }
        // If no explicit entry points, keep all functions (library mode)
        let mut reachable = if dce_entry_points.is_empty() {
            concrete_fns.iter().map(|f| f.name.clone()).collect()
        } else {
            compute_reachable(&dce_entry_points, &fn_bodies_for_dce)
        };
        drop(fn_bodies_for_dce);
        // Expand reachability for impl/module methods (fixpoint loop).
        // Mangled names like "Point_area" need to be reached when the short name
        // "area" or the type name "Point" is referenced from reachable code.
        loop {
            let mut all_referenced: HashSet<String> = HashSet::new();
            for fndef in &concrete_fns {
                if reachable.contains(&fndef.name) {
                    collect_called_fns(&fndef.body, &mut all_referenced);
                }
            }
            let mut changed = false;
            for fndef in &concrete_fns {
                if reachable.contains(&fndef.name) {
                    continue;
                }
                if let Some(idx) = fndef.name.find('_') {
                    let suffix = &fndef.name[idx + 1..];
                    let prefix = &fndef.name[..idx];
                    if all_referenced.contains(suffix) || all_referenced.contains(prefix) {
                        reachable.insert(fndef.name.clone());
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        // Mark @interrupt and @panic_handler functions as always reachable
        for fndef in &concrete_fns {
            if let Some(ref ann) = fndef.annotation {
                if ann.name == "interrupt" || ann.name == "panic_handler" || ann.name == "entry" {
                    reachable.insert(fndef.name.clone());
                }
            }
        }

        // Filter concrete_fns to only reachable functions.
        // In bare-metal (no_std) JIT mode, skip DCE — OS kernels use fn_addr,
        // interrupt vectors, and asm trampolines invisible to static analysis.
        if !self.no_std {
            concrete_fns.retain(|f| reachable.contains(&f.name));
        }

        // First pass: declare concrete functions (forward declarations)
        for fndef in &concrete_fns {
            if let Err(e) = self.declare_function(fndef) {
                errors.push(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        // Pre-scan for closures in all function bodies
        let mut known_names: HashSet<String> = self.functions.keys().cloned().collect();
        // Add builtins and enum variants to known names
        for names in self.enum_defs.values() {
            for v in names {
                known_names.insert(v.clone());
            }
        }
        for name in self.enum_defs.keys() {
            known_names.insert(name.clone());
        }
        for name in self.struct_defs.keys() {
            known_names.insert(name.clone());
        }
        // Add common builtins that should not be treated as captures
        for builtin in &[
            "print",
            "println",
            "eprintln",
            "eprint",
            "len",
            "assert",
            "assert_eq",
            "to_string",
            "to_int",
            "to_float",
            "type_of",
            "format",
            "dbg",
            "panic",
            "todo",
            "abs",
            "sqrt",
            "pow",
            "sin",
            "cos",
            "tan",
            "floor",
            "ceil",
            "round",
            "clamp",
            "min",
            "max",
            "log",
            "log2",
            "log10",
            "Some",
            "None",
            "Ok",
            "Err",
            "read_file",
            "write_file",
            "append_file",
            "file_exists",
            "wrapping_add",
            "wrapping_sub",
            "wrapping_mul",
            "saturating_add",
            "saturating_sub",
            "saturating_mul",
            "checked_add",
            "checked_sub",
            "checked_mul",
            "true",
            "false",
            "null",
        ] {
            known_names.insert(builtin.to_string());
        }

        let mut closure_fns: Vec<FnDef> = Vec::new();
        for fndef in &concrete_fns {
            let closures = scan_closures_in_body(&fndef.body, &known_names);
            let mut has_captured_closure = false;
            for ci in &closures {
                self.closure_fn_map
                    .insert(ci.var_name.clone(), ci.fn_name.clone());
                self.closure_span_to_fn
                    .insert((ci.span.start, ci.span.end), ci.fn_name.clone());
                self.closure_captures
                    .insert(ci.fn_name.clone(), ci.captures.clone());
                if !ci.captures.is_empty() {
                    has_captured_closure = true;
                }
            }
            for ci in closures {
                closure_fns.push(ci.fndef);
            }
            // If this function has closures with captures and returns a fn type,
            // mark it as returning a closure handle.
            if has_captured_closure && fndef.return_type.is_some() {
                if let Some(TypeExpr::Fn { .. }) = &fndef.return_type {
                    self.fn_returns_closure_handle.insert(fndef.name.clone());
                }
            }
        }

        // Declare and define closure functions
        for cfn in &closure_fns {
            if let Err(e) = self.declare_function(cfn) {
                errors.push(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        // Monomorphize: scan all concrete fn bodies for calls to generic fns
        let mono_fns = self.monomorphize(&concrete_fns);
        for mono_fn in &mono_fns {
            if let Err(e) = self.declare_function(mono_fn) {
                errors.push(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        // Build generic param mapping for multi-param type dispatch
        self.generic_fn_params = self.build_generic_fn_params();

        // Scan for @panic_handler, @section, @interrupt annotations and async functions
        for fndef in &concrete_fns {
            if let Some(ref ann) = fndef.annotation {
                if ann.name == "panic_handler" {
                    self.panic_handler_fn = Some(fndef.name.clone());
                }
                if ann.name == "section" {
                    if let Some(ref section_name) = ann.param {
                        self.fn_sections
                            .insert(fndef.name.clone(), section_name.clone());
                    }
                }
                if ann.name == "interrupt" {
                    self.interrupt_fns.push(fndef.name.clone());
                }
            }
            if fndef.is_async {
                self.async_fns.insert(fndef.name.clone());
            }
        }

        // Second pass: compile function bodies (concrete + monomorphized + closures).
        // Skip dead functions identified by the optimizer (if any).
        for fndef in &concrete_fns {
            if self.dead_functions.contains(&fndef.name) {
                continue; // dead function — optimizer says unreachable
            }
            if let Err(e) = self.define_function(fndef) {
                errors.push(e);
            }
        }
        for mono_fn in &mono_fns {
            if let Err(e) = self.define_function(mono_fn) {
                errors.push(e);
            }
        }
        for cfn in &closure_fns {
            if let Err(e) = self.define_function(cfn) {
                errors.push(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        // Create global data objects for section-annotated consts (JIT: no-op for sections)
        for (cname, cexpr, cty) in &self.const_defs {
            if let Some(section) = self.data_sections.get(cname).cloned() {
                let byte_size = clif_types::lower_type(cty)
                    .map(|t| t.bytes() as usize)
                    .unwrap_or(8);
                if let Ok(data_id) = self
                    .module
                    .declare_data(cname, Linkage::Export, true, false)
                {
                    let mut desc = cranelift_module::DataDescription::new();
                    let init_bytes = match cexpr {
                        Expr::Literal {
                            kind: LiteralKind::Int(v),
                            ..
                        } => v.to_le_bytes()[..byte_size].to_vec(),
                        Expr::Literal {
                            kind: LiteralKind::Float(f),
                            ..
                        } => f.to_le_bytes()[..byte_size].to_vec(),
                        _ => vec![0u8; byte_size],
                    };
                    desc.define(init_bytes.into_boxed_slice());
                    desc.set_segment_section("", &section);
                    let _ = self.module.define_data(data_id, &desc);
                    self.global_data.insert(cname.clone(), data_id);
                }
            }
        }

        self.module
            .finalize_definitions()
            .map_err(|e| vec![CodegenError::ModuleError(e.to_string())])?;

        Ok(())
    }

    /// Scans concrete functions for calls to generic functions and creates
    /// monomorphized (type-specialized) versions.
    fn monomorphize(&mut self, concrete_fns: &[FnDef]) -> Vec<FnDef> {
        let mut mono_fns = Vec::new();
        let mut mono_specs: HashSet<(String, String)> = HashSet::new();

        for fndef in concrete_fns {
            // Build param type map from function parameters for type inference
            let mut param_types = HashMap::new();
            for p in &fndef.params {
                if let TypeExpr::Simple { name: tn, .. } = &p.ty {
                    let clif_suffix = match tn.as_str() {
                        "f64" | "float" | "f32" => "f64",
                        _ => "i64",
                    };
                    param_types.insert(p.name.clone(), clif_suffix.to_string());
                }
            }
            collect_generic_calls(
                &fndef.body,
                &self.generic_fns,
                &mut self.mono_map,
                &mut mono_specs,
                &param_types,
            );
        }

        // Create specialized versions for each (fn_name, type_suffix) pair
        for (generic_name, type_suffix) in &mono_specs {
            if let Some(generic_def) = self.generic_fns.get(generic_name) {
                let mangled_name = format!("{generic_name}__mono_{type_suffix}");
                let specialized = specialize_fndef(generic_def, &mangled_name, type_suffix);
                mono_fns.push(specialized);
            }
        }

        mono_fns
    }

    /// Builds mapping from generic fn names to their parameter→generic_param associations.
    ///
    /// For `fn foo<T, U>(a: T, b: i32, c: U)`, produces:
    /// `"foo" → [(0, "T"), (2, "U")]` — arg index 0 maps to generic param T, index 2 to U.
    fn build_generic_fn_params(&self) -> HashMap<String, Vec<(usize, String)>> {
        let mut result = HashMap::new();
        for (name, fndef) in &self.generic_fns {
            let generic_param_names: Vec<String> = fndef
                .generic_params
                .iter()
                .map(|gp| gp.name.clone())
                .collect();
            let mut mappings = Vec::new();
            for (i, param) in fndef.params.iter().enumerate() {
                if let TypeExpr::Simple {
                    name: ptype_name, ..
                } = &param.ty
                {
                    if generic_param_names.contains(ptype_name) {
                        mappings.push((i, ptype_name.clone()));
                    }
                }
            }
            result.insert(name.clone(), mappings);
        }
        result
    }

    /// Returns whether a function's return type is void.
    fn is_void_return(fndef: &crate::parser::ast::FnDef) -> bool {
        fndef.return_type.as_ref().is_some_and(
            |ty| matches!(ty, crate::parser::ast::TypeExpr::Simple { name, .. } if name == "void"),
        )
    }

    /// Returns the Cranelift return type for a function definition.
    fn get_return_clif_type(
        fndef: &crate::parser::ast::FnDef,
    ) -> Option<cranelift_codegen::ir::Type> {
        fndef.return_type.as_ref().and_then(clif_types::lower_type)
    }

    /// Returns true if the function returns an enum type.
    fn is_enum_return(&self, fndef: &crate::parser::ast::FnDef) -> bool {
        if let Some(crate::parser::ast::TypeExpr::Simple { name, .. }) = &fndef.return_type {
            self.enum_defs.contains_key(name)
        } else {
            false
        }
    }

    /// Returns true if the function definition returns `str`.
    fn is_string_return(fndef: &crate::parser::ast::FnDef) -> bool {
        fndef.return_type.as_ref().is_some_and(
            |ty| matches!(ty, crate::parser::ast::TypeExpr::Simple { name, .. } if name == "str"),
        )
    }

    /// Declares a function signature (first pass).
    fn declare_function(&mut self, fndef: &crate::parser::ast::FnDef) -> Result<(), CodegenError> {
        let has_return = !Self::is_void_return(fndef);
        let call_conv = self.module.target_config().default_call_conv;

        // Check if this function returns a struct type
        let struct_ret_name = fndef.return_type.as_ref().and_then(|ty| {
            if let TypeExpr::Simple { name, .. } = ty {
                if self.struct_defs.contains_key(name) {
                    Some(name.clone())
                } else {
                    None
                }
            } else {
                None
            }
        });

        let is_enum_ret = self.is_enum_return(fndef);
        // For enum returns, force ret_type to I64 (tag) since lower_type("EnumName") is None
        let ret_type = if is_enum_ret {
            Some(clif_types::default_int_type())
        } else {
            Self::get_return_clif_type(fndef)
        };
        // For enum returns, has_return must be true
        let has_return = has_return || is_enum_ret;

        let mut sig = if let Some(ref sname) = struct_ret_name {
            // Struct-returning: no default return value in build_signature
            let mut s = super::abi::build_signature_with_return_type(
                call_conv,
                &fndef.params,
                false,
                None,
            )?;
            // Add one return value per field
            let fields = &self.struct_defs[sname];
            for (_fname, ftype) in fields {
                s.returns.push(cranelift_codegen::ir::AbiParam::new(*ftype));
            }
            s
        } else {
            super::abi::build_signature_with_return_type(
                call_conv,
                &fndef.params,
                has_return,
                ret_type,
            )?
        };

        // String-returning functions use two return values: (ptr, len)
        let is_str_ret = Self::is_string_return(fndef);
        if is_str_ret {
            // Already has one return (ptr as I64); add second (len as I64)
            sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        }
        // Enum-returning functions use two return values: (tag, payload)
        if is_enum_ret {
            sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        }

        let func_id = self
            .module
            .declare_function(&fndef.name, Linkage::Export, &sig)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;

        self.functions.insert(fndef.name.clone(), func_id);
        if let Some(rt) = ret_type {
            self.fn_return_types.insert(fndef.name.clone(), rt);
        }
        if is_str_ret {
            self.fn_returns_string.insert(fndef.name.clone());
        }
        if is_enum_ret {
            self.fn_returns_enum.insert(fndef.name.clone());
        }
        // Track array return metadata
        if let Some(TypeExpr::Array {
            ref element, size, ..
        }) = fndef.return_type
        {
            let elem_type =
                clif_types::lower_type(element).unwrap_or(clif_types::default_int_type());
            self.fn_array_returns
                .insert(fndef.name.clone(), (size as usize, elem_type));
        }
        // Track heap array (Slice) return
        if matches!(fndef.return_type, Some(TypeExpr::Slice { .. })) {
            self.fn_returns_heap_array.insert(fndef.name.clone());
        }
        // Track struct return
        if let Some(sname) = struct_ret_name {
            self.fn_returns_struct.insert(fndef.name.clone(), sname);
        }
        Ok(())
    }

    /// Declares an extern (imported) function with C ABI linkage.
    fn declare_extern_fn(&mut self, efn: &ExternFn) -> Result<(), CodegenError> {
        let has_return = efn.return_type.as_ref().is_some_and(
            |ty| !matches!(ty, crate::parser::ast::TypeExpr::Simple { name, .. } if name == "void"),
        );
        let ret_type = efn.return_type.as_ref().and_then(clif_types::lower_type);
        let call_conv = self.module.target_config().default_call_conv;
        let sig = super::abi::build_signature_with_return_type(
            call_conv,
            &efn.params,
            has_return,
            ret_type,
        )?;

        let func_id = self
            .module
            .declare_function(&efn.name, Linkage::Import, &sig)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;

        self.functions.insert(efn.name.clone(), func_id);
        if let Some(rt) = ret_type {
            self.fn_return_types.insert(efn.name.clone(), rt);
        }
        Ok(())
    }

    /// Defines (compiles the body of) a function (second pass).
    fn define_function(&mut self, fndef: &crate::parser::ast::FnDef) -> Result<(), CodegenError> {
        let func_id = *self
            .functions
            .get(&fndef.name)
            .ok_or_else(|| CodegenError::UndefinedFunction(fndef.name.clone()))?;

        // H4: Context enforcement — reject forbidden builtins before codegen
        if let Some(ref ann) = fndef.annotation {
            let ctx_name = &ann.name;
            if ctx_name == "kernel" || ctx_name == "device" {
                let forbidden = check_context_violations(&fndef.body, ctx_name);
                if !forbidden.is_empty() {
                    return Err(CodegenError::ContextViolation(format!(
                        "@{ctx_name} fn '{}': {}",
                        fndef.name,
                        forbidden.join("; ")
                    )));
                }
            }
        }

        let is_enum_ret = self.fn_returns_enum.contains(&fndef.name);
        let has_return = !Self::is_void_return(fndef) || is_enum_ret;
        let ret_type = if is_enum_ret {
            Some(clif_types::default_int_type())
        } else {
            Self::get_return_clif_type(fndef)
        };
        let call_conv = self.module.target_config().default_call_conv;
        let is_struct_ret = self.fn_returns_struct.contains_key(&fndef.name);

        let mut sig = if is_struct_ret {
            // Match the signature from declare_function: multi-value return per field
            let sname = &self.fn_returns_struct[&fndef.name];
            let mut s = super::abi::build_signature_with_return_type(
                call_conv,
                &fndef.params,
                false,
                None,
            )?;
            let fields = &self.struct_defs[sname];
            for (_fname, ftype) in fields {
                s.returns.push(cranelift_codegen::ir::AbiParam::new(*ftype));
            }
            s
        } else {
            super::abi::build_signature_with_return_type(
                call_conv,
                &fndef.params,
                has_return,
                ret_type,
            )?
        };
        // String-returning functions use two return values: (ptr, len)
        let is_str_ret = self.fn_returns_string.contains(&fndef.name);
        if is_str_ret {
            sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        }
        // Enum-returning functions use two return values: (tag, payload)
        if is_enum_ret {
            sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        }
        self.ctx.func.signature = sig;

        {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_ctx);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            // Stack canary prologue: generate and store canary for corruption detection.
            let canary_var = if self.security_enabled {
                if let (Some(&gen_id), Some(_)) = (
                    self.functions.get("__canary_generate"),
                    self.functions.get("__canary_check"),
                ) {
                    let int_ty = clif_types::default_int_type();
                    let call_site_hash = {
                        let mut h: u64 = 0xcbf29ce484222325;
                        for b in fndef.name.bytes() {
                            h ^= b as u64;
                            h = h.wrapping_mul(0x100000001b3);
                        }
                        h as i64
                    };
                    let site_id = builder.ins().iconst(int_ty, call_site_hash);
                    let callee = self.module.declare_func_in_func(gen_id, builder.func);
                    let call = builder.ins().call(callee, &[site_id]);
                    let canary_val = builder.inst_results(call)[0];
                    let var = builder.declare_var(int_ty);
                    builder.def_var(var, canary_val);
                    Some(var)
                } else {
                    None
                }
            } else {
                None
            };

            // Profiler prologue: emit fj_rt_profile_enter(name_ptr, name_len).
            if self.profiling_enabled {
                if let Some(&enter_id) = self.functions.get("__profile_enter") {
                    let fn_name_bytes = fndef.name.as_bytes();
                    let name_data_id = {
                        let data_key = format!("__profname_{}", fndef.name);
                        if let Some(&existing) = self.string_data.get(&data_key) {
                            existing
                        } else {
                            let mut data_desc = cranelift_module::DataDescription::new();
                            data_desc.define(fn_name_bytes.to_vec().into_boxed_slice());
                            let did = self
                                .module
                                .declare_data(
                                    &data_key,
                                    cranelift_module::Linkage::Local,
                                    false,
                                    false,
                                )
                                .unwrap_or_else(|_| panic!("declare data for {data_key}"));
                            let _ = self.module.define_data(did, &data_desc);
                            self.string_data.insert(data_key, did);
                            did
                        }
                    };
                    let gv = self.module.declare_data_in_func(name_data_id, builder.func);
                    let name_ptr = builder
                        .ins()
                        .global_value(self.module.target_config().pointer_type(), gv);
                    let name_len = builder
                        .ins()
                        .iconst(clif_types::default_int_type(), fn_name_bytes.len() as i64);
                    let callee = self.module.declare_func_in_func(enter_id, builder.func);
                    builder.ins().call(callee, &[name_ptr, name_len]);
                }
            }

            let mut var_map: HashMap<String, Variable> = HashMap::new();
            let mut var_types: HashMap<String, cranelift_codegen::ir::Type> = HashMap::new();
            let mut string_lens = HashMap::new();

            // Bind function parameters to variables
            // Use a separate block_param_idx because str params consume two block params
            let mut block_param_idx = 0usize;
            for param in &fndef.params {
                let param_type =
                    clif_types::lower_type(&param.ty).unwrap_or(clif_types::default_int_type());
                let var = builder.declare_var(param_type);
                let param_val = builder.block_params(entry_block)[block_param_idx];
                builder.def_var(var, param_val);
                var_map.insert(param.name.clone(), var);
                var_types.insert(param.name.clone(), param_type);
                block_param_idx += 1;

                // String params have a second block param for the length
                if matches!(&param.ty, TypeExpr::Simple { name, .. } if name == "str") {
                    let len_var = builder.declare_var(clif_types::default_int_type());
                    let len_val = builder.block_params(entry_block)[block_param_idx];
                    builder.def_var(len_var, len_val);
                    string_lens.insert(param.name.clone(), len_var);
                    block_param_idx += 1;
                }
            }

            // Compile the function body (which is a Block expression)
            let mut array_meta = HashMap::new();
            let mut heap_arrays = HashSet::new();
            let mut enum_vars = HashMap::new();
            let mut struct_slots = HashMap::new();

            // Array parameter setup: copy pointer-based arrays into local stack slots
            for param in &fndef.params {
                if let TypeExpr::Array {
                    ref element, size, ..
                } = param.ty
                {
                    let elem_type =
                        clif_types::lower_type(element).unwrap_or(clif_types::default_int_type());
                    let slot =
                        builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                            (size as u32) * 8,
                            3, // 8-byte alignment
                        ));
                    // Copy elements from param pointer to local stack slot
                    let param_ptr_var = var_map[&param.name];
                    let src_ptr = builder.use_var(param_ptr_var);
                    for idx in 0..size {
                        let src_offset = builder
                            .ins()
                            .iconst(clif_types::default_int_type(), (idx as i64) * 8);
                        let src_addr = builder.ins().iadd(src_ptr, src_offset);
                        let elem_val = builder.ins().load(
                            elem_type,
                            cranelift_codegen::ir::MemFlags::new(),
                            src_addr,
                            0,
                        );
                        builder.ins().stack_store(elem_val, slot, (idx as i32) * 8);
                    }
                    array_meta.insert(param.name.clone(), (slot, size as usize));
                    var_types.insert(param.name.clone(), elem_type);
                }
                // Slice (heap array) parameters: register in heap_arrays for .push()/.len() dispatch
                if matches!(&param.ty, TypeExpr::Slice { .. }) {
                    heap_arrays.insert(param.name.clone());
                }
            }

            // Struct parameter setup: copy from pointer into local stack slot (S4.8)
            for param in &fndef.params {
                if let TypeExpr::Simple {
                    name: ref type_name,
                    ..
                } = param.ty
                {
                    if let Some(fields) = self.struct_defs.get(type_name) {
                        let num_fields = fields.len();
                        let slot = builder.create_sized_stack_slot(
                            cranelift_codegen::ir::StackSlotData::new(
                                cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                                (num_fields as u32) * 8,
                                3, // 8-byte alignment
                            ),
                        );
                        // Copy fields from pointer parameter to local stack slot
                        let param_ptr_var = var_map[&param.name];
                        let src_ptr = builder.use_var(param_ptr_var);
                        for (idx, field) in fields.iter().enumerate().take(num_fields) {
                            let field_type = field.1;
                            let src_offset = builder
                                .ins()
                                .iconst(clif_types::default_int_type(), (idx as i64) * 8);
                            let src_addr = builder.ins().iadd(src_ptr, src_offset);
                            let val = builder.ins().load(
                                field_type,
                                cranelift_codegen::ir::MemFlags::new(),
                                src_addr,
                                0,
                            );
                            builder.ins().stack_store(val, slot, (idx as i32) * 8);
                        }
                        struct_slots.insert(param.name.clone(), (slot, type_name.clone()));
                    }
                }
            }

            // Function pointer parameter setup: track signature for call_indirect
            let mut fn_ptr_sigs = HashMap::new();
            for param in &fndef.params {
                if let TypeExpr::Fn {
                    ref params,
                    ref return_type,
                    ..
                } = param.ty
                {
                    let pt: Vec<_> = params
                        .iter()
                        .map(|p| {
                            clif_types::lower_type(p).unwrap_or(clif_types::default_int_type())
                        })
                        .collect();
                    let rt = clif_types::lower_type(return_type);
                    fn_ptr_sigs.insert(param.name.clone(), (pt, rt));
                }
            }

            // Determine if this function is an impl method by checking impl_methods
            let impl_type_for_fn = self
                .impl_methods
                .iter()
                .find(|(_, mangled)| *mangled == &fndef.name)
                .map(|((type_name, _), _)| type_name.clone());
            let mut cx = CodegenCtx {
                module: &mut self.module,
                functions: &self.functions,
                var_map: &mut var_map,
                string_data: &mut self.string_data,
                mono_map: &self.mono_map,
                array_meta: &mut array_meta,
                last_array: None,
                loop_exit: None,
                loop_header: None,
                labeled_loops: HashMap::new(),
                const_values: HashMap::new(),
                var_types: &mut var_types,
                fn_return_types: &self.fn_return_types,
                last_expr_type: None,
                string_lens: &mut string_lens,
                last_string_len: None,
                last_string_owned: false,
                heap_arrays: &mut heap_arrays,
                heap_maps: HashSet::new(),
                map_str_values: HashSet::new(),
                last_map_new: false,
                enum_defs: &self.enum_defs,
                enum_variant_types: &self.enum_variant_types,
                generic_enum_defs: &self.generic_enum_defs,
                enum_vars: &mut enum_vars,
                last_enum_payload: None,
                last_enum_payload_type: None,
                last_enum_multi_payload: None,
                enum_multi_vars: HashMap::new(),
                struct_defs: &self.struct_defs,
                union_names: &self.union_names,
                bitfield_layouts: &self.bitfield_layouts,
                struct_slots: &mut struct_slots,
                last_struct_init: None,
                tuple_types: HashMap::new(),
                last_tuple_elem_types: None,
                impl_methods: &self.impl_methods,
                trait_defs: &self.trait_defs,
                trait_impls: &self.trait_impls,
                owned_ptrs: Vec::new(),
                scope_stack: Vec::new(),
                current_impl_type: impl_type_for_fn,
                fn_array_returns: &self.fn_array_returns,
                last_heap_array: false,
                last_split_result: None,
                split_vars: HashSet::new(),
                fn_returns_string: &self.fn_returns_string,
                fn_returns_enum: &self.fn_returns_enum,
                fn_returns_heap_array: &self.fn_returns_heap_array,
                fn_returns_closure_handle: &self.fn_returns_closure_handle,
                fn_returns_struct: &self.fn_returns_struct,
                closure_fn_map: &self.closure_fn_map,
                closure_captures: &self.closure_captures,
                fn_ptr_sigs,
                closure_span_to_fn: self.closure_span_to_fn.clone(),
                closure_handle_vars: HashSet::new(),
                last_closure_handle: false,
                current_module: self.module_fns.get(&fndef.name).cloned(),
                thread_handles: HashSet::new(),
                last_thread_spawn: false,
                mutex_handles: HashSet::new(),
                last_mutex_new: false,
                channel_handles: HashSet::new(),
                last_channel_new: false,
                atomic_handles: HashSet::new(),
                last_atomic_new: false,
                last_atomic_subtype: "i64".to_string(),
                atomic_subtypes: std::collections::HashMap::new(),
                rwlock_handles: HashSet::new(),
                last_rwlock_new: false,
                barrier_handles: HashSet::new(),
                mutex_guard_handles: HashSet::new(),
                last_mutex_guard_new: false,
                condvar_handles: HashSet::new(),
                last_condvar_new: false,
                bounded_channel_handles: HashSet::new(),
                last_bounded_channel: false,
                last_barrier_new: false,
                arc_handles: HashSet::new(),
                last_arc_new: false,
                generic_fn_params: self.generic_fn_params.clone(),
                _async_fns: &self.async_fns,
                _future_handles: HashSet::new(),
                last_future_new: false,
                no_std: self.no_std,
                panic_handler_fn: self.panic_handler_fn.clone(),
                volatile_ptr_handles: HashSet::new(),
                last_volatile_ptr_new: false,
                mmio_regions: HashMap::new(),
                last_mmio_new: false,
                last_mmio_vals: None,
                bump_alloc_handles: HashSet::new(),
                last_bump_alloc_new: false,
                freelist_alloc_handles: HashSet::new(),
                last_freelist_alloc_new: false,
                pool_alloc_handles: HashSet::new(),
                last_pool_alloc_new: false,
                executor_handles: HashSet::new(),
                last_executor_new: false,
                waker_handles: HashSet::new(),
                last_waker_new: false,
                timer_handles: HashSet::new(),
                last_timer_new: false,
                threadpool_handles: HashSet::new(),
                last_threadpool_new: false,
                joinhandle_handles: HashSet::new(),
                last_joinhandle_new: false,
                async_channel_handles: HashSet::new(),
                last_async_channel_new: false,
                async_bchannel_handles: HashSet::new(),
                last_async_bchannel_new: false,
                stream_handles: HashSet::new(),
                last_stream_new: false,
                simd_f32x4_handles: HashSet::new(),
                last_simd_f32x4_new: false,
                simd_i32x4_handles: HashSet::new(),
                last_simd_i32x4_new: false,
                simd_f32x8_handles: HashSet::new(),
                last_simd_f32x8_new: false,
                simd_i32x8_handles: HashSet::new(),
                last_simd_i32x8_new: false,
                onnx_handles: HashSet::new(),
                last_onnx_new: false,
                async_io_handles: HashSet::new(),
                last_async_io_new: false,
                last_heap_array_return: false,
                fn_ret_type: ret_type,
                is_enum_return_fn: self.fn_returns_enum.contains(&fndef.name),
                current_context: fndef.annotation.as_ref().map(|a| a.name.clone()),
                security_enabled: self.security_enabled,
            };

            // Inject top-level const definitions as variables.
            // Try compile-time constant folding first for integer expressions.
            // Build const fn ref table for this scope
            let const_fn_refs: HashMap<String, &FnDef> = self
                .const_fn_defs
                .iter()
                .map(|(k, v)| (k.clone(), v))
                .collect();
            for (cname, cexpr, cty) in &self.const_defs {
                let const_folded =
                    compile::try_const_eval_with_fns(cexpr, &cx.const_values, &const_fn_refs, 0);
                let val = if let Some(cv) = const_folded {
                    builder.ins().iconst(clif_types::default_int_type(), cv)
                } else if let Ok(v) = compile_expr(&mut builder, &mut cx, cexpr) {
                    v
                } else {
                    continue;
                };
                if let Some(cv) = const_folded {
                    cx.const_values.insert(cname.clone(), cv);
                }
                let var_type = clif_types::lower_type(cty)
                    .unwrap_or(cx.last_expr_type.unwrap_or(clif_types::default_int_type()));
                let var = builder.declare_var(var_type);
                builder.def_var(var, val);
                cx.var_map.insert(cname.clone(), var);
                cx.var_types.insert(cname.clone(), var_type);
                if let Some(len_val) = cx.last_string_len.take() {
                    let len_var = builder.declare_var(clif_types::default_int_type());
                    builder.def_var(len_var, len_val);
                    cx.string_lens.insert(cname.clone(), len_var);
                }
            }
            // Check if this function returns an array (need heap copy for callee stack safety)
            let array_return_info = self.fn_array_returns.get(&fndef.name).copied();

            let is_async_fn = fndef.is_async;
            let compile_result = compile_expr(&mut builder, &mut cx, &fndef.body);
            match compile_result {
                Ok(result) => {
                    if is_async_fn {
                        // Async function: wrap body result in a future handle
                        let new_id = *cx.functions.get("__future_new").ok_or_else(|| {
                            CodegenError::Internal("__future_new not declared".into())
                        })?;
                        let new_callee = cx.module.declare_func_in_func(new_id, builder.func);
                        let new_call = builder.ins().call(new_callee, &[]);
                        let future_ptr = builder.inst_results(new_call)[0];

                        let set_id = *cx.functions.get("__future_set_result").ok_or_else(|| {
                            CodegenError::Internal("__future_set_result not declared".into())
                        })?;
                        let set_callee = cx.module.declare_func_in_func(set_id, builder.func);
                        builder.ins().call(set_callee, &[future_ptr, result]);

                        emit_owned_cleanup(&mut builder, &mut cx, Some(future_ptr))?;
                        builder.ins().return_(&[future_ptr]);
                    } else if is_struct_ret {
                        // Struct return: load each field from the struct's stack slot
                        // and return them as multi-value.
                        let sname = &self.fn_returns_struct[&fndef.name];
                        let fields = self.struct_defs[sname].clone();
                        if let Some((slot, _)) = cx.last_struct_init.take() {
                            let mut ret_vals = Vec::new();
                            for (i, (_fname, ftype)) in fields.iter().enumerate() {
                                let val = builder.ins().stack_load(*ftype, slot, (i as i32) * 8);
                                ret_vals.push(val);
                            }
                            emit_owned_cleanup(&mut builder, &mut cx, None)?;
                            builder.ins().return_(&ret_vals);
                        } else {
                            // Struct returned via variable/expression — load fields from pointer
                            let mut ret_vals = Vec::new();
                            for (i, (_fname, ftype)) in fields.iter().enumerate() {
                                let val = builder.ins().load(
                                    *ftype,
                                    cranelift_codegen::ir::MemFlags::new(),
                                    result,
                                    (i as i32) * 8,
                                );
                                ret_vals.push(val);
                            }
                            emit_owned_cleanup(&mut builder, &mut cx, None)?;
                            builder.ins().return_(&ret_vals);
                        }
                    } else if has_return {
                        let ret_val = if let Some((arr_len, elem_type)) = array_return_info {
                            // Array return: copy stack elements to heap buffer
                            let total_bytes = builder
                                .ins()
                                .iconst(clif_types::default_int_type(), (arr_len as i64) * 8);
                            let alloc_id = *cx.functions.get("__alloc").ok_or_else(|| {
                                CodegenError::Internal("__alloc not declared".into())
                            })?;
                            let local_alloc =
                                cx.module.declare_func_in_func(alloc_id, builder.func);
                            let alloc_call = builder.ins().call(local_alloc, &[total_bytes]);
                            let heap_ptr = builder.inst_results(alloc_call)[0];
                            // Copy elements from stack pointer to heap
                            for idx in 0..arr_len {
                                let offset = builder
                                    .ins()
                                    .iconst(clif_types::default_int_type(), (idx as i64) * 8);
                                let src_addr = builder.ins().iadd(result, offset);
                                let elem_val = builder.ins().load(
                                    elem_type,
                                    cranelift_codegen::ir::MemFlags::new(),
                                    src_addr,
                                    0,
                                );
                                let dst_addr = builder.ins().iadd(heap_ptr, offset);
                                builder.ins().store(
                                    cranelift_codegen::ir::MemFlags::new(),
                                    elem_val,
                                    dst_addr,
                                    0,
                                );
                            }
                            heap_ptr
                        } else {
                            result
                        };
                        // Profiler epilogue: emit fj_rt_profile_exit() before return.
                        if let Some(&exit_id) = cx.functions.get("__profile_exit") {
                            let callee = cx.module.declare_func_in_func(exit_id, builder.func);
                            builder.ins().call(callee, &[]);
                        }
                        // Stack canary epilogue: verify canary before returning.
                        if let Some(c_var) = canary_var {
                            if let Some(&chk_id) = cx.functions.get("__canary_check") {
                                let expected = builder.use_var(c_var);
                                let call_site_hash = {
                                    let mut h: u64 = 0xcbf29ce484222325;
                                    for b in fndef.name.bytes() {
                                        h ^= b as u64;
                                        h = h.wrapping_mul(0x100000001b3);
                                    }
                                    h as i64
                                };
                                let site_id = builder
                                    .ins()
                                    .iconst(clif_types::default_int_type(), call_site_hash);
                                if let Some(&gen_id) = cx.functions.get("__canary_generate") {
                                    let gen_callee =
                                        cx.module.declare_func_in_func(gen_id, builder.func);
                                    let gen_call = builder.ins().call(gen_callee, &[site_id]);
                                    let actual = builder.inst_results(gen_call)[0];
                                    let chk_callee =
                                        cx.module.declare_func_in_func(chk_id, builder.func);
                                    builder.ins().call(chk_callee, &[expected, actual]);
                                }
                            }
                        }
                        // Cleanup owned resources, excluding the returned value
                        emit_owned_cleanup(&mut builder, &mut cx, Some(ret_val))?;
                        if is_str_ret {
                            // String return: (ptr, len) — get len from last_string_len
                            let len_val = cx.last_string_len.take().unwrap_or_else(|| {
                                builder.ins().iconst(clif_types::default_int_type(), 0)
                            });
                            builder.ins().return_(&[ret_val, len_val]);
                        } else if cx.is_enum_return_fn {
                            // Enum return: (tag, payload)
                            let payload = cx.last_enum_payload.take().unwrap_or_else(|| {
                                builder.ins().iconst(clif_types::default_int_type(), 0)
                            });
                            cx.last_enum_payload_type.take();
                            builder.ins().return_(&[ret_val, payload]);
                        } else {
                            // Coerce return value to match declared return type
                            let ret_val = coerce_ret(&mut builder, ret_val, ret_type);
                            builder.ins().return_(&[ret_val]);
                        }
                    } else {
                        emit_owned_cleanup(&mut builder, &mut cx, None)?;
                        builder.ins().return_(&[]);
                    }
                    builder.finalize();
                }
                Err(e) => {
                    // Always finalize builder to reset builder_ctx, preventing
                    // "func_ctx.is_empty()" panics on subsequent function compilations.
                    // Emit trap to fill the current block (Cranelift requires all blocks
                    // to be terminated before finalize).
                    if !builder.is_unreachable() {
                        builder.ins().trap(
                            cranelift_codegen::ir::TrapCode::user(1).expect("trap code 1 is valid"),
                        );
                    }
                    builder.finalize();
                    self.module.clear_context(&mut self.ctx);
                    return Err(e);
                }
            }
        }

        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| {
                CodegenError::FunctionError(format!("Compilation error in '{}': {e}", fndef.name))
            })?;

        self.module.clear_context(&mut self.ctx);
        Ok(())
    }

    /// Returns a function pointer for a compiled function.
    ///
    /// # Safety
    ///
    /// The caller must ensure the function signature matches the cast type.
    pub fn get_fn_ptr(&self, name: &str) -> Result<*const u8, CodegenError> {
        let func_id = self
            .functions
            .get(name)
            .ok_or_else(|| CodegenError::UndefinedFunction(name.to_string()))?;
        Ok(self.module.get_finalized_function(*func_id))
    }

    /// Returns the collected global assembly sections.
    pub fn global_asm_sections(&self) -> &[String] {
        &self.global_asm_sections
    }

    /// Declares bare-metal HAL builtins for JIT mode (simulation via runtime_bare.rs).
    /// These are resolved at JIT link time to the hosted simulation functions.
    fn declare_bare_metal_jit_builtins(&mut self) -> Result<(), CodegenError> {
        let call_conv = self.module.target_config().default_call_conv;

        // Reusable signatures
        let sig_void = cranelift_codegen::ir::Signature::new(call_conv);
        let sig_ret_i64 = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            s.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            s
        };
        let sig_i64_void = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            s.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            s
        };
        let sig_i64_ret_i64 = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            s.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            s.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            s
        };
        let sig_2i64_ret_i64 = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..2 {
                s.params.push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
            }
            s.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            s
        };
        let sig_3i64_ret_i64 = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..3 {
                s.params.push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
            }
            s.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            s
        };
        let sig_4i64_ret_i64 = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..4 {
                s.params.push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
            }
            s.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            s
        };
        let sig_2i64_void = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..2 {
                s.params.push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
            }
            s
        };
        let sig_3i64_void = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..3 {
                s.params.push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
            }
            s
        };

        // GPIO: 4-arg, 2-arg, 1-arg functions
        let hal_fns: Vec<(&str, &str, &cranelift_codegen::ir::Signature)> = vec![
            // GPIO
            ("fj_rt_bare_gpio_config", "gpio_config", &sig_4i64_ret_i64),
            (
                "fj_rt_bare_gpio_set_output",
                "gpio_set_output",
                &sig_i64_ret_i64,
            ),
            (
                "fj_rt_bare_gpio_set_input",
                "gpio_set_input",
                &sig_i64_ret_i64,
            ),
            ("fj_rt_bare_gpio_write", "gpio_write", &sig_2i64_ret_i64),
            ("fj_rt_bare_gpio_read", "gpio_read", &sig_i64_ret_i64),
            ("fj_rt_bare_gpio_toggle", "gpio_toggle", &sig_i64_ret_i64),
            (
                "fj_rt_bare_gpio_set_pull",
                "gpio_set_pull",
                &sig_2i64_ret_i64,
            ),
            ("fj_rt_bare_gpio_set_irq", "gpio_set_irq", &sig_2i64_ret_i64),
            // UART
            ("fj_rt_bare_uart_init", "uart_init", &sig_2i64_ret_i64),
            (
                "fj_rt_bare_uart_write_byte",
                "uart_write_byte",
                &sig_2i64_ret_i64,
            ),
            (
                "fj_rt_bare_uart_read_byte",
                "uart_read_byte",
                &sig_i64_ret_i64,
            ),
            (
                "fj_rt_bare_uart_available",
                "uart_available",
                &sig_i64_ret_i64,
            ),
            // SPI
            ("fj_rt_bare_spi_init", "spi_init", &sig_2i64_ret_i64),
            ("fj_rt_bare_spi_transfer", "spi_transfer", &sig_2i64_ret_i64),
            ("fj_rt_bare_spi_cs_set", "spi_cs_set", &sig_3i64_ret_i64),
            // I2C
            ("fj_rt_bare_i2c_init", "i2c_init", &sig_2i64_ret_i64),
            // Timer
            (
                "fj_rt_bare_timer_get_ticks",
                "timer_get_ticks",
                &sig_ret_i64,
            ),
            ("fj_rt_bare_timer_get_freq", "timer_get_freq", &sig_ret_i64),
            (
                "fj_rt_bare_time_since_boot",
                "time_since_boot",
                &sig_ret_i64,
            ),
            (
                "fj_rt_bare_timer_set_deadline",
                "timer_set_deadline",
                &sig_i64_void,
            ),
            ("fj_rt_bare_sleep_ms", "sleep_ms", &sig_i64_void),
            ("fj_rt_bare_sleep_us", "sleep_us", &sig_i64_void),
            (
                "fj_rt_bare_timer_enable_virtual",
                "timer_enable_virtual",
                &sig_void,
            ),
            (
                "fj_rt_bare_timer_disable_virtual",
                "timer_disable_virtual",
                &sig_void,
            ),
            ("fj_rt_bare_timer_mark_boot", "timer_mark_boot", &sig_void),
            // DMA
            ("fj_rt_bare_dma_alloc", "dma_alloc", &sig_i64_ret_i64),
            ("fj_rt_bare_dma_config", "dma_config", &sig_4i64_ret_i64),
            ("fj_rt_bare_dma_start", "dma_start", &sig_i64_ret_i64),
            ("fj_rt_bare_dma_wait", "dma_wait", &sig_i64_ret_i64),
            ("fj_rt_bare_dma_status", "dma_status", &sig_i64_ret_i64),
            ("fj_rt_bare_dma_barrier", "dma_barrier", &sig_void),
            // Phase 4: Storage
            ("fj_rt_bare_nvme_init", "nvme_init", &sig_ret_i64),
            ("fj_rt_bare_sd_init", "sd_init", &sig_ret_i64),
            ("fj_rt_bare_vfs_close", "vfs_close", &sig_i64_ret_i64),
            // Phase 5: Network
            ("fj_rt_bare_eth_init", "eth_init", &sig_ret_i64),
            ("fj_rt_bare_net_socket", "net_socket", &sig_i64_ret_i64),
            ("fj_rt_bare_net_bind", "net_bind", &sig_2i64_ret_i64),
            ("fj_rt_bare_net_listen", "net_listen", &sig_i64_ret_i64),
            ("fj_rt_bare_net_accept", "net_accept", &sig_i64_ret_i64),
            ("fj_rt_bare_net_close", "net_close", &sig_i64_ret_i64),
            ("fj_rt_bare_net_connect", "net_connect", &sig_3i64_ret_i64),
            // Phase 6: Display & Input
            ("fj_rt_bare_fb_init", "fb_init", &sig_2i64_ret_i64),
            ("fj_rt_bare_fb_width", "fb_width", &sig_ret_i64),
            ("fj_rt_bare_fb_height", "fb_height", &sig_ret_i64),
            (
                "fj_rt_bare_fb_write_pixel",
                "fb_write_pixel",
                &sig_3i64_ret_i64,
            ),
            // V27.5 P1.4: VESA framebuffer extensions
            ("fj_rt_bare_fb_set_base", "fb_set_base", &sig_i64_ret_i64),
            ("fj_rt_bare_fb_scroll", "fb_scroll", &sig_i64_ret_i64),
            ("fj_rt_bare_kb_init", "kb_init", &sig_ret_i64),
            ("fj_rt_bare_kb_read", "kb_read", &sig_ret_i64),
            ("fj_rt_bare_kb_available", "kb_available", &sig_ret_i64),
            // Phase 8: OS Services
            ("fj_rt_bare_proc_spawn", "proc_spawn", &sig_i64_ret_i64),
            ("fj_rt_bare_proc_wait", "proc_wait", &sig_i64_ret_i64),
            ("fj_rt_bare_proc_kill", "proc_kill", &sig_i64_ret_i64),
            ("fj_rt_bare_proc_self", "proc_self", &sig_ret_i64),
            ("fj_rt_bare_sys_cpu_temp", "sys_cpu_temp", &sig_ret_i64),
            ("fj_rt_bare_sys_ram_total", "sys_ram_total", &sig_ret_i64),
            ("fj_rt_bare_sys_ram_free", "sys_ram_free", &sig_ret_i64),
            ("fj_rt_bare_proc_yield", "proc_yield", &sig_void),
            ("fj_rt_bare_sys_poweroff", "sys_poweroff", &sig_void),
            ("fj_rt_bare_sys_reboot", "sys_reboot", &sig_void),
            // Context switch builtins (scheduler in IRQ handler)
            (
                "fj_rt_bare_sched_get_saved_sp",
                "sched_get_saved_sp",
                &sig_ret_i64,
            ),
            (
                "fj_rt_bare_sched_set_next_sp",
                "sched_set_next_sp",
                &sig_i64_void,
            ),
            (
                "fj_rt_bare_sched_read_proc",
                "sched_read_proc",
                &sig_i64_ret_i64,
            ),
            (
                "fj_rt_bare_sched_write_proc",
                "sched_write_proc",
                &sig_2i64_ret_i64,
            ),
            // Syscall builtins
            ("fj_rt_bare_syscall_arg0", "syscall_arg0", &sig_ret_i64),
            ("fj_rt_bare_syscall_arg1", "syscall_arg1", &sig_ret_i64),
            ("fj_rt_bare_syscall_arg2", "syscall_arg2", &sig_ret_i64),
            (
                "fj_rt_bare_syscall_set_return",
                "syscall_set_return",
                &sig_i64_void,
            ),
            // svc(num, arg1, arg2) -> result
            ("fj_rt_bare_svc", "svc", &sig_3i64_ret_i64),
            // MMU builtins
            ("fj_rt_bare_switch_ttbr0", "switch_ttbr0", &sig_i64_void),
            ("fj_rt_bare_read_ttbr0", "read_ttbr0", &sig_ret_i64),
            ("fj_rt_bare_tlbi_va", "tlbi_va", &sig_i64_void),
            // x86_64 port I/O builtins
            ("fj_rt_bare_port_outb", "port_outb", &sig_2i64_ret_i64),
            ("fj_rt_bare_port_inb", "port_inb", &sig_i64_ret_i64),
            (
                "fj_rt_bare_x86_serial_init",
                "x86_serial_init",
                &sig_2i64_ret_i64,
            ),
            (
                "fj_rt_bare_set_uart_mode_x86",
                "set_uart_mode_x86",
                &sig_i64_void,
            ),
            // x86_64 CPUID + SSE builtins
            ("fj_rt_bare_cpuid_eax", "cpuid_eax", &sig_i64_ret_i64),
            ("fj_rt_bare_cpuid_ebx", "cpuid_ebx", &sig_i64_ret_i64),
            ("fj_rt_bare_cpuid_ecx", "cpuid_ecx", &sig_i64_ret_i64),
            ("fj_rt_bare_cpuid_edx", "cpuid_edx", &sig_i64_ret_i64),
            ("fj_rt_bare_sse_enable", "sse_enable", &sig_void),
            ("fj_rt_bare_read_cr0", "read_cr0", &sig_ret_i64),
            ("fj_rt_bare_read_cr3", "read_cr3", &sig_ret_i64),
            ("fj_rt_bare_write_cr3", "write_cr3", &sig_i64_void),
            ("fj_rt_bare_read_cr2", "read_cr2", &sig_ret_i64),
            ("fj_rt_bare_read_cr4", "read_cr4", &sig_ret_i64),
            // x86_64 IDT + PIC + PIT builtins
            ("fj_rt_bare_idt_init", "idt_init", &sig_void),
            ("fj_rt_bare_pic_remap", "pic_remap", &sig_void),
            ("fj_rt_bare_pic_eoi", "pic_eoi", &sig_i64_void),
            ("fj_rt_bare_pit_init", "pit_init", &sig_i64_void),
            (
                "fj_rt_bare_read_timer_ticks",
                "read_timer_ticks",
                &sig_ret_i64,
            ),
            // String byte access
            ("fj_rt_str_byte_at", "str_byte_at", &sig_2i64_ret_i64),
            ("fj_rt_str_len", "str_len", &sig_i64_ret_i64),
            // Process scheduler (Phase 4)
            (
                "fj_rt_bare_proc_table_addr",
                "proc_table_addr",
                &sig_ret_i64,
            ),
            (
                "fj_rt_bare_get_current_pid",
                "get_current_pid",
                &sig_ret_i64,
            ),
            (
                "fj_rt_bare_set_current_pid",
                "set_current_pid",
                &sig_i64_void,
            ),
            ("fj_rt_bare_get_proc_count", "get_proc_count", &sig_ret_i64),
            ("fj_rt_bare_proc_create", "proc_create", &sig_i64_ret_i64),
            ("fj_rt_bare_yield", "yield_proc", &sig_void),
            // Phase 5: Ring 3 + SYSCALL
            ("fj_rt_bare_tss_init", "tss_init", &sig_void),
            ("fj_rt_bare_syscall_init", "syscall_init", &sig_void),
            (
                "fj_rt_bare_proc_create_user",
                "proc_create_user",
                &sig_i64_ret_i64,
            ),
            // Phase 6: Keyboard + PCI
            (
                "fj_rt_bare_kb_read_scancode",
                "kb_read_scancode",
                &sig_ret_i64,
            ),
            ("fj_rt_bare_kb_has_data", "kb_has_data", &sig_ret_i64),
            ("fj_rt_bare_pci_read32", "pci_read32", &sig_4i64_ret_i64),
            // Phase 8: ACPI + SMP
            ("fj_rt_bare_acpi_shutdown", "acpi_shutdown", &sig_void),
            ("fj_rt_bare_acpi_find_rsdp", "acpi_find_rsdp", &sig_ret_i64),
            (
                "fj_rt_bare_acpi_get_cpu_count",
                "acpi_get_cpu_count",
                &sig_i64_ret_i64,
            ),
            ("fj_rt_bare_rdtsc", "rdtsc", &sig_ret_i64),
            ("fj_rt_bare_rdrand", "rdrand", &sig_ret_i64),
            // Phase 5+8: MSR, CR4, INVLPG, FPU, iretq
            (
                "fj_rt_bare_iretq_to_user",
                "iretq_to_user",
                &sig_3i64_ret_i64,
            ),
            ("fj_rt_bare_read_msr", "read_msr", &sig_i64_ret_i64),
            ("fj_rt_bare_write_msr", "write_msr", &sig_2i64_ret_i64),
            ("fj_rt_bare_read_cr3", "read_cr3", &sig_ret_i64),
            ("fj_rt_bare_write_cr3", "write_cr3", &sig_i64_void),
            ("fj_rt_bare_read_cr2", "read_cr2", &sig_ret_i64),
            ("fj_rt_bare_read_cr4", "read_cr4", &sig_ret_i64),
            ("fj_rt_bare_write_cr4", "write_cr4", &sig_i64_void),
            ("fj_rt_bare_invlpg", "invlpg", &sig_i64_void),
            ("fj_rt_bare_fxsave", "fxsave", &sig_i64_void),
            ("fj_rt_bare_fxrstor", "fxrstor", &sig_i64_void),
            // FajarOS Nova v0.2: system builtins
            ("fj_rt_bare_hlt", "hlt", &sig_void),
            ("fj_rt_bare_cli", "cli", &sig_void),
            ("fj_rt_bare_sti", "sti", &sig_void),
            ("fj_rt_bare_cpuid", "cpuid", &sig_2i64_ret_i64),
            // rdmsr/wrmsr aliases (map to same runtime as read_msr/write_msr)
            ("fj_rt_bare_read_msr", "rdmsr", &sig_i64_ret_i64),
            ("fj_rt_bare_write_msr", "wrmsr", &sig_2i64_ret_i64),
            // FajarOS Nova v0.3 Stage A: Extended Port I/O
            ("fj_rt_bare_port_inw", "port_inw", &sig_i64_ret_i64),
            ("fj_rt_bare_port_ind", "port_ind", &sig_i64_ret_i64),
            ("fj_rt_bare_port_outw", "port_outw", &sig_2i64_void),
            ("fj_rt_bare_port_outd", "port_outd", &sig_2i64_void),
            // FajarOS Nova v0.3 Stage A: CPU Control
            ("fj_rt_bare_ltr", "ltr", &sig_i64_void),
            ("fj_rt_bare_lgdt_mem", "lgdt_mem", &sig_i64_void),
            ("fj_rt_bare_lidt_mem", "lidt_mem", &sig_i64_void),
            ("fj_rt_bare_swapgs", "swapgs", &sig_void),
            ("fj_rt_bare_int_n", "int_n", &sig_i64_void),
            ("fj_rt_bare_pause", "pause", &sig_void),
            ("fj_rt_bare_stac", "stac", &sig_void),
            ("fj_rt_bare_clac", "clac", &sig_void),
            // FajarOS Nova v0.3 Stage A: Buffer Operations
            ("fj_rt_bare_memcmp_buf", "memcmp_buf", &sig_3i64_ret_i64),
            ("fj_rt_bare_memcpy_buf", "memcpy_buf", &sig_3i64_ret_i64),
            ("fj_rt_bare_memset_buf", "memset_buf", &sig_3i64_void),
        ];

        // fb_fill_rect(x, y, w, h, color) -> i64 — 5-arg function
        let sig_5i64_ret_i64 = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..5 {
                s.params.push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
            }
            s.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            s
        };
        // pci_write32(bus, dev, fn, offset, val) -> void — 5-arg function
        let sig_5i64_void = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..5 {
                s.params.push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
            }
            s
        };
        {
            let id = self
                .module
                .declare_function(
                    "fj_rt_bare_fb_fill_rect",
                    Linkage::Import,
                    &sig_5i64_ret_i64,
                )
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("fb_fill_rect".to_string(), id);
        }
        {
            let id = self
                .module
                .declare_function("fj_rt_bare_pci_write32", Linkage::Import, &sig_5i64_void)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("pci_write32".to_string(), id);
        }

        for (extern_name, builtin_name, sig) in hal_fns {
            let id = self
                .module
                .declare_function(extern_name, Linkage::Import, sig)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin_name.to_string(), id);
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AOT (Ahead-Of-Time) Object Compiler
// ═══════════════════════════════════════════════════════════════════════

/// AOT compiler that produces object files for linking into native binaries.
pub struct ObjectCompiler {
    /// The object module for AOT compilation.
    module: ObjectModule,
    /// Cranelift codegen context (reused per function).
    ctx: Context,
    /// Function builder context (reused per function).
    builder_ctx: FunctionBuilderContext,
    /// Map of function names to their Cranelift FuncIds.
    functions: HashMap<String, FuncId>,
    /// Map of string literal contents to their DataIds in the data section.
    string_data: HashMap<String, DataId>,
    /// Generic function definitions (stored for monomorphization).
    generic_fns: HashMap<String, FnDef>,
    /// Generic fn param mapping: fn_name → Vec of (param_index, generic_param_name).
    generic_fn_params: HashMap<String, Vec<(usize, String)>>,
    /// Monomorphization map: generic fn name → mangled specialized name.
    mono_map: HashMap<String, String>,
    /// Tracks the return type of each function for type-aware dispatch.
    fn_return_types: HashMap<String, cranelift_codegen::ir::Type>,
    /// Enum definitions: enum name → list of variant names (index = tag).
    enum_defs: HashMap<String, Vec<String>>,
    /// Enum variant payload types: (enum_name, variant_name) → list of Cranelift types.
    enum_variant_types: HashMap<(String, String), Vec<cranelift_codegen::ir::Type>>,
    /// Generic enum definitions: enum_name → list of generic param names.
    generic_enum_defs: HashMap<String, Vec<String>>,
    /// Struct definitions: struct name → ordered list of (field_name, clif_type).
    struct_defs: HashMap<String, Vec<(String, cranelift_codegen::ir::Type)>>,
    /// Set of type names that are unions (all fields at offset 0).
    union_names: HashSet<String>,
    /// Bitfield layouts: struct_name → vec of (field_name, bit_offset, bit_width).
    bitfield_layouts: HashMap<String, Vec<(String, u8, u8)>>,
    /// Impl methods: (type_name, method_name) → mangled function name.
    impl_methods: HashMap<(String, String), String>,
    /// Trait definitions: trait name → list of required method names.
    trait_defs: HashMap<String, Vec<String>>,
    /// Trait impls: (trait_name, type_name) → list of method names implemented.
    trait_impls: HashMap<(String, String), Vec<String>>,
    /// Top-level const definitions: (name, value expr, type).
    const_defs: Vec<(String, Expr, TypeExpr)>,
    /// Const fn definitions: fn_name → FnDef (for compile-time evaluation).
    const_fn_defs: HashMap<String, FnDef>,
    /// Functions that return fixed-size arrays: fn_name → (array_len, elem_type).
    fn_array_returns: HashMap<String, (usize, cranelift_codegen::ir::Type)>,
    /// Functions that return strings (two return values: ptr, len).
    fn_returns_string: HashSet<String>,
    /// Functions that return a heap-allocated dynamic array (Slice type like `[i64]`).
    fn_returns_heap_array: HashSet<String>,
    /// Functions that return a closure handle (closure with captures).
    fn_returns_closure_handle: HashSet<String>,
    /// Functions that return a struct type: fn_name → struct_name.
    fn_returns_struct: HashMap<String, String>,
    /// Functions that return an enum type (two return values: tag, payload).
    fn_returns_enum: HashSet<String>,
    /// Maps closure variable names to their generated function names.
    closure_fn_map: HashMap<String, String>,
    /// Maps closure function names to their list of captured variable names.
    closure_captures: HashMap<String, Vec<String>>,
    /// Maps closure source span → function name for inline closure lookup.
    closure_span_to_fn: HashMap<(usize, usize), String>,
    /// Maps mangled function name → module prefix for intra-module resolution.
    module_fns: HashMap<String, String>,
    /// When true, disables standard library (IO, heap) runtime declarations.
    no_std: bool,
    /// When true, generates user-mode (Ring 3) ELF with SYSCALL-based runtime.
    user_mode: bool,
    /// User-defined panic handler function name.
    panic_handler_fn: Option<String>,
    /// Set of async function names (their return is wrapped in a future handle).
    async_fns: HashSet<String>,
    /// Global assembly sections collected from `global_asm!()` items.
    global_asm_sections: Vec<String>,
    /// Function section annotations: fn_name → section name (from @section("name")).
    fn_sections: HashMap<String, String>,
    /// Data section annotations: const name → section name (from @section on ConstDef).
    data_sections: HashMap<String, String>,
    /// Global data objects for section-annotated consts: name → DataId.
    global_data: HashMap<String, DataId>,
    /// When true, emit debug information (source locations tracked per function).
    debug_info: bool,
    /// Source file name for debug info.
    source_file: Option<String>,
    /// Per-function source locations: fn_name → (start_line, end_line).
    fn_source_locations: HashMap<String, (u32, u32)>,
    /// Functions annotated with `@interrupt` — need assembly wrapper with
    /// register save/restore and `eret` instead of `ret`.
    interrupt_fns: Vec<String>,
    /// Security configuration for codegen hardening.
    security_enabled: bool,
    /// Run security linter before compilation.
    lint_on_compile: bool,
    /// Dead functions to skip during codegen (set by optimizer).
    dead_functions: HashSet<String>,
    /// When true, inject profiling instrumentation into compiled functions.
    profiling_enabled: bool,
}

impl ObjectCompiler {
    /// Creates a new AOT compiler for the host target.
    pub fn new(name: &str) -> Result<Self, CodegenError> {
        let mut flag_builder = settings::builder();
        flag_builder
            .set("is_pic", "true")
            .map_err(|e| CodegenError::Internal(e.to_string()))?;

        let isa_builder = cranelift_native::builder()
            .map_err(|e| CodegenError::Internal(format!("host ISA: {e}")))?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e: cranelift_codegen::CodegenError| CodegenError::Internal(e.to_string()))?;

        let mut obj_builder =
            ObjectBuilder::new(isa, name, cranelift_module::default_libcall_names())
                .map_err(|e| CodegenError::ModuleError(e.to_string()))?;
        // Enable per-function sections so linker scripts can control ordering
        obj_builder.per_function_section(true);
        let module = ObjectModule::new(obj_builder);
        let ctx = module.make_context();

        Ok(ObjectCompiler {
            module,
            ctx,
            builder_ctx: FunctionBuilderContext::new(),
            functions: HashMap::new(),
            string_data: HashMap::new(),
            generic_fns: HashMap::new(),
            generic_fn_params: HashMap::new(),
            mono_map: HashMap::new(),
            fn_return_types: HashMap::new(),
            enum_defs: HashMap::new(),
            enum_variant_types: HashMap::new(),
            generic_enum_defs: HashMap::new(),
            struct_defs: HashMap::new(),
            union_names: HashSet::new(),
            bitfield_layouts: HashMap::new(),
            impl_methods: HashMap::new(),
            trait_defs: HashMap::new(),
            trait_impls: HashMap::new(),
            const_defs: Vec::new(),
            const_fn_defs: HashMap::new(),
            fn_array_returns: HashMap::new(),
            fn_returns_string: HashSet::new(),
            fn_returns_heap_array: HashSet::new(),
            fn_returns_closure_handle: HashSet::new(),
            fn_returns_struct: HashMap::new(),
            fn_returns_enum: HashSet::new(),
            closure_fn_map: HashMap::new(),
            closure_captures: HashMap::new(),
            closure_span_to_fn: HashMap::new(),
            module_fns: HashMap::new(),
            no_std: false,
            user_mode: false,
            panic_handler_fn: None,
            async_fns: HashSet::new(),
            global_asm_sections: Vec::new(),
            fn_sections: HashMap::new(),
            data_sections: HashMap::new(),
            global_data: HashMap::new(),
            debug_info: false,
            source_file: None,
            fn_source_locations: HashMap::new(),
            interrupt_fns: Vec::new(),
            security_enabled: false,
            lint_on_compile: false,
            dead_functions: HashSet::new(),
            profiling_enabled: false,
        })
    }

    /// Creates a new AOT compiler for a specific target (cross-compilation).
    pub fn new_with_target(
        name: &str,
        target: &super::target::TargetConfig,
    ) -> Result<Self, CodegenError> {
        let isa = target.cranelift_isa()?;

        let mut obj_builder =
            ObjectBuilder::new(isa, name, cranelift_module::default_libcall_names())
                .map_err(|e| CodegenError::ModuleError(e.to_string()))?;
        // Enable per-function sections so linker scripts can control ordering
        obj_builder.per_function_section(true);
        let module = ObjectModule::new(obj_builder);
        let ctx = module.make_context();

        Ok(ObjectCompiler {
            module,
            ctx,
            builder_ctx: FunctionBuilderContext::new(),
            functions: HashMap::new(),
            string_data: HashMap::new(),
            generic_fns: HashMap::new(),
            generic_fn_params: HashMap::new(),
            mono_map: HashMap::new(),
            fn_return_types: HashMap::new(),
            enum_defs: HashMap::new(),
            enum_variant_types: HashMap::new(),
            generic_enum_defs: HashMap::new(),
            struct_defs: HashMap::new(),
            union_names: HashSet::new(),
            bitfield_layouts: HashMap::new(),
            impl_methods: HashMap::new(),
            trait_defs: HashMap::new(),
            trait_impls: HashMap::new(),
            const_defs: Vec::new(),
            const_fn_defs: HashMap::new(),
            fn_array_returns: HashMap::new(),
            fn_returns_string: HashSet::new(),
            fn_returns_heap_array: HashSet::new(),
            fn_returns_closure_handle: HashSet::new(),
            fn_returns_struct: HashMap::new(),
            fn_returns_enum: HashSet::new(),
            closure_fn_map: HashMap::new(),
            closure_captures: HashMap::new(),
            closure_span_to_fn: HashMap::new(),
            module_fns: HashMap::new(),
            no_std: false,
            user_mode: false,
            panic_handler_fn: None,
            async_fns: HashSet::new(),
            global_asm_sections: Vec::new(),
            fn_sections: HashMap::new(),
            data_sections: HashMap::new(),
            global_data: HashMap::new(),
            debug_info: false,
            source_file: None,
            fn_source_locations: HashMap::new(),
            interrupt_fns: Vec::new(),
            security_enabled: false,
            lint_on_compile: false,
            dead_functions: HashSet::new(),
            profiling_enabled: false,
        })
    }

    /// Declares bare-metal runtime functions (no libc, no heap).
    /// Only memcpy/memset/memcmp + optional UART print.
    fn declare_bare_metal_runtime(&mut self) -> Result<(), CodegenError> {
        let call_conv = self.module.target_config().default_call_conv;

        // fj_rt_bare_memcpy(dst: ptr, src: ptr, n: i64) -> ptr
        let mut sig_memcpy = cranelift_codegen::ir::Signature::new(call_conv);
        sig_memcpy.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::pointer_type(),
        ));
        sig_memcpy.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::pointer_type(),
        ));
        sig_memcpy.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        sig_memcpy
            .returns
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::pointer_type(),
            ));
        let memcpy_id = self
            .module
            .declare_function("fj_rt_bare_memcpy", Linkage::Import, &sig_memcpy)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("__memcpy".to_string(), memcpy_id);

        // fj_rt_bare_memset(dst: ptr, val: i64, n: i64) -> ptr
        let mut sig_memset = cranelift_codegen::ir::Signature::new(call_conv);
        sig_memset.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::pointer_type(),
        ));
        sig_memset.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        sig_memset.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        sig_memset
            .returns
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::pointer_type(),
            ));
        let memset_id = self
            .module
            .declare_function("fj_rt_bare_memset", Linkage::Import, &sig_memset)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("__memset".to_string(), memset_id);

        // fj_rt_bare_print(ptr: ptr, len: i64) -> void (UART output)
        let mut sig_print = cranelift_codegen::ir::Signature::new(call_conv);
        sig_print.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::pointer_type(),
        ));
        sig_print.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        let print_id = self
            .module
            .declare_function("fj_rt_bare_print", Linkage::Import, &sig_print)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("__print_str".to_string(), print_id);
        // println → fj_rt_bare_println (appends newline)
        let println_id = self
            .module
            .declare_function("fj_rt_bare_println", Linkage::Import, &sig_print)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("__println_str".to_string(), println_id);
        // Also register as println/print for compatibility
        let print_id2 = self
            .module
            .declare_function("fj_rt_bare_print_i64", Linkage::Import, &{
                let mut s = cranelift_codegen::ir::Signature::new(call_conv);
                s.params.push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
                s
            })
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("println".to_string(), print_id2);
        self.functions.insert("print".to_string(), print_id2);

        // fj_rt_bare_halt() -> void (halt CPU)
        let sig_halt = cranelift_codegen::ir::Signature::new(call_conv);
        let halt_id = self
            .module
            .declare_function("fj_rt_bare_halt", Linkage::Import, &sig_halt)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("__halt".to_string(), halt_id);

        // IRQ enable/disable (uses DAIFClr/DAIFSet which need special MSR encoding)
        let sig_void = cranelift_codegen::ir::Signature::new(call_conv);
        let irq_en_id = self
            .module
            .declare_function("fj_rt_bare_irq_enable", Linkage::Import, &sig_void)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("irq_enable".to_string(), irq_en_id);
        let sig_void2 = cranelift_codegen::ir::Signature::new(call_conv);
        let irq_dis_id = self
            .module
            .declare_function("fj_rt_bare_irq_disable", Linkage::Import, &sig_void2)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("irq_disable".to_string(), irq_dis_id);

        // System register stubs (workaround for asm! out() codegen bug)
        // These are implemented in startup assembly and actually execute mrs/msr.

        // () -> i64 return signatures
        let sig_ret_i64 = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            s.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            s
        };
        // (i64) -> void signatures
        let sig_i64_void = {
            let mut s = cranelift_codegen::ir::Signature::new(call_conv);
            s.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            s
        };

        // Timer builtins
        for (name, builtin) in [
            ("fj_rt_bare_timer_count", "timer_count"),
            ("fj_rt_bare_timer_freq", "timer_freq"),
            ("fj_rt_bare_timer_status", "timer_status"),
            ("fj_rt_bare_read_el", "read_el"),
            ("fj_rt_bare_read_midr", "read_midr"),
            ("fj_rt_bare_gic_ack", "gic_ack"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        for (name, builtin) in [
            ("fj_rt_bare_timer_set", "timer_set"),
            ("fj_rt_bare_timer_disable", "timer_disable"),
            ("fj_rt_bare_gic_eoi", "gic_eoi"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_i64_void)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }

        // gic_cpu_init(pmr: i64) -> void
        let gic_cpu_id = self
            .module
            .declare_function("fj_rt_bare_gic_cpu_init", Linkage::Import, &sig_i64_void)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("gic_cpu_init".to_string(), gic_cpu_id);

        // mmu_enable(mair: i64, tcr: i64, ttbr0: i64) -> void
        let mut sig_mmu = cranelift_codegen::ir::Signature::new(call_conv);
        sig_mmu.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        sig_mmu.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        sig_mmu.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        let mmu_id = self
            .module
            .declare_function("fj_rt_bare_mmu_enable", Linkage::Import, &sig_mmu)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("mmu_enable".to_string(), mmu_id);

        // eret_to_el0(entry: i64, sp_el0: i64, spsr: i64) -> void
        let eret_id = self
            .module
            .declare_function("fj_rt_bare_eret_to_el0", Linkage::Import, &sig_mmu)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("eret_to_el0".to_string(), eret_id);

        // ── Phase 3 HAL Driver Runtime Functions ──

        // GPIO: (i64, ...) -> i64 signatures
        // gpio_config(pin, func, output, pull) -> i64
        let mut sig_gpio4 = cranelift_codegen::ir::Signature::new(call_conv);
        for _ in 0..4 {
            sig_gpio4.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        }
        sig_gpio4.returns.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        let gpio_config_id = self
            .module
            .declare_function("fj_rt_bare_gpio_config", Linkage::Import, &sig_gpio4)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("gpio_config".to_string(), gpio_config_id);

        // (i64) -> i64 GPIO functions
        let mut sig_i64_ret_i64 = cranelift_codegen::ir::Signature::new(call_conv);
        sig_i64_ret_i64
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        sig_i64_ret_i64
            .returns
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        for (name, builtin) in [
            ("fj_rt_bare_gpio_set_output", "gpio_set_output"),
            ("fj_rt_bare_gpio_set_input", "gpio_set_input"),
            ("fj_rt_bare_gpio_read", "gpio_read"),
            ("fj_rt_bare_gpio_toggle", "gpio_toggle"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_i64_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }

        // (i64, i64) -> i64 GPIO functions
        let mut sig_2i64_ret_i64 = cranelift_codegen::ir::Signature::new(call_conv);
        sig_2i64_ret_i64
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        sig_2i64_ret_i64
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        sig_2i64_ret_i64
            .returns
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        for (name, builtin) in [
            ("fj_rt_bare_gpio_write", "gpio_write"),
            ("fj_rt_bare_gpio_set_pull", "gpio_set_pull"),
            ("fj_rt_bare_gpio_set_irq", "gpio_set_irq"),
            ("fj_rt_bare_uart_init", "uart_init"),
            ("fj_rt_bare_uart_write_byte", "uart_write_byte"),
            ("fj_rt_bare_spi_init", "spi_init"),
            ("fj_rt_bare_i2c_init", "i2c_init"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_2i64_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }

        // uart_read_byte(port) -> i64
        let uart_rb_id = self
            .module
            .declare_function(
                "fj_rt_bare_uart_read_byte",
                Linkage::Import,
                &sig_i64_ret_i64,
            )
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("uart_read_byte".to_string(), uart_rb_id);

        // uart_available(port) -> i64
        let uart_av_id = self
            .module
            .declare_function(
                "fj_rt_bare_uart_available",
                Linkage::Import,
                &sig_i64_ret_i64,
            )
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("uart_available".to_string(), uart_av_id);

        // uart_write_buf(port, ptr, len) -> i64
        let mut sig_uart_buf = cranelift_codegen::ir::Signature::new(call_conv);
        sig_uart_buf
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        sig_uart_buf
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::pointer_type(),
            ));
        sig_uart_buf
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        sig_uart_buf
            .returns
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        let uart_wb_id = self
            .module
            .declare_function("fj_rt_bare_uart_write_buf", Linkage::Import, &sig_uart_buf)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("uart_write_buf".to_string(), uart_wb_id);

        // uart_read_buf(port, ptr, max_len) -> i64
        let mut sig_uart_rbuf = cranelift_codegen::ir::Signature::new(call_conv);
        sig_uart_rbuf
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        sig_uart_rbuf
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::pointer_type(),
            ));
        sig_uart_rbuf
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        sig_uart_rbuf
            .returns
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        let uart_rb2_id = self
            .module
            .declare_function("fj_rt_bare_uart_read_buf", Linkage::Import, &sig_uart_rbuf)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("uart_read_buf".to_string(), uart_rb2_id);

        // uart_set_base(port, addr) -> void
        let mut sig_2i64_void = cranelift_codegen::ir::Signature::new(call_conv);
        sig_2i64_void
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        sig_2i64_void
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        let uart_sb_id = self
            .module
            .declare_function("fj_rt_bare_uart_set_base", Linkage::Import, &sig_2i64_void)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("uart_set_base".to_string(), uart_sb_id);

        // SPI: spi_transfer(bus, tx_byte) -> i64
        let spi_xfer_id = self
            .module
            .declare_function(
                "fj_rt_bare_spi_transfer",
                Linkage::Import,
                &sig_2i64_ret_i64,
            )
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("spi_transfer".to_string(), spi_xfer_id);

        // spi_cs_set(bus, cs, active) -> i64
        let mut sig_3i64_ret_i64 = cranelift_codegen::ir::Signature::new(call_conv);
        for _ in 0..3 {
            sig_3i64_ret_i64
                .params
                .push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
        }
        sig_3i64_ret_i64
            .returns
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        let spi_cs_id = self
            .module
            .declare_function("fj_rt_bare_spi_cs_set", Linkage::Import, &sig_3i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("spi_cs_set".to_string(), spi_cs_id);

        // I2C: i2c_write(bus, addr, ptr, len) -> i64
        let mut sig_i2c_w = cranelift_codegen::ir::Signature::new(call_conv);
        sig_i2c_w.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        sig_i2c_w.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        sig_i2c_w.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::pointer_type(),
        ));
        sig_i2c_w.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        sig_i2c_w.returns.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        for (name, builtin) in [
            ("fj_rt_bare_i2c_write", "i2c_write"),
            ("fj_rt_bare_i2c_read", "i2c_read"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_i2c_w)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }

        // i2c_write_read(bus, addr, tx, tx_len, rx, rx_len) -> i64
        let mut sig_i2c_wr = cranelift_codegen::ir::Signature::new(call_conv);
        for t in [
            clif_types::default_int_type(),
            clif_types::default_int_type(),
            clif_types::pointer_type(),
            clif_types::default_int_type(),
            clif_types::pointer_type(),
            clif_types::default_int_type(),
        ] {
            sig_i2c_wr
                .params
                .push(cranelift_codegen::ir::AbiParam::new(t));
        }
        sig_i2c_wr
            .returns
            .push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        let i2c_wr_id = self
            .module
            .declare_function("fj_rt_bare_i2c_write_read", Linkage::Import, &sig_i2c_wr)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("i2c_write_read".to_string(), i2c_wr_id);

        // Timer enhanced: () -> i64 return signatures
        for (name, builtin) in [
            ("fj_rt_bare_timer_get_ticks", "timer_get_ticks"),
            ("fj_rt_bare_timer_get_freq", "timer_get_freq"),
            ("fj_rt_bare_time_since_boot", "time_since_boot"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        // Timer: (i64) -> void
        for (name, builtin) in [
            ("fj_rt_bare_timer_set_deadline", "timer_set_deadline"),
            ("fj_rt_bare_sleep_ms", "sleep_ms"),
            ("fj_rt_bare_sleep_us", "sleep_us"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_i64_void)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        // Timer: () -> void
        let sig_void_void = cranelift_codegen::ir::Signature::new(call_conv);
        for (name, builtin) in [
            ("fj_rt_bare_timer_enable_virtual", "timer_enable_virtual"),
            ("fj_rt_bare_timer_disable_virtual", "timer_disable_virtual"),
            ("fj_rt_bare_timer_mark_boot", "timer_mark_boot"),
            ("fj_rt_bare_dma_barrier", "dma_barrier"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_void_void)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }

        // DMA: dma_alloc(size) -> u64
        let dma_alloc_id = self
            .module
            .declare_function("fj_rt_bare_dma_alloc", Linkage::Import, &sig_i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("dma_alloc".to_string(), dma_alloc_id);

        // dma_config(channel, src, dst, len) -> i64
        let dma_cfg_id = self
            .module
            .declare_function(
                "fj_rt_bare_dma_config",
                Linkage::Import,
                &sig_gpio4, // reuse 4-arg signature
            )
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("dma_config".to_string(), dma_cfg_id);

        // dma_start(channel) -> i64, dma_wait(channel) -> i64, dma_status(channel) -> i64
        for (name, builtin) in [
            ("fj_rt_bare_dma_start", "dma_start"),
            ("fj_rt_bare_dma_wait", "dma_wait"),
            ("fj_rt_bare_dma_status", "dma_status"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_i64_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }

        // dma_free(ptr, size) -> void
        let dma_free_id = self
            .module
            .declare_function("fj_rt_bare_dma_free", Linkage::Import, &sig_2i64_void)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("dma_free".to_string(), dma_free_id);

        // Phase 4: Storage — () -> i64
        for (name, builtin) in [
            ("fj_rt_bare_nvme_init", "nvme_init"),
            ("fj_rt_bare_sd_init", "sd_init"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        // nvme_read/write(lba, count, buf) -> i64, sd_read/write_block(lba, buf) -> i64
        for (name, builtin) in [
            ("fj_rt_bare_nvme_read", "nvme_read"),
            ("fj_rt_bare_nvme_write", "nvme_write"),
        ] {
            let mut sig = cranelift_codegen::ir::Signature::new(call_conv);
            sig.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::pointer_type(),
            ));
            sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        // VFS + Network: (i64) -> i64
        for (name, builtin) in [
            ("fj_rt_bare_vfs_close", "vfs_close"),
            ("fj_rt_bare_net_socket", "net_socket"),
            ("fj_rt_bare_net_listen", "net_listen"),
            ("fj_rt_bare_net_accept", "net_accept"),
            ("fj_rt_bare_net_close", "net_close"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_i64_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        // () -> i64
        {
            let id = self
                .module
                .declare_function("fj_rt_bare_eth_init", Linkage::Import, &sig_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("eth_init".to_string(), id);
        }
        // (i64, i64) -> i64
        {
            let id = self
                .module
                .declare_function("fj_rt_bare_net_bind", Linkage::Import, &sig_2i64_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("net_bind".to_string(), id);
        }

        // Context switch builtins (scheduler in IRQ handler)
        {
            let id = self
                .module
                .declare_function(
                    "fj_rt_bare_sched_get_saved_sp",
                    Linkage::Import,
                    &sig_ret_i64,
                )
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("sched_get_saved_sp".to_string(), id);
        }
        {
            let id = self
                .module
                .declare_function(
                    "fj_rt_bare_sched_set_next_sp",
                    Linkage::Import,
                    &sig_i64_void,
                )
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("sched_set_next_sp".to_string(), id);
        }
        {
            let id = self
                .module
                .declare_function(
                    "fj_rt_bare_sched_read_proc",
                    Linkage::Import,
                    &sig_i64_ret_i64,
                )
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("sched_read_proc".to_string(), id);
        }
        {
            let mut sig_2i = cranelift_codegen::ir::Signature::new(call_conv);
            sig_2i.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig_2i.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            let id = self
                .module
                .declare_function("fj_rt_bare_sched_write_proc", Linkage::Import, &sig_2i)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("sched_write_proc".to_string(), id);
        }

        // Syscall builtins
        for (name, builtin) in [
            ("fj_rt_bare_syscall_arg0", "syscall_arg0"),
            ("fj_rt_bare_syscall_arg1", "syscall_arg1"),
            ("fj_rt_bare_syscall_arg2", "syscall_arg2"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        {
            let id = self
                .module
                .declare_function(
                    "fj_rt_bare_syscall_set_return",
                    Linkage::Import,
                    &sig_i64_void,
                )
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("syscall_set_return".to_string(), id);
        }
        // svc(num, arg1, arg2) -> i64
        {
            let mut sig_svc = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..3 {
                sig_svc.params.push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
            }
            sig_svc.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            let id = self
                .module
                .declare_function("fj_rt_bare_svc", Linkage::Import, &sig_svc)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("svc".to_string(), id);
        }

        // MMU builtins
        {
            let id = self
                .module
                .declare_function("fj_rt_bare_switch_ttbr0", Linkage::Import, &sig_i64_void)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("switch_ttbr0".to_string(), id);
        }
        {
            let id = self
                .module
                .declare_function("fj_rt_bare_read_ttbr0", Linkage::Import, &sig_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("read_ttbr0".to_string(), id);
        }
        {
            let id = self
                .module
                .declare_function("fj_rt_bare_tlbi_va", Linkage::Import, &sig_i64_void)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("tlbi_va".to_string(), id);
        }

        // Volatile I/O (essential for bare-metal MMIO)
        // volatile_write(addr: i64, value: i64) -> void
        let mut sig_vw = cranelift_codegen::ir::Signature::new(call_conv);
        sig_vw.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        sig_vw.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        let vw_id = self
            .module
            .declare_function("fj_rt_volatile_write", Linkage::Import, &sig_vw)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("__volatile_write".to_string(), vw_id);

        // volatile_read(addr: i64) -> i64
        let mut sig_vr = cranelift_codegen::ir::Signature::new(call_conv);
        sig_vr.params.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        sig_vr.returns.push(cranelift_codegen::ir::AbiParam::new(
            clif_types::default_int_type(),
        ));
        let vr_id = self
            .module
            .declare_function("fj_rt_volatile_read", Linkage::Import, &sig_vr)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("__volatile_read".to_string(), vr_id);

        // volatile_write_u8/u16/u32/u64 + volatile_read_u8/u16/u32/u64
        for suffix in ["_u8", "_u16", "_u32", "_u64"] {
            let write_name = format!("fj_rt_volatile_write{suffix}");
            let read_name = format!("fj_rt_volatile_read{suffix}");
            let wid = self
                .module
                .declare_function(&write_name, Linkage::Import, &sig_vw)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions
                .insert(format!("__volatile_write{suffix}"), wid);
            let rid = self
                .module
                .declare_function(&read_name, Linkage::Import, &sig_vr)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions
                .insert(format!("__volatile_read{suffix}"), rid);
        }

        // ── x86_64 Port I/O Builtins (FajarOS Nova) ──

        // port_outb(port, value) -> i64
        let port_outb_id = self
            .module
            .declare_function("fj_rt_bare_port_outb", Linkage::Import, &sig_2i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("port_outb".to_string(), port_outb_id);

        // port_inb(port) -> i64
        let port_inb_id = self
            .module
            .declare_function("fj_rt_bare_port_inb", Linkage::Import, &sig_i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("port_inb".to_string(), port_inb_id);

        // x86_serial_init(port, baud) -> i64
        let x86_serial_id = self
            .module
            .declare_function(
                "fj_rt_bare_x86_serial_init",
                Linkage::Import,
                &sig_2i64_ret_i64,
            )
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("x86_serial_init".to_string(), x86_serial_id);

        // set_uart_mode_x86(base_port) -> void
        let set_mode_id = self
            .module
            .declare_function(
                "fj_rt_bare_set_uart_mode_x86",
                Linkage::Import,
                &sig_i64_void,
            )
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("set_uart_mode_x86".to_string(), set_mode_id);

        // x86_64 CPUID builtins
        for (name, builtin) in [
            ("fj_rt_bare_cpuid_eax", "cpuid_eax"),
            ("fj_rt_bare_cpuid_ebx", "cpuid_ebx"),
            ("fj_rt_bare_cpuid_ecx", "cpuid_ecx"),
            ("fj_rt_bare_cpuid_edx", "cpuid_edx"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_i64_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }

        // () -> i64 builtins (no params, returns i64)
        for (name, builtin) in [
            ("fj_rt_bare_read_cr0", "read_cr0"),
            ("fj_rt_bare_read_cr3", "read_cr3"),
            ("fj_rt_bare_read_cr2", "read_cr2"),
            ("fj_rt_bare_read_cr4", "read_cr4"),
            ("fj_rt_bare_read_timer_ticks", "read_timer_ticks"),
            ("fj_rt_bare_rdrand", "rdrand"),
            ("fj_rt_bare_rdtsc", "rdtsc"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }

        // sse_enable() -> void
        let sse_id = self
            .module
            .declare_function("fj_rt_bare_sse_enable", Linkage::Import, &sig_halt)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("sse_enable".to_string(), sse_id);

        // x86_64 IDT + PIC + PIT
        for (name, builtin) in [
            ("fj_rt_bare_idt_init", "idt_init"),
            ("fj_rt_bare_pic_remap", "pic_remap"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_halt)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        for (name, builtin) in [
            ("fj_rt_bare_pic_eoi", "pic_eoi"),
            ("fj_rt_bare_pit_init", "pit_init"),
            ("fj_rt_bare_write_cr3", "write_cr3"),
            ("fj_rt_bare_write_cr4", "write_cr4"),
            ("fj_rt_bare_invlpg", "invlpg"),
            ("fj_rt_bare_fxsave", "fxsave"),
            ("fj_rt_bare_fxrstor", "fxrstor"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_i64_void)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }

        // iretq_to_user (Phase 5: Ring 3 transition)
        let iretq_id = self
            .module
            .declare_function(
                "fj_rt_bare_iretq_to_user",
                Linkage::Import,
                &sig_3i64_ret_i64,
            )
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("iretq_to_user".to_string(), iretq_id);

        // MSR read/write (Phase 5+8)
        let read_msr_id = self
            .module
            .declare_function("fj_rt_bare_read_msr", Linkage::Import, &sig_i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("read_msr".to_string(), read_msr_id);

        let write_msr_id = self
            .module
            .declare_function("fj_rt_bare_write_msr", Linkage::Import, &sig_2i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("write_msr".to_string(), write_msr_id);

        // rdmsr/wrmsr aliases (reuse same function IDs as read_msr/write_msr)
        self.functions.insert("rdmsr".to_string(), read_msr_id);
        self.functions.insert("wrmsr".to_string(), write_msr_id);

        // FajarOS Nova v0.2: hlt/cli/sti (void), cpuid (2i64 -> i64)
        for (name, builtin) in [
            ("fj_rt_bare_hlt", "hlt"),
            ("fj_rt_bare_cli", "cli"),
            ("fj_rt_bare_sti", "sti"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_halt)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        let cpuid_id = self
            .module
            .declare_function("fj_rt_bare_cpuid", Linkage::Import, &sig_2i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("cpuid".to_string(), cpuid_id);

        // FajarOS Nova v0.3 Stage A: Extended Port I/O
        for (name, builtin) in [
            ("fj_rt_bare_port_inw", "port_inw"),
            ("fj_rt_bare_port_ind", "port_ind"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_i64_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        {
            let mut sig_2i64_void = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..2 {
                sig_2i64_void
                    .params
                    .push(cranelift_codegen::ir::AbiParam::new(
                        clif_types::default_int_type(),
                    ));
            }
            for (name, builtin) in [
                ("fj_rt_bare_port_outw", "port_outw"),
                ("fj_rt_bare_port_outd", "port_outd"),
            ] {
                let id = self
                    .module
                    .declare_function(name, Linkage::Import, &sig_2i64_void)
                    .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
                self.functions.insert(builtin.to_string(), id);
            }
        }
        // FajarOS Nova v0.3 Stage A: CPU Control
        for (name, builtin) in [
            ("fj_rt_bare_ltr", "ltr"),
            ("fj_rt_bare_lgdt_mem", "lgdt_mem"),
            ("fj_rt_bare_lidt_mem", "lidt_mem"),
            ("fj_rt_bare_int_n", "int_n"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_i64_void)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        for (name, builtin) in [
            ("fj_rt_bare_swapgs", "swapgs"),
            ("fj_rt_bare_pause", "pause"),
            ("fj_rt_bare_stac", "stac"),
            ("fj_rt_bare_clac", "clac"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_halt)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        // FajarOS Nova v0.3 Stage A: Buffer Operations
        let memcmp_buf_id = self
            .module
            .declare_function("fj_rt_bare_memcmp_buf", Linkage::Import, &sig_3i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("memcmp_buf".to_string(), memcmp_buf_id);
        let memcpy_buf_id = self
            .module
            .declare_function("fj_rt_bare_memcpy_buf", Linkage::Import, &sig_3i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("memcpy_buf".to_string(), memcpy_buf_id);
        {
            let mut sig_3i64_void = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..3 {
                sig_3i64_void
                    .params
                    .push(cranelift_codegen::ir::AbiParam::new(
                        clif_types::default_int_type(),
                    ));
            }
            let memset_buf_id = self
                .module
                .declare_function("fj_rt_bare_memset_buf", Linkage::Import, &sig_3i64_void)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions
                .insert("memset_buf".to_string(), memset_buf_id);
        }

        // String byte access
        let str_byte_at_id = self
            .module
            .declare_function("fj_rt_str_byte_at", Linkage::Import, &sig_2i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("str_byte_at".to_string(), str_byte_at_id);

        let str_len_id = self
            .module
            .declare_function("fj_rt_str_len", Linkage::Import, &sig_i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("str_len".to_string(), str_len_id);

        // Process scheduler (Phase 4)
        for (name, builtin) in [
            ("fj_rt_bare_proc_table_addr", "proc_table_addr"),
            ("fj_rt_bare_get_current_pid", "get_current_pid"),
            ("fj_rt_bare_get_proc_count", "get_proc_count"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        let proc_create_id = self
            .module
            .declare_function("fj_rt_bare_proc_create", Linkage::Import, &sig_i64_ret_i64)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("proc_create".to_string(), proc_create_id);
        let set_pid_id = self
            .module
            .declare_function("fj_rt_bare_set_current_pid", Linkage::Import, &sig_i64_void)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("set_current_pid".to_string(), set_pid_id);
        let yield_id = self
            .module
            .declare_function("fj_rt_bare_yield", Linkage::Import, &sig_halt)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("yield_proc".to_string(), yield_id);

        // Phase 5: Ring 3 + SYSCALL
        for (name, builtin) in [
            ("fj_rt_bare_tss_init", "tss_init"),
            ("fj_rt_bare_syscall_init", "syscall_init"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_halt)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        let pcu_id = self
            .module
            .declare_function(
                "fj_rt_bare_proc_create_user",
                Linkage::Import,
                &sig_i64_ret_i64,
            )
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("proc_create_user".to_string(), pcu_id);

        // Phase 6: Keyboard + PCI
        for (name, builtin) in [
            ("fj_rt_bare_kb_read_scancode", "kb_read_scancode"),
            ("fj_rt_bare_kb_has_data", "kb_has_data"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        let pci_id = self
            .module
            .declare_function("fj_rt_bare_pci_read32", Linkage::Import, &sig_gpio4)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("pci_read32".to_string(), pci_id);
        // pci_write32(bus, dev, fn, offset, val) -> void — 5-arg
        {
            let mut sig_5v = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..5 {
                sig_5v.params.push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
            }
            let id = self
                .module
                .declare_function("fj_rt_bare_pci_write32", Linkage::Import, &sig_5v)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("pci_write32".to_string(), id);
        }

        // Phase 8: ACPI + SMP
        let acpi_shutdown_id = self
            .module
            .declare_function("fj_rt_bare_acpi_shutdown", Linkage::Import, &sig_halt)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("acpi_shutdown".to_string(), acpi_shutdown_id);
        for (name, builtin) in [
            ("fj_rt_bare_acpi_find_rsdp", "acpi_find_rsdp"),
            ("fj_rt_bare_rdtsc", "rdtsc"),
            ("fj_rt_bare_read_cr3", "read_cr3"),
            ("fj_rt_bare_read_cr2", "read_cr2"),
            ("fj_rt_bare_read_cr4", "read_cr4"),
            ("fj_rt_bare_rdrand", "rdrand"),
        ] {
            let id = self
                .module
                .declare_function(name, Linkage::Import, &sig_ret_i64)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(builtin.to_string(), id);
        }
        // write_cr3 (i64 → void) for ObjectCompiler
        let wcr3_id = self
            .module
            .declare_function("fj_rt_bare_write_cr3", Linkage::Import, &sig_i64_void)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("write_cr3".to_string(), wcr3_id);
        let acpi_cpu_id = self
            .module
            .declare_function(
                "fj_rt_bare_acpi_get_cpu_count",
                Linkage::Import,
                &sig_i64_ret_i64,
            )
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("acpi_get_cpu_count".to_string(), acpi_cpu_id);

        // ── Memory barriers (needed by NVMe driver) ──
        let sig_void_void = cranelift_codegen::ir::Signature::new(call_conv);
        let fence_id = self
            .module
            .declare_function("fj_rt_memory_fence", Linkage::Import, &sig_void_void)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("__memory_fence".to_string(), fence_id);
        let cfence_id = self
            .module
            .declare_function("fj_rt_compiler_fence", Linkage::Import, &sig_void_void)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("__compiler_fence".to_string(), cfence_id);

        // ── LE/BE buffer helpers (needed by FAT32 + NVMe) ──
        for name in &[
            "buffer_read_u16_le",
            "buffer_read_u32_le",
            "buffer_read_u64_le",
            "buffer_read_u16_be",
            "buffer_read_u32_be",
            "buffer_read_u64_be",
        ] {
            let mut sig = cranelift_codegen::ir::Signature::new(call_conv);
            sig.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            let id = self
                .module
                .declare_function(&format!("fj_rt_{name}"), Linkage::Import, &sig)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(format!("__{name}"), id);
        }
        for name in &[
            "buffer_write_u16_le",
            "buffer_write_u32_le",
            "buffer_write_u64_le",
            "buffer_write_u16_be",
            "buffer_write_u32_be",
            "buffer_write_u64_be",
        ] {
            let mut sig = cranelift_codegen::ir::Signature::new(call_conv);
            sig.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            let id = self
                .module
                .declare_function(&format!("fj_rt_{name}"), Linkage::Import, &sig)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(format!("__{name}"), id);
        }

        // ── Bare-metal atomic primitives (address-based, no heap) ──
        // These operate directly on memory addresses, unlike the standard runtime
        // atomics which use heap-allocated opaque handles.
        // Implementations use x86 LOCK-prefixed instructions for real SMP safety.

        // atomic_cas(addr: i64, expected: i64, desired: i64) -> i64 (returns old value)
        {
            let mut sig_cas = cranelift_codegen::ir::Signature::new(call_conv);
            sig_cas.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig_cas.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig_cas.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig_cas.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            let id = self
                .module
                .declare_function("fj_rt_bare_atomic_cas", Linkage::Import, &sig_cas)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("atomic_cas".to_string(), id);
        }

        // atomic_load(addr: i64) -> i64
        {
            let mut sig_load = cranelift_codegen::ir::Signature::new(call_conv);
            sig_load.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig_load.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            let id = self
                .module
                .declare_function("fj_rt_bare_atomic_load", Linkage::Import, &sig_load)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("atomic_load".to_string(), id);
        }

        // atomic_store(addr: i64, value: i64)
        {
            let mut sig_store = cranelift_codegen::ir::Signature::new(call_conv);
            sig_store.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig_store.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            let id = self
                .module
                .declare_function("fj_rt_bare_atomic_store", Linkage::Import, &sig_store)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("atomic_store".to_string(), id);
        }

        // atomic_add(addr: i64, value: i64) -> i64 (returns old value)
        // atomic_sub(addr: i64, value: i64) -> i64 (returns old value)
        for name in ["atomic_add", "atomic_sub"] {
            let mut sig_op = cranelift_codegen::ir::Signature::new(call_conv);
            sig_op.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig_op.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig_op.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            let id = self
                .module
                .declare_function(&format!("fj_rt_bare_{name}"), Linkage::Import, &sig_op)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert(name.to_string(), id);
        }

        // ── Dynamic string operations for bare-metal ──
        // str_concat(a_ptr, a_len, b_ptr, b_len, out_ptr, out_len_ptr)
        {
            let mut sig = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in 0..6 {
                sig.params.push(cranelift_codegen::ir::AbiParam::new(
                    clif_types::default_int_type(),
                ));
            }
            let id = self
                .module
                .declare_function("fj_rt_str_concat", Linkage::Import, &sig)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("__str_concat".to_string(), id);
        }
        // Note: __println_str and __print_str are already declared above
        // (mapped to fj_rt_bare_println / fj_rt_bare_print at lines ~6525-6532).
        // Do NOT re-declare with fj_rt_println_str — that symbol doesn't exist in x86 startup .o.
        // to_string(val) -> ptr (integer to decimal string)
        {
            let mut sig = cranelift_codegen::ir::Signature::new(call_conv);
            sig.params.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
            let id = self
                .module
                .declare_function("fj_rt_to_string", Linkage::Import, &sig)
                .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
            self.functions.insert("to_string".to_string(), id);
        }

        Ok(())
    }

    /// Declares user-mode (Ring 3) runtime functions.
    /// These are syscall wrappers for microkernel services compiled with --target x86_64-user.
    /// Each function uses the SYSCALL instruction to invoke kernel services.
    fn declare_user_mode_runtime(&mut self) -> Result<(), CodegenError> {
        let call_conv = self.module.target_config().default_call_conv;

        // --- I/O: println(val: i64) via fj_rt_bare_print_i64 ---
        let mut sig_1i = cranelift_codegen::ir::Signature::new(call_conv);
        sig_1i.params.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        let println_id = self
            .module
            .declare_function("fj_rt_bare_print_i64", Linkage::Import, &sig_1i)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("println".to_string(), println_id);

        // --- Exit: exit(code: i64) via SYS_EXIT(0) ---
        let exit_id = self
            .module
            .declare_function("fj_user_exit", Linkage::Import, &sig_1i)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("exit".to_string(), exit_id);

        // --- Getpid: getpid() -> i64 via SYS_GETPID(3) ---
        let mut sig_0_r = cranelift_codegen::ir::Signature::new(call_conv);
        sig_0_r.returns.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        let getpid_id = self
            .module
            .declare_function("fj_user_getpid", Linkage::Import, &sig_0_r)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("getpid".to_string(), getpid_id);

        // --- IPC: send(dst: i64, msg: i64) -> i64 via SYS_SEND(10) ---
        let mut sig_2i_r = cranelift_codegen::ir::Signature::new(call_conv);
        sig_2i_r.params.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        sig_2i_r.params.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        sig_2i_r.returns.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        let send_id = self
            .module
            .declare_function("fj_user_ipc_send", Linkage::Import, &sig_2i_r)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("ipc_send".to_string(), send_id);

        // --- IPC: recv(src: i64, buf: i64) -> i64 via SYS_RECV(11) ---
        let recv_id = self
            .module
            .declare_function("fj_user_ipc_recv", Linkage::Import, &sig_2i_r)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("ipc_recv".to_string(), recv_id);

        // --- IPC: call(dst: i64, msg: i64, reply: i64) -> i64 via SYS_CALL(12) ---
        let mut sig_3i_r = cranelift_codegen::ir::Signature::new(call_conv);
        sig_3i_r.params.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        sig_3i_r.params.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        sig_3i_r.params.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        sig_3i_r.returns.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        let call_id = self
            .module
            .declare_function("fj_user_ipc_call", Linkage::Import, &sig_3i_r)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("ipc_call".to_string(), call_id);

        // --- IPC: reply(caller: i64, msg: i64) -> i64 via SYS_REPLY(13) ---
        let reply_id = self
            .module
            .declare_function("fj_user_ipc_reply", Linkage::Import, &sig_2i_r)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("ipc_reply".to_string(), reply_id);

        // --- Memory: mmap(size: i64) -> i64 via SYS_MMAP(6) ---
        let mut sig_1i_r = cranelift_codegen::ir::Signature::new(call_conv);
        sig_1i_r.params.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        sig_1i_r.returns.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        let mmap_id = self
            .module
            .declare_function("fj_user_mmap", Linkage::Import, &sig_1i_r)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("mmap".to_string(), mmap_id);

        // --- Process: getpid() -> i64 via SYS_GETPID(3) ---
        let mut sig_0_r = cranelift_codegen::ir::Signature::new(call_conv);
        sig_0_r.returns.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        let getpid_id = self
            .module
            .declare_function("fj_user_getpid", Linkage::Import, &sig_0_r)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("getpid".to_string(), getpid_id);

        // --- Volatile reads/writes (for shared memory regions) ---
        let vol_read_id = self
            .module
            .declare_function("fj_rt_volatile_read_u64", Linkage::Import, &sig_1i_r)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("volatile_read_u64".to_string(), vol_read_id);

        let mut sig_2i_void = cranelift_codegen::ir::Signature::new(call_conv);
        sig_2i_void
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                cranelift_codegen::ir::types::I64,
            ));
        sig_2i_void
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                cranelift_codegen::ir::types::I64,
            ));
        let vol_write_id = self
            .module
            .declare_function("fj_rt_volatile_write_u64", Linkage::Import, &sig_2i_void)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("volatile_write_u64".to_string(), vol_write_id);

        // Also need string builtins for println
        let str_len_id = self
            .module
            .declare_function("fj_rt_bare_str_len", Linkage::Import, &sig_1i_r)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("str_len".to_string(), str_len_id);

        let str_byte_id = self
            .module
            .declare_function("fj_rt_bare_str_byte_at", Linkage::Import, &sig_2i_r)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("str_byte_at".to_string(), str_byte_id);

        // --- String printing: use fj_rt_bare_println/fj_rt_bare_print ---
        // (same symbols as x86 bare-metal; user startup .o provides SYSCALL-based versions)
        let mut sig_2i_void2 = cranelift_codegen::ir::Signature::new(call_conv);
        sig_2i_void2
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                cranelift_codegen::ir::types::I64,
            ));
        sig_2i_void2
            .params
            .push(cranelift_codegen::ir::AbiParam::new(
                cranelift_codegen::ir::types::I64,
            ));
        let println_str_id = self
            .module
            .declare_function("fj_rt_bare_println", Linkage::Import, &sig_2i_void2)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("__println_str".to_string(), println_str_id);

        let print_str_id = self
            .module
            .declare_function("fj_rt_bare_print", Linkage::Import, &sig_2i_void2)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions
            .insert("__print_str".to_string(), print_str_id);

        // to_string(val: i64) -> (ptr, len) — returns string representation of integer
        let mut sig_1i_2r = cranelift_codegen::ir::Signature::new(call_conv);
        sig_1i_2r.params.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        sig_1i_2r.returns.push(cranelift_codegen::ir::AbiParam::new(
            cranelift_codegen::ir::types::I64,
        ));
        let to_str_id = self
            .module
            .declare_function("fj_rt_to_string", Linkage::Import, &sig_1i_2r)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
        self.functions.insert("to_string".to_string(), to_str_id);

        // Note: do NOT call declare_bare_metal_runtime() here — it has
        // conflicting signatures (e.g., print takes 2 args in bare-metal).
        // User-mode services link against a separate user-mode runtime library.

        Ok(())
    }

    /// Declares built-in runtime functions as imports (resolved at link time).
    fn declare_runtime_functions(&mut self) -> Result<(), CodegenError> {
        // User-mode (Ring 3): syscall-based runtime for microkernel services
        if self.user_mode {
            return self.declare_user_mode_runtime();
        }
        // Bare-metal: only declare minimal runtime (no libc/heap)
        if self.no_std {
            return self.declare_bare_metal_runtime();
        }

        runtime_hosted::declare_hosted_runtime(
            &mut self.module,
            &mut self.functions,
            /* jit_extras */ false,
            /* profiling */ false,
        )
    }

    /// Enables or disables no_std mode (skips IO runtime functions).
    pub fn set_no_std(&mut self, enabled: bool) {
        self.no_std = enabled;
    }

    /// Enables user-mode compilation (Ring 3 programs for FajarOS).
    /// Uses SYSCALL for I/O instead of direct hardware access.
    pub fn set_user_mode(&mut self, enabled: bool) {
        self.user_mode = enabled;
    }

    /// Enables runtime security hardening (bounds checks, overflow checks).
    pub fn enable_security(&mut self) {
        self.security_enabled = true;
    }

    /// Enables the security linter pre-pass before compilation.
    pub fn enable_lint(&mut self) {
        self.lint_on_compile = true;
    }

    /// Sets the dead function list from the optimizer's dead function elimination pass.
    pub fn set_dead_functions(&mut self, dead: Vec<String>) {
        self.dead_functions = dead.into_iter().collect();
    }

    /// Enables debug information generation.
    pub fn set_debug_info(&mut self, enabled: bool, source_file: Option<String>) {
        self.debug_info = enabled;
        self.source_file = source_file;
    }

    /// Records the source location of a function for debug info.
    pub fn record_fn_location(&mut self, name: &str, start_line: u32, end_line: u32) {
        self.fn_source_locations
            .insert(name.to_string(), (start_line, end_line));
    }

    /// Returns the source locations map (for testing/inspection).
    pub fn source_locations(&self) -> &HashMap<String, (u32, u32)> {
        &self.fn_source_locations
    }

    /// Compiles all functions in a program to object code.
    pub fn compile_program(&mut self, program: &Program) -> Result<(), Vec<CodegenError>> {
        let mut errors = Vec::new();

        // Security linter pre-pass: emit warnings for known unsafe patterns.
        if self.lint_on_compile {
            let source_repr = format!("{program:?}");
            let linter = crate::codegen::security::SecurityLinter::new();
            let violations = linter.lint(&source_repr);
            for v in &violations {
                eprintln!("security: [{}] line {}: {}", v.rule_id, v.line, v.message);
            }
        }

        // H1: Enforce no_std compliance for bare-metal targets
        if self.no_std {
            let config = crate::codegen::nostd::NoStdConfig::bare_metal();
            let violations = crate::codegen::nostd::check_nostd_compliance(program, &config);
            for v in violations {
                errors.push(CodegenError::NoStdViolation(v.to_string()));
            }
            if !errors.is_empty() {
                return Err(errors);
            }
        }

        // Declare runtime built-in functions
        if let Err(e) = self.declare_runtime_functions() {
            errors.push(e);
            return Err(errors);
        }

        // Register built-in Poll<T> enum: Ready(T)=0, Pending=1 (S4.1)
        self.enum_defs.insert(
            "Poll".to_string(),
            vec!["Ready".to_string(), "Pending".to_string()],
        );
        self.enum_variant_types.insert(
            ("Poll".to_string(), "Ready".to_string()),
            vec![clif_types::default_int_type()],
        );
        self.generic_enum_defs
            .insert("Poll".to_string(), vec!["T".to_string()]);

        // Collect enum and struct definitions
        for item in &program.items {
            match item {
                Item::EnumDef(edef) => {
                    let variants: Vec<String> =
                        edef.variants.iter().map(|v| v.name.clone()).collect();
                    // Track payload types for each variant
                    for v in &edef.variants {
                        let payload_types: Vec<cranelift_codegen::ir::Type> = v
                            .fields
                            .iter()
                            .map(|f| {
                                clif_types::lower_type(f).unwrap_or(clif_types::default_int_type())
                            })
                            .collect();
                        self.enum_variant_types
                            .insert((edef.name.clone(), v.name.clone()), payload_types);
                    }
                    // Track generic enum definitions (S1.2)
                    if !edef.generic_params.is_empty() {
                        let param_names: Vec<String> = edef
                            .generic_params
                            .iter()
                            .map(|gp| gp.name.clone())
                            .collect();
                        self.generic_enum_defs
                            .insert(edef.name.clone(), param_names);
                    }
                    self.enum_defs.insert(edef.name.clone(), variants);
                }
                Item::StructDef(sdef) => {
                    let fields: Vec<(String, cranelift_codegen::ir::Type)> = sdef
                        .fields
                        .iter()
                        .map(|f| {
                            let ty = clif_types::lower_type(&f.ty)
                                .unwrap_or(clif_types::default_int_type());
                            (f.name.clone(), ty)
                        })
                        .collect();
                    // Check for bitfield fields (u1-u7 types) and compute layout
                    let mut bit_offset: u8 = 0;
                    let mut bf_layout = Vec::new();
                    for f in &sdef.fields {
                        if let TypeExpr::Simple { name: tname, .. } = &f.ty {
                            if let Some(width) = clif_types::bitfield_width(tname) {
                                bf_layout.push((f.name.clone(), bit_offset, width));
                                bit_offset += width;
                            }
                        }
                    }
                    if !bf_layout.is_empty() {
                        self.bitfield_layouts.insert(sdef.name.clone(), bf_layout);
                    }
                    self.struct_defs.insert(sdef.name.clone(), fields);
                }
                Item::UnionDef(udef) => {
                    let fields: Vec<(String, cranelift_codegen::ir::Type)> = udef
                        .fields
                        .iter()
                        .map(|f| {
                            let ty = clif_types::lower_type(&f.ty)
                                .unwrap_or(clif_types::default_int_type());
                            (f.name.clone(), ty)
                        })
                        .collect();
                    self.struct_defs.insert(udef.name.clone(), fields);
                    self.union_names.insert(udef.name.clone());
                }
                Item::ConstDef(cdef) => {
                    self.const_defs
                        .push((cdef.name.clone(), *cdef.value.clone(), cdef.ty.clone()));
                    if let Some(ref ann) = cdef.annotation {
                        if ann.name == "section" {
                            if let Some(ref sec) = ann.param {
                                self.data_sections.insert(cdef.name.clone(), sec.clone());
                            }
                        }
                    }
                }
                Item::StaticDef(sdef) => {
                    // Treat static mut like const for codegen
                    self.const_defs
                        .push((sdef.name.clone(), *sdef.value.clone(), sdef.ty.clone()));
                }
                Item::ServiceDef(_svc) => {
                    // Service handlers are processed when encountered as FnDef
                    // (ServiceDef expands to FnDef items in the analyzer)
                }
                Item::ModDecl(mdecl) => {
                    if let Some(ref body) = mdecl.body {
                        for mod_item in body {
                            match mod_item {
                                Item::EnumDef(edef) => {
                                    let variants: Vec<String> =
                                        edef.variants.iter().map(|v| v.name.clone()).collect();
                                    self.enum_defs.insert(edef.name.clone(), variants);
                                }
                                Item::StructDef(sdef) => {
                                    let fields: Vec<(String, cranelift_codegen::ir::Type)> = sdef
                                        .fields
                                        .iter()
                                        .map(|f| {
                                            let ty = clif_types::lower_type(&f.ty)
                                                .unwrap_or(clif_types::default_int_type());
                                            (f.name.clone(), ty)
                                        })
                                        .collect();
                                    self.struct_defs.insert(sdef.name.clone(), fields);
                                }
                                Item::ConstDef(cdef) => {
                                    self.const_defs.push((
                                        cdef.name.clone(),
                                        *cdef.value.clone(),
                                        cdef.ty.clone(),
                                    ));
                                    if let Some(ref ann) = cdef.annotation {
                                        if ann.name == "section" {
                                            if let Some(ref sec) = ann.param {
                                                self.data_sections
                                                    .insert(cdef.name.clone(), sec.clone());
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Collect global_asm sections
        for item in &program.items {
            if let Item::GlobalAsm(ga) = item {
                self.global_asm_sections.push(ga.template.clone());
            }
        }

        // Collect trait definitions and impls (shared helper)
        let (td, ti) = collect_trait_info(program);
        self.trait_defs = td;
        self.trait_impls = ti;

        // Declare extern functions (imported C symbols)
        for item in &program.items {
            if let Item::ExternFn(efn) = item {
                if let Err(e) = self.declare_extern_fn(efn) {
                    errors.push(e);
                }
            }
        }

        // Separate generic from concrete functions (including module functions)
        let mut concrete_fns = Vec::new();
        for item in &program.items {
            if let Item::FnDef(fndef) = item {
                if fndef.generic_params.is_empty() {
                    concrete_fns.push(fndef.clone());
                    // Extract nested function definitions from body
                    extract_nested_fns(&fndef.body, &mut concrete_fns);
                } else {
                    self.generic_fns.insert(fndef.name.clone(), fndef.clone());
                }
            }
            // Flatten module functions with mangled names: modname_fnname
            if let Item::ModDecl(mdecl) = item {
                if let Some(ref body) = mdecl.body {
                    for mod_item in body {
                        if let Item::FnDef(fndef) = mod_item {
                            let mut mangled_fn = fndef.clone();
                            let mangled_name = format!("{}_{}", mdecl.name, fndef.name);
                            mangled_fn.name = mangled_name.clone();
                            self.module_fns.insert(mangled_name, mdecl.name.clone());
                            if mangled_fn.generic_params.is_empty() {
                                concrete_fns.push(mangled_fn);
                            } else {
                                self.generic_fns.insert(mangled_fn.name.clone(), mangled_fn);
                            }
                        }
                    }
                }
            }
        }

        // Collect const fn definitions for compile-time evaluation
        for fndef in &concrete_fns {
            if fndef.is_const {
                self.const_fn_defs.insert(fndef.name.clone(), fndef.clone());
            }
        }

        // Collect impl blocks: mangle methods as TypeName_method_name
        for item in &program.items {
            if let Item::ImplBlock(impl_block) = item {
                for method in &impl_block.methods {
                    let mangled = format!("{}_{}", impl_block.target_type, method.name);
                    self.impl_methods.insert(
                        (impl_block.target_type.clone(), method.name.clone()),
                        mangled.clone(),
                    );
                    let mut mangled_fn = method.clone();
                    mangled_fn.name = mangled;
                    concrete_fns.push(mangled_fn);
                }
            }
        }

        // Dead function elimination: compute reachable set from entry points
        // This must happen BEFORE declarations so we only declare reachable functions.
        let mut fn_bodies_for_dce: HashMap<String, &Expr> = HashMap::new();
        let mut dce_entry_points = Vec::new();
        for fndef in &concrete_fns {
            fn_bodies_for_dce.insert(fndef.name.clone(), &fndef.body);
            if fndef.name == "main" {
                dce_entry_points.push(fndef.name.clone());
            }
            if let Some(ref ann) = fndef.annotation {
                if ann.name == "entry" || ann.name == "panic_handler" {
                    dce_entry_points.push(fndef.name.clone());
                }
            }
            // Bare-metal: functions called from startup assembly (not from .fj code)
            if self.no_std
                && (fndef.name == "kernel_main"
                    || fndef.name == "fj_exception_sync"
                    || fndef.name == "fj_exception_irq")
            {
                dce_entry_points.push(fndef.name.clone());
            }
        }
        // Scan for fn_addr("name") calls and add targets as entry points
        // (fn_addr references functions by string, invisible to call-graph DCE)
        for fndef in &concrete_fns {
            crate::codegen::cranelift::scan_fn_addr_targets(&fndef.body, &mut dce_entry_points);
        }

        // If no explicit entry points, keep all functions (library mode)
        let mut reachable = if dce_entry_points.is_empty() {
            concrete_fns.iter().map(|f| f.name.clone()).collect()
        } else {
            compute_reachable(&dce_entry_points, &fn_bodies_for_dce)
        };
        drop(fn_bodies_for_dce);
        // Expand reachability for impl/module methods
        let all_fn_names: Vec<String> = concrete_fns.iter().map(|f| f.name.clone()).collect();
        let reachable_snapshot: Vec<String> = reachable.iter().cloned().collect();
        for mangled in &all_fn_names {
            if reachable.contains(mangled) {
                continue;
            }
            if let Some(idx) = mangled.find('_') {
                let suffix = &mangled[idx + 1..];
                let prefix = &mangled[..idx];
                if reachable_snapshot
                    .iter()
                    .any(|r| r == suffix || r == prefix || r == mangled)
                {
                    reachable.insert(mangled.clone());
                }
            }
        }
        // Mark @interrupt and @panic_handler functions as always reachable
        for fndef in &concrete_fns {
            if let Some(ref ann) = fndef.annotation {
                if ann.name == "interrupt" || ann.name == "panic_handler" || ann.name == "entry" {
                    reachable.insert(fndef.name.clone());
                }
            }
        }

        // Filter concrete_fns to only reachable functions.
        // In bare-metal (no_std) mode, skip DCE — OS kernels use fn_addr,
        // interrupt vectors, and asm trampolines invisible to static analysis.
        // The linker's --gc-sections handles unused function removal instead.
        if !self.no_std {
            concrete_fns.retain(|f| reachable.contains(&f.name));
        }

        // First pass: declare concrete functions
        for fndef in &concrete_fns {
            if let Err(e) = self.declare_function(fndef) {
                errors.push(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        // Pre-scan for closures in all function bodies
        let mut known_names: HashSet<String> = self.functions.keys().cloned().collect();
        for names in self.enum_defs.values() {
            for v in names {
                known_names.insert(v.clone());
            }
        }
        for name in self.enum_defs.keys() {
            known_names.insert(name.clone());
        }
        for name in self.struct_defs.keys() {
            known_names.insert(name.clone());
        }
        for builtin in &[
            "print",
            "println",
            "eprintln",
            "eprint",
            "len",
            "assert",
            "assert_eq",
            "to_string",
            "to_int",
            "to_float",
            "type_of",
            "format",
            "dbg",
            "panic",
            "todo",
            "abs",
            "sqrt",
            "pow",
            "sin",
            "cos",
            "tan",
            "floor",
            "ceil",
            "round",
            "clamp",
            "min",
            "max",
            "log",
            "log2",
            "log10",
            "Some",
            "None",
            "Ok",
            "Err",
            "read_file",
            "write_file",
            "append_file",
            "file_exists",
            "wrapping_add",
            "wrapping_sub",
            "wrapping_mul",
            "saturating_add",
            "saturating_sub",
            "saturating_mul",
            "checked_add",
            "checked_sub",
            "checked_mul",
            "true",
            "false",
            "null",
        ] {
            known_names.insert(builtin.to_string());
        }

        let mut closure_fns: Vec<FnDef> = Vec::new();
        for fndef in &concrete_fns {
            let closures = scan_closures_in_body(&fndef.body, &known_names);
            let mut has_captured_closure = false;
            for ci in &closures {
                self.closure_fn_map
                    .insert(ci.var_name.clone(), ci.fn_name.clone());
                self.closure_span_to_fn
                    .insert((ci.span.start, ci.span.end), ci.fn_name.clone());
                self.closure_captures
                    .insert(ci.fn_name.clone(), ci.captures.clone());
                if !ci.captures.is_empty() {
                    has_captured_closure = true;
                }
            }
            for ci in closures {
                closure_fns.push(ci.fndef);
            }
            if has_captured_closure && fndef.return_type.is_some() {
                if let Some(TypeExpr::Fn { .. }) = &fndef.return_type {
                    self.fn_returns_closure_handle.insert(fndef.name.clone());
                }
            }
        }

        for cfn in &closure_fns {
            if let Err(e) = self.declare_function(cfn) {
                errors.push(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        // Monomorphize: scan all concrete fn bodies for calls to generic fns
        let mono_fns = self.monomorphize(&concrete_fns);
        for mono_fn in &mono_fns {
            if let Err(e) = self.declare_function(mono_fn) {
                errors.push(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        // Build generic param mapping for multi-param type dispatch
        self.generic_fn_params = self.build_generic_fn_params();

        // Scan for @panic_handler, @section, @interrupt annotations and async functions
        for fndef in &concrete_fns {
            if let Some(ref ann) = fndef.annotation {
                if ann.name == "panic_handler" {
                    self.panic_handler_fn = Some(fndef.name.clone());
                }
                if ann.name == "section" {
                    if let Some(ref section_name) = ann.param {
                        self.fn_sections
                            .insert(fndef.name.clone(), section_name.clone());
                    }
                }
                if ann.name == "interrupt" {
                    self.interrupt_fns.push(fndef.name.clone());
                }
            }
            if fndef.is_async {
                self.async_fns.insert(fndef.name.clone());
            }
        }

        // Second pass: compile function bodies (concrete + monomorphized + closures).
        // Skip dead functions identified by the optimizer (if any).
        for fndef in &concrete_fns {
            if self.dead_functions.contains(&fndef.name) {
                continue; // dead function — optimizer says unreachable
            }
            if let Err(e) = self.define_function(fndef) {
                errors.push(e);
            }
        }
        for mono_fn in &mono_fns {
            if let Err(e) = self.define_function(mono_fn) {
                errors.push(e);
            }
        }
        for cfn in &closure_fns {
            if let Err(e) = self.define_function(cfn) {
                errors.push(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        // Emit `_start` symbol as alias for @entry function (bare metal entry point)
        for fndef in &concrete_fns {
            if let Some(ref ann) = fndef.annotation {
                if ann.name == "entry" && fndef.name != "_start" {
                    // Declare _start as an exported function with same signature
                    let sig = cranelift_codegen::ir::Signature {
                        params: vec![],
                        returns: vec![],
                        call_conv: self.module.isa().default_call_conv(),
                    };
                    if let Ok(start_id) =
                        self.module
                            .declare_function("_start", Linkage::Export, &sig)
                    {
                        // Define _start as a wrapper that calls the entry function
                        let mut ctx = self.module.make_context();
                        ctx.func.signature = sig;
                        let mut builder_ctx = FunctionBuilderContext::new();
                        {
                            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
                            let entry_block = builder.create_block();
                            builder.switch_to_block(entry_block);
                            builder.seal_block(entry_block); // No predecessors

                            if self.no_std {
                                // B4: Bare-metal _start: BSS zeroing + call @entry + halt loop

                                // Try to declare BSS linker symbols
                                let bss_start_data = self
                                    .module
                                    .declare_data("__bss_start", Linkage::Import, false, false)
                                    .ok();
                                let bss_end_data = self
                                    .module
                                    .declare_data("__bss_end", Linkage::Import, false, false)
                                    .ok();

                                // BSS zeroing (if linker symbols available)
                                if let (Some(bss_start_id), Some(bss_end_id)) =
                                    (bss_start_data, bss_end_data)
                                {
                                    let bss_start_gv = self
                                        .module
                                        .declare_data_in_func(bss_start_id, builder.func);
                                    let bss_end_gv =
                                        self.module.declare_data_in_func(bss_end_id, builder.func);
                                    let bss_start_addr = builder.ins().global_value(
                                        cranelift_codegen::ir::types::I64,
                                        bss_start_gv,
                                    );
                                    let bss_end_addr = builder.ins().global_value(
                                        cranelift_codegen::ir::types::I64,
                                        bss_end_gv,
                                    );

                                    // Call fj_rt_memset_zero(start, end) as a runtime function
                                    // For simplicity, emit the zeroing as a call to memset-like
                                    // runtime function. The linker resolves __bss_start/end.
                                    // We use a simple subtraction + zero-fill via Cranelift store.
                                    let ptr_var =
                                        builder.declare_var(cranelift_codegen::ir::types::I64);
                                    builder.def_var(ptr_var, bss_start_addr);

                                    let loop_hdr = builder.create_block();
                                    let loop_body = builder.create_block();
                                    let loop_done = builder.create_block();

                                    builder.ins().jump(loop_hdr, &[]);

                                    // Loop header
                                    builder.switch_to_block(loop_hdr);
                                    let cur = builder.use_var(ptr_var);
                                    let still_going = builder.ins().icmp(
                                        cranelift_codegen::ir::condcodes::IntCC::UnsignedLessThan,
                                        cur,
                                        bss_end_addr,
                                    );
                                    builder
                                        .ins()
                                        .brif(still_going, loop_body, &[], loop_done, &[]);

                                    // Loop body: *ptr = 0; ptr += 8
                                    builder.switch_to_block(loop_body);
                                    let cur2 = builder.use_var(ptr_var);
                                    let zero =
                                        builder.ins().iconst(cranelift_codegen::ir::types::I64, 0);
                                    builder.ins().store(
                                        cranelift_codegen::ir::MemFlags::new(),
                                        zero,
                                        cur2,
                                        0,
                                    );
                                    let eight =
                                        builder.ins().iconst(cranelift_codegen::ir::types::I64, 8);
                                    let next = builder.ins().iadd(cur2, eight);
                                    builder.def_var(ptr_var, next);
                                    builder.ins().jump(loop_hdr, &[]);
                                    builder.seal_block(loop_body); // Only from loop_hdr

                                    builder.switch_to_block(loop_done);
                                    builder.seal_block(loop_hdr); // From entry + loop_body
                                    builder.seal_block(loop_done); // Only from loop_hdr
                                } else {
                                    // No BSS symbols available — skip zeroing
                                }

                                // Call @entry function
                                if let Some(&entry_id) = self.functions.get(&fndef.name) {
                                    let entry_ref =
                                        self.module.declare_func_in_func(entry_id, builder.func);
                                    builder.ins().call(entry_ref, &[]);
                                }

                                // Infinite halt loop (bare-metal: never returns)
                                let halt_block = builder.create_block();
                                builder.ins().jump(halt_block, &[]);
                                builder.switch_to_block(halt_block);
                                builder.ins().jump(halt_block, &[]); // spin forever
                                builder.seal_block(halt_block); // Self-loop predecessor
                            } else {
                                // Normal mode: just call entry and return
                                if let Some(&entry_id) = self.functions.get(&fndef.name) {
                                    let entry_ref =
                                        self.module.declare_func_in_func(entry_id, builder.func);
                                    builder.ins().call(entry_ref, &[]);
                                }
                                builder.ins().return_(&[]);
                            }

                            builder.finalize();
                        }
                        let _ = self.module.define_function(start_id, &mut ctx);
                        self.module.clear_context(&mut ctx);
                    }
                    break;
                }
            }
        }

        // User-mode _start: calls main() then SYS_EXIT(0)
        if self.user_mode {
            if let Some(main_id) = self.functions.get("main").copied() {
                let sig = cranelift_codegen::ir::Signature {
                    params: vec![],
                    returns: vec![],
                    call_conv: self.module.isa().default_call_conv(),
                };
                if let Ok(start_id) = self
                    .module
                    .declare_function("_start", Linkage::Export, &sig)
                {
                    let mut ctx = self.module.make_context();
                    ctx.func.signature = sig;
                    let mut builder_ctx = FunctionBuilderContext::new();
                    {
                        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
                        let entry_block = builder.create_block();
                        builder.switch_to_block(entry_block);
                        builder.seal_block(entry_block);

                        // Call main()
                        let main_ref = self.module.declare_func_in_func(main_id, builder.func);
                        builder.ins().call(main_ref, &[]);

                        // After main() returns, call exit(0) via SYSCALL
                        if let Some(&exit_fn_id) = self.functions.get("exit") {
                            let exit_ref =
                                self.module.declare_func_in_func(exit_fn_id, builder.func);
                            let zero = builder.ins().iconst(cranelift_codegen::ir::types::I64, 0);
                            builder.ins().call(exit_ref, &[zero]);
                        }
                        // Halt loop as safety net (exit should never return)
                        let halt = builder.create_block();
                        builder.ins().jump(halt, &[]);
                        builder.switch_to_block(halt);
                        builder.ins().jump(halt, &[]);
                        builder.seal_block(halt);
                    }
                    self.module
                        .define_function(start_id, &mut ctx)
                        .map_err(|e| vec![CodegenError::FunctionError(e.to_string())])?;
                }
            }
        }

        // Create global data objects for section-annotated consts (AOT: placed in specified sections)
        for (cname, cexpr, cty) in &self.const_defs {
            if let Some(section) = self.data_sections.get(cname).cloned() {
                let byte_size = clif_types::lower_type(cty)
                    .map(|t| t.bytes() as usize)
                    .unwrap_or(8);
                let data_id = self
                    .module
                    .declare_data(cname, Linkage::Export, true, false)
                    .map_err(|e| vec![CodegenError::Internal(e.to_string())])?;
                let mut desc = cranelift_module::DataDescription::new();
                let init_bytes = match cexpr {
                    Expr::Literal {
                        kind: LiteralKind::Int(v),
                        ..
                    } => v.to_le_bytes()[..byte_size].to_vec(),
                    Expr::Literal {
                        kind: LiteralKind::Float(f),
                        ..
                    } => f.to_le_bytes()[..byte_size].to_vec(),
                    _ => vec![0u8; byte_size],
                };
                desc.define(init_bytes.into_boxed_slice());
                desc.set_segment_section("", &section);
                self.module
                    .define_data(data_id, &desc)
                    .map_err(|e| vec![CodegenError::Internal(e.to_string())])?;
                self.global_data.insert(cname.clone(), data_id);
            }
        }

        // V27.5 P1.3a: Generate ISR wrappers for @interrupt-annotated functions.
        // Each wrapper saves all GP registers, calls the handler, restores
        // registers, and uses the architecture-specific return instruction
        // (eret on ARM64, iretq on x86_64).
        for irq_fn in &self.interrupt_fns.clone() {
            let wrapper = super::linker::generate_interrupt_wrapper(irq_fn);
            self.global_asm_sections.push(wrapper);
        }

        Ok(())
    }

    /// Scans concrete functions for calls to generic functions and creates
    /// monomorphized (type-specialized) versions.
    fn monomorphize(&mut self, concrete_fns: &[FnDef]) -> Vec<FnDef> {
        let mut mono_fns = Vec::new();
        let mut mono_specs: HashSet<(String, String)> = HashSet::new();

        for fndef in concrete_fns {
            // Build param type map from function parameters for type inference
            let mut param_types = HashMap::new();
            for p in &fndef.params {
                if let TypeExpr::Simple { name: tn, .. } = &p.ty {
                    let clif_suffix = match tn.as_str() {
                        "f64" | "float" | "f32" => "f64",
                        _ => "i64",
                    };
                    param_types.insert(p.name.clone(), clif_suffix.to_string());
                }
            }
            collect_generic_calls(
                &fndef.body,
                &self.generic_fns,
                &mut self.mono_map,
                &mut mono_specs,
                &param_types,
            );
        }

        // Create specialized versions for each (fn_name, type_suffix) pair
        for (generic_name, type_suffix) in &mono_specs {
            if let Some(generic_def) = self.generic_fns.get(generic_name) {
                let mangled_name = format!("{generic_name}__mono_{type_suffix}");
                let specialized = specialize_fndef(generic_def, &mangled_name, type_suffix);
                mono_fns.push(specialized);
            }
        }

        mono_fns
    }

    /// Builds mapping from generic fn names to their parameter→generic_param associations.
    fn build_generic_fn_params(&self) -> HashMap<String, Vec<(usize, String)>> {
        let mut result = HashMap::new();
        for (name, fndef) in &self.generic_fns {
            let generic_param_names: Vec<String> = fndef
                .generic_params
                .iter()
                .map(|gp| gp.name.clone())
                .collect();
            let mut mappings = Vec::new();
            for (i, param) in fndef.params.iter().enumerate() {
                if let TypeExpr::Simple {
                    name: ptype_name, ..
                } = &param.ty
                {
                    if generic_param_names.contains(ptype_name) {
                        mappings.push((i, ptype_name.clone()));
                    }
                }
            }
            result.insert(name.clone(), mappings);
        }
        result
    }

    /// Declares an extern (imported) function for AOT object emission.
    fn declare_extern_fn(&mut self, efn: &ExternFn) -> Result<(), CodegenError> {
        let has_return = efn.return_type.as_ref().is_some_and(
            |ty| !matches!(ty, crate::parser::ast::TypeExpr::Simple { name, .. } if name == "void"),
        );
        let ret_type = efn.return_type.as_ref().and_then(clif_types::lower_type);
        let call_conv = self.module.target_config().default_call_conv;
        let sig = super::abi::build_signature_with_return_type(
            call_conv,
            &efn.params,
            has_return,
            ret_type,
        )?;

        let func_id = self
            .module
            .declare_function(&efn.name, Linkage::Import, &sig)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;

        self.functions.insert(efn.name.clone(), func_id);
        if let Some(rt) = ret_type {
            self.fn_return_types.insert(efn.name.clone(), rt);
        }
        Ok(())
    }

    /// Declares a function signature (first pass).
    fn declare_function(&mut self, fndef: &crate::parser::ast::FnDef) -> Result<(), CodegenError> {
        let has_return = !CraneliftCompiler::is_void_return(fndef);
        let call_conv = self.module.target_config().default_call_conv;

        // Check if this function returns a struct type
        let struct_ret_name = fndef.return_type.as_ref().and_then(|ty| {
            if let TypeExpr::Simple { name, .. } = ty {
                if self.struct_defs.contains_key(name) {
                    Some(name.clone())
                } else {
                    None
                }
            } else {
                None
            }
        });

        let is_enum_ret =
            if let Some(crate::parser::ast::TypeExpr::Simple { name, .. }) = &fndef.return_type {
                self.enum_defs.contains_key(name)
            } else {
                false
            };
        let ret_type = if is_enum_ret {
            Some(clif_types::default_int_type())
        } else {
            CraneliftCompiler::get_return_clif_type(fndef)
        };
        let has_return = has_return || is_enum_ret;

        let mut sig = if let Some(ref sname) = struct_ret_name {
            let mut s = super::abi::build_signature_with_return_type(
                call_conv,
                &fndef.params,
                false,
                None,
            )?;
            let fields = &self.struct_defs[sname];
            for (_fname, ftype) in fields {
                s.returns.push(cranelift_codegen::ir::AbiParam::new(*ftype));
            }
            s
        } else {
            super::abi::build_signature_with_return_type(
                call_conv,
                &fndef.params,
                has_return,
                ret_type,
            )?
        };

        // String-returning functions use two return values: (ptr, len)
        let is_str_ret = CraneliftCompiler::is_string_return(fndef);
        if is_str_ret {
            sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        }
        // Enum-returning functions use two return values: (tag, payload)
        if is_enum_ret {
            sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        }

        let func_id = self
            .module
            .declare_function(&fndef.name, Linkage::Export, &sig)
            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;

        self.functions.insert(fndef.name.clone(), func_id);
        if let Some(rt) = ret_type {
            self.fn_return_types.insert(fndef.name.clone(), rt);
        }
        if is_str_ret {
            self.fn_returns_string.insert(fndef.name.clone());
        }
        if is_enum_ret {
            self.fn_returns_enum.insert(fndef.name.clone());
        }
        // Track array return metadata
        if let Some(TypeExpr::Array {
            ref element, size, ..
        }) = fndef.return_type
        {
            let elem_type =
                clif_types::lower_type(element).unwrap_or(clif_types::default_int_type());
            self.fn_array_returns
                .insert(fndef.name.clone(), (size as usize, elem_type));
        }
        // Track heap array (Slice) return
        if matches!(fndef.return_type, Some(TypeExpr::Slice { .. })) {
            self.fn_returns_heap_array.insert(fndef.name.clone());
        }
        // Track struct return
        if let Some(sname) = struct_ret_name {
            self.fn_returns_struct.insert(fndef.name.clone(), sname);
        }
        Ok(())
    }

    /// Defines (compiles the body of) a function (second pass).
    fn define_function(&mut self, fndef: &crate::parser::ast::FnDef) -> Result<(), CodegenError> {
        let func_id = *self
            .functions
            .get(&fndef.name)
            .ok_or_else(|| CodegenError::UndefinedFunction(fndef.name.clone()))?;

        // H4: Context enforcement — reject forbidden builtins before codegen
        if let Some(ref ann) = fndef.annotation {
            let ctx_name = &ann.name;
            if ctx_name == "kernel" || ctx_name == "device" {
                let forbidden = check_context_violations(&fndef.body, ctx_name);
                if !forbidden.is_empty() {
                    return Err(CodegenError::ContextViolation(format!(
                        "@{ctx_name} fn '{}': {}",
                        fndef.name,
                        forbidden.join("; ")
                    )));
                }
            }
        }

        let is_enum_ret = self.fn_returns_enum.contains(&fndef.name);
        let has_return = !CraneliftCompiler::is_void_return(fndef) || is_enum_ret;
        let ret_type = if is_enum_ret {
            Some(clif_types::default_int_type())
        } else {
            CraneliftCompiler::get_return_clif_type(fndef)
        };
        let call_conv = self.module.target_config().default_call_conv;
        let is_struct_ret = self.fn_returns_struct.contains_key(&fndef.name);

        let mut sig = if is_struct_ret {
            let sname = &self.fn_returns_struct[&fndef.name];
            let mut s = super::abi::build_signature_with_return_type(
                call_conv,
                &fndef.params,
                false,
                None,
            )?;
            let fields = &self.struct_defs[sname];
            for (_fname, ftype) in fields {
                s.returns.push(cranelift_codegen::ir::AbiParam::new(*ftype));
            }
            s
        } else {
            super::abi::build_signature_with_return_type(
                call_conv,
                &fndef.params,
                has_return,
                ret_type,
            )?
        };
        // String-returning functions use two return values: (ptr, len)
        let is_str_ret = self.fn_returns_string.contains(&fndef.name);
        if is_str_ret {
            sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        }
        // Enum-returning functions use two return values: (tag, payload)
        if is_enum_ret {
            sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                clif_types::default_int_type(),
            ));
        }
        self.ctx.func.signature = sig;

        {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_ctx);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            // Stack canary prologue: generate and store canary for corruption detection.
            let _canary_var = if self.security_enabled {
                if let (Some(&gen_id), Some(_)) = (
                    self.functions.get("__canary_generate"),
                    self.functions.get("__canary_check"),
                ) {
                    let int_ty = clif_types::default_int_type();
                    let call_site_hash = {
                        let mut h: u64 = 0xcbf29ce484222325;
                        for b in fndef.name.bytes() {
                            h ^= b as u64;
                            h = h.wrapping_mul(0x100000001b3);
                        }
                        h as i64
                    };
                    let site_id = builder.ins().iconst(int_ty, call_site_hash);
                    let callee = self.module.declare_func_in_func(gen_id, builder.func);
                    let call = builder.ins().call(callee, &[site_id]);
                    let canary_val = builder.inst_results(call)[0];
                    let var = builder.declare_var(int_ty);
                    builder.def_var(var, canary_val);
                    Some(var)
                } else {
                    None
                }
            } else {
                None
            };

            // Profiler prologue: emit fj_rt_profile_enter(name_ptr, name_len).
            if self.profiling_enabled {
                if let Some(&enter_id) = self.functions.get("__profile_enter") {
                    let fn_name_bytes = fndef.name.as_bytes();
                    let name_data_id = {
                        let data_key = format!("__profname_{}", fndef.name);
                        if let Some(&existing) = self.string_data.get(&data_key) {
                            existing
                        } else {
                            let mut data_desc = cranelift_module::DataDescription::new();
                            data_desc.define(fn_name_bytes.to_vec().into_boxed_slice());
                            let did = self
                                .module
                                .declare_data(
                                    &data_key,
                                    cranelift_module::Linkage::Local,
                                    false,
                                    false,
                                )
                                .unwrap_or_else(|_| panic!("declare data for {data_key}"));
                            let _ = self.module.define_data(did, &data_desc);
                            self.string_data.insert(data_key, did);
                            did
                        }
                    };
                    let gv = self.module.declare_data_in_func(name_data_id, builder.func);
                    let name_ptr = builder
                        .ins()
                        .global_value(self.module.target_config().pointer_type(), gv);
                    let name_len = builder
                        .ins()
                        .iconst(clif_types::default_int_type(), fn_name_bytes.len() as i64);
                    let callee = self.module.declare_func_in_func(enter_id, builder.func);
                    builder.ins().call(callee, &[name_ptr, name_len]);
                }
            }

            let mut var_map: HashMap<String, Variable> = HashMap::new();
            let mut var_types: HashMap<String, cranelift_codegen::ir::Type> = HashMap::new();
            let mut string_lens = HashMap::new();

            // Bind function parameters to variables
            // Use a separate block_param_idx because str params consume two block params
            let mut block_param_idx = 0usize;
            for param in &fndef.params {
                let param_type =
                    clif_types::lower_type(&param.ty).unwrap_or(clif_types::default_int_type());
                let var = builder.declare_var(param_type);
                let param_val = builder.block_params(entry_block)[block_param_idx];
                builder.def_var(var, param_val);
                var_map.insert(param.name.clone(), var);
                var_types.insert(param.name.clone(), param_type);
                block_param_idx += 1;

                // String params have a second block param for the length
                if matches!(&param.ty, TypeExpr::Simple { name, .. } if name == "str") {
                    let len_var = builder.declare_var(clif_types::default_int_type());
                    let len_val = builder.block_params(entry_block)[block_param_idx];
                    builder.def_var(len_var, len_val);
                    string_lens.insert(param.name.clone(), len_var);
                    block_param_idx += 1;
                }
            }

            let mut array_meta = HashMap::new();
            let mut heap_arrays = HashSet::new();
            let mut enum_vars = HashMap::new();
            let mut struct_slots = HashMap::new();

            // Array parameter setup: copy pointer-based arrays into local stack slots
            for param in &fndef.params {
                if let TypeExpr::Array {
                    ref element, size, ..
                } = param.ty
                {
                    let elem_type =
                        clif_types::lower_type(element).unwrap_or(clif_types::default_int_type());
                    let slot =
                        builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                            (size as u32) * 8,
                            3, // 8-byte alignment
                        ));
                    let param_ptr_var = var_map[&param.name];
                    let src_ptr = builder.use_var(param_ptr_var);
                    for idx in 0..size {
                        let src_offset = builder
                            .ins()
                            .iconst(clif_types::default_int_type(), (idx as i64) * 8);
                        let src_addr = builder.ins().iadd(src_ptr, src_offset);
                        let elem_val = builder.ins().load(
                            elem_type,
                            cranelift_codegen::ir::MemFlags::new(),
                            src_addr,
                            0,
                        );
                        builder.ins().stack_store(elem_val, slot, (idx as i32) * 8);
                    }
                    array_meta.insert(param.name.clone(), (slot, size as usize));
                    var_types.insert(param.name.clone(), elem_type);
                }
                // Slice (heap array) parameters: register in heap_arrays for .push()/.len() dispatch
                if matches!(&param.ty, TypeExpr::Slice { .. }) {
                    heap_arrays.insert(param.name.clone());
                }
            }

            // Struct parameter setup: copy from pointer into local stack slot (S4.8)
            for param in &fndef.params {
                if let TypeExpr::Simple {
                    name: ref type_name,
                    ..
                } = param.ty
                {
                    if let Some(fields) = self.struct_defs.get(type_name) {
                        let num_fields = fields.len();
                        let slot = builder.create_sized_stack_slot(
                            cranelift_codegen::ir::StackSlotData::new(
                                cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                                (num_fields as u32) * 8,
                                3, // 8-byte alignment
                            ),
                        );
                        // Copy fields from pointer parameter to local stack slot
                        let param_ptr_var = var_map[&param.name];
                        let src_ptr = builder.use_var(param_ptr_var);
                        for (idx, field) in fields.iter().enumerate().take(num_fields) {
                            let field_type = field.1;
                            let src_offset = builder
                                .ins()
                                .iconst(clif_types::default_int_type(), (idx as i64) * 8);
                            let src_addr = builder.ins().iadd(src_ptr, src_offset);
                            let val = builder.ins().load(
                                field_type,
                                cranelift_codegen::ir::MemFlags::new(),
                                src_addr,
                                0,
                            );
                            builder.ins().stack_store(val, slot, (idx as i32) * 8);
                        }
                        struct_slots.insert(param.name.clone(), (slot, type_name.clone()));
                    }
                }
            }

            // Function pointer parameter setup: track signature for call_indirect
            let mut fn_ptr_sigs = HashMap::new();
            for param in &fndef.params {
                if let TypeExpr::Fn {
                    ref params,
                    ref return_type,
                    ..
                } = param.ty
                {
                    let pt: Vec<_> = params
                        .iter()
                        .map(|p| {
                            clif_types::lower_type(p).unwrap_or(clif_types::default_int_type())
                        })
                        .collect();
                    let rt = clif_types::lower_type(return_type);
                    fn_ptr_sigs.insert(param.name.clone(), (pt, rt));
                }
            }

            let impl_type_for_fn = self
                .impl_methods
                .iter()
                .find(|(_, mangled)| *mangled == &fndef.name)
                .map(|((type_name, _), _)| type_name.clone());
            let mut cx = CodegenCtx {
                module: &mut self.module,
                functions: &self.functions,
                var_map: &mut var_map,
                string_data: &mut self.string_data,
                mono_map: &self.mono_map,
                array_meta: &mut array_meta,
                last_array: None,
                loop_exit: None,
                loop_header: None,
                labeled_loops: HashMap::new(),
                const_values: HashMap::new(),
                var_types: &mut var_types,
                fn_return_types: &self.fn_return_types,
                last_expr_type: None,
                string_lens: &mut string_lens,
                last_string_len: None,
                last_string_owned: false,
                heap_arrays: &mut heap_arrays,
                heap_maps: HashSet::new(),
                map_str_values: HashSet::new(),
                last_map_new: false,
                enum_defs: &self.enum_defs,
                enum_variant_types: &self.enum_variant_types,
                generic_enum_defs: &self.generic_enum_defs,
                enum_vars: &mut enum_vars,
                last_enum_payload: None,
                last_enum_payload_type: None,
                last_enum_multi_payload: None,
                enum_multi_vars: HashMap::new(),
                struct_defs: &self.struct_defs,
                union_names: &self.union_names,
                bitfield_layouts: &self.bitfield_layouts,
                struct_slots: &mut struct_slots,
                last_struct_init: None,
                tuple_types: HashMap::new(),
                last_tuple_elem_types: None,
                impl_methods: &self.impl_methods,
                trait_defs: &self.trait_defs,
                trait_impls: &self.trait_impls,
                owned_ptrs: Vec::new(),
                scope_stack: Vec::new(),
                current_impl_type: impl_type_for_fn,
                fn_array_returns: &self.fn_array_returns,
                last_heap_array: false,
                last_split_result: None,
                split_vars: HashSet::new(),
                fn_returns_string: &self.fn_returns_string,
                fn_returns_enum: &self.fn_returns_enum,
                fn_returns_heap_array: &self.fn_returns_heap_array,
                fn_returns_closure_handle: &self.fn_returns_closure_handle,
                fn_returns_struct: &self.fn_returns_struct,
                closure_fn_map: &self.closure_fn_map,
                closure_captures: &self.closure_captures,
                fn_ptr_sigs,
                closure_span_to_fn: self.closure_span_to_fn.clone(),
                closure_handle_vars: HashSet::new(),
                last_closure_handle: false,
                current_module: self.module_fns.get(&fndef.name).cloned(),
                thread_handles: HashSet::new(),
                last_thread_spawn: false,
                mutex_handles: HashSet::new(),
                last_mutex_new: false,
                channel_handles: HashSet::new(),
                last_channel_new: false,
                atomic_handles: HashSet::new(),
                last_atomic_new: false,
                last_atomic_subtype: "i64".to_string(),
                atomic_subtypes: std::collections::HashMap::new(),
                rwlock_handles: HashSet::new(),
                last_rwlock_new: false,
                barrier_handles: HashSet::new(),
                mutex_guard_handles: HashSet::new(),
                last_mutex_guard_new: false,
                condvar_handles: HashSet::new(),
                last_condvar_new: false,
                bounded_channel_handles: HashSet::new(),
                last_bounded_channel: false,
                last_barrier_new: false,
                arc_handles: HashSet::new(),
                last_arc_new: false,
                generic_fn_params: self.generic_fn_params.clone(),
                _async_fns: &self.async_fns,
                _future_handles: HashSet::new(),
                last_future_new: false,
                no_std: self.no_std,
                panic_handler_fn: self.panic_handler_fn.clone(),
                volatile_ptr_handles: HashSet::new(),
                last_volatile_ptr_new: false,
                mmio_regions: HashMap::new(),
                last_mmio_new: false,
                last_mmio_vals: None,
                bump_alloc_handles: HashSet::new(),
                last_bump_alloc_new: false,
                freelist_alloc_handles: HashSet::new(),
                last_freelist_alloc_new: false,
                pool_alloc_handles: HashSet::new(),
                last_pool_alloc_new: false,
                executor_handles: HashSet::new(),
                last_executor_new: false,
                waker_handles: HashSet::new(),
                last_waker_new: false,
                timer_handles: HashSet::new(),
                last_timer_new: false,
                threadpool_handles: HashSet::new(),
                last_threadpool_new: false,
                joinhandle_handles: HashSet::new(),
                last_joinhandle_new: false,
                async_channel_handles: HashSet::new(),
                last_async_channel_new: false,
                async_bchannel_handles: HashSet::new(),
                last_async_bchannel_new: false,
                stream_handles: HashSet::new(),
                last_stream_new: false,
                simd_f32x4_handles: HashSet::new(),
                last_simd_f32x4_new: false,
                simd_i32x4_handles: HashSet::new(),
                last_simd_i32x4_new: false,
                simd_f32x8_handles: HashSet::new(),
                last_simd_f32x8_new: false,
                simd_i32x8_handles: HashSet::new(),
                last_simd_i32x8_new: false,
                onnx_handles: HashSet::new(),
                last_onnx_new: false,
                async_io_handles: HashSet::new(),
                last_async_io_new: false,
                last_heap_array_return: false,
                fn_ret_type: ret_type,
                is_enum_return_fn: self.fn_returns_enum.contains(&fndef.name),
                current_context: fndef.annotation.as_ref().map(|a| a.name.clone()),
                security_enabled: self.security_enabled,
            };

            // Inject top-level const definitions as variables.
            // Try compile-time constant folding first for integer expressions.
            // Build const fn ref table for this scope
            let const_fn_refs: HashMap<String, &FnDef> = self
                .const_fn_defs
                .iter()
                .map(|(k, v)| (k.clone(), v))
                .collect();
            for (cname, cexpr, cty) in &self.const_defs {
                let const_folded =
                    compile::try_const_eval_with_fns(cexpr, &cx.const_values, &const_fn_refs, 0);
                let val = if let Some(cv) = const_folded {
                    builder.ins().iconst(clif_types::default_int_type(), cv)
                } else if let Ok(v) = compile_expr(&mut builder, &mut cx, cexpr) {
                    v
                } else {
                    continue;
                };
                if let Some(cv) = const_folded {
                    cx.const_values.insert(cname.clone(), cv);
                }
                let var_type = clif_types::lower_type(cty)
                    .unwrap_or(cx.last_expr_type.unwrap_or(clif_types::default_int_type()));
                let var = builder.declare_var(var_type);
                builder.def_var(var, val);
                cx.var_map.insert(cname.clone(), var);
                cx.var_types.insert(cname.clone(), var_type);
                if let Some(len_val) = cx.last_string_len.take() {
                    let len_var = builder.declare_var(clif_types::default_int_type());
                    builder.def_var(len_var, len_val);
                    cx.string_lens.insert(cname.clone(), len_var);
                }
            }
            // Check if this function returns an array (need heap copy for callee stack safety)
            let array_return_info = self.fn_array_returns.get(&fndef.name).copied();

            let is_async_fn = fndef.is_async;
            let compile_result = compile_expr(&mut builder, &mut cx, &fndef.body);
            match compile_result {
                Ok(result) => {
                    if is_async_fn {
                        // Async function: wrap body result in a future handle
                        let new_id = *cx.functions.get("__future_new").ok_or_else(|| {
                            CodegenError::Internal("__future_new not declared".into())
                        })?;
                        let new_callee = cx.module.declare_func_in_func(new_id, builder.func);
                        let new_call = builder.ins().call(new_callee, &[]);
                        let future_ptr = builder.inst_results(new_call)[0];

                        let set_id = *cx.functions.get("__future_set_result").ok_or_else(|| {
                            CodegenError::Internal("__future_set_result not declared".into())
                        })?;
                        let set_callee = cx.module.declare_func_in_func(set_id, builder.func);
                        builder.ins().call(set_callee, &[future_ptr, result]);

                        emit_owned_cleanup(&mut builder, &mut cx, Some(future_ptr))?;
                        builder.ins().return_(&[future_ptr]);
                    } else if is_struct_ret {
                        // Struct return: load each field and return as multi-value
                        let sname = &self.fn_returns_struct[&fndef.name];
                        let fields = self.struct_defs[sname].clone();
                        if let Some((slot, _)) = cx.last_struct_init.take() {
                            let mut ret_vals = Vec::new();
                            for (i, (_fname, ftype)) in fields.iter().enumerate() {
                                let val = builder.ins().stack_load(*ftype, slot, (i as i32) * 8);
                                ret_vals.push(val);
                            }
                            emit_owned_cleanup(&mut builder, &mut cx, None)?;
                            builder.ins().return_(&ret_vals);
                        } else {
                            let mut ret_vals = Vec::new();
                            for (i, (_fname, ftype)) in fields.iter().enumerate() {
                                let val = builder.ins().load(
                                    *ftype,
                                    cranelift_codegen::ir::MemFlags::new(),
                                    result,
                                    (i as i32) * 8,
                                );
                                ret_vals.push(val);
                            }
                            emit_owned_cleanup(&mut builder, &mut cx, None)?;
                            builder.ins().return_(&ret_vals);
                        }
                    } else if has_return {
                        let ret_val = if let Some((arr_len, elem_type)) = array_return_info {
                            // Array return: copy stack elements to heap buffer
                            let total_bytes = builder
                                .ins()
                                .iconst(clif_types::default_int_type(), (arr_len as i64) * 8);
                            let alloc_id = *cx.functions.get("__alloc").ok_or_else(|| {
                                CodegenError::Internal("__alloc not declared".into())
                            })?;
                            let local_alloc =
                                cx.module.declare_func_in_func(alloc_id, builder.func);
                            let alloc_call = builder.ins().call(local_alloc, &[total_bytes]);
                            let heap_ptr = builder.inst_results(alloc_call)[0];
                            for idx in 0..arr_len {
                                let offset = builder
                                    .ins()
                                    .iconst(clif_types::default_int_type(), (idx as i64) * 8);
                                let src_addr = builder.ins().iadd(result, offset);
                                let elem_val = builder.ins().load(
                                    elem_type,
                                    cranelift_codegen::ir::MemFlags::new(),
                                    src_addr,
                                    0,
                                );
                                let dst_addr = builder.ins().iadd(heap_ptr, offset);
                                builder.ins().store(
                                    cranelift_codegen::ir::MemFlags::new(),
                                    elem_val,
                                    dst_addr,
                                    0,
                                );
                            }
                            heap_ptr
                        } else {
                            result
                        };
                        emit_owned_cleanup(&mut builder, &mut cx, Some(ret_val))?;
                        if is_str_ret {
                            let len_val = cx.last_string_len.take().unwrap_or_else(|| {
                                builder.ins().iconst(clif_types::default_int_type(), 0)
                            });
                            builder.ins().return_(&[ret_val, len_val]);
                        } else if cx.is_enum_return_fn {
                            let payload = cx.last_enum_payload.take().unwrap_or_else(|| {
                                builder.ins().iconst(clif_types::default_int_type(), 0)
                            });
                            cx.last_enum_payload_type.take();
                            builder.ins().return_(&[ret_val, payload]);
                        } else {
                            let ret_val = coerce_ret(&mut builder, ret_val, ret_type);
                            builder.ins().return_(&[ret_val]);
                        }
                    } else {
                        emit_owned_cleanup(&mut builder, &mut cx, None)?;
                        builder.ins().return_(&[]);
                    }
                    builder.finalize();
                }
                Err(e) => {
                    if !builder.is_unreachable() {
                        builder.ins().trap(
                            cranelift_codegen::ir::TrapCode::user(1).expect("trap code 1 is valid"),
                        );
                    }
                    builder.finalize();
                    self.module.clear_context(&mut self.ctx);
                    return Err(e);
                }
            }
        }

        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| {
                CodegenError::FunctionError(format!("Compilation error in '{}': {e}", fndef.name))
            })?;

        self.module.clear_context(&mut self.ctx);
        Ok(())
    }

    /// Finishes compilation and returns the object product.
    pub fn finish(self) -> ObjectProduct {
        self.module.finish()
    }

    /// Returns the function section annotations collected during compilation.
    pub fn fn_sections(&self) -> &HashMap<String, String> {
        &self.fn_sections
    }

    /// Returns the data section annotations collected during compilation.
    pub fn data_sections(&self) -> &HashMap<String, String> {
        &self.data_sections
    }

    /// Returns the collected global assembly sections (V27.5 P1.3a).
    /// ISR wrappers for @interrupt functions are emitted here.
    pub fn global_asm_sections(&self) -> &[String] {
        &self.global_asm_sections
    }

    /// Returns the names of `@interrupt`-annotated functions (V27.5 P1.3a).
    pub fn interrupt_functions(&self) -> &[String] {
        &self.interrupt_fns
    }
}

/// Scan an expression tree for `fn_addr("name")` calls and collect the target
/// function names. Used to prevent DCE from removing functions referenced by
/// `fn_addr` (which passes names as string literals, invisible to call-graph analysis).
pub(crate) fn scan_fn_addr_targets(expr: &Expr, targets: &mut Vec<String>) {
    match expr {
        Expr::Call { callee, args, .. } => {
            if let Expr::Ident { name, .. } = callee.as_ref() {
                if name == "fn_addr" && !args.is_empty() {
                    match &args[0].value {
                        Expr::Ident { name: target, .. } => {
                            targets.push(target.clone());
                        }
                        Expr::Literal {
                            kind: LiteralKind::String(s),
                            ..
                        } => {
                            targets.push(s.clone());
                        }
                        _ => {}
                    }
                }
            }
            scan_fn_addr_targets(callee, targets);
            for arg in args {
                scan_fn_addr_targets(&arg.value, targets);
            }
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                scan_fn_addr_targets_stmt(stmt, targets);
            }
            if let Some(tail) = expr {
                scan_fn_addr_targets(tail, targets);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            scan_fn_addr_targets(condition, targets);
            scan_fn_addr_targets(then_branch, targets);
            if let Some(eb) = else_branch {
                scan_fn_addr_targets(eb, targets);
            }
        }
        _ => {}
    }
}

fn scan_fn_addr_targets_stmt(stmt: &Stmt, targets: &mut Vec<String>) {
    match stmt {
        Stmt::Expr { expr, .. } => {
            scan_fn_addr_targets(expr, targets);
        }
        Stmt::Let { value, .. } => {
            scan_fn_addr_targets(value, targets);
        }
        _ => {}
    }
}
