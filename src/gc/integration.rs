//! GC integration — --gc compiler flag, automatic Rc insertion,
//! ownership bypass, @kernel prohibition, mixed-mode, migration, REPL.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S23.1: --gc Compiler Flag
// ═══════════════════════════════════════════════════════════════════════

/// Memory management mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryMode {
    /// Ownership-based (default, no GC).
    Owned,
    /// Reference-counted GC.
    RefCounted,
    /// Tracing GC.
    Tracing,
}

impl fmt::Display for MemoryMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryMode::Owned => write!(f, "ownership"),
            MemoryMode::RefCounted => write!(f, "ref-counted GC"),
            MemoryMode::Tracing => write!(f, "tracing GC"),
        }
    }
}

/// GC mode configuration parsed from CLI flags.
#[derive(Debug, Clone)]
pub struct GcConfig {
    /// Memory management mode.
    pub mode: MemoryMode,
    /// Whether GC statistics are printed at exit.
    pub print_stats: bool,
    /// Initial heap size in bytes (for tracing GC).
    pub initial_heap: usize,
    /// Maximum pause time in microseconds.
    pub max_pause_us: u64,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            mode: MemoryMode::Owned,
            print_stats: false,
            initial_heap: 1024 * 1024,
            max_pause_us: 1000,
        }
    }
}

/// Parses GC-related CLI flags.
pub fn parse_gc_flags(flags: &[&str]) -> GcConfig {
    let mut config = GcConfig::default();
    for flag in flags {
        match *flag {
            "--gc" => config.mode = MemoryMode::Tracing,
            "--gc=rc" => config.mode = MemoryMode::RefCounted,
            "--gc=tracing" => config.mode = MemoryMode::Tracing,
            "--no-gc" => config.mode = MemoryMode::Owned,
            "--gc-stats" => config.print_stats = true,
            _ => {}
        }
    }
    config
}

// ═══════════════════════════════════════════════════════════════════════
// S23.2: Automatic Rc Insertion
// ═══════════════════════════════════════════════════════════════════════

/// Describes an automatic Rc wrapping transformation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcInsertion {
    /// Variable name.
    pub variable: String,
    /// Original type.
    pub original_type: String,
    /// Wrapped type.
    pub wrapped_type: String,
}

impl fmt::Display for RcInsertion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} -> {}",
            self.variable, self.original_type, self.wrapped_type
        )
    }
}

/// Determines whether a type should be Rc-wrapped in GC mode.
pub fn needs_rc_wrap(type_name: &str, mode: MemoryMode) -> bool {
    if mode == MemoryMode::Owned {
        return false;
    }
    // Primitive types don't need wrapping
    let primitives = [
        "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f32", "f64", "bool",
        "char", "void",
    ];
    !primitives.contains(&type_name)
}

/// Wraps a type in Rc for GC mode.
pub fn wrap_in_rc(type_name: &str) -> String {
    format!("Rc<{type_name}>")
}

// ═══════════════════════════════════════════════════════════════════════
// S23.3: Ownership System Bypass
// ═══════════════════════════════════════════════════════════════════════

/// Ownership check configuration based on memory mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipConfig {
    /// Whether move semantics are enforced.
    pub enforce_moves: bool,
    /// Whether borrow checking is enforced.
    pub enforce_borrows: bool,
    /// Whether values can be freely shared.
    pub allow_sharing: bool,
}

/// Returns the ownership configuration for a given memory mode.
pub fn ownership_config(mode: MemoryMode) -> OwnershipConfig {
    match mode {
        MemoryMode::Owned => OwnershipConfig {
            enforce_moves: true,
            enforce_borrows: true,
            allow_sharing: false,
        },
        MemoryMode::RefCounted | MemoryMode::Tracing => OwnershipConfig {
            enforce_moves: false,
            enforce_borrows: false,
            allow_sharing: true,
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S23.4: @kernel GC Prohibition
// ═══════════════════════════════════════════════════════════════════════

/// Context annotation for GC compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcContext {
    /// @safe — GC allowed.
    Safe,
    /// @device — GC allowed.
    Device,
    /// @kernel — GC NEVER allowed.
    Kernel,
    /// @unsafe — GC allowed.
    Unsafe,
}

/// Checks if GC is allowed in a given context.
pub fn is_gc_allowed(ctx: GcContext) -> bool {
    ctx != GcContext::Kernel
}

/// Error for using GC in @kernel context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcKernelError {
    /// Function name.
    pub function: String,
    /// Attempted GC operation.
    pub operation: String,
}

impl fmt::Display for GcKernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GC operation `{}` not allowed in @kernel function `{}`",
            self.operation, self.function
        )
    }
}

/// Validates that a function doesn't use GC in @kernel context.
pub fn check_gc_context(
    function: &str,
    ctx: GcContext,
    uses_gc: bool,
) -> Result<(), GcKernelError> {
    if uses_gc && !is_gc_allowed(ctx) {
        Err(GcKernelError {
            function: function.into(),
            operation: "Rc allocation".into(),
        })
    } else {
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S23.5: Mixed-Mode Modules
// ═══════════════════════════════════════════════════════════════════════

/// A module's GC mode annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleGcMode {
    /// Module name.
    pub module: String,
    /// GC mode for this module.
    pub mode: MemoryMode,
}

/// Checks compatibility of cross-module calls with different GC modes.
pub fn check_cross_module(caller: &ModuleGcMode, callee: &ModuleGcMode) -> CrossModuleResult {
    match (caller.mode, callee.mode) {
        (MemoryMode::Owned, MemoryMode::Owned) => CrossModuleResult::Compatible,
        (MemoryMode::Tracing, MemoryMode::Tracing)
        | (MemoryMode::RefCounted, MemoryMode::RefCounted) => CrossModuleResult::Compatible,
        (MemoryMode::Owned, _) => CrossModuleResult::Warning {
            message: format!(
                "owned module `{}` calling GC module `{}`",
                caller.module, callee.module
            ),
        },
        (_, MemoryMode::Owned) => CrossModuleResult::NeedsBridge {
            from: caller.mode,
            to: callee.mode,
        },
        _ => CrossModuleResult::Compatible,
    }
}

/// Result of cross-module GC compatibility check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossModuleResult {
    /// Modes are compatible.
    Compatible,
    /// Warning about potential issues.
    Warning { message: String },
    /// Needs a bridge function.
    NeedsBridge { from: MemoryMode, to: MemoryMode },
}

// ═══════════════════════════════════════════════════════════════════════
// S23.6: GC-to-Owned Migration
// ═══════════════════════════════════════════════════════════════════════

/// A migration action to remove GC from code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationAction {
    /// Replace `Rc<T>` with `T` and add ownership annotations.
    RemoveRc {
        variable: String,
        inner_type: String,
    },
    /// Add explicit `clone()` where shared references exist.
    AddClone { variable: String, location: String },
    /// Add lifetime annotation.
    AddLifetime { variable: String, lifetime: String },
    /// Convert `Rc<RefCell<T>>` to `&mut T`.
    ConvertRefCell { variable: String },
}

impl fmt::Display for MigrationAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationAction::RemoveRc {
                variable,
                inner_type,
            } => {
                write!(
                    f,
                    "Replace Rc<{inner_type}> with {inner_type} for `{variable}`"
                )
            }
            MigrationAction::AddClone { variable, location } => {
                write!(f, "Add .clone() for `{variable}` at {location}")
            }
            MigrationAction::AddLifetime { variable, lifetime } => {
                write!(f, "Add lifetime '{lifetime} to `{variable}`")
            }
            MigrationAction::ConvertRefCell { variable } => {
                write!(f, "Convert RefCell to &mut for `{variable}`")
            }
        }
    }
}

/// Generates migration actions for removing GC from a function.
pub fn plan_migration(rc_vars: &[(&str, &str)]) -> Vec<MigrationAction> {
    rc_vars
        .iter()
        .map(|(var, inner)| MigrationAction::RemoveRc {
            variable: var.to_string(),
            inner_type: inner.to_string(),
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S23.7 / S23.8: GC Mode Warnings & Performance Switch
// ═══════════════════════════════════════════════════════════════════════

/// Warning for GC mode usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcWarning {
    /// Warning message.
    pub message: String,
    /// Severity.
    pub severity: WarningSeverity,
}

/// Warning severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningSeverity {
    /// Informational.
    Info,
    /// Potential issue.
    Warning,
}

/// Checks for GC-to-owned interaction warnings.
pub fn check_gc_warnings(mode: MemoryMode, calls_owned: bool) -> Vec<GcWarning> {
    let mut warnings = Vec::new();
    if mode != MemoryMode::Owned && calls_owned {
        warnings.push(GcWarning {
            message: "GC mode code calls ownership-based function — value lifetime may differ"
                .into(),
            severity: WarningSeverity::Warning,
        });
    }
    warnings
}

// ═══════════════════════════════════════════════════════════════════════
// S23.9: GC Mode in REPL
// ═══════════════════════════════════════════════════════════════════════

/// REPL GC configuration.
#[derive(Debug, Clone)]
pub struct ReplGcConfig {
    /// Default GC mode for REPL.
    pub default_mode: MemoryMode,
    /// Whether to print GC stats after each evaluation.
    pub show_gc_stats: bool,
}

impl Default for ReplGcConfig {
    fn default() -> Self {
        Self {
            default_mode: MemoryMode::Tracing,
            show_gc_stats: false,
        }
    }
}

/// Returns the effective memory mode for REPL.
pub fn repl_memory_mode(config: &ReplGcConfig, explicit_flag: Option<MemoryMode>) -> MemoryMode {
    explicit_flag.unwrap_or(config.default_mode)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S23.1 — --gc Compiler Flag
    #[test]
    fn s23_1_parse_gc_flag() {
        let config = parse_gc_flags(&["--gc"]);
        assert_eq!(config.mode, MemoryMode::Tracing);
    }

    #[test]
    fn s23_1_parse_no_gc_flag() {
        let config = parse_gc_flags(&["--no-gc"]);
        assert_eq!(config.mode, MemoryMode::Owned);
    }

    #[test]
    fn s23_1_parse_gc_rc() {
        let config = parse_gc_flags(&["--gc=rc"]);
        assert_eq!(config.mode, MemoryMode::RefCounted);
    }

    #[test]
    fn s23_1_default_is_owned() {
        let config = GcConfig::default();
        assert_eq!(config.mode, MemoryMode::Owned);
    }

    // S23.2 — Automatic Rc Insertion
    #[test]
    fn s23_2_needs_rc_wrap_struct() {
        assert!(needs_rc_wrap("MyStruct", MemoryMode::Tracing));
        assert!(!needs_rc_wrap("i32", MemoryMode::Tracing));
    }

    #[test]
    fn s23_2_no_wrap_in_owned_mode() {
        assert!(!needs_rc_wrap("MyStruct", MemoryMode::Owned));
    }

    #[test]
    fn s23_2_wrap_in_rc() {
        assert_eq!(wrap_in_rc("Vec<i32>"), "Rc<Vec<i32>>");
    }

    #[test]
    fn s23_2_rc_insertion_display() {
        let insertion = RcInsertion {
            variable: "data".into(),
            original_type: "Vec<i32>".into(),
            wrapped_type: "Rc<Vec<i32>>".into(),
        };
        assert!(insertion.to_string().contains("data"));
    }

    // S23.3 — Ownership System Bypass
    #[test]
    fn s23_3_owned_mode_enforces() {
        let config = ownership_config(MemoryMode::Owned);
        assert!(config.enforce_moves);
        assert!(config.enforce_borrows);
        assert!(!config.allow_sharing);
    }

    #[test]
    fn s23_3_gc_mode_relaxes() {
        let config = ownership_config(MemoryMode::Tracing);
        assert!(!config.enforce_moves);
        assert!(!config.enforce_borrows);
        assert!(config.allow_sharing);
    }

    // S23.4 — @kernel GC Prohibition
    #[test]
    fn s23_4_kernel_prohibits_gc() {
        assert!(!is_gc_allowed(GcContext::Kernel));
        assert!(is_gc_allowed(GcContext::Safe));
        assert!(is_gc_allowed(GcContext::Device));
    }

    #[test]
    fn s23_4_kernel_gc_error() {
        let err = check_gc_context("init_kernel", GcContext::Kernel, true).unwrap_err();
        assert!(err.to_string().contains("@kernel"));
    }

    #[test]
    fn s23_4_safe_gc_ok() {
        assert!(check_gc_context("my_fn", GcContext::Safe, true).is_ok());
    }

    // S23.5 — Mixed-Mode Modules
    #[test]
    fn s23_5_compatible_same_mode() {
        let caller = ModuleGcMode {
            module: "a".into(),
            mode: MemoryMode::Owned,
        };
        let callee = ModuleGcMode {
            module: "b".into(),
            mode: MemoryMode::Owned,
        };
        assert_eq!(
            check_cross_module(&caller, &callee),
            CrossModuleResult::Compatible
        );
    }

    #[test]
    fn s23_5_warning_owned_calls_gc() {
        let caller = ModuleGcMode {
            module: "core".into(),
            mode: MemoryMode::Owned,
        };
        let callee = ModuleGcMode {
            module: "proto".into(),
            mode: MemoryMode::Tracing,
        };
        match check_cross_module(&caller, &callee) {
            CrossModuleResult::Warning { message } => {
                assert!(message.contains("core"));
            }
            _ => panic!("expected warning"),
        }
    }

    // S23.6 — GC-to-Owned Migration
    #[test]
    fn s23_6_plan_migration() {
        let actions = plan_migration(&[("data", "Vec<i32>"), ("config", "Config")]);
        assert_eq!(actions.len(), 2);
        match &actions[0] {
            MigrationAction::RemoveRc { variable, .. } => assert_eq!(variable, "data"),
            _ => panic!("expected RemoveRc"),
        }
    }

    #[test]
    fn s23_6_migration_action_display() {
        let action = MigrationAction::AddClone {
            variable: "buf".into(),
            location: "line 42".into(),
        };
        assert!(action.to_string().contains("clone"));
    }

    // S23.7 — GC Mode Warnings
    #[test]
    fn s23_7_gc_calls_owned_warning() {
        let warnings = check_gc_warnings(MemoryMode::Tracing, true);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].severity, WarningSeverity::Warning);
    }

    #[test]
    fn s23_7_no_warning_same_mode() {
        let warnings = check_gc_warnings(MemoryMode::Owned, true);
        assert!(warnings.is_empty());
    }

    // S23.8 — Performance Mode Switch
    #[test]
    fn s23_8_mode_display() {
        assert_eq!(MemoryMode::Owned.to_string(), "ownership");
        assert_eq!(MemoryMode::RefCounted.to_string(), "ref-counted GC");
        assert_eq!(MemoryMode::Tracing.to_string(), "tracing GC");
    }

    // S23.9 — GC Mode in REPL
    #[test]
    fn s23_9_repl_defaults_gc() {
        let config = ReplGcConfig::default();
        assert_eq!(config.default_mode, MemoryMode::Tracing);
    }

    #[test]
    fn s23_9_repl_explicit_override() {
        let config = ReplGcConfig::default();
        let mode = repl_memory_mode(&config, Some(MemoryMode::Owned));
        assert_eq!(mode, MemoryMode::Owned);
    }

    #[test]
    fn s23_9_repl_no_override() {
        let config = ReplGcConfig::default();
        let mode = repl_memory_mode(&config, None);
        assert_eq!(mode, MemoryMode::Tracing);
    }

    // S23.10 — Additional
    #[test]
    fn s23_10_gc_stats_flag() {
        let config = parse_gc_flags(&["--gc", "--gc-stats"]);
        assert!(config.print_stats);
    }

    #[test]
    fn s23_10_kernel_error_display() {
        let err = GcKernelError {
            function: "irq_handler".into(),
            operation: "Rc allocation".into(),
        };
        assert!(err.to_string().contains("irq_handler"));
        assert!(err.to_string().contains("@kernel"));
    }
}
