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
        self.configs.entry(name).or_insert_with(|| PluginConfig {
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

// ═══════════════════════════════════════════════════════════════════════
// PQ9.1: Dynamic Plugin Loading
// ═══════════════════════════════════════════════════════════════════════

/// Load a plugin from a dynamic library (.so on Linux, .dylib on macOS).
///
/// The library must export a `create_plugin` function with signature:
/// `extern "C" fn() -> *mut dyn CompilerPlugin`
///
/// Returns the loaded plugin, or an error if loading fails.
pub fn load_plugin_from_path(path: &str) -> Result<Box<dyn CompilerPlugin>, String> {
    // Verify file exists
    if !std::path::Path::new(path).exists() {
        return Err(format!("plugin not found: {path}"));
    }

    // Load the library
    // SAFETY: loading a dynamic library that exports the expected symbol.
    // The plugin author must ensure ABI compatibility.
    let lib = unsafe { libloading::Library::new(path) }
        .map_err(|e| format!("failed to load plugin '{path}': {e}"))?;

    // Look up the factory function
    #[allow(improper_ctypes_definitions)]
    type CreatePluginFn = unsafe extern "C" fn() -> *mut dyn CompilerPlugin;
    // SAFETY: Plugin symbol lookup via libloading — caller ensures ABI compatibility
    let create_fn: libloading::Symbol<CreatePluginFn> = unsafe { lib.get(b"create_plugin") }
        .map_err(|e| format!("plugin '{path}' missing 'create_plugin' symbol: {e}"))?;

    // Call the factory
    // SAFETY: the factory returns a valid pointer to a CompilerPlugin.
    let plugin_ptr = unsafe { create_fn() };
    if plugin_ptr.is_null() {
        return Err(format!("plugin '{path}' returned null"));
    }

    // SAFETY: the pointer was created by Box::into_raw in the plugin.
    let plugin = unsafe { Box::from_raw(plugin_ptr) };

    // Don't drop the library — it needs to stay loaded for the plugin's lifetime.
    // In a real implementation, we'd store the Library alongside the plugin.
    std::mem::forget(lib);

    Ok(plugin)
}

/// PQ9.4: Discover plugins in a directory.
pub fn discover_plugins(dir: &str) -> Vec<String> {
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };

    let path = std::path::Path::new(dir);
    if !path.is_dir() {
        return Vec::new();
    }

    let mut plugins = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == ext) {
                plugins.push(p.to_string_lossy().to_string());
            }
        }
    }
    plugins.sort();
    plugins
}

// ═══════════════════════════════════════════════════════════════════════
// PQ9.2: Plugin API Versioning
// ═══════════════════════════════════════════════════════════════════════

/// Current plugin API version. Plugins must match this to load.
pub const PLUGIN_API_VERSION: u32 = 1;

/// Check if a plugin's API version is compatible.
pub fn check_api_version(plugin_api_version: u32) -> Result<(), String> {
    if plugin_api_version != PLUGIN_API_VERSION {
        Err(format!(
            "incompatible plugin API: plugin has v{plugin_api_version}, need v{PLUGIN_API_VERSION}"
        ))
    } else {
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PQ9.3: Plugin Configuration
// ═══════════════════════════════════════════════════════════════════════

impl PluginRegistry {
    /// Set a plugin option.
    pub fn set_option(&mut self, plugin_name: &str, key: &str, value: &str) {
        if let Some(cfg) = self.configs.get_mut(plugin_name) {
            cfg.options.insert(key.to_string(), value.to_string());
        }
    }

    /// Get a plugin option.
    pub fn get_option(&self, plugin_name: &str, key: &str) -> Option<&str> {
        self.configs
            .get(plugin_name)
            .and_then(|c| c.options.get(key).map(|s| s.as_str()))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PQ9.7: Auto-fix API
// ═══════════════════════════════════════════════════════════════════════

/// A suggested code fix from a plugin.
#[derive(Debug, Clone)]
pub struct CodeFix {
    /// File path.
    pub file: String,
    /// Line to replace (1-based).
    pub line: u32,
    /// Original text.
    pub original: String,
    /// Replacement text.
    pub replacement: String,
    /// Description of the fix.
    pub description: String,
}

impl std::fmt::Display for CodeFix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}: {} → {}",
            self.file, self.line, self.original, self.replacement
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PQ9.8: Performance Budget
// ═══════════════════════════════════════════════════════════════════════

/// Run a plugin with a performance budget.
/// Returns diagnostics if completed within budget, or a timeout error.
pub fn run_plugin_with_budget(
    plugin: &dyn CompilerPlugin,
    source: &str,
    file: &str,
    budget_ms: u64,
) -> Result<Vec<PluginDiagnostic>, String> {
    let start = std::time::Instant::now();
    let diags = plugin.on_ast(source, file);
    let elapsed = start.elapsed().as_millis() as u64;
    if elapsed > budget_ms {
        Err(format!(
            "plugin '{}' exceeded budget: {}ms > {}ms",
            plugin.name(),
            elapsed,
            budget_ms
        ))
    } else {
        Ok(diags)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PQ9.9: 5 Built-in Plugins (3 new + 2 existing)
// ═══════════════════════════════════════════════════════════════════════

/// Plugin 3: Naming convention checker.
/// Enforces snake_case for functions/variables, PascalCase for types.
pub struct NamingConventionLint;

impl CompilerPlugin for NamingConventionLint {
    fn name(&self) -> &str {
        "naming-convention"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn on_ast(&self, source: &str, file: &str) -> Vec<PluginDiagnostic> {
        let mut diags = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            // Check fn names are snake_case
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                let name = rest.split('(').next().unwrap_or("").trim();
                if !name.is_empty() && name.chars().any(|c| c.is_uppercase()) {
                    diags.push(PluginDiagnostic {
                        severity: DiagnosticSeverity::Warning,
                        message: format!("function `{name}` should be snake_case"),
                        file: file.to_string(),
                        line: (i + 1) as u32,
                        column: 1,
                        plugin: self.name().to_string(),
                        fix: Some(format!("rename to `{}`", to_snake_case(name))),
                    });
                }
            }
            // Check struct/enum names are PascalCase
            for keyword in ["struct ", "enum "] {
                if let Some(rest) = trimmed.strip_prefix(keyword) {
                    let name = rest.split(['{', '<', ' ']).next().unwrap_or("").trim();
                    if !name.is_empty() && name.chars().next().is_some_and(|c| c.is_lowercase()) {
                        diags.push(PluginDiagnostic {
                            severity: DiagnosticSeverity::Warning,
                            message: format!("{}{name}` should be PascalCase", &keyword.trim()),
                            file: file.to_string(),
                            line: (i + 1) as u32,
                            column: 1,
                            plugin: self.name().to_string(),
                            fix: Some(format!("rename to `{}`", to_pascal_case(name))),
                        });
                    }
                }
            }
        }
        diags
    }
}

/// Plugin 4: Complexity checker — warns about functions with too many lines.
pub struct ComplexityLint {
    /// Maximum lines per function (default: 50).
    pub max_lines: usize,
}

impl Default for ComplexityLint {
    fn default() -> Self {
        Self { max_lines: 50 }
    }
}

impl CompilerPlugin for ComplexityLint {
    fn name(&self) -> &str {
        "complexity"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn on_ast(&self, source: &str, file: &str) -> Vec<PluginDiagnostic> {
        let mut diags = Vec::new();
        let mut fn_start: Option<(String, usize)> = None;
        let mut brace_depth = 0i32;

        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                let name = trimmed
                    .trim_start_matches("pub ")
                    .trim_start_matches("fn ")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                fn_start = Some((name, i));
                brace_depth = 0;
            }
            brace_depth += trimmed.matches('{').count() as i32;
            brace_depth -= trimmed.matches('}').count() as i32;

            if brace_depth == 0 {
                if let Some((ref name, start)) = fn_start {
                    let lines = i - start + 1;
                    if lines > self.max_lines {
                        diags.push(PluginDiagnostic {
                            severity: DiagnosticSeverity::Warning,
                            message: format!(
                                "function `{name}` is {lines} lines (max {})",
                                self.max_lines
                            ),
                            file: file.to_string(),
                            line: (start + 1) as u32,
                            column: 1,
                            plugin: self.name().to_string(),
                            fix: Some("consider splitting into smaller functions".to_string()),
                        });
                    }
                    fn_start = None;
                }
            }
        }
        diags
    }
}

/// Plugin 5: Security lint — detect hardcoded secrets.
pub struct SecurityLint;

impl CompilerPlugin for SecurityLint {
    fn name(&self) -> &str {
        "security"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn on_ast(&self, source: &str, file: &str) -> Vec<PluginDiagnostic> {
        let mut diags = Vec::new();
        let secret_patterns = [
            "password",
            "secret",
            "api_key",
            "apikey",
            "token",
            "private_key",
        ];
        for (i, line) in source.lines().enumerate() {
            let lower = line.to_lowercase();
            if lower.contains("let ") && lower.contains("= \"") {
                for pattern in &secret_patterns {
                    if lower.contains(pattern) {
                        diags.push(PluginDiagnostic {
                            severity: DiagnosticSeverity::Error,
                            message: format!(
                                "potential hardcoded secret: `{pattern}` in string literal"
                            ),
                            file: file.to_string(),
                            line: (i + 1) as u32,
                            column: 1,
                            plugin: self.name().to_string(),
                            fix: Some("use environment variable instead".to_string()),
                        });
                    }
                }
            }
        }
        diags
    }
}

/// Convert to snake_case.
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

/// Convert to PascalCase.
fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_uppercase().next().unwrap_or(c));
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Create a default registry with all 5 built-in plugins.
pub fn default_registry() -> PluginRegistry {
    let mut reg = PluginRegistry::new();
    reg.register(Box::new(UnusedVariableLint));
    reg.register(Box::new(TodoLint));
    reg.register(Box::new(NamingConventionLint));
    reg.register(Box::new(ComplexityLint::default()));
    reg.register(Box::new(SecurityLint));
    reg
}

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
            diags
                .iter()
                .any(|d| d.message.contains("unused variable: `y`")),
            "should detect unused `y`, got: {diags:?}"
        );
        // x is used — should NOT be flagged
        assert!(
            !diags
                .iter()
                .any(|d| d.message.contains("unused variable: `x`")),
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
        assert!(
            diags.len() >= 2,
            "expected 2+ diagnostics, got {}",
            diags.len()
        );
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

    // ═══════════════════════════════════════════════════════════════════
    // PQ9: Quality improvement tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn pq9_2_api_version_match() {
        assert!(check_api_version(PLUGIN_API_VERSION).is_ok());
    }

    #[test]
    fn pq9_2_api_version_mismatch() {
        let result = check_api_version(99);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("incompatible"));
    }

    #[test]
    fn pq9_3_plugin_config() {
        let mut reg = PluginRegistry::new();
        reg.register(Box::new(UnusedVariableLint));
        reg.set_option("unused-variables", "ignore_prefix", "_");
        assert_eq!(
            reg.get_option("unused-variables", "ignore_prefix"),
            Some("_")
        );
        assert_eq!(reg.get_option("unused-variables", "nonexistent"), None);
    }

    #[test]
    fn pq9_8_performance_budget_ok() {
        let lint = UnusedVariableLint;
        let result = run_plugin_with_budget(&lint, "let x = 42\nprintln(x)", "test.fj", 1000);
        assert!(result.is_ok());
    }

    #[test]
    fn pq9_9_naming_convention() {
        let lint = NamingConventionLint;
        let source = "fn myFunction() {}\nstruct point {}";
        let diags = lint.on_ast(source, "test.fj");
        assert!(
            diags.iter().any(|d| d.message.contains("snake_case")),
            "should flag camelCase function"
        );
        assert!(
            diags.iter().any(|d| d.message.contains("PascalCase")),
            "should flag lowercase struct"
        );
    }

    #[test]
    fn pq9_9_naming_convention_correct() {
        let lint = NamingConventionLint;
        let source = "fn my_function() {}\nstruct MyStruct {}";
        let diags = lint.on_ast(source, "test.fj");
        assert!(diags.is_empty(), "correct naming should have no warnings");
    }

    #[test]
    fn pq9_9_complexity_lint() {
        let lint = ComplexityLint { max_lines: 5 };
        // 10-line function should trigger
        let source = "fn big() {\n1\n2\n3\n4\n5\n6\n7\n8\n}";
        let diags = lint.on_ast(source, "test.fj");
        assert!(
            diags.iter().any(|d| d.message.contains("lines")),
            "should warn about long function"
        );
    }

    #[test]
    fn pq9_9_complexity_lint_ok() {
        let lint = ComplexityLint { max_lines: 50 };
        let source = "fn small() {\n    42\n}";
        let diags = lint.on_ast(source, "test.fj");
        assert!(diags.is_empty(), "small function should not trigger");
    }

    #[test]
    fn pq9_9_security_lint() {
        let lint = SecurityLint;
        let source = "let password = \"hunter2\"\nlet api_key = \"sk-abc123\"";
        let diags = lint.on_ast(source, "test.fj");
        assert!(
            diags.iter().any(|d| d.message.contains("password")),
            "should detect password"
        );
        assert!(
            diags.iter().any(|d| d.message.contains("api_key")),
            "should detect api_key"
        );
        assert!(
            diags
                .iter()
                .all(|d| d.severity == DiagnosticSeverity::Error),
            "security issues should be errors"
        );
    }

    #[test]
    fn pq9_9_security_lint_clean() {
        let lint = SecurityLint;
        let source = "let name = \"Fajar\"\nlet count = 42";
        let diags = lint.on_ast(source, "test.fj");
        assert!(diags.is_empty(), "no secrets should produce no warnings");
    }

    #[test]
    fn pq9_9_default_registry() {
        let reg = default_registry();
        assert_eq!(reg.len(), 5, "should have 5 built-in plugins");
        let names = reg.plugin_names();
        assert!(names.contains(&"unused-variables"));
        assert!(names.contains(&"todo-comments"));
        assert!(names.contains(&"naming-convention"));
        assert!(names.contains(&"complexity"));
        assert!(names.contains(&"security"));
    }

    #[test]
    fn pq9_9_all_plugins_on_real_code() {
        let reg = default_registry();
        let source = r#"fn myBadName() {
    let password = "secret123"
    let unused = 42
    // TODO: fix this
}

struct point { x: i64 }
"#;
        let diags = reg.run_ast_phase(source, "test.fj");
        // Should find: camelCase fn, hardcoded password, unused var, TODO, lowercase struct
        assert!(
            diags.len() >= 4,
            "should find 4+ issues, got {}: {:?}",
            diags.len(),
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn pq9_1_load_nonexistent_plugin() {
        let result = load_plugin_from_path("/nonexistent/plugin.so");
        assert!(result.is_err());
        match result {
            Err(e) => assert!(e.contains("not found"), "error: {e}"),
            Ok(_) => panic!("should fail for nonexistent path"),
        }
    }

    #[test]
    fn pq9_4_discover_empty_dir() {
        let dir = format!(
            "{}/fj_plugin_test_{}",
            crate::stdlib_v3::system::temp_dir(),
            std::process::id()
        );
        let _ = std::fs::create_dir_all(&dir);
        let plugins = discover_plugins(&dir);
        assert!(plugins.is_empty(), "empty dir should have no plugins");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pq9_4_discover_nonexistent_dir() {
        let plugins = discover_plugins("/nonexistent/plugin/dir");
        assert!(plugins.is_empty());
    }

    #[test]
    fn pq9_7_code_fix_display() {
        let fix = CodeFix {
            file: "main.fj".to_string(),
            line: 5,
            original: "let Password".to_string(),
            replacement: "let _password".to_string(),
            description: "prefix unused variable".to_string(),
        };
        let s = format!("{fix}");
        assert!(s.contains("main.fj:5"));
        assert!(s.contains("Password"));
    }
}
