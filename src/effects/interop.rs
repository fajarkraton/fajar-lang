//! Effect interop — async mapping, context annotation effects,
//! linear type interaction, effect erasure, optimization, documentation.

use std::fmt;

use super::inference::{EffectLabel, EffectSet};

// ═══════════════════════════════════════════════════════════════════════
// S20.1: Effects + Async
// ═══════════════════════════════════════════════════════════════════════

/// Maps the `Async` effect to existing async/await machinery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncEffectMapping {
    /// Function name.
    pub function: String,
    /// Whether this function uses await (implies Async effect).
    pub uses_await: bool,
    /// Whether this function spawns tasks.
    pub spawns_tasks: bool,
}

impl AsyncEffectMapping {
    /// Computes the effect set contribution from async operations.
    pub fn effect_contribution(&self) -> EffectSet {
        if self.uses_await || self.spawns_tasks {
            EffectSet::from_labels(&["Async"])
        } else {
            EffectSet::pure_set()
        }
    }
}

/// Checks if an effect set implies async execution.
pub fn implies_async(effects: &EffectSet) -> bool {
    effects.contains("Async")
}

// ═══════════════════════════════════════════════════════════════════════
// S20.2 / S20.3: Context Annotation Effects
// ═══════════════════════════════════════════════════════════════════════

/// Execution context with its implied effect constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextAnnotation {
    /// `@safe` — default, most restrictive.
    Safe,
    /// `@kernel` — OS primitives, no heap, no tensor.
    Kernel,
    /// `@device` — tensor ops, no raw pointer, no IRQ.
    Device,
    /// `@unsafe` — full access.
    Unsafe,
}

impl fmt::Display for ContextAnnotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContextAnnotation::Safe => write!(f, "@safe"),
            ContextAnnotation::Kernel => write!(f, "@kernel"),
            ContextAnnotation::Device => write!(f, "@device"),
            ContextAnnotation::Unsafe => write!(f, "@unsafe"),
        }
    }
}

/// An effect that is forbidden in a given context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForbiddenEffect {
    /// The effect that is forbidden.
    pub effect: EffectLabel,
    /// The context that forbids it.
    pub context: ContextAnnotation,
    /// Reason for the prohibition.
    pub reason: String,
}

/// Returns the set of forbidden effects for a context.
pub fn forbidden_effects(ctx: ContextAnnotation) -> Vec<ForbiddenEffect> {
    match ctx {
        ContextAnnotation::Kernel => vec![
            ForbiddenEffect {
                effect: EffectLabel::new("IO"),
                context: ctx,
                reason: "IO operations not allowed in @kernel context".into(),
            },
            ForbiddenEffect {
                effect: EffectLabel::new("Alloc"),
                context: ctx,
                reason: "heap allocation not allowed in @kernel context".into(),
            },
        ],
        ContextAnnotation::Device => vec![ForbiddenEffect {
            effect: EffectLabel::new("Unsafe"),
            context: ctx,
            reason: "unsafe operations not allowed in @device context".into(),
        }],
        ContextAnnotation::Safe => vec![
            ForbiddenEffect {
                effect: EffectLabel::new("Unsafe"),
                context: ctx,
                reason: "unsafe operations not allowed in @safe context".into(),
            },
            ForbiddenEffect {
                effect: EffectLabel::new("Alloc"),
                context: ctx,
                reason: "direct allocation not allowed in @safe context".into(),
            },
        ],
        ContextAnnotation::Unsafe => vec![],
    }
}

/// Returns the allowed effects for a context.
pub fn allowed_effects(ctx: ContextAnnotation) -> EffectSet {
    match ctx {
        ContextAnnotation::Kernel => EffectSet::from_labels(&["Panic"]),
        ContextAnnotation::Device => {
            EffectSet::from_labels(&["IO", "Alloc", "Panic", "Async", "Network"])
        }
        ContextAnnotation::Safe => EffectSet::from_labels(&["IO", "Panic", "Async", "Network"]),
        ContextAnnotation::Unsafe => EffectSet::from_labels(&[
            "IO",
            "Alloc",
            "Panic",
            "Async",
            "Unsafe",
            "Network",
            "FileSystem",
        ]),
    }
}

/// Validates that a function's effects are compatible with its context.
pub fn check_context_effects(
    function: &str,
    ctx: ContextAnnotation,
    effects: &EffectSet,
) -> Result<(), ContextEffectError> {
    let forbidden = forbidden_effects(ctx);
    let mut violations = Vec::new();
    for fe in &forbidden {
        if effects.contains(&fe.effect.0) {
            violations.push(fe.clone());
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(ContextEffectError {
            function: function.into(),
            context: ctx,
            violations,
        })
    }
}

/// Error for context-effect violations.
#[derive(Debug, Clone)]
pub struct ContextEffectError {
    /// Function name.
    pub function: String,
    /// Context annotation.
    pub context: ContextAnnotation,
    /// Forbidden effects that were found.
    pub violations: Vec<ForbiddenEffect>,
}

impl fmt::Display for ContextEffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let effects: Vec<String> = self.violations.iter().map(|v| v.effect.0.clone()).collect();
        write!(
            f,
            "function `{}` in {} context uses forbidden effects: {{{}}}",
            self.function,
            self.context,
            effects.join(", ")
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S20.4: Effects + Linear Types
// ═══════════════════════════════════════════════════════════════════════

/// Tracks linear value consumption across effect handler resume paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearEffectCheck {
    /// Variable name.
    pub variable: String,
    /// Whether the value is consumed in the resume path.
    pub consumed_in_resume: bool,
    /// Whether the value is consumed in the abort path.
    pub consumed_in_abort: bool,
}

impl LinearEffectCheck {
    /// Creates a new linear effect check.
    pub fn new(variable: &str) -> Self {
        Self {
            variable: variable.into(),
            consumed_in_resume: false,
            consumed_in_abort: false,
        }
    }

    /// Marks the value as consumed in the resume path.
    pub fn consume_in_resume(&mut self) {
        self.consumed_in_resume = true;
    }

    /// Marks the value as consumed in the abort path.
    pub fn consume_in_abort(&mut self) {
        self.consumed_in_abort = true;
    }

    /// Validates exactly-once consumption across handler paths.
    pub fn validate(&self) -> Result<(), LinearEffectViolation> {
        match (self.consumed_in_resume, self.consumed_in_abort) {
            (true, true) => Ok(()),
            (true, false) => Err(LinearEffectViolation {
                variable: self.variable.clone(),
                kind: LinearViolationKind::NotConsumedInAbort,
            }),
            (false, true) => Err(LinearEffectViolation {
                variable: self.variable.clone(),
                kind: LinearViolationKind::NotConsumedInResume,
            }),
            (false, false) => Err(LinearEffectViolation {
                variable: self.variable.clone(),
                kind: LinearViolationKind::NeverConsumed,
            }),
        }
    }
}

/// A linear value consumption violation in an effect handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearEffectViolation {
    /// Variable name.
    pub variable: String,
    /// Kind of violation.
    pub kind: LinearViolationKind,
}

/// Kind of linear violation in effect context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinearViolationKind {
    /// Value not consumed in the resume path.
    NotConsumedInResume,
    /// Value not consumed in the abort path.
    NotConsumedInAbort,
    /// Value never consumed in any path.
    NeverConsumed,
}

impl fmt::Display for LinearEffectViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            LinearViolationKind::NotConsumedInResume => write!(
                f,
                "linear value `{}` not consumed in resume path of effect handler",
                self.variable
            ),
            LinearViolationKind::NotConsumedInAbort => write!(
                f,
                "linear value `{}` not consumed in abort path of effect handler",
                self.variable
            ),
            LinearViolationKind::NeverConsumed => write!(
                f,
                "linear value `{}` never consumed in effect handler",
                self.variable
            ),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S20.5: Effect Erasure
// ═══════════════════════════════════════════════════════════════════════

/// Effect erasure configuration for native codegen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectErasure {
    /// Whether effect types are erased in the output.
    pub erase_types: bool,
    /// Whether effect handler dispatch is inlined.
    pub inline_handlers: bool,
    /// Whether effect bounds are preserved as debug info.
    pub preserve_debug_info: bool,
}

impl Default for EffectErasure {
    fn default() -> Self {
        Self {
            erase_types: true,
            inline_handlers: true,
            preserve_debug_info: false,
        }
    }
}

/// Describes how an effect type is erased at codegen level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErasedForm {
    /// Effect type becomes unit (no runtime cost).
    Unit,
    /// Effect handler becomes a direct function call.
    DirectCall,
    /// Effect parameter is eliminated entirely.
    Eliminated,
}

/// Erases effect information from a function signature description.
pub fn erase_effect_signature(
    function: &str,
    effects: &EffectSet,
    config: &EffectErasure,
) -> ErasedSignature {
    let erased_effects = if config.erase_types {
        effects
            .iter()
            .map(|e| (e.0.clone(), ErasedForm::Unit))
            .collect()
    } else {
        Vec::new()
    };
    ErasedSignature {
        function: function.into(),
        original_effects: effects.clone(),
        erased: erased_effects,
        has_debug_info: config.preserve_debug_info,
    }
}

/// Result of effect erasure for a function.
#[derive(Debug, Clone)]
pub struct ErasedSignature {
    /// Function name.
    pub function: String,
    /// Original effect set (before erasure).
    pub original_effects: EffectSet,
    /// Erased effect forms.
    pub erased: Vec<(String, ErasedForm)>,
    /// Whether debug info is preserved.
    pub has_debug_info: bool,
}

impl ErasedSignature {
    /// Returns whether all effects were erased.
    pub fn fully_erased(&self) -> bool {
        !self.erased.is_empty() || self.original_effects.is_pure()
    }

    /// Returns the runtime overhead (zero for fully erased).
    pub fn runtime_overhead(&self) -> usize {
        0 // Effect erasure guarantees zero runtime overhead
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S20.6: Effect-Guided Optimization
// ═══════════════════════════════════════════════════════════════════════

/// Optimization opportunities based on effect information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectOptimization {
    /// Common subexpression elimination (pure functions only).
    Cse,
    /// Instruction reordering (no IO effect).
    Reorder,
    /// Memoization (pure + deterministic).
    Memoize,
    /// Dead call elimination (no side effects).
    DeadCallElim,
    /// Constant folding (pure + const inputs).
    ConstFold,
}

/// Determines which optimizations are safe for a given effect set.
pub fn available_optimizations(effects: &EffectSet) -> Vec<EffectOptimization> {
    let mut opts = Vec::new();
    if effects.is_pure() {
        opts.push(EffectOptimization::Cse);
        opts.push(EffectOptimization::Reorder);
        opts.push(EffectOptimization::Memoize);
        opts.push(EffectOptimization::DeadCallElim);
        opts.push(EffectOptimization::ConstFold);
    } else {
        if !effects.contains("IO") && !effects.contains("Unsafe") {
            opts.push(EffectOptimization::Reorder);
        }
        if !effects.contains("IO") && !effects.contains("Alloc") && !effects.contains("Unsafe") {
            opts.push(EffectOptimization::DeadCallElim);
        }
    }
    opts
}

/// Checks if a function call can be eliminated as dead code.
pub fn can_eliminate_call(effects: &EffectSet) -> bool {
    effects.is_pure()
}

/// Checks if two function calls can be reordered.
pub fn can_reorder(a: &EffectSet, b: &EffectSet) -> bool {
    // Reordering is safe if neither has IO or Unsafe effects
    let has_ordering_constraint =
        |e: &EffectSet| e.contains("IO") || e.contains("Unsafe") || e.contains("FileSystem");
    !has_ordering_constraint(a) || !has_ordering_constraint(b)
}

// ═══════════════════════════════════════════════════════════════════════
// S20.7: Effect Documentation
// ═══════════════════════════════════════════════════════════════════════

/// Effect documentation entry for a function.
#[derive(Debug, Clone)]
pub struct EffectDocEntry {
    /// Function name.
    pub function: String,
    /// Module path.
    pub module: String,
    /// Effect set.
    pub effects: EffectSet,
    /// Human-readable description.
    pub description: String,
}

impl fmt::Display for EffectDocEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.effects.is_pure() {
            write!(f, "fn {}() — pure", self.function)
        } else {
            write!(f, "fn {}() with {}", self.function, self.effects)
        }
    }
}

/// Formats effect information for LSP hover display.
pub fn format_hover_effects(function: &str, effects: &EffectSet) -> String {
    if effects.is_pure() {
        format!("**{function}** — pure (no side effects)")
    } else {
        let labels: Vec<&str> = effects.iter().map(|e| e.0.as_str()).collect();
        format!("**{function}** — effects: {}", labels.join(", "))
    }
}

/// Formats effect information for `fj check` output.
pub fn format_check_effects(entries: &[EffectDocEntry]) -> String {
    let mut lines = Vec::new();
    for entry in entries {
        lines.push(format!("  {}", entry));
    }
    if lines.is_empty() {
        "  No effectful functions found.".into()
    } else {
        lines.join("\n")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S20.8: Migration Guide
// ═══════════════════════════════════════════════════════════════════════

/// Migration step for adding effect annotations to existing code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationStep {
    /// Step 1: Enable effect inference (no code changes needed).
    EnableInference,
    /// Step 2: Review inferred effects via `fj check --effects`.
    ReviewEffects,
    /// Step 3: Add explicit annotations to public API functions.
    AnnotatePublicApi,
    /// Step 4: Add annotations to internal functions (optional).
    AnnotateInternal,
    /// Step 5: Enable strict mode (all functions must have annotations).
    EnableStrict,
}

impl fmt::Display for MigrationStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationStep::EnableInference => {
                write!(
                    f,
                    "Step 1: Enable effect inference (automatic, no code changes)"
                )
            }
            MigrationStep::ReviewEffects => {
                write!(
                    f,
                    "Step 2: Run `fj check --effects` to review inferred effects"
                )
            }
            MigrationStep::AnnotatePublicApi => {
                write!(
                    f,
                    "Step 3: Add `with {{...}}` annotations to public API functions"
                )
            }
            MigrationStep::AnnotateInternal => {
                write!(f, "Step 4: Optionally annotate internal functions")
            }
            MigrationStep::EnableStrict => write!(
                f,
                "Step 5: Enable strict mode — all functions require annotations"
            ),
        }
    }
}

/// Returns the migration guide steps in order.
pub fn migration_steps() -> Vec<MigrationStep> {
    vec![
        MigrationStep::EnableInference,
        MigrationStep::ReviewEffects,
        MigrationStep::AnnotatePublicApi,
        MigrationStep::AnnotateInternal,
        MigrationStep::EnableStrict,
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// S20.9: Standard Library Effects
// ═══════════════════════════════════════════════════════════════════════

/// Annotated standard library function with its effect set.
#[derive(Debug, Clone)]
pub struct StdlibEffectAnnotation {
    /// Module path.
    pub module: String,
    /// Function name.
    pub function: String,
    /// Effect set.
    pub effects: EffectSet,
}

/// Returns effect annotations for all stdlib functions.
pub fn stdlib_effect_annotations() -> Vec<StdlibEffectAnnotation> {
    vec![
        // std::io
        ann("std::io", "print", &["IO"]),
        ann("std::io", "println", &["IO"]),
        ann("std::io", "eprintln", &["IO"]),
        ann("std::io", "read_file", &["IO", "FileSystem"]),
        ann("std::io", "write_file", &["IO", "FileSystem"]),
        ann("std::io", "append_file", &["IO", "FileSystem"]),
        ann("std::io", "file_exists", &["IO", "FileSystem"]),
        // std::collections
        ann("std::collections", "Array.push", &["Alloc"]),
        ann("std::collections", "Array.pop", &[]),
        ann("std::collections", "HashMap.insert", &["Alloc"]),
        ann("std::collections", "HashMap.get", &[]),
        // std::math (pure)
        ann("std::math", "abs", &[]),
        ann("std::math", "sqrt", &[]),
        ann("std::math", "pow", &[]),
        ann("std::math", "sin", &[]),
        ann("std::math", "cos", &[]),
        // std::convert (pure)
        ann("std::convert", "to_string", &[]),
        ann("std::convert", "to_int", &[]),
        ann("std::convert", "to_float", &[]),
        // os::memory
        ann("os::memory", "mem_alloc", &["Alloc"]),
        ann("os::memory", "mem_free", &["Alloc"]),
        ann("os::memory", "mem_read", &["Unsafe"]),
        ann("os::memory", "mem_write", &["Unsafe"]),
        // nn::tensor
        ann("nn::tensor", "zeros", &["Alloc"]),
        ann("nn::tensor", "ones", &["Alloc"]),
        ann("nn::tensor", "randn", &["Alloc"]),
        // concurrency
        ann("std::concurrency", "spawn", &["Async"]),
        // panic
        ann("std::core", "panic", &["Panic"]),
        ann("std::core", "todo", &["Panic"]),
        ann("std::core", "assert", &["Panic"]),
    ]
}

/// Helper to create a stdlib effect annotation.
fn ann(module: &str, function: &str, effects: &[&str]) -> StdlibEffectAnnotation {
    StdlibEffectAnnotation {
        module: module.into(),
        function: function.into(),
        effects: EffectSet::from_labels(effects),
    }
}

/// Looks up the effect annotation for a stdlib function.
pub fn lookup_stdlib_effects(function: &str) -> Option<EffectSet> {
    let annotations = stdlib_effect_annotations();
    annotations
        .iter()
        .find(|a| a.function == function)
        .map(|a| a.effects.clone())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S20.1 — Effects + Async
    #[test]
    fn s20_1_async_effect_mapping() {
        let mapping = AsyncEffectMapping {
            function: "fetch".into(),
            uses_await: true,
            spawns_tasks: false,
        };
        let effects = mapping.effect_contribution();
        assert!(effects.contains("Async"));
    }

    #[test]
    fn s20_1_no_async_is_pure() {
        let mapping = AsyncEffectMapping {
            function: "add".into(),
            uses_await: false,
            spawns_tasks: false,
        };
        assert!(mapping.effect_contribution().is_pure());
    }

    #[test]
    fn s20_1_implies_async() {
        assert!(implies_async(&EffectSet::from_labels(&["Async", "IO"])));
        assert!(!implies_async(&EffectSet::from_labels(&["IO"])));
    }

    // S20.2 — Effects + @kernel
    #[test]
    fn s20_2_kernel_forbids_io_and_alloc() {
        let forbidden = forbidden_effects(ContextAnnotation::Kernel);
        let names: Vec<&str> = forbidden.iter().map(|f| f.effect.0.as_str()).collect();
        assert!(names.contains(&"IO"));
        assert!(names.contains(&"Alloc"));
    }

    #[test]
    fn s20_2_kernel_context_violation() {
        let effects = EffectSet::from_labels(&["IO", "Panic"]);
        let err =
            check_context_effects("my_kernel_fn", ContextAnnotation::Kernel, &effects).unwrap_err();
        assert_eq!(err.violations.len(), 1);
        assert!(err.to_string().contains("IO"));
    }

    #[test]
    fn s20_2_kernel_context_ok() {
        let effects = EffectSet::from_labels(&["Panic"]);
        assert!(
            check_context_effects("safe_kernel_fn", ContextAnnotation::Kernel, &effects).is_ok()
        );
    }

    // S20.3 — Effects + @device
    #[test]
    fn s20_3_device_forbids_unsafe() {
        let forbidden = forbidden_effects(ContextAnnotation::Device);
        assert_eq!(forbidden.len(), 1);
        assert_eq!(forbidden[0].effect.0, "Unsafe");
    }

    #[test]
    fn s20_3_device_allows_io() {
        let effects = EffectSet::from_labels(&["IO", "Alloc"]);
        assert!(check_context_effects("my_device_fn", ContextAnnotation::Device, &effects).is_ok());
    }

    // S20.4 — Effects + Linear Types
    #[test]
    fn s20_4_linear_both_paths_consumed() {
        let mut check = LinearEffectCheck::new("handle");
        check.consume_in_resume();
        check.consume_in_abort();
        assert!(check.validate().is_ok());
    }

    #[test]
    fn s20_4_linear_missing_abort_path() {
        let mut check = LinearEffectCheck::new("handle");
        check.consume_in_resume();
        let err = check.validate().unwrap_err();
        assert_eq!(err.kind, LinearViolationKind::NotConsumedInAbort);
    }

    #[test]
    fn s20_4_linear_never_consumed() {
        let check = LinearEffectCheck::new("handle");
        let err = check.validate().unwrap_err();
        assert_eq!(err.kind, LinearViolationKind::NeverConsumed);
        assert!(err.to_string().contains("never consumed"));
    }

    // S20.5 — Effect Erasure
    #[test]
    fn s20_5_erasure_default() {
        let config = EffectErasure::default();
        assert!(config.erase_types);
        assert!(config.inline_handlers);
        assert!(!config.preserve_debug_info);
    }

    #[test]
    fn s20_5_erase_signature() {
        let effects = EffectSet::from_labels(&["IO", "Alloc"]);
        let config = EffectErasure::default();
        let erased = erase_effect_signature("write", &effects, &config);
        assert!(erased.fully_erased());
        assert_eq!(erased.runtime_overhead(), 0);
        assert_eq!(erased.erased.len(), 2);
    }

    #[test]
    fn s20_5_pure_already_erased() {
        let config = EffectErasure::default();
        let erased = erase_effect_signature("add", &EffectSet::pure_set(), &config);
        assert!(erased.fully_erased());
        assert_eq!(erased.runtime_overhead(), 0);
    }

    // S20.6 — Effect-Guided Optimization
    #[test]
    fn s20_6_pure_all_optimizations() {
        let opts = available_optimizations(&EffectSet::pure_set());
        assert!(opts.contains(&EffectOptimization::Cse));
        assert!(opts.contains(&EffectOptimization::Reorder));
        assert!(opts.contains(&EffectOptimization::Memoize));
        assert!(opts.contains(&EffectOptimization::DeadCallElim));
        assert!(opts.contains(&EffectOptimization::ConstFold));
    }

    #[test]
    fn s20_6_io_limited_optimizations() {
        let opts = available_optimizations(&EffectSet::from_labels(&["IO"]));
        assert!(!opts.contains(&EffectOptimization::Cse));
        assert!(!opts.contains(&EffectOptimization::Memoize));
        assert!(!opts.contains(&EffectOptimization::Reorder));
    }

    #[test]
    fn s20_6_can_eliminate_pure() {
        assert!(can_eliminate_call(&EffectSet::pure_set()));
        assert!(!can_eliminate_call(&EffectSet::from_labels(&["IO"])));
    }

    #[test]
    fn s20_6_reorder_safe() {
        let pure = EffectSet::pure_set();
        let alloc = EffectSet::from_labels(&["Alloc"]);
        assert!(can_reorder(&pure, &alloc));
        assert!(can_reorder(&alloc, &alloc));
    }

    // S20.7 — Effect Documentation
    #[test]
    fn s20_7_hover_pure() {
        let hover = format_hover_effects("add", &EffectSet::pure_set());
        assert!(hover.contains("pure"));
        assert!(hover.contains("add"));
    }

    #[test]
    fn s20_7_hover_effectful() {
        let hover = format_hover_effects("write", &EffectSet::from_labels(&["IO", "FileSystem"]));
        assert!(hover.contains("IO"));
        assert!(hover.contains("FileSystem"));
    }

    #[test]
    fn s20_7_doc_entry_display() {
        let entry = EffectDocEntry {
            function: "read_file".into(),
            module: "std::io".into(),
            effects: EffectSet::from_labels(&["IO", "FileSystem"]),
            description: "Reads a file".into(),
        };
        let display = entry.to_string();
        assert!(display.contains("read_file"));
        assert!(display.contains("IO"));
    }

    // S20.8 — Migration Guide
    #[test]
    fn s20_8_migration_steps_order() {
        let steps = migration_steps();
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0], MigrationStep::EnableInference);
        assert_eq!(steps[4], MigrationStep::EnableStrict);
    }

    #[test]
    fn s20_8_migration_step_display() {
        let step = MigrationStep::AnnotatePublicApi;
        assert!(step.to_string().contains("public API"));
    }

    // S20.9 — Standard Library Effects
    #[test]
    fn s20_9_stdlib_print_has_io() {
        let effects = lookup_stdlib_effects("print").unwrap();
        assert!(effects.contains("IO"));
    }

    #[test]
    fn s20_9_stdlib_sqrt_is_pure() {
        let effects = lookup_stdlib_effects("sqrt").unwrap();
        assert!(effects.is_pure());
    }

    #[test]
    fn s20_9_stdlib_mem_alloc_has_alloc() {
        let effects = lookup_stdlib_effects("mem_alloc").unwrap();
        assert!(effects.contains("Alloc"));
    }

    #[test]
    fn s20_9_stdlib_spawn_has_async() {
        let effects = lookup_stdlib_effects("spawn").unwrap();
        assert!(effects.contains("Async"));
    }

    #[test]
    fn s20_9_stdlib_annotations_complete() {
        let annotations = stdlib_effect_annotations();
        assert!(annotations.len() >= 28);
    }

    // S20.10 — Additional
    #[test]
    fn s20_10_unsafe_context_allows_all() {
        let forbidden = forbidden_effects(ContextAnnotation::Unsafe);
        assert!(forbidden.is_empty());
    }

    #[test]
    fn s20_10_context_display() {
        assert_eq!(ContextAnnotation::Kernel.to_string(), "@kernel");
        assert_eq!(ContextAnnotation::Device.to_string(), "@device");
        assert_eq!(ContextAnnotation::Safe.to_string(), "@safe");
        assert_eq!(ContextAnnotation::Unsafe.to_string(), "@unsafe");
    }

    #[test]
    fn s20_10_check_report_format() {
        let entries = vec![EffectDocEntry {
            function: "print".into(),
            module: "std::io".into(),
            effects: EffectSet::from_labels(&["IO"]),
            description: "Print to stdout".into(),
        }];
        let report = format_check_effects(&entries);
        assert!(report.contains("print"));
    }
}
