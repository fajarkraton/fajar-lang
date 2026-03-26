//! Compiler Plugin System — extensible compiler passes via trait objects.
//!
//! V8 GC5.15-GC5.19: Plugins can inspect and transform the AST,
//! emit diagnostics, and generate additional code. Plugins are loaded
//! as dynamic libraries (.so/.dylib) or can be built-in.

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// GC5.15: Plugin Trait
// ═══════════════════════════════════════════════════════════════════════

/// A compiler plugin that can inspect and modify compilation.
pub trait CompilerPlugin: Send + Sync {
    /// Plugin name (e.g., "unused-variables").
    fn name(&self) -> &str;

    /// Plugin version.
    fn version(&self) -> &str;

    /// Called before analysis. Can inspect raw AST.
    fn on_ast(&self, _source: &str, _file: &str) -> Vec<PluginDiagnostic> {
        vec![]
    }

    /// Called after type checking. Can inspect typed AST.
    fn on_post_analysis(&self, _source: &str, _file: &str) -> Vec<PluginDiagnostic> {
        vec![]
    }

    /// Called during code generation. Can emit extra code.
    fn on_codegen(&self, _source: &str, _file: &str) -> Option<String> {
        None
    }
}

/// A diagnostic emitted by a plugin.
#[derive(Debug, Clone)]
pub struct PluginDiagnostic {
    /// Diagnostic severity.
    pub severity: DiagnosticSeverity,
    /// Human-readable message.
    pub message: String,
    /// Source file.
    pub file: String,
    /// Line number (1-based).
    pub line: u32,
    /// Column number (1-based).
    pub column: u32,
    /// Plugin that emitted this diagnostic.
    pub plugin: String,
    /// Suggested fix (if any).
    pub fix: Option<String>,
}

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl std::fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Info => write!(f, "info"),
            Self::Hint => write!(f, "hint"),
        }
    }
}

impl std::fmt::Display for PluginDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {}:{}:{}: {} ({})",
            self.severity, self.file, self.line, self.column, self.message, self.plugin
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GC5.16: Plugin Registry & Loading
// ═══════════════════════════════════════════════════════════════════════

/// Plugin configuration from fj.toml.
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Plugin name.
    pub name: String,
    /// Plugin-specific options.
    pub options: HashMap<String, String>,
    /// Whether this plugin is enabled.
    pub enabled: bool,
}

/// The plugin registry holds all loaded plugins.
pub struct PluginRegistry {
    /// Loaded plugins.
    plugins: Vec<Box<dyn CompilerPlugin>>,
    /// Plugin configurations.
    configs: HashMap<String, PluginConfig>,
}

impl PluginRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            configs: HashMap::new(),
        }
    }

    /// Register a built-in plugin.
    pub fn register(&mut self, plugin: Box<dyn CompilerPlugin>) {
        let name = plugin.name().to_string();
        self.configs
            .entry(name)
            .or_insert_with(|| PluginConfig {
                name: plugin.name().to_string(),
                options: HashMap::new(),
                enabled: true,
            });
        self.plugins.push(plugin);
    }

    /// Get all registered plugin names.
    pub fn plugin_names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }

    /// Number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Run all plugins' on_ast phase and collect diagnostics.
    pub fn run_ast_phase(&self, source: &str, file: &str) -> Vec<PluginDiagnostic> {
        let mut diagnostics = Vec::new();
        for plugin in &self.plugins {
            let cfg = self.configs.get(plugin.name());
            if cfg.is_some_and(|c| !c.enabled) {
                continue;
            }
            diagnostics.extend(plugin.on_ast(source, file));
        }
        diagnostics
    }

    /// Run all plugins' post-analysis phase.
    pub fn run_post_analysis_phase(&self, source: &str, file: &str) -> Vec<PluginDiagnostic> {
        let mut diagnostics = Vec::new();
        for plugin in &self.plugins {
            let cfg = self.configs.get(plugin.name());
            if cfg.is_some_and(|c| !c.enabled) {
                continue;
            }
            diagnostics.extend(plugin.on_post_analysis(source, file));
        }
        diagnostics
    }

    /// Enable or disable a plugin by name.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) {
        if let Some(cfg) = self.configs.get_mut(name) {
            cfg.enabled = enabled;
        }
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GC5.18: Built-in Lint Plugin — Unused Variables
// ═══════════════════════════════════════════════════════════════════════

/// A lint plugin that detects unused variables in source code.
///
/// Scans for `let x =` patterns where `x` is not used later in the source.
/// This is a simple text-based heuristic; the real analyzer does deeper checks.
pub struct UnusedVariableLint;

impl CompilerPlugin for UnusedVariableLint {
    fn name(&self) -> &str {
        "unused-variables"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn on_ast(&self, source: &str, file: &str) -> Vec<PluginDiagnostic> {
        let mut diagnostics = Vec::new();

        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            // Detect "let <name> =" or "let mut <name> ="
            if let Some(rest) = trimmed.strip_prefix("let ") {
                let rest = rest.strip_prefix("mut ").unwrap_or(rest);
                if let Some(var_name) = rest.split(['=', ':', ' ']).next() {
                    let var_name = var_name.trim();
                    if var_name.is_empty() || var_name.starts_with('_') {
                        continue; // _ prefixed variables are intentionally unused
                    }
                    // Check if variable appears elsewhere in the source
                    let usage_count = source
                        .lines()
                        .enumerate()
                        .filter(|(i, l)| {
                            *i != line_idx && l.contains(var_name) && !l.trim().starts_with("//")
                        })
                        .count();
                    if usage_count == 0 {
                        diagnostics.push(PluginDiagnostic {
                            severity: DiagnosticSeverity::Warning,
                            message: format!("unused variable: `{var_name}`"),
                            file: file.to_string(),
                            line: (line_idx + 1) as u32,
                            column: 1,
                            plugin: self.name().to_string(),
                            fix: Some(format!("prefix with underscore: `_{var_name}`")),
                        });
                    }
                }
            }
        }

        diagnostics
    }
}

/// A lint plugin that warns about `TODO` and `FIXME` comments.
pub struct TodoLint;

impl CompilerPlugin for TodoLint {
    fn name(&self) -> &str {
        "todo-comments"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn on_ast(&self, source: &str, file: &str) -> Vec<PluginDiagnostic> {
        let mut diagnostics = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            if line.contains("TODO") || line.contains("FIXME") || line.contains("HACK") {
                diagnostics.push(PluginDiagnostic {
                    severity: DiagnosticSeverity::Info,
                    message: format!("found annotation: {}", line.trim()),
                    file: file.to_string(),
                    line: (line_idx + 1) as u32,
                    column: 1,
                    plugin: self.name().to_string(),
                    fix: None,
                });
            }
        }
        diagnostics
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gc5_plugin_registry_basic() {
        let mut registry = PluginRegistry::new();
        assert!(registry.is_empty());

        registry.register(Box::new(UnusedVariableLint));
        registry.register(Box::new(TodoLint));

        assert_eq!(registry.len(), 2);
        assert!(registry.plugin_names().contains(&"unused-variables"));
        assert!(registry.plugin_names().contains(&"todo-comments"));
    }

    #[test]
    fn gc5_unused_variable_detection() {
        let lint = UnusedVariableLint;
        let source = r#"
let x = 42
let y = 10
println(x)
"#;
        let diags = lint.on_ast(source, "test.fj");
        // y is unused (only x is used in println)
        assert!(
            diags.iter().any(|d| d.message.contains("unused variable: `y`")),
            "should detect unused `y`, got: {diags:?}"
        );
        // x is used — should NOT be flagged
        assert!(
            !diags.iter().any(|d| d.message.contains("unused variable: `x`")),
            "should not flag `x` as unused"
        );
    }

    #[test]
    fn gc5_underscore_not_flagged() {
        let lint = UnusedVariableLint;
        let source = "let _unused = 42\n";
        let diags = lint.on_ast(source, "test.fj");
        assert!(diags.is_empty(), "_ prefixed vars should not be flagged");
    }

    #[test]
    fn gc5_todo_lint() {
        let lint = TodoLint;
        let source = "// TODO: implement this\nlet x = 42\n// FIXME: broken\n";
        let diags = lint.on_ast(source, "test.fj");
        assert_eq!(diags.len(), 2);
        assert!(diags[0].message.contains("TODO"));
        assert!(diags[1].message.contains("FIXME"));
    }

    #[test]
    fn gc5_registry_run_all_plugins() {
        let mut registry = PluginRegistry::new();
        registry.register(Box::new(UnusedVariableLint));
        registry.register(Box::new(TodoLint));

        let source = "let unused_var = 42\n// TODO: use this\n";
        let diags = registry.run_ast_phase(source, "test.fj");

        // Should get both unused variable + TODO comment
        assert!(diags.len() >= 2, "expected 2+ diagnostics, got {}", diags.len());
    }

    #[test]
    fn gc5_plugin_disable() {
        let mut registry = PluginRegistry::new();
        registry.register(Box::new(TodoLint));

        let source = "// TODO: test\n";
        assert!(!registry.run_ast_phase(source, "test.fj").is_empty());

        registry.set_enabled("todo-comments", false);
        assert!(registry.run_ast_phase(source, "test.fj").is_empty());
    }

    #[test]
    fn gc5_diagnostic_display() {
        let diag = PluginDiagnostic {
            severity: DiagnosticSeverity::Warning,
            message: "unused variable: `x`".to_string(),
            file: "main.fj".to_string(),
            line: 5,
            column: 1,
            plugin: "unused-variables".to_string(),
            fix: Some("prefix with underscore: `_x`".to_string()),
        };
        let s = format!("{diag}");
        assert!(s.contains("warning"));
        assert!(s.contains("main.fj:5:1"));
        assert!(s.contains("unused variable"));
    }
}
