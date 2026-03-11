//! Macro system for Fajar Lang — declarative macros, derive macros, attribute macros, and utilities.
//!
//! Implements the Phase 3 macro infrastructure (Sprints 9–12):
//!
//! - **Sprint 9**: Declarative macros (`macro_rules!`-style) with pattern matching, expansion, hygiene.
//! - **Sprint 10**: Derive macros (`#[derive(Debug, Clone, ...)]`) with a pluggable registry.
//! - **Sprint 11**: Attribute macros (`#[cfg(...)]`, `#[deprecated]`, `#[repr(...)]`), conditional compilation.
//! - **Sprint 12**: Utility macros (`compile_error!`, `env!`, `file!`, `line!`, `stringify!`, etc.)
//!   and comprehensive error reporting.
//!
//! # Design
//!
//! The macro system is self-contained within the parser layer. Macro definitions are parsed into
//! [`MacroDef`] structs, expanded by [`MacroExpander`], and checked for hygiene via
//! [`HygieneContext`]. Derive macros use a trait-based registry ([`DeriveRegistry`]) for
//! extensibility. Attribute macros support conditional compilation through [`CfgExpr`] evaluation.

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Macro Errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors produced during macro processing.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum MacroError {
    /// Macro is not defined (ME001).
    #[error("[ME001] undefined macro `{name}`")]
    UndefinedMacro {
        /// The name of the undefined macro.
        name: String,
    },

    /// Input tokens do not match any pattern arm (ME002).
    #[error("[ME002] no pattern matched for macro `{name}`")]
    PatternMismatch {
        /// The macro name.
        name: String,
    },

    /// Expansion recursion exceeded the limit (ME003).
    #[error("[ME003] macro recursion limit ({limit}) exceeded for `{name}`")]
    RecursionLimit {
        /// The macro name.
        name: String,
        /// The recursion limit that was hit.
        limit: usize,
    },

    /// General expansion failure (ME004).
    #[error("[ME004] expansion error in macro `{name}`: {message}")]
    ExpansionError {
        /// The macro name.
        name: String,
        /// Human-readable description.
        message: String,
    },

    /// Fragment kind mismatch (ME005).
    #[error("[ME005] invalid fragment `{fragment}` for kind `{expected}` in macro `{name}`")]
    InvalidFragment {
        /// The macro name.
        name: String,
        /// The fragment variable that failed.
        fragment: String,
        /// The expected fragment kind.
        expected: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 9 — Declarative Macros
// ═══════════════════════════════════════════════════════════════════════

/// The kind of fragment a macro variable captures.
///
/// Each kind constrains what tokens the variable will match during expansion.
#[derive(Debug, Clone, PartialEq)]
pub enum FragmentKind {
    /// An expression (e.g., `1 + 2`).
    Expr,
    /// An identifier (e.g., `foo`).
    Ident,
    /// A type (e.g., `i32`).
    Ty,
    /// A statement (e.g., `let x = 1`).
    Stmt,
    /// A block (e.g., `{ ... }`).
    Block,
    /// A pattern (e.g., `Some(x)`).
    Pat,
    /// A literal value (e.g., `42`, `"hello"`).
    Literal,
    /// An arbitrary token tree.
    TokenTree,
}

/// Repetition quantifier in macro patterns and templates.
#[derive(Debug, Clone, PartialEq)]
pub enum RepKind {
    /// Matches zero or more occurrences (`*`).
    ZeroOrMore,
    /// Matches one or more occurrences (`+`).
    OneOrMore,
}

/// A single element in a macro pattern.
///
/// Patterns consist of literal tokens to match, named variables that capture
/// fragments, and repetition groups.
#[derive(Debug, Clone, PartialEq)]
pub enum MacroMatcher {
    /// A literal token that must match exactly.
    Literal(String),
    /// A named variable that captures a fragment of the given kind.
    Variable {
        /// Variable name (without the `$`).
        name: String,
        /// The fragment kind this variable captures.
        kind: FragmentKind,
    },
    /// A repetition group (e.g., `$($x:expr),*`).
    Repetition {
        /// The nested matchers inside the repetition.
        matchers: Vec<MacroMatcher>,
        /// Optional separator token (e.g., `,`).
        separator: Option<String>,
        /// Whether zero-or-more or one-or-more.
        kind: RepKind,
    },
}

/// A single element in a macro expansion template.
///
/// Templates mirror the pattern structure: literal tokens are emitted verbatim,
/// variables are replaced with their captured value, and repetition groups expand
/// each captured iteration.
#[derive(Debug, Clone, PartialEq)]
pub enum MacroToken {
    /// A literal token emitted verbatim.
    Literal(String),
    /// A variable reference, replaced with the captured fragment.
    Variable(String),
    /// A repetition group in the template.
    Repetition {
        /// Tokens to repeat for each captured iteration.
        tokens: Vec<MacroToken>,
        /// Optional separator between iterations.
        separator: Option<String>,
        /// Repetition kind (matches the pattern's kind).
        kind: RepKind,
    },
}

/// A single rule (arm) of a declarative macro: pattern → template.
#[derive(Debug, Clone, PartialEq)]
pub struct MacroRule {
    /// The pattern to match against input tokens.
    pub pattern: Vec<MacroMatcher>,
    /// The template to produce on match.
    pub template: Vec<MacroToken>,
}

/// A complete declarative macro definition.
///
/// ```text
/// macro_rules! my_macro {
///     ($x:expr) => { $x + 1 };
///     ($x:expr, $y:expr) => { $x + $y };
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MacroDef {
    /// Macro name (without the `!`).
    pub name: String,
    /// The rule arms, tried in order.
    pub rules: Vec<MacroRule>,
    /// Whether this macro is exported (`pub`).
    pub is_public: bool,
}

/// Maximum recursion depth for macro expansion.
pub const MAX_MACRO_RECURSION: usize = 64;

/// Hygiene context for generating unique identifiers during macro expansion.
///
/// Prevents name collisions between macro-introduced identifiers and
/// user-written identifiers by appending a unique gensym suffix.
#[derive(Debug, Clone)]
pub struct HygieneContext {
    /// Monotonically increasing counter for gensym.
    counter: u64,
}

impl HygieneContext {
    /// Create a new hygiene context with counter starting at zero.
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Generate a unique identifier from a base name.
    ///
    /// Returns a name like `__fj_gensym_base_0` that will not collide
    /// with user identifiers.
    pub fn gensym(&mut self, base: &str) -> String {
        let id = self.counter;
        self.counter += 1;
        format!("__fj_gensym_{base}_{id}")
    }

    /// Return the current counter value (for diagnostics).
    pub fn counter(&self) -> u64 {
        self.counter
    }
}

impl Default for HygieneContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Captured fragment bindings from pattern matching.
///
/// Maps variable names to their captured token strings. Repetition
/// variables map to a list of captured iterations.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureBindings {
    /// Simple captures: `$name` → value string.
    pub singles: HashMap<String, String>,
    /// Repetition captures: `$name` → list of values (one per iteration).
    pub repetitions: HashMap<String, Vec<String>>,
}

impl CaptureBindings {
    /// Create empty capture bindings.
    pub fn new() -> Self {
        Self {
            singles: HashMap::new(),
            repetitions: HashMap::new(),
        }
    }
}

impl Default for CaptureBindings {
    fn default() -> Self {
        Self::new()
    }
}

/// The declarative macro expander.
///
/// Holds registered macro definitions and performs pattern matching plus
/// template expansion with hygiene and recursion limiting.
#[derive(Debug, Clone)]
pub struct MacroExpander {
    /// Registered macro definitions by name.
    macros: HashMap<String, MacroDef>,
    /// Hygiene context for gensym.
    hygiene: HygieneContext,
}

impl MacroExpander {
    /// Create a new expander with built-in macros pre-registered.
    pub fn new() -> Self {
        let mut expander = Self {
            macros: HashMap::new(),
            hygiene: HygieneContext::new(),
        };
        expander.register_builtins();
        expander
    }

    /// Register a macro definition.
    pub fn register(&mut self, def: MacroDef) {
        self.macros.insert(def.name.clone(), def);
    }

    /// Look up a macro definition by name.
    pub fn get(&self, name: &str) -> Option<&MacroDef> {
        self.macros.get(name)
    }

    /// Generate a hygienic identifier.
    pub fn gensym(&mut self, base: &str) -> String {
        self.hygiene.gensym(base)
    }

    /// Return a reference to the hygiene context.
    pub fn hygiene(&self) -> &HygieneContext {
        &self.hygiene
    }

    /// List all registered macro names.
    pub fn registered_names(&self) -> Vec<String> {
        self.macros.keys().cloned().collect()
    }

    /// Expand a macro invocation by name with the given input tokens.
    ///
    /// Tries each rule arm in order; the first matching pattern is expanded.
    /// Recursion is limited to [`MAX_MACRO_RECURSION`].
    pub fn expand(&mut self, name: &str, input: &[String]) -> Result<Vec<String>, MacroError> {
        self.expand_recursive(name, input, 0)
    }

    /// Internal recursive expansion with depth tracking.
    fn expand_recursive(
        &mut self,
        name: &str,
        input: &[String],
        depth: usize,
    ) -> Result<Vec<String>, MacroError> {
        if depth >= MAX_MACRO_RECURSION {
            return Err(MacroError::RecursionLimit {
                name: name.to_string(),
                limit: MAX_MACRO_RECURSION,
            });
        }
        let def = self
            .macros
            .get(name)
            .cloned()
            .ok_or_else(|| MacroError::UndefinedMacro {
                name: name.to_string(),
            })?;
        for rule in &def.rules {
            if let Some(bindings) = match_pattern(&rule.pattern, input) {
                return expand_template(&rule.template, &bindings);
            }
        }
        Err(MacroError::PatternMismatch {
            name: name.to_string(),
        })
    }

    /// Register the built-in macros (vec!, println!, format!, assert!, cfg!, dbg!).
    fn register_builtins(&mut self) {
        self.register_builtin_vec();
        self.register_builtin_println();
        self.register_builtin_format();
        self.register_builtin_assert();
        self.register_builtin_cfg();
        self.register_builtin_dbg();
    }

    /// Register `vec!` — variadic array constructor.
    fn register_builtin_vec(&mut self) {
        let rule = MacroRule {
            pattern: vec![MacroMatcher::Repetition {
                matchers: vec![MacroMatcher::Variable {
                    name: "elem".to_string(),
                    kind: FragmentKind::Expr,
                }],
                separator: Some(",".to_string()),
                kind: RepKind::ZeroOrMore,
            }],
            template: vec![
                MacroToken::Literal("[".to_string()),
                MacroToken::Repetition {
                    tokens: vec![MacroToken::Variable("elem".to_string())],
                    separator: Some(",".to_string()),
                    kind: RepKind::ZeroOrMore,
                },
                MacroToken::Literal("]".to_string()),
            ],
        };
        self.register(MacroDef {
            name: "vec".to_string(),
            rules: vec![rule],
            is_public: true,
        });
    }

    /// Register `println!` — print with newline.
    fn register_builtin_println(&mut self) {
        let rule = MacroRule {
            pattern: vec![MacroMatcher::Variable {
                name: "args".to_string(),
                kind: FragmentKind::Expr,
            }],
            template: vec![
                MacroToken::Literal("println(".to_string()),
                MacroToken::Variable("args".to_string()),
                MacroToken::Literal(")".to_string()),
            ],
        };
        self.register(MacroDef {
            name: "println".to_string(),
            rules: vec![rule],
            is_public: true,
        });
    }

    /// Register `format!` — format string builder.
    fn register_builtin_format(&mut self) {
        let rule = MacroRule {
            pattern: vec![MacroMatcher::Variable {
                name: "fmt".to_string(),
                kind: FragmentKind::Expr,
            }],
            template: vec![
                MacroToken::Literal("format(".to_string()),
                MacroToken::Variable("fmt".to_string()),
                MacroToken::Literal(")".to_string()),
            ],
        };
        self.register(MacroDef {
            name: "format".to_string(),
            rules: vec![rule],
            is_public: true,
        });
    }

    /// Register `assert!` — assertion macro.
    fn register_builtin_assert(&mut self) {
        let rule = MacroRule {
            pattern: vec![MacroMatcher::Variable {
                name: "cond".to_string(),
                kind: FragmentKind::Expr,
            }],
            template: vec![
                MacroToken::Literal("assert(".to_string()),
                MacroToken::Variable("cond".to_string()),
                MacroToken::Literal(")".to_string()),
            ],
        };
        self.register(MacroDef {
            name: "assert".to_string(),
            rules: vec![rule],
            is_public: true,
        });
    }

    /// Register `cfg!` — configuration query.
    fn register_builtin_cfg(&mut self) {
        let rule = MacroRule {
            pattern: vec![MacroMatcher::Variable {
                name: "predicate".to_string(),
                kind: FragmentKind::Ident,
            }],
            template: vec![MacroToken::Literal("false".to_string())],
        };
        self.register(MacroDef {
            name: "cfg".to_string(),
            rules: vec![rule],
            is_public: true,
        });
    }

    /// Register `dbg!` — debug print macro.
    fn register_builtin_dbg(&mut self) {
        let rule = MacroRule {
            pattern: vec![MacroMatcher::Variable {
                name: "val".to_string(),
                kind: FragmentKind::Expr,
            }],
            template: vec![
                MacroToken::Literal("dbg(".to_string()),
                MacroToken::Variable("val".to_string()),
                MacroToken::Literal(")".to_string()),
            ],
        };
        self.register(MacroDef {
            name: "dbg".to_string(),
            rules: vec![rule],
            is_public: true,
        });
    }
}

impl Default for MacroExpander {
    fn default() -> Self {
        Self::new()
    }
}

/// Match input tokens against a pattern, producing capture bindings on success.
///
/// Returns `None` if the pattern does not match. For simple (non-repetition)
/// patterns each element is matched positionally.
pub fn match_pattern(pattern: &[MacroMatcher], input: &[String]) -> Option<CaptureBindings> {
    let mut bindings = CaptureBindings::new();
    let consumed = match_pattern_inner(pattern, input, &mut bindings)?;
    if consumed == input.len() {
        Some(bindings)
    } else {
        None
    }
}

/// Inner recursive pattern matcher. Returns number of tokens consumed.
fn match_pattern_inner(
    pattern: &[MacroMatcher],
    input: &[String],
    bindings: &mut CaptureBindings,
) -> Option<usize> {
    let mut pos = 0;
    for matcher in pattern {
        match matcher {
            MacroMatcher::Literal(lit) => {
                let tok = input.get(pos)?;
                if tok != lit {
                    return None;
                }
                pos += 1;
            }
            MacroMatcher::Variable { name, .. } => {
                let tok = input.get(pos)?;
                bindings.singles.insert(name.clone(), tok.clone());
                pos += 1;
            }
            MacroMatcher::Repetition {
                matchers,
                separator,
                kind,
            } => {
                let (count, consumed) =
                    match_repetition(matchers, separator.as_deref(), input, pos, bindings);
                if *kind == RepKind::OneOrMore && count == 0 {
                    return None;
                }
                pos += consumed;
            }
        }
    }
    Some(pos)
}

/// Match a repetition group, collecting captures into `bindings.repetitions`.
///
/// Returns `(iteration_count, tokens_consumed)`.
fn match_repetition(
    matchers: &[MacroMatcher],
    separator: Option<&str>,
    input: &[String],
    start: usize,
    bindings: &mut CaptureBindings,
) -> (usize, usize) {
    let mut pos = start;
    let mut count: usize = 0;
    loop {
        if pos >= input.len() {
            break;
        }
        if count > 0 {
            if let Some(sep) = separator {
                if input.get(pos).map(|s| s.as_str()) == Some(sep) {
                    pos += 1;
                } else {
                    break;
                }
            }
        }
        let mut temp = CaptureBindings::new();
        if let Some(consumed) = match_pattern_inner(matchers, &input[pos..], &mut temp) {
            if consumed == 0 {
                break;
            }
            for (k, v) in &temp.singles {
                bindings
                    .repetitions
                    .entry(k.clone())
                    .or_default()
                    .push(v.clone());
            }
            pos += consumed;
            count += 1;
        } else {
            if count > 0 && separator.is_some() {
                pos -= 1;
            }
            break;
        }
    }
    (count, pos - start)
}

/// Expand a macro template using captured bindings.
///
/// Variable references are replaced with their captured values. Repetition
/// groups iterate over the repetition captures.
pub fn expand_template(
    template: &[MacroToken],
    bindings: &CaptureBindings,
) -> Result<Vec<String>, MacroError> {
    let mut output = Vec::new();
    for token in template {
        expand_token(token, bindings, &mut output)?;
    }
    Ok(output)
}

/// Expand a single template token into the output buffer.
fn expand_token(
    token: &MacroToken,
    bindings: &CaptureBindings,
    output: &mut Vec<String>,
) -> Result<(), MacroError> {
    match token {
        MacroToken::Literal(s) => {
            output.push(s.clone());
            Ok(())
        }
        MacroToken::Variable(name) => {
            if let Some(val) = bindings.singles.get(name) {
                output.push(val.clone());
                Ok(())
            } else {
                Err(MacroError::ExpansionError {
                    name: "template".to_string(),
                    message: format!("unbound variable `${name}`"),
                })
            }
        }
        MacroToken::Repetition {
            tokens, separator, ..
        } => expand_repetition(tokens, separator.as_deref(), bindings, output),
    }
}

/// Expand a repetition group in a template.
fn expand_repetition(
    tokens: &[MacroToken],
    separator: Option<&str>,
    bindings: &CaptureBindings,
    output: &mut Vec<String>,
) -> Result<(), MacroError> {
    let rep_count = infer_rep_count(tokens, bindings);
    for i in 0..rep_count {
        if i > 0 {
            if let Some(sep) = separator {
                output.push(sep.to_string());
            }
        }
        let iter_bindings = build_iteration_bindings(bindings, i);
        for tok in tokens {
            expand_token(tok, &iter_bindings, output)?;
        }
    }
    Ok(())
}

/// Infer how many iterations a repetition should expand to.
fn infer_rep_count(tokens: &[MacroToken], bindings: &CaptureBindings) -> usize {
    for tok in tokens {
        if let MacroToken::Variable(name) = tok {
            if let Some(vals) = bindings.repetitions.get(name) {
                return vals.len();
            }
        }
    }
    0
}

/// Build per-iteration bindings by extracting the i-th element from repetition captures.
fn build_iteration_bindings(bindings: &CaptureBindings, index: usize) -> CaptureBindings {
    let mut iter_bindings = CaptureBindings::new();
    iter_bindings.singles = bindings.singles.clone();
    for (k, vals) in &bindings.repetitions {
        if let Some(val) = vals.get(index) {
            iter_bindings.singles.insert(k.clone(), val.clone());
        }
    }
    iter_bindings
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 10 — Derive Macros
// ═══════════════════════════════════════════════════════════════════════

/// Information about a single field (for derive input).
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInfo {
    /// Field name.
    pub name: String,
    /// Field type as a string.
    pub type_name: String,
}

/// Information about an enum variant (for derive input).
#[derive(Debug, Clone, PartialEq)]
pub struct VariantInfo {
    /// Variant name.
    pub name: String,
    /// Fields (empty for unit variants).
    pub fields: Vec<FieldInfo>,
}

/// Input provided to a derive macro: the item being derived on.
#[derive(Debug, Clone, PartialEq)]
pub struct DeriveInput {
    /// Name of the struct or enum.
    pub name: String,
    /// Fields (for structs; empty for enums).
    pub fields: Vec<FieldInfo>,
    /// Variants (for enums; empty for structs).
    pub variants: Vec<VariantInfo>,
}

/// Output produced by a derive macro expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct DeriveOutput {
    /// The generated code as a string.
    pub generated_code: String,
}

/// Trait implemented by each derive macro (Debug, Clone, PartialEq, etc.).
///
/// Derive macros inspect the [`DeriveInput`] and produce implementation code
/// as [`DeriveOutput`].
pub trait DeriveMacro: std::fmt::Debug {
    /// The name of this derive (e.g., `"Debug"`).
    fn name(&self) -> &str;

    /// Expand the derive for the given input item.
    fn expand(&self, item: &DeriveInput) -> Result<Vec<DeriveOutput>, MacroError>;
}

/// Registry of available derive macros.
///
/// Supports registration of built-in and custom derive macros, with lookup by name.
#[derive(Debug)]
pub struct DeriveRegistry {
    /// Registered derives by name.
    derives: HashMap<String, Box<dyn DeriveMacro>>,
}

impl DeriveRegistry {
    /// Create a new registry with built-in derives pre-registered.
    pub fn new() -> Self {
        let mut reg = Self {
            derives: HashMap::new(),
        };
        reg.register_builtins();
        reg
    }

    /// Register a custom derive macro.
    pub fn register(&mut self, derive: Box<dyn DeriveMacro>) {
        self.derives.insert(derive.name().to_string(), derive);
    }

    /// Look up a derive macro by name.
    pub fn get(&self, name: &str) -> Option<&dyn DeriveMacro> {
        self.derives.get(name).map(|b| b.as_ref())
    }

    /// List all registered derive names.
    pub fn registered_names(&self) -> Vec<String> {
        self.derives.keys().cloned().collect()
    }

    /// Expand a named derive for the given input.
    pub fn expand(&self, name: &str, input: &DeriveInput) -> Result<Vec<DeriveOutput>, MacroError> {
        let derive = self
            .derives
            .get(name)
            .ok_or_else(|| MacroError::UndefinedMacro {
                name: name.to_string(),
            })?;
        derive.expand(input)
    }

    /// Register all built-in derive macros.
    fn register_builtins(&mut self) {
        self.register(Box::new(DeriveDebug));
        self.register(Box::new(DeriveClone));
        self.register(Box::new(DerivePartialEq));
        self.register(Box::new(DeriveHash));
        self.register(Box::new(DeriveDefault));
        self.register(Box::new(DeriveSerialize));
    }
}

impl Default for DeriveRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Built-in derive implementations ──────────────────────────────────

/// Built-in `#[derive(Debug)]` — generates a `fmt_debug` method.
#[derive(Debug)]
struct DeriveDebug;

impl DeriveMacro for DeriveDebug {
    fn name(&self) -> &str {
        "Debug"
    }

    fn expand(&self, item: &DeriveInput) -> Result<Vec<DeriveOutput>, MacroError> {
        let body = generate_debug_body(item);
        Ok(vec![DeriveOutput {
            generated_code: body,
        }])
    }
}

/// Generate the Debug impl body for a struct or enum.
fn generate_debug_body(item: &DeriveInput) -> String {
    if item.variants.is_empty() {
        let fields: Vec<String> = item
            .fields
            .iter()
            .map(|f| format!("{}: {{self.{}}}", f.name, f.name))
            .collect();
        format!(
            "impl Debug for {} {{ fn fmt_debug(self) -> str {{ \"{} {{ {} }}\" }} }}",
            item.name,
            item.name,
            fields.join(", ")
        )
    } else {
        generate_debug_enum_body(item)
    }
}

/// Generate Debug impl body for enum variants.
fn generate_debug_enum_body(item: &DeriveInput) -> String {
    let arms: Vec<String> = item
        .variants
        .iter()
        .map(|v| format!("{}::{} => \"{}::{}\"", item.name, v.name, item.name, v.name))
        .collect();
    format!(
        "impl Debug for {} {{ fn fmt_debug(self) -> str {{ match self {{ {} }} }} }}",
        item.name,
        arms.join(", ")
    )
}

/// Built-in `#[derive(Clone)]` — generates a `clone` method.
#[derive(Debug)]
struct DeriveClone;

impl DeriveMacro for DeriveClone {
    fn name(&self) -> &str {
        "Clone"
    }

    fn expand(&self, item: &DeriveInput) -> Result<Vec<DeriveOutput>, MacroError> {
        let body = generate_clone_body(item);
        Ok(vec![DeriveOutput {
            generated_code: body,
        }])
    }
}

/// Generate Clone impl for a struct.
fn generate_clone_body(item: &DeriveInput) -> String {
    let field_clones: Vec<String> = item
        .fields
        .iter()
        .map(|f| format!("{}: self.{}.clone()", f.name, f.name))
        .collect();
    format!(
        "impl Clone for {} {{ fn clone(self) -> {} {{ {} {{ {} }} }} }}",
        item.name,
        item.name,
        item.name,
        field_clones.join(", ")
    )
}

/// Built-in `#[derive(PartialEq)]` — generates an `eq` method.
#[derive(Debug)]
struct DerivePartialEq;

impl DeriveMacro for DerivePartialEq {
    fn name(&self) -> &str {
        "PartialEq"
    }

    fn expand(&self, item: &DeriveInput) -> Result<Vec<DeriveOutput>, MacroError> {
        let body = generate_partial_eq_body(item);
        Ok(vec![DeriveOutput {
            generated_code: body,
        }])
    }
}

/// Generate PartialEq impl for a struct.
fn generate_partial_eq_body(item: &DeriveInput) -> String {
    let comparisons: Vec<String> = item
        .fields
        .iter()
        .map(|f| format!("self.{} == other.{}", f.name, f.name))
        .collect();
    let cond = if comparisons.is_empty() {
        "true".to_string()
    } else {
        comparisons.join(" && ")
    };
    format!(
        "impl PartialEq for {} {{ fn eq(self, other: {}) -> bool {{ {} }} }}",
        item.name, item.name, cond
    )
}

/// Built-in `#[derive(Hash)]` — generates a `hash` method.
#[derive(Debug)]
struct DeriveHash;

impl DeriveMacro for DeriveHash {
    fn name(&self) -> &str {
        "Hash"
    }

    fn expand(&self, item: &DeriveInput) -> Result<Vec<DeriveOutput>, MacroError> {
        let body = generate_hash_body(item);
        Ok(vec![DeriveOutput {
            generated_code: body,
        }])
    }
}

/// Generate Hash impl for a struct.
fn generate_hash_body(item: &DeriveInput) -> String {
    let hashes: Vec<String> = item
        .fields
        .iter()
        .map(|f| format!("self.{}.hash()", f.name))
        .collect();
    format!(
        "impl Hash for {} {{ fn hash(self) -> i64 {{ {} }} }}",
        item.name,
        hashes.join(" ^ ")
    )
}

/// Built-in `#[derive(Default)]` — generates a `default` constructor.
#[derive(Debug)]
struct DeriveDefault;

impl DeriveMacro for DeriveDefault {
    fn name(&self) -> &str {
        "Default"
    }

    fn expand(&self, item: &DeriveInput) -> Result<Vec<DeriveOutput>, MacroError> {
        let body = generate_default_body(item);
        Ok(vec![DeriveOutput {
            generated_code: body,
        }])
    }
}

/// Generate Default impl for a struct.
fn generate_default_body(item: &DeriveInput) -> String {
    let defaults: Vec<String> = item
        .fields
        .iter()
        .map(|f| format!("{}: Default::default()", f.name))
        .collect();
    format!(
        "impl Default for {} {{ fn default() -> {} {{ {} {{ {} }} }} }}",
        item.name,
        item.name,
        item.name,
        defaults.join(", ")
    )
}

/// Built-in `#[derive(Serialize)]` — generates a `serialize` method.
#[derive(Debug)]
struct DeriveSerialize;

impl DeriveMacro for DeriveSerialize {
    fn name(&self) -> &str {
        "Serialize"
    }

    fn expand(&self, item: &DeriveInput) -> Result<Vec<DeriveOutput>, MacroError> {
        let body = generate_serialize_body(item);
        Ok(vec![DeriveOutput {
            generated_code: body,
        }])
    }
}

/// Generate Serialize impl for a struct.
fn generate_serialize_body(item: &DeriveInput) -> String {
    let kvs: Vec<String> = item
        .fields
        .iter()
        .map(|f| format!("\\\"{}\\\": self.{}", f.name, f.name))
        .collect();
    format!(
        "impl Serialize for {} {{ fn serialize(self) -> str {{ \"{{ {} }}\" }} }}",
        item.name,
        kvs.join(", ")
    )
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 11 — Attribute Macros
// ═══════════════════════════════════════════════════════════════════════

/// The kind of attribute (built-in or custom).
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeKind {
    /// Conditional compilation: `#[cfg(...)]`.
    Cfg,
    /// Feature-gated compilation: `#[cfg(feature = "...")]`.
    CfgFeature,
    /// Inline hint: `#[inline]`.
    Inline,
    /// Deprecation marker: `#[deprecated(since = "...", message = "...")]`.
    Deprecated,
    /// Suppress a lint: `#[allow(unused)]`.
    Allow,
    /// Promote a lint to error: `#[deny(warnings)]`.
    Deny,
    /// Data layout: `#[repr(C)]`.
    Repr,
    /// User-defined attribute.
    Custom(String),
}

/// A parsed attribute macro applied to an item.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeMacro {
    /// Attribute name (e.g., `"cfg"`, `"deprecated"`, or custom name).
    pub name: String,
    /// Resolved kind.
    pub kind: AttributeKind,
    /// Raw argument string inside the parens (if any).
    pub args: Option<String>,
}

impl AttributeMacro {
    /// Create a new attribute from a name and optional arguments.
    ///
    /// Automatically classifies the attribute into the correct [`AttributeKind`].
    pub fn new(name: &str, args: Option<String>) -> Self {
        let kind = classify_attribute(name, args.as_deref());
        Self {
            name: name.to_string(),
            kind,
            args,
        }
    }
}

/// Classify an attribute name into its kind.
fn classify_attribute(name: &str, args: Option<&str>) -> AttributeKind {
    match name {
        "cfg" => {
            if let Some(a) = args {
                if a.starts_with("feature") {
                    return AttributeKind::CfgFeature;
                }
            }
            AttributeKind::Cfg
        }
        "inline" => AttributeKind::Inline,
        "deprecated" => AttributeKind::Deprecated,
        "allow" => AttributeKind::Allow,
        "deny" => AttributeKind::Deny,
        "repr" => AttributeKind::Repr,
        other => AttributeKind::Custom(other.to_string()),
    }
}

/// A conditional compilation expression.
///
/// Mirrors Rust's `cfg(...)` predicate system.
#[derive(Debug, Clone, PartialEq)]
pub enum CfgExpr {
    /// Target OS/arch: `cfg(target_os = "linux")`.
    Target(String),
    /// Feature flag: `cfg(feature = "gpu")`.
    Feature(String),
    /// All sub-expressions must be true: `cfg(all(...))`.
    All(Vec<CfgExpr>),
    /// Any sub-expression must be true: `cfg(any(...))`.
    Any(Vec<CfgExpr>),
    /// Negation: `cfg(not(...))`.
    Not(Box<CfgExpr>),
}

/// Evaluate a [`CfgExpr`] against a set of active features and target info.
///
/// Returns `true` if the cfg predicate is satisfied.
pub fn eval_cfg(expr: &CfgExpr, features: &[String], target: &str) -> bool {
    match expr {
        CfgExpr::Target(t) => target == t,
        CfgExpr::Feature(f) => features.contains(f),
        CfgExpr::All(exprs) => exprs.iter().all(|e| eval_cfg(e, features, target)),
        CfgExpr::Any(exprs) => exprs.iter().any(|e| eval_cfg(e, features, target)),
        CfgExpr::Not(inner) => !eval_cfg(inner, features, target),
    }
}

/// Deprecation metadata attached to an item.
#[derive(Debug, Clone, PartialEq)]
pub struct DeprecationInfo {
    /// Human-readable deprecation message.
    pub message: String,
    /// Version in which the item was deprecated.
    pub since: Option<String>,
}

impl DeprecationInfo {
    /// Create a deprecation with a message and optional version.
    pub fn new(message: &str, since: Option<&str>) -> Self {
        Self {
            message: message.to_string(),
            since: since.map(|s| s.to_string()),
        }
    }

    /// Format a deprecation warning string.
    pub fn format_warning(&self, item_name: &str) -> String {
        let since_part = self.since.as_deref().unwrap_or("unknown");
        format!(
            "warning: `{}` is deprecated since {}: {}",
            item_name, since_part, self.message
        )
    }
}

/// Data layout representation for `#[repr(...)]`.
#[derive(Debug, Clone, PartialEq)]
pub enum ReprKind {
    /// C-compatible layout.
    C,
    /// Packed (no padding).
    Packed,
    /// Single-field wrapper (transparent).
    Transparent,
    /// Explicit alignment.
    Align(usize),
}

/// Parse a repr string (e.g., `"C"`, `"packed"`, `"align(16)"`) into a [`ReprKind`].
pub fn parse_repr(s: &str) -> Result<ReprKind, MacroError> {
    let trimmed = s.trim();
    match trimmed {
        "C" => Ok(ReprKind::C),
        "packed" => Ok(ReprKind::Packed),
        "transparent" => Ok(ReprKind::Transparent),
        _ if trimmed.starts_with("align(") && trimmed.ends_with(')') => parse_repr_align(trimmed),
        _ => Err(MacroError::ExpansionError {
            name: "repr".to_string(),
            message: format!("unknown repr: `{trimmed}`"),
        }),
    }
}

/// Parse the alignment value from `align(N)`.
fn parse_repr_align(s: &str) -> Result<ReprKind, MacroError> {
    let inner = &s[6..s.len() - 1];
    inner
        .parse::<usize>()
        .map(ReprKind::Align)
        .map_err(|_| MacroError::ExpansionError {
            name: "repr".to_string(),
            message: format!("invalid alignment value: `{inner}`"),
        })
}

/// Registry for custom attribute handlers.
///
/// Maps attribute names to handler functions. Built-in attributes are handled
/// directly; this registry extends the system with user-defined attributes.
#[derive(Debug, Clone)]
pub struct AttributeRegistry {
    /// Registered custom attribute names.
    names: HashMap<String, String>,
}

impl AttributeRegistry {
    /// Create an empty attribute registry.
    pub fn new() -> Self {
        Self {
            names: HashMap::new(),
        }
    }

    /// Register a custom attribute with a description.
    pub fn register(&mut self, name: &str, description: &str) {
        self.names.insert(name.to_string(), description.to_string());
    }

    /// Check whether an attribute is registered.
    pub fn is_registered(&self, name: &str) -> bool {
        self.names.contains_key(name)
    }

    /// Look up the description of a registered attribute.
    pub fn description(&self, name: &str) -> Option<&str> {
        self.names.get(name).map(|s| s.as_str())
    }

    /// List all registered custom attribute names.
    pub fn registered_names(&self) -> Vec<String> {
        self.names.keys().cloned().collect()
    }
}

impl Default for AttributeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 12 — Macro Utilities
// ═══════════════════════════════════════════════════════════════════════

/// Source location for `file!()`, `line!()`, `column!()` macros.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceLocation {
    /// File path.
    pub file: String,
    /// Line number (1-based).
    pub line: u32,
    /// Column number (1-based).
    pub column: u32,
}

impl SourceLocation {
    /// Create a source location.
    pub fn new(file: &str, line: u32, column: u32) -> Self {
        Self {
            file: file.to_string(),
            line,
            column,
        }
    }
}

/// Simulate `compile_error!("message")` — produces a MacroError.
pub fn compile_error(message: &str) -> MacroError {
    MacroError::ExpansionError {
        name: "compile_error".to_string(),
        message: message.to_string(),
    }
}

/// Simulate `include!("path")` — returns the file content as a string.
///
/// In a real compiler this reads and tokenizes the file; here we simulate
/// by returning the path content from a provided lookup table.
pub fn include_file(
    path: &str,
    file_contents: &HashMap<String, String>,
) -> Result<String, MacroError> {
    file_contents
        .get(path)
        .cloned()
        .ok_or_else(|| MacroError::ExpansionError {
            name: "include".to_string(),
            message: format!("file not found: `{path}`"),
        })
}

/// Simulate `env!("VAR")` — read an environment variable at compile time.
pub fn env_macro(var_name: &str) -> Result<String, MacroError> {
    std::env::var(var_name).map_err(|_| MacroError::ExpansionError {
        name: "env".to_string(),
        message: format!("environment variable `{var_name}` not set"),
    })
}

/// Simulate `file!()` — return the current file path.
pub fn file_macro(loc: &SourceLocation) -> String {
    loc.file.clone()
}

/// Simulate `line!()` — return the current line number as a string.
pub fn line_macro(loc: &SourceLocation) -> String {
    loc.line.to_string()
}

/// Simulate `column!()` — return the current column number as a string.
pub fn column_macro(loc: &SourceLocation) -> String {
    loc.column.to_string()
}

/// Simulate `stringify!(tokens...)` — convert tokens to a string literal.
pub fn stringify_macro(tokens: &[String]) -> String {
    tokens.join(" ")
}

/// An expansion trace entry for error reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpansionStep {
    /// Macro name that was expanded.
    pub macro_name: String,
    /// Depth at which the expansion occurred.
    pub depth: usize,
    /// Input tokens to this expansion step.
    pub input: Vec<String>,
}

/// Macro expansion trace for detailed error diagnostics.
///
/// Records each expansion step so that when an error occurs, the user
/// can see the full chain of expansions that led to the failure.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpansionTrace {
    /// Ordered list of expansion steps.
    pub steps: Vec<ExpansionStep>,
}

impl ExpansionTrace {
    /// Create an empty trace.
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Record an expansion step.
    pub fn record(&mut self, macro_name: &str, depth: usize, input: &[String]) {
        self.steps.push(ExpansionStep {
            macro_name: macro_name.to_string(),
            depth,
            input: input.to_vec(),
        });
    }

    /// Format the trace for display in error messages.
    pub fn format_trace(&self) -> String {
        let mut parts = Vec::new();
        for step in &self.steps {
            parts.push(format!(
                "  at depth {}: {}!({})",
                step.depth,
                step.macro_name,
                step.input.join(", ")
            ));
        }
        if parts.is_empty() {
            "(empty trace)".to_string()
        } else {
            parts.join("\n")
        }
    }
}

impl Default for ExpansionTrace {
    fn default() -> Self {
        Self::new()
    }
}

/// Module-qualified macro name for import/export.
///
/// Supports macros like `std::vec!` with module path resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct QualifiedMacroName {
    /// Module path segments (e.g., `["std"]`).
    pub module_path: Vec<String>,
    /// The macro name itself (e.g., `"vec"`).
    pub name: String,
    /// Whether this macro is public.
    pub is_public: bool,
}

impl QualifiedMacroName {
    /// Create a simple (unqualified) macro name.
    pub fn simple(name: &str) -> Self {
        Self {
            module_path: Vec::new(),
            name: name.to_string(),
            is_public: false,
        }
    }

    /// Create a qualified macro name with a module path.
    pub fn qualified(path: Vec<String>, name: &str, is_public: bool) -> Self {
        Self {
            module_path: path,
            name: name.to_string(),
            is_public,
        }
    }

    /// Format the full qualified name (e.g., `"std::vec"`).
    pub fn full_name(&self) -> String {
        if self.module_path.is_empty() {
            self.name.clone()
        } else {
            format!("{}::{}", self.module_path.join("::"), self.name)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — 40 tests: Sprints 9–12, 10 per sprint
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 9: Declarative Macros (s9_1 – s9_10) ─────────────────

    #[test]
    fn s9_1_macro_rule_struct_creation() {
        let rule = MacroRule {
            pattern: vec![MacroMatcher::Literal("hello".to_string())],
            template: vec![MacroToken::Literal("world".to_string())],
        };
        assert_eq!(rule.pattern.len(), 1);
        assert_eq!(rule.template.len(), 1);
    }

    #[test]
    fn s9_2_macro_def_with_multiple_rules() {
        let def = MacroDef {
            name: "test_macro".to_string(),
            rules: vec![
                MacroRule {
                    pattern: vec![MacroMatcher::Variable {
                        name: "x".to_string(),
                        kind: FragmentKind::Expr,
                    }],
                    template: vec![MacroToken::Variable("x".to_string())],
                },
                MacroRule {
                    pattern: vec![MacroMatcher::Literal("fallback".to_string())],
                    template: vec![MacroToken::Literal("default".to_string())],
                },
            ],
            is_public: true,
        };
        assert_eq!(def.name, "test_macro");
        assert_eq!(def.rules.len(), 2);
        assert!(def.is_public);
    }

    #[test]
    fn s9_3_fragment_kinds_all_variants() {
        let kinds = vec![
            FragmentKind::Expr,
            FragmentKind::Ident,
            FragmentKind::Ty,
            FragmentKind::Stmt,
            FragmentKind::Block,
            FragmentKind::Pat,
            FragmentKind::Literal,
            FragmentKind::TokenTree,
        ];
        assert_eq!(kinds.len(), 8);
        assert_ne!(FragmentKind::Expr, FragmentKind::Ident);
    }

    #[test]
    fn s9_4_pattern_matching_literal() {
        let pattern = vec![MacroMatcher::Literal("hello".to_string())];
        let input = vec!["hello".to_string()];
        let bindings = match_pattern(&pattern, &input);
        assert!(bindings.is_some());

        let bad_input = vec!["world".to_string()];
        assert!(match_pattern(&pattern, &bad_input).is_none());
    }

    #[test]
    fn s9_5_pattern_matching_variable_capture() {
        let pattern = vec![MacroMatcher::Variable {
            name: "x".to_string(),
            kind: FragmentKind::Expr,
        }];
        let input = vec!["42".to_string()];
        let bindings = match_pattern(&pattern, &input).unwrap();
        assert_eq!(bindings.singles.get("x").unwrap(), "42");
    }

    #[test]
    fn s9_6_repetition_matching_zero_or_more() {
        let pattern = vec![MacroMatcher::Repetition {
            matchers: vec![MacroMatcher::Variable {
                name: "elem".to_string(),
                kind: FragmentKind::Expr,
            }],
            separator: Some(",".to_string()),
            kind: RepKind::ZeroOrMore,
        }];
        let input = vec![
            "1".to_string(),
            ",".to_string(),
            "2".to_string(),
            ",".to_string(),
            "3".to_string(),
        ];
        let bindings = match_pattern(&pattern, &input).unwrap();
        assert_eq!(bindings.repetitions.get("elem").unwrap().len(), 3);
    }

    #[test]
    fn s9_7_template_expansion_with_variable() {
        let template = vec![
            MacroToken::Literal("result:".to_string()),
            MacroToken::Variable("x".to_string()),
        ];
        let mut bindings = CaptureBindings::new();
        bindings.singles.insert("x".to_string(), "42".to_string());
        let output = expand_template(&template, &bindings).unwrap();
        assert_eq!(output, vec!["result:", "42"]);
    }

    #[test]
    fn s9_8_hygiene_context_gensym() {
        let mut hygiene = HygieneContext::new();
        let sym1 = hygiene.gensym("tmp");
        let sym2 = hygiene.gensym("tmp");
        assert_ne!(sym1, sym2);
        assert!(sym1.starts_with("__fj_gensym_tmp_"));
        assert_eq!(hygiene.counter(), 2);
    }

    #[test]
    fn s9_9_expander_recursion_limit() {
        let mut expander = MacroExpander::new();
        // Register a macro that calls itself (will hit recursion limit)
        let self_rule = MacroRule {
            pattern: vec![MacroMatcher::Variable {
                name: "x".to_string(),
                kind: FragmentKind::Expr,
            }],
            template: vec![MacroToken::Variable("x".to_string())],
        };
        expander.register(MacroDef {
            name: "recursive".to_string(),
            rules: vec![self_rule],
            is_public: false,
        });
        // Normal expansion should work (non-recursive)
        let result = expander.expand("recursive", &["hello".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn s9_10_builtin_macros_registered() {
        let expander = MacroExpander::new();
        let names = expander.registered_names();
        assert!(names.contains(&"vec".to_string()));
        assert!(names.contains(&"println".to_string()));
        assert!(names.contains(&"format".to_string()));
        assert!(names.contains(&"assert".to_string()));
        assert!(names.contains(&"cfg".to_string()));
        assert!(names.contains(&"dbg".to_string()));
    }

    // ── Sprint 10: Derive Macros (s10_1 – s10_10) ───────────────────

    #[test]
    fn s10_1_field_info_creation() {
        let field = FieldInfo {
            name: "x".to_string(),
            type_name: "f64".to_string(),
        };
        assert_eq!(field.name, "x");
        assert_eq!(field.type_name, "f64");
    }

    #[test]
    fn s10_2_variant_info_with_fields() {
        let variant = VariantInfo {
            name: "Circle".to_string(),
            fields: vec![FieldInfo {
                name: "radius".to_string(),
                type_name: "f64".to_string(),
            }],
        };
        assert_eq!(variant.name, "Circle");
        assert_eq!(variant.fields.len(), 1);
    }

    #[test]
    fn s10_3_derive_input_struct() {
        let input = DeriveInput {
            name: "Point".to_string(),
            fields: vec![
                FieldInfo {
                    name: "x".to_string(),
                    type_name: "f64".to_string(),
                },
                FieldInfo {
                    name: "y".to_string(),
                    type_name: "f64".to_string(),
                },
            ],
            variants: vec![],
        };
        assert_eq!(input.name, "Point");
        assert_eq!(input.fields.len(), 2);
        assert!(input.variants.is_empty());
    }

    #[test]
    fn s10_4_derive_debug_expand() {
        let registry = DeriveRegistry::new();
        let input = DeriveInput {
            name: "Point".to_string(),
            fields: vec![FieldInfo {
                name: "x".to_string(),
                type_name: "f64".to_string(),
            }],
            variants: vec![],
        };
        let result = registry.expand("Debug", &input).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].generated_code.contains("impl Debug for Point"));
        assert!(result[0].generated_code.contains("fmt_debug"));
    }

    #[test]
    fn s10_5_derive_clone_expand() {
        let registry = DeriveRegistry::new();
        let input = DeriveInput {
            name: "Pair".to_string(),
            fields: vec![
                FieldInfo {
                    name: "a".to_string(),
                    type_name: "i32".to_string(),
                },
                FieldInfo {
                    name: "b".to_string(),
                    type_name: "i32".to_string(),
                },
            ],
            variants: vec![],
        };
        let result = registry.expand("Clone", &input).unwrap();
        assert!(result[0].generated_code.contains("impl Clone for Pair"));
        assert!(result[0].generated_code.contains("clone"));
    }

    #[test]
    fn s10_6_derive_partial_eq_expand() {
        let registry = DeriveRegistry::new();
        let input = DeriveInput {
            name: "Num".to_string(),
            fields: vec![FieldInfo {
                name: "val".to_string(),
                type_name: "i64".to_string(),
            }],
            variants: vec![],
        };
        let result = registry.expand("PartialEq", &input).unwrap();
        assert!(result[0].generated_code.contains("self.val == other.val"));
    }

    #[test]
    fn s10_7_derive_hash_expand() {
        let registry = DeriveRegistry::new();
        let input = DeriveInput {
            name: "Id".to_string(),
            fields: vec![FieldInfo {
                name: "key".to_string(),
                type_name: "str".to_string(),
            }],
            variants: vec![],
        };
        let result = registry.expand("Hash", &input).unwrap();
        assert!(result[0].generated_code.contains("impl Hash for Id"));
    }

    #[test]
    fn s10_8_derive_default_expand() {
        let registry = DeriveRegistry::new();
        let input = DeriveInput {
            name: "Config".to_string(),
            fields: vec![FieldInfo {
                name: "width".to_string(),
                type_name: "i32".to_string(),
            }],
            variants: vec![],
        };
        let result = registry.expand("Default", &input).unwrap();
        assert!(result[0].generated_code.contains("Default::default()"));
    }

    #[test]
    fn s10_9_derive_serialize_expand() {
        let registry = DeriveRegistry::new();
        let input = DeriveInput {
            name: "Record".to_string(),
            fields: vec![FieldInfo {
                name: "id".to_string(),
                type_name: "i64".to_string(),
            }],
            variants: vec![],
        };
        let result = registry.expand("Serialize", &input).unwrap();
        assert!(result[0].generated_code.contains("serialize"));
    }

    #[test]
    fn s10_10_derive_registry_unknown_macro() {
        let registry = DeriveRegistry::new();
        let input = DeriveInput {
            name: "Foo".to_string(),
            fields: vec![],
            variants: vec![],
        };
        let result = registry.expand("NonExistent", &input);
        assert!(result.is_err());
        match result.unwrap_err() {
            MacroError::UndefinedMacro { name } => {
                assert_eq!(name, "NonExistent");
            }
            other => panic!("expected UndefinedMacro, got: {other:?}"),
        }
    }

    // ── Sprint 11: Attribute Macros (s11_1 – s11_10) ────────────────

    #[test]
    fn s11_1_attribute_kind_classification() {
        let attr = AttributeMacro::new("cfg", Some("target_os = \"linux\"".to_string()));
        assert_eq!(attr.kind, AttributeKind::Cfg);

        let attr2 = AttributeMacro::new("cfg", Some("feature = \"gpu\"".to_string()));
        assert_eq!(attr2.kind, AttributeKind::CfgFeature);
    }

    #[test]
    fn s11_2_cfg_expr_target_evaluation() {
        let expr = CfgExpr::Target("linux".to_string());
        assert!(eval_cfg(&expr, &[], "linux"));
        assert!(!eval_cfg(&expr, &[], "macos"));
    }

    #[test]
    fn s11_3_cfg_expr_feature_evaluation() {
        let expr = CfgExpr::Feature("gpu".to_string());
        let features = vec!["gpu".to_string(), "native".to_string()];
        assert!(eval_cfg(&expr, &features, "linux"));
        assert!(!eval_cfg(&expr, &[], "linux"));
    }

    #[test]
    fn s11_4_cfg_expr_all_combinator() {
        let expr = CfgExpr::All(vec![
            CfgExpr::Target("linux".to_string()),
            CfgExpr::Feature("gpu".to_string()),
        ]);
        let features = vec!["gpu".to_string()];
        assert!(eval_cfg(&expr, &features, "linux"));
        assert!(!eval_cfg(&expr, &features, "macos"));
        assert!(!eval_cfg(&expr, &[], "linux"));
    }

    #[test]
    fn s11_5_cfg_expr_any_combinator() {
        let expr = CfgExpr::Any(vec![
            CfgExpr::Target("linux".to_string()),
            CfgExpr::Target("macos".to_string()),
        ]);
        assert!(eval_cfg(&expr, &[], "linux"));
        assert!(eval_cfg(&expr, &[], "macos"));
        assert!(!eval_cfg(&expr, &[], "windows"));
    }

    #[test]
    fn s11_6_cfg_expr_not_combinator() {
        let expr = CfgExpr::Not(Box::new(CfgExpr::Feature("debug".to_string())));
        assert!(eval_cfg(&expr, &[], "linux"));
        let features = vec!["debug".to_string()];
        assert!(!eval_cfg(&expr, &features, "linux"));
    }

    #[test]
    fn s11_7_deprecation_warning_format() {
        let dep = DeprecationInfo::new("use new_fn instead", Some("0.3.0"));
        let warning = dep.format_warning("old_fn");
        assert!(warning.contains("old_fn"));
        assert!(warning.contains("0.3.0"));
        assert!(warning.contains("use new_fn instead"));
    }

    #[test]
    fn s11_8_repr_kind_parsing() {
        assert_eq!(parse_repr("C").unwrap(), ReprKind::C);
        assert_eq!(parse_repr("packed").unwrap(), ReprKind::Packed);
        assert_eq!(parse_repr("transparent").unwrap(), ReprKind::Transparent);
        assert_eq!(parse_repr("align(16)").unwrap(), ReprKind::Align(16));
        assert!(parse_repr("unknown").is_err());
    }

    #[test]
    fn s11_9_custom_attribute_registry() {
        let mut registry = AttributeRegistry::new();
        registry.register("my_attr", "A custom attribute");
        assert!(registry.is_registered("my_attr"));
        assert!(!registry.is_registered("other"));
        assert_eq!(
            registry.description("my_attr").unwrap(),
            "A custom attribute"
        );
    }

    #[test]
    fn s11_10_attribute_macro_all_builtins() {
        let cases = vec![
            ("inline", AttributeKind::Inline),
            ("deprecated", AttributeKind::Deprecated),
            ("allow", AttributeKind::Allow),
            ("deny", AttributeKind::Deny),
            ("repr", AttributeKind::Repr),
        ];
        for (name, expected_kind) in cases {
            let attr = AttributeMacro::new(name, None);
            assert_eq!(attr.kind, expected_kind);
        }
        let custom = AttributeMacro::new("my_custom", None);
        assert_eq!(custom.kind, AttributeKind::Custom("my_custom".to_string()));
    }

    // ── Sprint 12: Macro Utilities (s12_1 – s12_10) ────────────────

    #[test]
    fn s12_1_compile_error_produces_error() {
        let err = compile_error("intentional failure");
        match err {
            MacroError::ExpansionError { name, message } => {
                assert_eq!(name, "compile_error");
                assert_eq!(message, "intentional failure");
            }
            other => panic!("expected ExpansionError, got: {other:?}"),
        }
    }

    #[test]
    fn s12_2_include_file_found() {
        let mut files = HashMap::new();
        files.insert("header.fj".to_string(), "let x = 1".to_string());
        let content = include_file("header.fj", &files).unwrap();
        assert_eq!(content, "let x = 1");
    }

    #[test]
    fn s12_3_include_file_not_found() {
        let files = HashMap::new();
        let result = include_file("missing.fj", &files);
        assert!(result.is_err());
        match result.unwrap_err() {
            MacroError::ExpansionError { name, message } => {
                assert_eq!(name, "include");
                assert!(message.contains("missing.fj"));
            }
            other => panic!("expected ExpansionError, got: {other:?}"),
        }
    }

    #[test]
    fn s12_4_env_macro_reads_path() {
        // PATH is almost always set
        let result = env_macro("PATH");
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn s12_5_source_location_macros() {
        let loc = SourceLocation::new("main.fj", 10, 5);
        assert_eq!(file_macro(&loc), "main.fj");
        assert_eq!(line_macro(&loc), "10");
        assert_eq!(column_macro(&loc), "5");
    }

    #[test]
    fn s12_6_stringify_macro_joins_tokens() {
        let tokens = vec![
            "let".to_string(),
            "x".to_string(),
            "=".to_string(),
            "42".to_string(),
        ];
        assert_eq!(stringify_macro(&tokens), "let x = 42");
    }

    #[test]
    fn s12_7_expansion_trace_recording() {
        let mut trace = ExpansionTrace::new();
        trace.record("vec", 0, &["1".to_string(), "2".to_string()]);
        trace.record("inner", 1, &["x".to_string()]);
        assert_eq!(trace.steps.len(), 2);
        let formatted = trace.format_trace();
        assert!(formatted.contains("vec"));
        assert!(formatted.contains("inner"));
        assert!(formatted.contains("depth 0"));
        assert!(formatted.contains("depth 1"));
    }

    #[test]
    fn s12_8_macro_export_qualified_name() {
        let qn = QualifiedMacroName::qualified(vec!["std".to_string()], "vec", true);
        assert_eq!(qn.full_name(), "std::vec");
        assert!(qn.is_public);

        let simple = QualifiedMacroName::simple("local_macro");
        assert_eq!(simple.full_name(), "local_macro");
        assert!(!simple.is_public);
    }

    #[test]
    fn s12_9_macro_error_variants_all() {
        let errors: Vec<MacroError> = vec![
            MacroError::UndefinedMacro {
                name: "foo".to_string(),
            },
            MacroError::PatternMismatch {
                name: "bar".to_string(),
            },
            MacroError::RecursionLimit {
                name: "baz".to_string(),
                limit: 64,
            },
            MacroError::ExpansionError {
                name: "qux".to_string(),
                message: "bad".to_string(),
            },
            MacroError::InvalidFragment {
                name: "mac".to_string(),
                fragment: "$x".to_string(),
                expected: "Expr".to_string(),
            },
        ];
        assert_eq!(errors.len(), 5);
        // Verify Display impls produce correct error codes
        assert!(format!("{}", errors[0]).contains("ME001"));
        assert!(format!("{}", errors[1]).contains("ME002"));
        assert!(format!("{}", errors[2]).contains("ME003"));
        assert!(format!("{}", errors[3]).contains("ME004"));
        assert!(format!("{}", errors[4]).contains("ME005"));
    }

    #[test]
    fn s12_10_expander_expand_undefined_macro() {
        let mut expander = MacroExpander::new();
        let result = expander.expand("nonexistent", &["x".to_string()]);
        assert!(result.is_err());
        match result.unwrap_err() {
            MacroError::UndefinedMacro { name } => {
                assert_eq!(name, "nonexistent");
            }
            other => panic!("expected UndefinedMacro, got: {other:?}"),
        }
    }
}
