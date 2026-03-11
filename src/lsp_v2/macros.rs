//! Macro expansion — declarative macro system, expansion engine, preview,
//! step-through, hygiene, error mapping, completion, documentation.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S30.1: Macro System Design
// ═══════════════════════════════════════════════════════════════════════

/// A fragment specifier for macro pattern variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FragmentSpec {
    /// `$x:expr` — any expression.
    Expr,
    /// `$x:ident` — an identifier.
    Ident,
    /// `$x:ty` — a type.
    Ty,
    /// `$x:pat` — a pattern.
    Pat,
    /// `$x:stmt` — a statement.
    Stmt,
    /// `$x:block` — a block.
    Block,
    /// `$x:literal` — a literal.
    Literal,
    /// `$x:tt` — a single token tree.
    Tt,
}

impl fmt::Display for FragmentSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FragmentSpec::Expr => write!(f, "expr"),
            FragmentSpec::Ident => write!(f, "ident"),
            FragmentSpec::Ty => write!(f, "ty"),
            FragmentSpec::Pat => write!(f, "pat"),
            FragmentSpec::Stmt => write!(f, "stmt"),
            FragmentSpec::Block => write!(f, "block"),
            FragmentSpec::Literal => write!(f, "literal"),
            FragmentSpec::Tt => write!(f, "tt"),
        }
    }
}

/// A token in the macro pattern/template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroToken {
    /// A literal token (punctuation, keyword).
    Literal(String),
    /// A captured variable: `$name:spec`.
    Capture(String, FragmentSpec),
    /// A repetition: `$($tokens),*`.
    Repetition {
        tokens: Vec<MacroToken>,
        separator: Option<String>,
        kind: RepetitionKind,
    },
}

/// Repetition quantifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepetitionKind {
    /// `*` — zero or more.
    ZeroOrMore,
    /// `+` — one or more.
    OneOrMore,
    /// `?` — zero or one.
    ZeroOrOne,
}

/// A single pattern arm in a macro_rules! definition.
#[derive(Debug, Clone)]
pub struct MacroArm {
    /// The pattern to match.
    pub pattern: Vec<MacroToken>,
    /// The expansion template.
    pub template: Vec<MacroToken>,
}

/// A declarative macro definition.
#[derive(Debug, Clone)]
pub struct MacroDefinition {
    /// Macro name.
    pub name: String,
    /// Pattern arms.
    pub arms: Vec<MacroArm>,
    /// Documentation string.
    pub doc: Option<String>,
    /// Source file.
    pub file: String,
    /// Line number.
    pub line: usize,
}

// ═══════════════════════════════════════════════════════════════════════
// S30.2: Macro Expansion Engine
// ═══════════════════════════════════════════════════════════════════════

/// Captured bindings from pattern matching.
#[derive(Debug, Clone, Default)]
pub struct MacroBindings {
    /// Single captures.
    singles: HashMap<String, String>,
    /// Repeated captures.
    repeated: HashMap<String, Vec<String>>,
}

impl MacroBindings {
    /// Creates empty bindings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a single capture.
    pub fn bind(&mut self, name: &str, value: &str) {
        self.singles.insert(name.into(), value.into());
    }

    /// Binds a repeated capture.
    pub fn bind_repeated(&mut self, name: &str, values: Vec<String>) {
        self.repeated.insert(name.into(), values);
    }

    /// Looks up a single binding.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.singles.get(name).map(|s| s.as_str())
    }

    /// Looks up a repeated binding.
    pub fn get_repeated(&self, name: &str) -> Option<&[String]> {
        self.repeated.get(name).map(|v| v.as_slice())
    }
}

/// Result of macro expansion.
#[derive(Debug, Clone)]
pub struct ExpansionResult {
    /// The expanded source text.
    pub expanded: String,
    /// Which arm was matched (0-indexed).
    pub matched_arm: usize,
    /// Captured bindings.
    pub bindings: MacroBindings,
}

/// Error during macro expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroError {
    /// No arm matched.
    NoMatch { macro_name: String, input: String },
    /// Recursion depth exceeded.
    RecursionLimit { macro_name: String, depth: usize },
    /// Undefined variable in template.
    UndefinedVar {
        var_name: String,
        macro_name: String,
    },
    /// Hygiene conflict.
    HygieneConflict { name: String, macro_name: String },
}

impl fmt::Display for MacroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MacroError::NoMatch { macro_name, input } => {
                write!(f, "no rules matched `{macro_name}!({input})`")
            }
            MacroError::RecursionLimit { macro_name, depth } => {
                write!(
                    f,
                    "recursion limit ({depth}) reached expanding `{macro_name}!`"
                )
            }
            MacroError::UndefinedVar {
                var_name,
                macro_name,
            } => {
                write!(
                    f,
                    "undefined variable `${var_name}` in macro `{macro_name}`"
                )
            }
            MacroError::HygieneConflict { name, macro_name } => {
                write!(
                    f,
                    "hygiene conflict: `{name}` clashes in macro `{macro_name}`"
                )
            }
        }
    }
}

/// Attempts to match input tokens against a pattern and expand.
pub fn try_expand(
    def: &MacroDefinition,
    input: &[&str],
    bindings: &MacroBindings,
) -> Result<ExpansionResult, MacroError> {
    for (idx, arm) in def.arms.iter().enumerate() {
        if let Some(result) = try_match_arm(def, arm, input, bindings, idx) {
            return Ok(result);
        }
    }
    Err(MacroError::NoMatch {
        macro_name: def.name.clone(),
        input: input.join(" "),
    })
}

fn try_match_arm(
    _def: &MacroDefinition,
    arm: &MacroArm,
    input: &[&str],
    base_bindings: &MacroBindings,
    arm_idx: usize,
) -> Option<ExpansionResult> {
    let mut bindings = base_bindings.clone();
    let mut pos = 0;

    for token in &arm.pattern {
        match token {
            MacroToken::Literal(lit) => {
                if pos >= input.len() || input[pos] != lit.as_str() {
                    return None;
                }
                pos += 1;
            }
            MacroToken::Capture(name, _spec) => {
                if pos >= input.len() {
                    return None;
                }
                bindings.bind(name, input[pos]);
                pos += 1;
            }
            MacroToken::Repetition { .. } => {
                // Consume remaining tokens as repeated
                let remaining: Vec<String> = input[pos..].iter().map(|s| s.to_string()).collect();
                if let Some(cap_name) = find_capture_in_tokens(&arm.pattern) {
                    bindings.bind_repeated(&cap_name, remaining);
                }
                pos = input.len();
            }
        }
    }

    if pos != input.len() {
        return None;
    }

    // Substitute template
    let expanded = substitute_template(&arm.template, &bindings);
    Some(ExpansionResult {
        expanded,
        matched_arm: arm_idx,
        bindings,
    })
}

fn find_capture_in_tokens(tokens: &[MacroToken]) -> Option<String> {
    for token in tokens {
        if let MacroToken::Repetition { tokens: inner, .. } = token {
            for t in inner {
                if let MacroToken::Capture(name, _) = t {
                    return Some(name.clone());
                }
            }
        }
    }
    None
}

fn substitute_template(template: &[MacroToken], bindings: &MacroBindings) -> String {
    let mut result = String::new();
    for token in template {
        match token {
            MacroToken::Literal(lit) => {
                if !result.is_empty() {
                    result.push(' ');
                }
                result.push_str(lit);
            }
            MacroToken::Capture(name, _) => {
                if let Some(val) = bindings.get(name) {
                    if !result.is_empty() {
                        result.push(' ');
                    }
                    result.push_str(val);
                }
            }
            MacroToken::Repetition {
                tokens, separator, ..
            } => {
                // Find the repeated capture
                let cap_name = tokens.iter().find_map(|t| {
                    if let MacroToken::Capture(name, _) = t {
                        Some(name.clone())
                    } else {
                        None
                    }
                });
                if let Some(name) = cap_name {
                    if let Some(values) = bindings.get_repeated(&name) {
                        let sep = separator.as_deref().unwrap_or(" ");
                        let joined = values.join(sep);
                        if !result.is_empty() {
                            result.push(' ');
                        }
                        result.push_str(&joined);
                    }
                }
            }
        }
    }
    result
}

// ═══════════════════════════════════════════════════════════════════════
// S30.3: Expansion Preview
// ═══════════════════════════════════════════════════════════════════════

/// A virtual document showing macro expansion result.
#[derive(Debug, Clone)]
pub struct ExpansionPreview {
    /// Original macro invocation.
    pub invocation: String,
    /// Expanded output.
    pub expanded: String,
    /// Virtual document URI.
    pub uri: String,
}

/// Generates an expansion preview for LSP.
pub fn preview_expansion(
    def: &MacroDefinition,
    input: &[&str],
) -> Result<ExpansionPreview, MacroError> {
    let result = try_expand(def, input, &MacroBindings::new())?;
    Ok(ExpansionPreview {
        invocation: format!("{}!({})", def.name, input.join(", ")),
        expanded: result.expanded,
        uri: format!("fajar-expand:///{}", def.name),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S30.4: Step-Through Expansion
// ═══════════════════════════════════════════════════════════════════════

/// A single step in macro expansion.
#[derive(Debug, Clone)]
pub struct ExpansionStep {
    /// Step number (1-indexed).
    pub step: usize,
    /// Description of what happened.
    pub description: String,
    /// State after this step.
    pub state: String,
}

/// Expands a macro step-by-step for IDE visualization.
pub fn step_through_expansion(
    def: &MacroDefinition,
    input: &[&str],
) -> Result<Vec<ExpansionStep>, MacroError> {
    let mut steps = Vec::new();

    steps.push(ExpansionStep {
        step: 1,
        description: "Parse invocation".into(),
        state: format!("{}!({})", def.name, input.join(", ")),
    });

    let result = try_expand(def, input, &MacroBindings::new())?;

    steps.push(ExpansionStep {
        step: 2,
        description: format!("Match arm #{}", result.matched_arm),
        state: format!("Matched arm {} with bindings", result.matched_arm),
    });

    // Show each binding
    let mut step_num = 3;
    for (name, val) in &result.bindings.singles {
        steps.push(ExpansionStep {
            step: step_num,
            description: format!("Bind ${name} = {val}"),
            state: format!("${name} → {val}"),
        });
        step_num += 1;
    }

    steps.push(ExpansionStep {
        step: step_num,
        description: "Substitute template".into(),
        state: result.expanded,
    });

    Ok(steps)
}

// ═══════════════════════════════════════════════════════════════════════
// S30.5: Macro Hygiene
// ═══════════════════════════════════════════════════════════════════════

/// A hygiene context for scoping macro-generated identifiers.
#[derive(Debug, Clone)]
pub struct HygieneContext {
    /// Macro-scoped identifier counter.
    counter: usize,
    /// Renamed identifiers: original -> hygienic.
    renames: HashMap<String, String>,
}

impl HygieneContext {
    /// Creates a new hygiene context.
    pub fn new() -> Self {
        Self {
            counter: 0,
            renames: HashMap::new(),
        }
    }

    /// Generates a hygienic name for a macro-local identifier.
    pub fn freshen(&mut self, name: &str) -> String {
        let fresh = format!("__{name}_hygiene_{}", self.counter);
        self.counter += 1;
        self.renames.insert(name.into(), fresh.clone());
        fresh
    }

    /// Looks up the hygienic name for an identifier.
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.renames.get(name).map(|s| s.as_str())
    }

    /// Checks if a user-defined name conflicts with macro-generated names.
    pub fn check_conflict(&self, user_name: &str) -> bool {
        self.renames.values().any(|v| v == user_name)
    }

    /// Returns the number of freshened identifiers.
    pub fn rename_count(&self) -> usize {
        self.renames.len()
    }
}

impl Default for HygieneContext {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S30.6: Error Locations
// ═══════════════════════════════════════════════════════════════════════

/// Source location mapping for macro-expanded code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroSourceMap {
    /// The macro name.
    pub macro_name: String,
    /// Invocation site (file, line, col).
    pub invocation_site: (String, usize, usize),
    /// Definition site (file, line, col).
    pub definition_site: (String, usize, usize),
    /// Expanded code range (start, end offset in expanded output).
    pub expanded_range: (usize, usize),
}

/// Maps an error in expanded code back to its origin.
pub fn map_error_location(source_map: &MacroSourceMap, _error_offset: usize) -> ErrorOrigin {
    ErrorOrigin {
        macro_name: source_map.macro_name.clone(),
        invocation_file: source_map.invocation_site.0.clone(),
        invocation_line: source_map.invocation_site.1,
        definition_file: source_map.definition_site.0.clone(),
        definition_line: source_map.definition_site.1,
    }
}

/// Error origin after mapping through macro expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorOrigin {
    /// Macro that caused the error.
    pub macro_name: String,
    /// File where macro was invoked.
    pub invocation_file: String,
    /// Line where macro was invoked.
    pub invocation_line: usize,
    /// File where macro is defined.
    pub definition_file: String,
    /// Line where macro is defined.
    pub definition_line: usize,
}

impl fmt::Display for ErrorOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "in expansion of `{}!` (invoked at {}:{}, defined at {}:{})",
            self.macro_name,
            self.invocation_file,
            self.invocation_line,
            self.definition_file,
            self.definition_line,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S30.7: Macro Completion
// ═══════════════════════════════════════════════════════════════════════

/// A completion item for a macro invocation.
#[derive(Debug, Clone)]
pub struct MacroCompletionItem {
    /// Macro name.
    pub name: String,
    /// Completion label (e.g., `vec![...]`).
    pub label: String,
    /// Insert text (snippet).
    pub insert_text: String,
    /// Detail string.
    pub detail: String,
}

/// Generates completion items for available macros.
pub fn complete_macros(macros: &[MacroDefinition], prefix: &str) -> Vec<MacroCompletionItem> {
    macros
        .iter()
        .filter(|m| m.name.starts_with(prefix))
        .map(|m| {
            let pattern_hint = m
                .arms
                .first()
                .map(|arm| {
                    arm.pattern
                        .iter()
                        .map(|t| match t {
                            MacroToken::Capture(name, spec) => format!("${name}:{spec}"),
                            MacroToken::Literal(l) => l.clone(),
                            MacroToken::Repetition { .. } => "...".into(),
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            MacroCompletionItem {
                name: m.name.clone(),
                label: format!("{}!(...)", m.name),
                insert_text: format!("{}!(${{1}})", m.name),
                detail: format!("macro_rules! {} ({})", m.name, pattern_hint),
            }
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S30.8: Macro Documentation
// ═══════════════════════════════════════════════════════════════════════

/// Documentation entry for a macro.
#[derive(Debug, Clone)]
pub struct MacroDocEntry {
    /// Macro name.
    pub name: String,
    /// Documentation string.
    pub doc: String,
    /// Pattern arm descriptions.
    pub arms: Vec<String>,
    /// Example invocations and their expansions.
    pub examples: Vec<(String, String)>,
}

/// Generates hover documentation for a macro.
pub fn macro_hover_doc(def: &MacroDefinition) -> MacroDocEntry {
    let arms: Vec<String> = def
        .arms
        .iter()
        .map(|arm| {
            let pattern = arm
                .pattern
                .iter()
                .map(|t| match t {
                    MacroToken::Literal(l) => l.clone(),
                    MacroToken::Capture(n, s) => format!("${n}:{s}"),
                    MacroToken::Repetition { kind, .. } => {
                        let q = match kind {
                            RepetitionKind::ZeroOrMore => "*",
                            RepetitionKind::OneOrMore => "+",
                            RepetitionKind::ZeroOrOne => "?",
                        };
                        format!("$(...){q}")
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("({pattern}) => {{ ... }}")
        })
        .collect();

    MacroDocEntry {
        name: def.name.clone(),
        doc: def
            .doc
            .clone()
            .unwrap_or_else(|| format!("Macro `{}`", def.name)),
        arms,
        examples: vec![],
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S30.9: Recursive Macros
// ═══════════════════════════════════════════════════════════════════════

/// Default recursion depth limit.
pub const DEFAULT_RECURSION_LIMIT: usize = 128;

/// Configuration for macro expansion.
#[derive(Debug, Clone)]
pub struct MacroExpansionConfig {
    /// Maximum recursion depth.
    pub recursion_limit: usize,
    /// Whether to enable hygiene.
    pub hygiene: bool,
    /// Whether to track source maps.
    pub source_maps: bool,
}

impl Default for MacroExpansionConfig {
    fn default() -> Self {
        Self {
            recursion_limit: DEFAULT_RECURSION_LIMIT,
            hygiene: true,
            source_maps: true,
        }
    }
}

/// Expands a macro with recursion tracking.
pub fn expand_recursive(
    defs: &HashMap<String, MacroDefinition>,
    macro_name: &str,
    input: &[&str],
    config: &MacroExpansionConfig,
    depth: usize,
) -> Result<String, MacroError> {
    if depth >= config.recursion_limit {
        return Err(MacroError::RecursionLimit {
            macro_name: macro_name.into(),
            depth,
        });
    }

    let def = defs.get(macro_name).ok_or_else(|| MacroError::NoMatch {
        macro_name: macro_name.into(),
        input: input.join(" "),
    })?;

    let result = try_expand(def, input, &MacroBindings::new())?;

    // Check if the expansion contains another macro call
    let expanded = &result.expanded;
    for other_name in defs.keys() {
        let invocation = format!("{other_name}!");
        if expanded.contains(&invocation) {
            // Simplified: just note recursion is needed
            return expand_recursive(defs, other_name, &[], config, depth + 1);
        }
    }

    Ok(result.expanded)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_macro() -> MacroDefinition {
        MacroDefinition {
            name: "my_vec".into(),
            arms: vec![MacroArm {
                pattern: vec![MacroToken::Capture("val".into(), FragmentSpec::Expr)],
                template: vec![
                    MacroToken::Literal("vec".into()),
                    MacroToken::Literal("[".into()),
                    MacroToken::Capture("val".into(), FragmentSpec::Expr),
                    MacroToken::Literal("]".into()),
                ],
            }],
            doc: Some("Creates a vector".into()),
            file: "lib.fj".into(),
            line: 1,
        }
    }

    fn make_two_arg_macro() -> MacroDefinition {
        MacroDefinition {
            name: "pair".into(),
            arms: vec![MacroArm {
                pattern: vec![
                    MacroToken::Capture("a".into(), FragmentSpec::Expr),
                    MacroToken::Literal(",".into()),
                    MacroToken::Capture("b".into(), FragmentSpec::Expr),
                ],
                template: vec![
                    MacroToken::Literal("(".into()),
                    MacroToken::Capture("a".into(), FragmentSpec::Expr),
                    MacroToken::Literal(",".into()),
                    MacroToken::Capture("b".into(), FragmentSpec::Expr),
                    MacroToken::Literal(")".into()),
                ],
            }],
            doc: None,
            file: "lib.fj".into(),
            line: 5,
        }
    }

    // S30.1 — Macro System Design
    #[test]
    fn s30_1_fragment_spec_display() {
        assert_eq!(FragmentSpec::Expr.to_string(), "expr");
        assert_eq!(FragmentSpec::Ident.to_string(), "ident");
        assert_eq!(FragmentSpec::Ty.to_string(), "ty");
    }

    #[test]
    fn s30_1_macro_definition_structure() {
        let def = make_simple_macro();
        assert_eq!(def.name, "my_vec");
        assert_eq!(def.arms.len(), 1);
        assert!(def.doc.is_some());
    }

    // S30.2 — Macro Expansion Engine
    #[test]
    fn s30_2_simple_expansion() {
        let def = make_simple_macro();
        let result = try_expand(&def, &["42"], &MacroBindings::new()).unwrap();
        assert!(result.expanded.contains("42"));
        assert_eq!(result.matched_arm, 0);
    }

    #[test]
    fn s30_2_two_arg_expansion() {
        let def = make_two_arg_macro();
        let result = try_expand(&def, &["x", ",", "y"], &MacroBindings::new()).unwrap();
        assert!(result.expanded.contains("x"));
        assert!(result.expanded.contains("y"));
    }

    #[test]
    fn s30_2_no_match_error() {
        let def = make_simple_macro();
        let err = try_expand(&def, &["a", "b", "c"], &MacroBindings::new()).unwrap_err();
        assert!(matches!(err, MacroError::NoMatch { .. }));
    }

    // S30.3 — Expansion Preview
    #[test]
    fn s30_3_preview() {
        let def = make_simple_macro();
        let preview = preview_expansion(&def, &["42"]).unwrap();
        assert!(preview.uri.contains("my_vec"));
        assert!(preview.expanded.contains("42"));
    }

    // S30.4 — Step-Through Expansion
    #[test]
    fn s30_4_step_through() {
        let def = make_simple_macro();
        let steps = step_through_expansion(&def, &["42"]).unwrap();
        assert!(steps.len() >= 3);
        assert_eq!(steps[0].step, 1);
        assert!(steps[0].description.contains("Parse"));
    }

    // S30.5 — Macro Hygiene
    #[test]
    fn s30_5_hygiene_freshen() {
        let mut ctx = HygieneContext::new();
        let fresh = ctx.freshen("tmp");
        assert!(fresh.contains("tmp"));
        assert!(fresh.contains("hygiene"));
        assert_eq!(ctx.rename_count(), 1);
    }

    #[test]
    fn s30_5_hygiene_no_clash() {
        let mut ctx = HygieneContext::new();
        let fresh1 = ctx.freshen("x");
        let fresh2 = ctx.freshen("x");
        assert_ne!(fresh1, fresh2);
    }

    #[test]
    fn s30_5_hygiene_conflict_detection() {
        let mut ctx = HygieneContext::new();
        let fresh = ctx.freshen("tmp");
        assert!(ctx.check_conflict(&fresh));
        assert!(!ctx.check_conflict("user_var"));
    }

    // S30.6 — Error Locations
    #[test]
    fn s30_6_error_origin_mapping() {
        let source_map = MacroSourceMap {
            macro_name: "vec".into(),
            invocation_site: ("main.fj".into(), 10, 5),
            definition_site: ("macros.fj".into(), 1, 1),
            expanded_range: (0, 50),
        };
        let origin = map_error_location(&source_map, 25);
        assert_eq!(origin.macro_name, "vec");
        assert_eq!(origin.invocation_line, 10);
        assert_eq!(origin.definition_line, 1);
    }

    #[test]
    fn s30_6_error_origin_display() {
        let origin = ErrorOrigin {
            macro_name: "assert_eq".into(),
            invocation_file: "test.fj".into(),
            invocation_line: 5,
            definition_file: "core.fj".into(),
            definition_line: 100,
        };
        let s = origin.to_string();
        assert!(s.contains("assert_eq!"));
        assert!(s.contains("test.fj:5"));
        assert!(s.contains("core.fj:100"));
    }

    // S30.7 — Macro Completion
    #[test]
    fn s30_7_completion() {
        let macros = vec![make_simple_macro(), make_two_arg_macro()];
        let items = complete_macros(&macros, "my");
        assert_eq!(items.len(), 1);
        assert!(items[0].label.contains("my_vec"));
    }

    #[test]
    fn s30_7_completion_empty_prefix() {
        let macros = vec![make_simple_macro(), make_two_arg_macro()];
        let items = complete_macros(&macros, "");
        assert_eq!(items.len(), 2);
    }

    // S30.8 — Macro Documentation
    #[test]
    fn s30_8_hover_doc() {
        let def = make_simple_macro();
        let doc = macro_hover_doc(&def);
        assert_eq!(doc.name, "my_vec");
        assert!(doc.doc.contains("Creates a vector"));
        assert_eq!(doc.arms.len(), 1);
    }

    // S30.9 — Recursive Macros
    #[test]
    fn s30_9_recursion_limit() {
        let mut defs = HashMap::new();
        // Create a macro that references itself
        let def = MacroDefinition {
            name: "recurse".into(),
            arms: vec![MacroArm {
                pattern: vec![],
                template: vec![MacroToken::Literal("recurse!()".into())],
            }],
            doc: None,
            file: "lib.fj".into(),
            line: 1,
        };
        defs.insert("recurse".into(), def);

        let config = MacroExpansionConfig {
            recursion_limit: 5,
            ..Default::default()
        };
        let err = expand_recursive(&defs, "recurse", &[], &config, 0).unwrap_err();
        assert!(matches!(err, MacroError::RecursionLimit { depth: 5, .. }));
    }

    #[test]
    fn s30_9_default_recursion_limit() {
        assert_eq!(DEFAULT_RECURSION_LIMIT, 128);
        let config = MacroExpansionConfig::default();
        assert_eq!(config.recursion_limit, 128);
        assert!(config.hygiene);
    }

    // S30.10 — Integration
    #[test]
    fn s30_10_bindings_api() {
        let mut bindings = MacroBindings::new();
        bindings.bind("x", "42");
        bindings.bind_repeated("items", vec!["a".into(), "b".into()]);
        assert_eq!(bindings.get("x"), Some("42"));
        assert_eq!(bindings.get_repeated("items").unwrap().len(), 2);
        assert_eq!(bindings.get("y"), None);
    }

    #[test]
    fn s30_10_macro_error_display() {
        let err = MacroError::UndefinedVar {
            var_name: "x".into(),
            macro_name: "test".into(),
        };
        assert!(err.to_string().contains("undefined variable"));
    }
}
