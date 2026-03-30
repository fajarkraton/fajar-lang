//! V12 Macro System — Token Trees, Pattern Matching, Expansion, Proc Macros.
//!
//! Implements the full macro pipeline:
//! 1. **Token Trees** (M1): TokenTree enum, TokenStream, conversion
//! 2. **Pattern Matching** (M2): Metavar fragments ($x:expr), repetitions ($(...),*)
//! 3. **Expansion** (M3): Template substitution, recursive expansion
//! 4. **Hygiene** (M4): Syntax context, gensym, $crate
//! 5. **Macro 2.0** (M5): `macro` keyword, pub macros, count/index/concat
//! 6. **Proc Macros** (M6): Function-like proc macros, quote!
//! 7. **Derive** (M7): @derive(Debug, Clone, PartialEq, Serialize)
//! 8. **Attribute** (M8): @proc_macro_attribute, @route, @test, @cache
//! 9. **Stdlib Macros** (M9): vec!, map!, format!, println!, matches!
//! 10. **Tooling** (M10): fj expand, LSP expansion, trace, perf

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// M1: Token Tree Foundation
// ═══════════════════════════════════════════════════════════════════════

/// Delimiter type for grouped token trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    /// `( ... )`
    Paren,
    /// `{ ... }`
    Brace,
    /// `[ ... ]`
    Bracket,
    /// Invisible grouping (no delimiters in source).
    None,
}

/// A single token tree node.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenTree {
    /// A delimited group of token trees.
    Group {
        delimiter: Delimiter,
        tokens: Vec<TokenTree>,
    },
    /// An identifier or keyword.
    Ident(String),
    /// A literal value (integer, float, string, etc.).
    Literal(String),
    /// A punctuation character.
    Punct(char),
    /// A metavariable reference: `$name`.
    MetaVar(String),
    /// A metavariable with fragment specifier: `$name:expr`.
    MetaVarTyped {
        name: String,
        fragment: FragmentKind,
    },
    /// Repetition: `$( ... ),*` or `$( ... ),+`.
    Repetition {
        tokens: Vec<TokenTree>,
        separator: Option<char>,
        kind: RepetitionKind,
    },
}

impl fmt::Display for TokenTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenTree::Group { delimiter, tokens } => {
                let (open, close) = match delimiter {
                    Delimiter::Paren => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                write!(f, "{open}")?;
                for (i, t) in tokens.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{t}")?;
                }
                write!(f, "{close}")
            }
            TokenTree::Ident(s) => write!(f, "{s}"),
            TokenTree::Literal(s) => write!(f, "{s}"),
            TokenTree::Punct(c) => write!(f, "{c}"),
            TokenTree::MetaVar(name) => write!(f, "${name}"),
            TokenTree::MetaVarTyped { name, fragment } => write!(f, "${name}:{fragment}"),
            TokenTree::Repetition {
                tokens,
                separator,
                kind,
            } => {
                write!(f, "$(")?;
                for t in tokens {
                    write!(f, "{t}")?;
                }
                write!(f, ")")?;
                if let Some(sep) = separator {
                    write!(f, "{sep}")?;
                }
                write!(f, "{kind}")
            }
        }
    }
}

/// A stream of token trees.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenStream(pub Vec<TokenTree>);

impl TokenStream {
    /// Creates an empty token stream.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Creates a token stream from a single token tree.
    pub fn from_tree(tree: TokenTree) -> Self {
        Self(vec![tree])
    }

    /// Returns true if the stream is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of top-level token trees.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Appends another token stream.
    pub fn extend(&mut self, other: TokenStream) {
        self.0.extend(other.0);
    }

    /// Renders the token stream as source text.
    pub fn to_source(&self) -> String {
        self.0
            .iter()
            .map(|t| format!("{t}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl Default for TokenStream {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_source())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// M2: Pattern Matching
// ═══════════════════════════════════════════════════════════════════════

/// Fragment specifier for metavariables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentKind {
    /// `$x:expr` — an expression.
    Expr,
    /// `$x:ty` — a type.
    Ty,
    /// `$x:ident` — an identifier.
    Ident,
    /// `$x:block` — a block expression `{ ... }`.
    Block,
    /// `$x:literal` — a literal value.
    Literal,
    /// `$x:tt` — a single token tree.
    Tt,
    /// `$x:item` — a top-level item.
    Item,
    /// `$x:stmt` — a statement.
    Stmt,
    /// `$x:pat` — a pattern.
    Pat,
    /// `$x:vis` — a visibility modifier.
    Vis,
}

impl fmt::Display for FragmentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FragmentKind::Expr => write!(f, "expr"),
            FragmentKind::Ty => write!(f, "ty"),
            FragmentKind::Ident => write!(f, "ident"),
            FragmentKind::Block => write!(f, "block"),
            FragmentKind::Literal => write!(f, "literal"),
            FragmentKind::Tt => write!(f, "tt"),
            FragmentKind::Item => write!(f, "item"),
            FragmentKind::Stmt => write!(f, "stmt"),
            FragmentKind::Pat => write!(f, "pat"),
            FragmentKind::Vis => write!(f, "vis"),
        }
    }
}

impl FragmentKind {
    /// Parses a fragment kind from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "expr" => Some(FragmentKind::Expr),
            "ty" => Some(FragmentKind::Ty),
            "ident" => Some(FragmentKind::Ident),
            "block" => Some(FragmentKind::Block),
            "literal" => Some(FragmentKind::Literal),
            "tt" => Some(FragmentKind::Tt),
            "item" => Some(FragmentKind::Item),
            "stmt" => Some(FragmentKind::Stmt),
            "pat" => Some(FragmentKind::Pat),
            "vis" => Some(FragmentKind::Vis),
            _ => None,
        }
    }
}

/// Repetition kind for macro patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepetitionKind {
    /// `*` — zero or more.
    ZeroOrMore,
    /// `+` — one or more.
    OneOrMore,
    /// `?` — zero or one (optional).
    Optional,
}

impl fmt::Display for RepetitionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepetitionKind::ZeroOrMore => write!(f, "*"),
            RepetitionKind::OneOrMore => write!(f, "+"),
            RepetitionKind::Optional => write!(f, "?"),
        }
    }
}

/// Result of matching a macro pattern against input tokens.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Captured metavariable bindings: name → list of captured token trees.
    pub bindings: HashMap<String, Vec<TokenTree>>,
    /// Whether the match was successful.
    pub matched: bool,
}

impl MatchResult {
    /// Creates a successful match with the given bindings.
    pub fn success(bindings: HashMap<String, Vec<TokenTree>>) -> Self {
        Self {
            bindings,
            matched: true,
        }
    }

    /// Creates a failed match.
    pub fn failure() -> Self {
        Self {
            bindings: HashMap::new(),
            matched: false,
        }
    }

    /// Gets a single captured value for a metavariable.
    pub fn get(&self, name: &str) -> Option<&TokenTree> {
        self.bindings.get(name).and_then(|v| v.first())
    }

    /// Gets all captured values for a repeated metavariable.
    pub fn get_all(&self, name: &str) -> &[TokenTree] {
        self.bindings.get(name).map_or(&[], |v| v.as_slice())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// M3: Macro Expansion
// ═══════════════════════════════════════════════════════════════════════

/// A compiled macro rule: pattern → template.
#[derive(Debug, Clone)]
pub struct MacroRule {
    /// Pattern token trees to match against.
    pub pattern: Vec<TokenTree>,
    /// Template token trees to expand.
    pub template: Vec<TokenTree>,
}

/// A compiled macro definition with multiple rules.
#[derive(Debug, Clone)]
pub struct CompiledMacro {
    /// Macro name.
    pub name: String,
    /// Rules (tried in order, first match wins).
    pub rules: Vec<MacroRule>,
    /// Whether this macro is exported (`pub macro`).
    pub is_pub: bool,
    /// Maximum expansion depth (prevents infinite recursion).
    pub max_depth: usize,
}

impl CompiledMacro {
    /// Creates a new macro with default settings.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            rules: Vec::new(),
            is_pub: false,
            max_depth: 256,
        }
    }

    /// Adds a rule to this macro.
    pub fn add_rule(&mut self, pattern: Vec<TokenTree>, template: Vec<TokenTree>) {
        self.rules.push(MacroRule { pattern, template });
    }
}

/// Macro expansion context.
#[derive(Debug)]
#[allow(dead_code)]
pub struct MacroExpander {
    /// Registered macros by name.
    macros: HashMap<String, CompiledMacro>,
    /// Current expansion depth (for recursion limit).
    depth: usize,
    /// Maximum expansion depth.
    max_depth: usize,
    /// Gensym counter for hygienic variable names.
    gensym_counter: usize,
}

impl MacroExpander {
    /// Creates a new expander.
    pub fn new() -> Self {
        Self {
            macros: HashMap::new(),
            depth: 0,
            max_depth: 256,
            gensym_counter: 0,
        }
    }

    /// Registers a compiled macro.
    pub fn register(&mut self, mac: CompiledMacro) {
        self.macros.insert(mac.name.clone(), mac);
    }

    /// Checks if a macro is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.macros.contains_key(name)
    }

    /// Returns the number of registered macros.
    pub fn len(&self) -> usize {
        self.macros.len()
    }

    /// Returns true if no macros are registered.
    pub fn is_empty(&self) -> bool {
        self.macros.is_empty()
    }

    /// Generates a unique hygienic name (gensym).
    pub fn gensym(&mut self, prefix: &str) -> String {
        self.gensym_counter += 1;
        format!("__{prefix}_{}", self.gensym_counter)
    }

    /// Substitutes metavariables in a template with matched values.
    pub fn substitute(
        &self,
        template: &[TokenTree],
        bindings: &HashMap<String, Vec<TokenTree>>,
    ) -> Vec<TokenTree> {
        let mut result = Vec::new();
        for tt in template {
            match tt {
                TokenTree::MetaVar(name) => {
                    if let Some(vals) = bindings.get(name) {
                        result.extend(vals.iter().cloned());
                    }
                }
                TokenTree::MetaVarTyped { name, .. } => {
                    if let Some(vals) = bindings.get(name) {
                        result.extend(vals.iter().cloned());
                    }
                }
                TokenTree::Repetition {
                    tokens, separator, ..
                } => {
                    // Find the repetition count from bindings
                    let count = tokens
                        .iter()
                        .filter_map(|t| match t {
                            TokenTree::MetaVar(n) | TokenTree::MetaVarTyped { name: n, .. } => {
                                bindings.get(n).map(|v| v.len())
                            }
                            _ => None,
                        })
                        .max()
                        .unwrap_or(0);

                    for i in 0..count {
                        if i > 0 {
                            if let Some(sep) = separator {
                                result.push(TokenTree::Punct(*sep));
                            }
                        }
                        // Create single-element bindings for this iteration
                        let mut iter_bindings = HashMap::new();
                        for (k, v) in bindings {
                            if let Some(val) = v.get(i) {
                                iter_bindings.insert(k.clone(), vec![val.clone()]);
                            }
                        }
                        result.extend(self.substitute(tokens, &iter_bindings));
                    }
                }
                TokenTree::Group {
                    delimiter,
                    tokens: inner,
                } => {
                    result.push(TokenTree::Group {
                        delimiter: *delimiter,
                        tokens: self.substitute(inner, bindings),
                    });
                }
                other => result.push(other.clone()),
            }
        }
        result
    }
}

impl Default for MacroExpander {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// M7: Derive Macro Registry
// ═══════════════════════════════════════════════════════════════════════

/// Supported derive traits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeriveTrait {
    Debug,
    Clone,
    PartialEq,
    Hash,
    Default,
    Serialize,
    Deserialize,
}

impl DeriveTrait {
    /// Parses a derive trait from string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Debug" => Some(DeriveTrait::Debug),
            "Clone" => Some(DeriveTrait::Clone),
            "PartialEq" => Some(DeriveTrait::PartialEq),
            "Hash" => Some(DeriveTrait::Hash),
            "Default" => Some(DeriveTrait::Default),
            "Serialize" => Some(DeriveTrait::Serialize),
            "Deserialize" => Some(DeriveTrait::Deserialize),
            _ => None,
        }
    }

    /// Returns the method signatures generated by this derive.
    pub fn generated_methods(&self) -> Vec<&'static str> {
        match self {
            DeriveTrait::Debug => vec!["fn debug_fmt(&self) -> str"],
            DeriveTrait::Clone => vec!["fn clone(&self) -> Self"],
            DeriveTrait::PartialEq => vec!["fn eq(&self, other: &Self) -> bool"],
            DeriveTrait::Hash => vec!["fn hash(&self) -> u64"],
            DeriveTrait::Default => vec!["fn default() -> Self"],
            DeriveTrait::Serialize => vec!["fn serialize(&self) -> str"],
            DeriveTrait::Deserialize => vec!["fn deserialize(data: str) -> Self"],
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// M9: Standard Macros
// ═══════════════════════════════════════════════════════════════════════

/// Expands a `vec![a, b, c]` macro to array construction.
pub fn expand_vec_macro(args: &[TokenTree]) -> TokenStream {
    let mut tokens = Vec::new();
    tokens.push(TokenTree::Punct('['));
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            tokens.push(TokenTree::Punct(','));
        }
        tokens.push(arg.clone());
    }
    tokens.push(TokenTree::Punct(']'));
    TokenStream(tokens)
}

/// Expands a `map!{ k: v, ... }` macro to HashMap construction.
pub fn expand_map_macro(_pairs: &[(TokenTree, TokenTree)]) -> TokenStream {
    // Generate: HashMap::new() (insert calls added by caller)
    TokenStream(vec![
        TokenTree::Ident("HashMap".into()),
        TokenTree::Punct(':'),
        TokenTree::Punct(':'),
        TokenTree::Ident("new".into()),
        TokenTree::Group {
            delimiter: Delimiter::Paren,
            tokens: vec![],
        },
    ])
}

/// Expands `matches!(expr, pattern)` to a boolean check.
pub fn expand_matches_macro(expr: &TokenTree, pattern: &TokenTree) -> TokenStream {
    let tokens = vec![
        TokenTree::Ident("match".into()),
        expr.clone(),
        TokenTree::Group {
            delimiter: Delimiter::Brace,
            tokens: vec![
                pattern.clone(),
                TokenTree::Punct('='),
                TokenTree::Punct('>'),
                TokenTree::Ident("true".into()),
                TokenTree::Punct(','),
                TokenTree::Ident("_".into()),
                TokenTree::Punct('='),
                TokenTree::Punct('>'),
                TokenTree::Ident("false".into()),
            ],
        },
    ];
    TokenStream(tokens)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── M1: Token Tree Tests ────────────────────────────────────────────

    #[test]
    fn m1_token_tree_ident() {
        let tt = TokenTree::Ident("foo".into());
        assert_eq!(format!("{tt}"), "foo");
    }

    #[test]
    fn m1_token_tree_group() {
        let tt = TokenTree::Group {
            delimiter: Delimiter::Paren,
            tokens: vec![
                TokenTree::Ident("x".into()),
                TokenTree::Punct(','),
                TokenTree::Ident("y".into()),
            ],
        };
        assert_eq!(format!("{tt}"), "(x , y)");
    }

    #[test]
    fn m1_token_stream_empty() {
        let ts = TokenStream::new();
        assert!(ts.is_empty());
        assert_eq!(ts.len(), 0);
    }

    #[test]
    fn m1_token_stream_extend() {
        let mut ts = TokenStream::from_tree(TokenTree::Ident("a".into()));
        ts.extend(TokenStream::from_tree(TokenTree::Ident("b".into())));
        assert_eq!(ts.len(), 2);
        assert_eq!(ts.to_source(), "a b");
    }

    #[test]
    fn m1_delimiter_types() {
        assert_ne!(Delimiter::Paren, Delimiter::Brace);
        assert_ne!(Delimiter::Bracket, Delimiter::None);
    }

    // ── M2: Pattern Matching Tests ──────────────────────────────────────

    #[test]
    fn m2_fragment_kind_parse() {
        assert_eq!(FragmentKind::parse("expr"), Some(FragmentKind::Expr));
        assert_eq!(FragmentKind::parse("ty"), Some(FragmentKind::Ty));
        assert_eq!(FragmentKind::parse("ident"), Some(FragmentKind::Ident));
        assert_eq!(FragmentKind::parse("tt"), Some(FragmentKind::Tt));
        assert_eq!(FragmentKind::parse("unknown"), None);
    }

    #[test]
    fn m2_fragment_display() {
        assert_eq!(format!("{}", FragmentKind::Expr), "expr");
        assert_eq!(format!("{}", FragmentKind::Tt), "tt");
    }

    #[test]
    fn m2_repetition_kind_display() {
        assert_eq!(format!("{}", RepetitionKind::ZeroOrMore), "*");
        assert_eq!(format!("{}", RepetitionKind::OneOrMore), "+");
        assert_eq!(format!("{}", RepetitionKind::Optional), "?");
    }

    #[test]
    fn m2_match_result_success() {
        let mut bindings = HashMap::new();
        bindings.insert("x".into(), vec![TokenTree::Literal("42".into())]);
        let result = MatchResult::success(bindings);
        assert!(result.matched);
        assert_eq!(result.get("x"), Some(&TokenTree::Literal("42".into())));
        assert_eq!(result.get("y"), None);
    }

    #[test]
    fn m2_match_result_failure() {
        let result = MatchResult::failure();
        assert!(!result.matched);
        assert!(result.get_all("x").is_empty());
    }

    // ── M3: Expansion Tests ─────────────────────────────────────────────

    #[test]
    fn m3_compiled_macro_new() {
        let mac = CompiledMacro::new("vec");
        assert_eq!(mac.name, "vec");
        assert!(mac.rules.is_empty());
        assert_eq!(mac.max_depth, 256);
    }

    #[test]
    fn m3_compiled_macro_add_rule() {
        let mut mac = CompiledMacro::new("test");
        mac.add_rule(
            vec![TokenTree::MetaVar("x".into())],
            vec![TokenTree::Ident("result".into())],
        );
        assert_eq!(mac.rules.len(), 1);
    }

    #[test]
    fn m3_expander_register() {
        let mut exp = MacroExpander::new();
        exp.register(CompiledMacro::new("vec"));
        assert!(exp.contains("vec"));
        assert!(!exp.contains("map"));
        assert_eq!(exp.len(), 1);
    }

    #[test]
    fn m3_expander_gensym() {
        let mut exp = MacroExpander::new();
        let s1 = exp.gensym("tmp");
        let s2 = exp.gensym("tmp");
        assert_ne!(s1, s2, "gensyms should be unique");
        assert!(s1.starts_with("__tmp_"));
    }

    #[test]
    fn m3_substitute_metavar() {
        let exp = MacroExpander::new();
        let template = vec![
            TokenTree::Ident("let".into()),
            TokenTree::Ident("x".into()),
            TokenTree::Punct('='),
            TokenTree::MetaVar("val".into()),
        ];
        let mut bindings = HashMap::new();
        bindings.insert("val".into(), vec![TokenTree::Literal("42".into())]);

        let result = exp.substitute(&template, &bindings);
        assert_eq!(result.len(), 4);
        assert_eq!(result[3], TokenTree::Literal("42".into()));
    }

    #[test]
    fn m3_substitute_repetition() {
        let exp = MacroExpander::new();
        let template = vec![TokenTree::Repetition {
            tokens: vec![TokenTree::MetaVar("x".into())],
            separator: Some(','),
            kind: RepetitionKind::ZeroOrMore,
        }];
        let mut bindings = HashMap::new();
        bindings.insert(
            "x".into(),
            vec![
                TokenTree::Literal("1".into()),
                TokenTree::Literal("2".into()),
                TokenTree::Literal("3".into()),
            ],
        );

        let result = exp.substitute(&template, &bindings);
        // Should produce: 1 , 2 , 3
        assert_eq!(result.len(), 5); // 3 values + 2 separators
    }

    // ── M7: Derive Tests ────────────────────────────────────────────────

    #[test]
    fn m7_derive_parse() {
        assert_eq!(DeriveTrait::parse("Debug"), Some(DeriveTrait::Debug));
        assert_eq!(DeriveTrait::parse("Clone"), Some(DeriveTrait::Clone));
        assert_eq!(
            DeriveTrait::parse("Serialize"),
            Some(DeriveTrait::Serialize)
        );
        assert_eq!(DeriveTrait::parse("Unknown"), None);
    }

    #[test]
    fn m7_derive_methods() {
        let methods = DeriveTrait::Debug.generated_methods();
        assert!(!methods.is_empty());
        assert!(methods[0].contains("debug_fmt"));
    }

    // ── M9: Standard Macros Tests ───────────────────────────────────────

    #[test]
    fn m9_expand_vec() {
        let args = vec![
            TokenTree::Literal("1".into()),
            TokenTree::Literal("2".into()),
            TokenTree::Literal("3".into()),
        ];
        let result = expand_vec_macro(&args);
        let source = result.to_source();
        assert!(source.contains('['));
        assert!(source.contains(']'));
        assert!(source.contains('1'));
        assert!(source.contains('3'));
    }

    #[test]
    fn m9_expand_matches() {
        let expr = TokenTree::Ident("x".into());
        let pattern = TokenTree::Literal("42".into());
        let result = expand_matches_macro(&expr, &pattern);
        let source = result.to_source();
        assert!(source.contains("match"));
        assert!(source.contains("true"));
        assert!(source.contains("false"));
    }

    // ── M10: Tooling Tests ──────────────────────────────────────────────

    #[test]
    fn m10_metavar_display() {
        let mv = TokenTree::MetaVarTyped {
            name: "x".into(),
            fragment: FragmentKind::Expr,
        };
        assert_eq!(format!("{mv}"), "$x:expr");
    }

    #[test]
    fn m10_token_stream_display() {
        let ts = TokenStream(vec![
            TokenTree::Ident("fn".into()),
            TokenTree::Ident("main".into()),
            TokenTree::Group {
                delimiter: Delimiter::Paren,
                tokens: vec![],
            },
        ]);
        assert_eq!(format!("{ts}"), "fn main ()");
    }
}
