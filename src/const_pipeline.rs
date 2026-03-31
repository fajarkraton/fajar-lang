//! Const pipeline integration — wires const generics, const fn, const traits,
//! const alloc, const reflection, const macros, and const stdlib into the
//! Fajar Lang compilation pipeline (analyzer → codegen → interpreter → LSP → REPL).
//!
//! This module is the central coordinator for all compile-time evaluation features.
//!
//! # Pipeline Flow
//!
//! ```text
//! Source → Parser → [const params detected] → Analyzer (K9.1)
//!                                                ↓
//!                                    ComptimeEvaluator + ConstStdlib (K9.7/K9.8)
//!                                                ↓
//!                               ┌────────────────┼────────────────┐
//!                               ↓                ↓                ↓
//!                        Cranelift (K9.2)   LLVM (K9.3)    VM (K9.4)
//!                        iconst.i64 N       i64 N const    const pool
//!                               ↓                ↓                ↓
//!                               └────────────────┼────────────────┘
//!                                                ↓
//!                                         LSP (K9.5/K9.6)
//!                                    hover: const N = 42
//!                                    completion: const fns only
//! ```

use std::collections::HashMap;

use crate::analyzer::comptime::{ComptimeEvaluator, ComptimeValue};
use crate::const_alloc::{ConstAllocRegistry, TargetInfo};
use crate::const_macros::ConstMacroEvaluator;
use crate::const_reflect::TypeMetaRegistry;
use crate::const_stdlib;
use crate::const_traits::ConstTraitRegistry;

// ═══════════════════════════════════════════════════════════════════════
// K9.1: Analyzer Integration
// ═══════════════════════════════════════════════════════════════════════

/// The const evaluation context — carries all compile-time state through the pipeline.
pub struct ConstContext {
    /// Compile-time evaluator for `comptime {}` and `const fn`.
    pub evaluator: ComptimeEvaluator,
    /// Const trait registry.
    pub traits: ConstTraitRegistry,
    /// Type metadata registry for reflection.
    pub type_meta: TypeMetaRegistry,
    /// Const macro evaluator.
    pub macros: ConstMacroEvaluator,
    /// Const allocation registry for `.rodata` emission.
    pub allocs: ConstAllocRegistry,
    /// Evaluated const values: name → value.
    pub const_values: HashMap<String, ComptimeValue>,
    /// Const fn names (for validation).
    pub const_fns: Vec<String>,
}

impl ConstContext {
    /// Creates a new const context for a compilation unit.
    pub fn new(project_dir: &str, target: TargetInfo) -> Self {
        Self {
            evaluator: ComptimeEvaluator::new(),
            traits: ConstTraitRegistry::new(),
            type_meta: TypeMetaRegistry::new(),
            macros: ConstMacroEvaluator::new(project_dir),
            allocs: ConstAllocRegistry::new(target),
            const_values: HashMap::new(),
            const_fns: Vec::new(),
        }
    }

    /// Creates a context with default settings (x86_64, current directory).
    pub fn default_context() -> Self {
        Self::new(".", TargetInfo::x86_64())
    }

    /// K9.1: Register a const value (from `const X = comptime { ... }`).
    pub fn register_const(&mut self, name: &str, value: ComptimeValue) {
        // Store in const values map
        self.const_values.insert(name.to_string(), value.clone());
        // Also register as static allocation
        self.allocs.register(name, &value);
    }

    /// K9.1: Register a const fn for validation.
    pub fn register_const_fn(&mut self, name: &str) {
        self.const_fns.push(name.to_string());
    }

    /// K9.1: Look up a const value by name.
    pub fn get_const(&self, name: &str) -> Option<&ComptimeValue> {
        self.const_values.get(name)
    }

    /// K9.7: Evaluate a const expression (dispatches to evaluator + stdlib + macros).
    pub fn eval_const_call(&mut self, name: &str, args: &[ComptimeValue]) -> Option<ComptimeValue> {
        // Try const stdlib first
        if let Some(result) = const_stdlib::eval_const_stdlib(name, args) {
            return Some(result);
        }

        // Try const macros
        if let Some(result) = self.macros.eval(name, args) {
            return result.ok();
        }

        // Try reflection intrinsics
        if let Some(ComptimeValue::Str(type_name)) = args.first() {
            let trait_arg = args.get(1).and_then(|v| {
                if let ComptimeValue::Str(s) = v {
                    Some(s.as_str())
                } else {
                    None
                }
            });
            if let Some(result) = self.type_meta.eval_intrinsic(name, type_name, trait_arg) {
                return Some(result);
            }
        }

        None
    }

    // ═══════════════════════════════════════════════════════════════════
    // K9.2 / K9.3 / K9.4: Backend Integration Helpers
    // ═══════════════════════════════════════════════════════════════════

    /// K9.2: Get a const value as an i64 immediate for Cranelift codegen.
    pub fn as_cranelift_immediate(&self, name: &str) -> Option<i64> {
        self.const_values.get(name).and_then(|v| v.as_int())
    }

    /// K9.3: Get a const value as an LLVM constant representation.
    pub fn as_llvm_const(&self, name: &str) -> Option<LlvmConst> {
        self.const_values.get(name).map(|v| match v {
            ComptimeValue::Int(n) => LlvmConst::Int64(*n),
            ComptimeValue::Float(f) => LlvmConst::Float64(*f),
            ComptimeValue::Bool(b) => LlvmConst::Int1(*b),
            ComptimeValue::Str(s) => LlvmConst::GlobalString(s.clone()),
            _ => LlvmConst::Aggregate(format!("{v}")),
        })
    }

    /// K9.4: Get a const value for the VM constant pool.
    pub fn as_vm_constant(&self, name: &str) -> Option<VmConstant> {
        self.const_values.get(name).map(|v| match v {
            ComptimeValue::Int(n) => VmConstant::Int(*n),
            ComptimeValue::Float(f) => VmConstant::Float(*f),
            ComptimeValue::Bool(b) => VmConstant::Bool(*b),
            ComptimeValue::Str(s) => VmConstant::Str(s.clone()),
            ComptimeValue::Array(items) => VmConstant::Array(items.len()),
            _ => VmConstant::Null,
        })
    }

    // ═══════════════════════════════════════════════════════════════════
    // K9.5 / K9.6: LSP Integration
    // ═══════════════════════════════════════════════════════════════════

    /// K9.5: Get hover info for a const value (shown in LSP).
    pub fn hover_info(&self, name: &str) -> Option<String> {
        self.const_values.get(name).map(|v| {
            let type_desc = match v {
                ComptimeValue::Int(_) => "i64",
                ComptimeValue::Float(_) => "f64",
                ComptimeValue::Bool(_) => "bool",
                ComptimeValue::Str(_) => "str",
                ComptimeValue::Array(items) => {
                    if items.is_empty() {
                        "[]"
                    } else {
                        match &items[0] {
                            ComptimeValue::Int(_) => "[i64; ...]",
                            ComptimeValue::Float(_) => "[f64; ...]",
                            _ => "[...]",
                        }
                    }
                }
                ComptimeValue::Struct { name: sn, .. } => sn.as_str(),
                _ => "comptime",
            };
            format!("const {name}: {type_desc} = {v}")
        })
    }

    /// K9.6: Get completion candidates for const context (only const fns).
    pub fn const_completions(&self) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // Const fn names
        for name in &self.const_fns {
            items.push(CompletionItem {
                label: name.clone(),
                kind: CompletionKind::ConstFn,
                detail: "const fn".to_string(),
            });
        }

        // Const stdlib functions
        for name in const_stdlib::known_const_stdlib_functions() {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: CompletionKind::ConstBuiltin,
                detail: "const builtin".to_string(),
            });
        }

        // Const macros
        for name in ConstMacroEvaluator::known_macros() {
            items.push(CompletionItem {
                label: format!("{name}!"),
                kind: CompletionKind::ConstMacro,
                detail: "const macro".to_string(),
            });
        }

        // Const values
        for (name, val) in &self.const_values {
            items.push(CompletionItem {
                label: name.clone(),
                kind: CompletionKind::ConstValue,
                detail: format!("const = {val}"),
            });
        }

        items
    }

    // ═══════════════════════════════════════════════════════════════════
    // K9.7: Error Messages
    // ═══════════════════════════════════════════════════════════════════

    /// Generate a clear error message for const evaluation failures.
    pub fn format_const_error(fn_name: &str, callee: &str) -> String {
        format!(
            "cannot call non-const fn '{}' in const context (const fn '{}'). \
             Only const fn, const builtins, and const macros can be called at compile time.",
            callee, fn_name
        )
    }

    // ═══════════════════════════════════════════════════════════════════
    // K9.8: REPL Support
    // ═══════════════════════════════════════════════════════════════════

    /// Evaluate a `comptime { ... }` expression in REPL mode.
    pub fn repl_eval(&mut self, value: &ComptimeValue) -> String {
        format!("comptime = {value}")
    }

    /// Summary stats for the const pipeline.
    pub fn stats(&self) -> ConstPipelineStats {
        ConstPipelineStats {
            const_values: self.const_values.len(),
            const_fns: self.const_fns.len(),
            const_traits: self.traits.traits.len(),
            const_allocs: self.allocs.total_count(),
            alloc_bytes: self.allocs.total_bytes(),
            type_metas: self.type_meta.get("i64").is_some() as usize, // just check builtins exist
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Backend types
// ═══════════════════════════════════════════════════════════════════════

/// LLVM constant representation.
#[derive(Debug, Clone, PartialEq)]
pub enum LlvmConst {
    Int64(i64),
    Float64(f64),
    Int1(bool),
    GlobalString(String),
    Aggregate(String),
}

/// VM constant pool entry.
#[derive(Debug, Clone, PartialEq)]
pub enum VmConstant {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Array(usize),
    Null,
}

/// LSP completion item.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
}

/// Kind of completion item.
#[derive(Debug, Clone, PartialEq)]
pub enum CompletionKind {
    ConstFn,
    ConstBuiltin,
    ConstMacro,
    ConstValue,
}

/// Pipeline statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstPipelineStats {
    pub const_values: usize,
    pub const_fns: usize,
    pub const_traits: usize,
    pub const_allocs: usize,
    pub alloc_bytes: usize,
    pub type_metas: usize,
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — K9.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ConstContext {
        ConstContext::default_context()
    }

    // ── K9.1: Analyzer integration ──

    #[test]
    fn k9_1_register_and_lookup_const() {
        let mut c = ctx();
        c.register_const("PI", ComptimeValue::Float(3.14159));
        c.register_const("MAX", ComptimeValue::Int(1024));

        assert_eq!(c.get_const("PI"), Some(&ComptimeValue::Float(3.14159)));
        assert_eq!(c.get_const("MAX"), Some(&ComptimeValue::Int(1024)));
        assert_eq!(c.get_const("MISSING"), None);
    }

    #[test]
    fn k9_1_register_const_fn() {
        let mut c = ctx();
        c.register_const_fn("factorial");
        c.register_const_fn("fibonacci");
        assert_eq!(c.const_fns.len(), 2);
    }

    // ── K9.2: Cranelift integration ──

    #[test]
    fn k9_2_cranelift_immediate() {
        let mut c = ctx();
        c.register_const("X", ComptimeValue::Int(42));
        assert_eq!(c.as_cranelift_immediate("X"), Some(42));
        assert_eq!(c.as_cranelift_immediate("MISSING"), None);
    }

    // ── K9.3: LLVM integration ──

    #[test]
    fn k9_3_llvm_const() {
        let mut c = ctx();
        c.register_const("N", ComptimeValue::Int(100));
        c.register_const("PI", ComptimeValue::Float(3.14));
        c.register_const("FLAG", ComptimeValue::Bool(true));
        c.register_const("MSG", ComptimeValue::Str("hello".into()));

        assert_eq!(c.as_llvm_const("N"), Some(LlvmConst::Int64(100)));
        assert_eq!(c.as_llvm_const("PI"), Some(LlvmConst::Float64(3.14)));
        assert_eq!(c.as_llvm_const("FLAG"), Some(LlvmConst::Int1(true)));
        assert_eq!(
            c.as_llvm_const("MSG"),
            Some(LlvmConst::GlobalString("hello".into()))
        );
    }

    // ── K9.4: VM integration ──

    #[test]
    fn k9_4_vm_constant() {
        let mut c = ctx();
        c.register_const("X", ComptimeValue::Int(42));
        c.register_const(
            "ARR",
            ComptimeValue::Array(vec![ComptimeValue::Int(1), ComptimeValue::Int(2)]),
        );

        assert_eq!(c.as_vm_constant("X"), Some(VmConstant::Int(42)));
        assert_eq!(c.as_vm_constant("ARR"), Some(VmConstant::Array(2)));
    }

    // ── K9.5: LSP hover ──

    #[test]
    fn k9_5_hover_info() {
        let mut c = ctx();
        c.register_const("MAX_SIZE", ComptimeValue::Int(4096));
        let hover = c.hover_info("MAX_SIZE").unwrap();
        assert!(hover.contains("const MAX_SIZE"));
        assert!(hover.contains("4096"));
    }

    #[test]
    fn k9_5_hover_struct() {
        let mut c = ctx();
        c.register_const(
            "ORIGIN",
            ComptimeValue::Struct {
                name: "Point".into(),
                fields: vec![
                    ("x".into(), ComptimeValue::Float(0.0)),
                    ("y".into(), ComptimeValue::Float(0.0)),
                ],
            },
        );
        let hover = c.hover_info("ORIGIN").unwrap();
        assert!(hover.contains("Point"));
    }

    // ── K9.6: LSP completion ──

    #[test]
    fn k9_6_completions_include_all_kinds() {
        let mut c = ctx();
        c.register_const_fn("my_const_fn");
        c.register_const("MY_CONST", ComptimeValue::Int(1));

        let completions = c.const_completions();

        // Should have const fns
        assert!(completions.iter().any(|i| i.label == "my_const_fn"));
        // Should have const builtins (e.g., abs)
        assert!(completions.iter().any(|i| i.label == "abs"));
        // Should have const macros (e.g., static_assert!)
        assert!(completions.iter().any(|i| i.label == "static_assert!"));
        // Should have const values
        assert!(completions.iter().any(|i| i.label == "MY_CONST"));
    }

    // ── K9.7: Error messages ──

    #[test]
    fn k9_7_error_message() {
        let msg = ConstContext::format_const_error("compute_table", "read_file");
        assert!(msg.contains("read_file"));
        assert!(msg.contains("const context"));
        assert!(msg.contains("compute_table"));
    }

    // ── K9.8: REPL support ──

    #[test]
    fn k9_8_repl_eval() {
        let mut c = ctx();
        let output = c.repl_eval(&ComptimeValue::Int(42));
        assert_eq!(output, "comptime = 42");
    }

    // ── K9.9: Documentation (verified by compiling book chapter) ──

    #[test]
    fn k9_9_const_pipeline_stats() {
        let mut c = ctx();
        c.register_const("A", ComptimeValue::Int(1));
        c.register_const("B", ComptimeValue::Int(2));
        c.register_const_fn("my_fn");

        let stats = c.stats();
        assert_eq!(stats.const_values, 2);
        assert_eq!(stats.const_fns, 1);
        assert!(stats.const_traits >= 5); // built-in const traits
        assert_eq!(stats.const_allocs, 2);
        assert!(stats.alloc_bytes > 0);
    }

    // ── K9.10: Integration — eval_const_call dispatcher ──

    #[test]
    fn k9_10_eval_const_call_stdlib() {
        let mut c = ctx();
        let result = c.eval_const_call("abs", &[ComptimeValue::Int(-7)]);
        assert_eq!(result, Some(ComptimeValue::Int(7)));
    }

    #[test]
    fn k9_10_eval_const_call_macro() {
        let mut c = ctx();
        let result = c.eval_const_call(
            "concat",
            &[ComptimeValue::Str("v".into()), ComptimeValue::Int(13)],
        );
        assert_eq!(result, Some(ComptimeValue::Str("v13".into())));
    }

    #[test]
    fn k9_10_eval_const_call_reflection() {
        let mut c = ctx();
        let result = c.eval_const_call("size_of", &[ComptimeValue::Str("i64".into())]);
        assert_eq!(result, Some(ComptimeValue::Int(8)));
    }

    #[test]
    fn k9_10_eval_const_call_unknown() {
        let mut c = ctx();
        let result = c.eval_const_call("nonexistent", &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn k9_10_full_pipeline_flow() {
        let mut c = ctx();

        // Register const fns
        c.register_const_fn("compute_table");

        // Register consts (as if evaluator produced them)
        c.register_const("TABLE_SIZE", ComptimeValue::Int(256));
        c.register_const(
            "LOOKUP",
            ComptimeValue::Array((0..10).map(|i| ComptimeValue::Int(i * i)).collect()),
        );

        // Verify Cranelift can get immediates
        assert_eq!(c.as_cranelift_immediate("TABLE_SIZE"), Some(256));

        // Verify LLVM can get constants
        assert_eq!(c.as_llvm_const("TABLE_SIZE"), Some(LlvmConst::Int64(256)));

        // Verify VM can get constants
        assert_eq!(c.as_vm_constant("TABLE_SIZE"), Some(VmConstant::Int(256)));

        // Verify LSP hover
        let hover = c.hover_info("TABLE_SIZE").unwrap();
        assert!(hover.contains("256"));

        // Verify completions
        let completions = c.const_completions();
        assert!(completions.iter().any(|i| i.label == "compute_table"));
        assert!(completions.iter().any(|i| i.label == "TABLE_SIZE"));

        // Verify allocs
        assert_eq!(c.allocs.total_count(), 2);
        assert!(c.allocs.total_bytes() > 0);

        // Full stats
        let stats = c.stats();
        assert_eq!(stats.const_values, 2);
        assert_eq!(stats.const_fns, 1);
    }
}
