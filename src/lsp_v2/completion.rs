//! Type-driven completion — expected type analysis, expression synthesis,
//! fill-in-the-blank, argument completion, pattern completion, import
//! suggestions, postfix completion, snippets, ranking.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S31.1: Expected Type Analysis
// ═══════════════════════════════════════════════════════════════════════

/// The expected type at a cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedType {
    /// A concrete named type.
    Named(String),
    /// A generic type with parameters.
    Generic(String, Vec<String>),
    /// A function type.
    Function { params: Vec<String>, ret: String },
    /// Any type (no constraint).
    Any,
    /// Unknown — can't determine.
    Unknown,
}

impl fmt::Display for ExpectedType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpectedType::Named(n) => write!(f, "{n}"),
            ExpectedType::Generic(n, params) => write!(f, "{}<{}>", n, params.join(", ")),
            ExpectedType::Function { params, ret } => {
                write!(f, "fn({}) -> {}", params.join(", "), ret)
            }
            ExpectedType::Any => write!(f, "_"),
            ExpectedType::Unknown => write!(f, "?"),
        }
    }
}

/// Context surrounding the cursor for type analysis.
#[derive(Debug, Clone)]
pub struct CursorContext {
    /// The expected type (from surrounding expression).
    pub expected_type: ExpectedType,
    /// Whether we are inside a function argument.
    pub in_argument: bool,
    /// Current argument index (if in function call).
    pub argument_index: Option<usize>,
    /// Whether we are in a let binding RHS.
    pub in_let_rhs: bool,
    /// Whether we are in a match arm.
    pub in_match_arm: bool,
    /// Whether we are in a return position.
    pub in_return: bool,
}

/// Determines the expected type at a cursor position from surrounding code.
pub fn analyze_expected_type(line: &str, col: usize, context_lines: &[&str]) -> CursorContext {
    let trimmed = line.trim();

    // Check for let binding with type annotation
    if let Some(rest) = trimmed.strip_prefix("let") {
        let rest = rest.trim();
        if let Some(colon_pos) = rest.find(':') {
            let after_colon = &rest[colon_pos + 1..];
            if let Some(eq_pos) = after_colon.find('=') {
                let type_str = after_colon[..eq_pos].trim();
                return CursorContext {
                    expected_type: parse_type_str(type_str),
                    in_argument: false,
                    argument_index: None,
                    in_let_rhs: true,
                    in_match_arm: false,
                    in_return: false,
                };
            }
        }
    }

    // Check for return position
    if trimmed.starts_with("return") || trimmed.ends_with("->") {
        let ret_type = context_lines.iter().find_map(|l| {
            let l = l.trim();
            if let Some(arrow) = l.find("->") {
                let after = l[arrow + 2..].trim();
                let end = after.find('{').unwrap_or(after.len());
                Some(after[..end].trim().to_string())
            } else {
                None
            }
        });

        return CursorContext {
            expected_type: ret_type.map_or(ExpectedType::Unknown, |t| parse_type_str(&t)),
            in_argument: false,
            argument_index: None,
            in_let_rhs: false,
            in_match_arm: false,
            in_return: true,
        };
    }

    // Check for function argument
    let before_cursor = if col < line.len() { &line[..col] } else { line };
    if let Some(paren_pos) = before_cursor.rfind('(') {
        let in_parens = &before_cursor[paren_pos + 1..];
        let arg_idx = in_parens.matches(',').count();
        return CursorContext {
            expected_type: ExpectedType::Unknown,
            in_argument: true,
            argument_index: Some(arg_idx),
            in_let_rhs: false,
            in_match_arm: false,
            in_return: false,
        };
    }

    // Check for match arm
    if trimmed.contains("=>") || context_lines.iter().any(|l| l.trim().starts_with("match")) {
        return CursorContext {
            expected_type: ExpectedType::Unknown,
            in_argument: false,
            argument_index: None,
            in_let_rhs: false,
            in_match_arm: true,
            in_return: false,
        };
    }

    CursorContext {
        expected_type: ExpectedType::Unknown,
        in_argument: false,
        argument_index: None,
        in_let_rhs: false,
        in_match_arm: false,
        in_return: false,
    }
}

fn parse_type_str(s: &str) -> ExpectedType {
    let s = s.trim();
    if s.is_empty() || s == "_" {
        return ExpectedType::Any;
    }
    if let Some(angle) = s.find('<') {
        let name = &s[..angle];
        let params_str = &s[angle + 1..s.len().saturating_sub(1)];
        let params: Vec<String> = params_str
            .split(',')
            .map(|p| p.trim().to_string())
            .collect();
        ExpectedType::Generic(name.into(), params)
    } else {
        ExpectedType::Named(s.into())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S31.2: Expression Synthesis
// ═══════════════════════════════════════════════════════════════════════

/// A synthesized expression completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesizedExpr {
    /// The expression text.
    pub text: String,
    /// The type it produces.
    pub result_type: String,
    /// Relevance score (higher = better).
    pub score: u32,
    /// Label for display.
    pub label: String,
}

/// Synthesizes expressions that produce the expected type.
pub fn synthesize_expressions(
    expected: &ExpectedType,
    scope_vars: &[(&str, &str)],
) -> Vec<SynthesizedExpr> {
    let mut results = Vec::new();

    match expected {
        ExpectedType::Named(name) => {
            // Suggest variables of matching type from scope
            for &(var_name, var_type) in scope_vars {
                if var_type == name {
                    results.push(SynthesizedExpr {
                        text: var_name.into(),
                        result_type: var_type.into(),
                        score: 100,
                        label: format!("{var_name}: {var_type}"),
                    });
                }
            }

            // Suggest constructors
            match name.as_str() {
                "bool" => {
                    results.push(SynthesizedExpr {
                        text: "true".into(),
                        result_type: "bool".into(),
                        score: 80,
                        label: "true".into(),
                    });
                    results.push(SynthesizedExpr {
                        text: "false".into(),
                        result_type: "bool".into(),
                        score: 80,
                        label: "false".into(),
                    });
                }
                "String" | "str" => {
                    results.push(SynthesizedExpr {
                        text: "String::new()".into(),
                        result_type: "String".into(),
                        score: 70,
                        label: "String::new()".into(),
                    });
                }
                _ => {}
            }
        }
        ExpectedType::Generic(name, params) => match name.as_str() {
            "Option" => {
                results.push(SynthesizedExpr {
                    text: "None".into(),
                    result_type: format!("Option<{}>", params.first().map_or("_", |s| s.as_str())),
                    score: 90,
                    label: "None".into(),
                });
                results.push(SynthesizedExpr {
                    text: "Some($1)".into(),
                    result_type: format!("Option<{}>", params.first().map_or("_", |s| s.as_str())),
                    score: 85,
                    label: "Some(...)".into(),
                });
            }
            "Result" => {
                results.push(SynthesizedExpr {
                    text: "Ok($1)".into(),
                    result_type: format!("Result<{}>", params.join(", ")),
                    score: 90,
                    label: "Ok(...)".into(),
                });
                results.push(SynthesizedExpr {
                    text: "Err($1)".into(),
                    result_type: format!("Result<{}>", params.join(", ")),
                    score: 85,
                    label: "Err(...)".into(),
                });
            }
            "Vec" => {
                results.push(SynthesizedExpr {
                    text: "Vec::new()".into(),
                    result_type: format!("Vec<{}>", params.first().map_or("_", |s| s.as_str())),
                    score: 90,
                    label: "Vec::new()".into(),
                });
                results.push(SynthesizedExpr {
                    text: "vec![]".into(),
                    result_type: format!("Vec<{}>", params.first().map_or("_", |s| s.as_str())),
                    score: 85,
                    label: "vec![]".into(),
                });
            }
            _ => {}
        },
        _ => {}
    }

    // Sort by score descending
    results.sort_by_key(|x| std::cmp::Reverse(x.score));
    results
}

// ═══════════════════════════════════════════════════════════════════════
// S31.3: Fill-in-the-Blank
// ═══════════════════════════════════════════════════════════════════════

/// A fill-in-the-blank suggestion.
#[derive(Debug, Clone)]
pub struct FillSuggestion {
    /// Completion text.
    pub text: String,
    /// Display label.
    pub label: String,
    /// Documentation.
    pub doc: String,
}

/// Suggests completions for a typed let binding.
pub fn fill_in_blank(type_name: &str, type_params: &[&str]) -> Vec<FillSuggestion> {
    let mut suggestions = Vec::new();

    match type_name {
        "Vec" => {
            let elem = type_params.first().copied().unwrap_or("_");
            suggestions.push(FillSuggestion {
                text: "Vec::new()".into(),
                label: "Vec::new()".into(),
                doc: format!("Creates an empty Vec<{elem}>"),
            });
            suggestions.push(FillSuggestion {
                text: "vec![]".into(),
                label: "vec![]".into(),
                doc: format!("Creates a Vec<{elem}> with elements"),
            });
            suggestions.push(FillSuggestion {
                text: "Vec::with_capacity($1)".into(),
                label: "Vec::with_capacity(cap)".into(),
                doc: format!("Creates a Vec<{elem}> with pre-allocated capacity"),
            });
        }
        "HashMap" => {
            suggestions.push(FillSuggestion {
                text: "HashMap::new()".into(),
                label: "HashMap::new()".into(),
                doc: "Creates an empty HashMap".into(),
            });
        }
        "String" => {
            suggestions.push(FillSuggestion {
                text: "String::new()".into(),
                label: "String::new()".into(),
                doc: "Creates an empty String".into(),
            });
            suggestions.push(FillSuggestion {
                text: "String::from($1)".into(),
                label: "String::from(...)".into(),
                doc: "Creates a String from a str".into(),
            });
        }
        "Option" => {
            suggestions.push(FillSuggestion {
                text: "None".into(),
                label: "None".into(),
                doc: "No value".into(),
            });
            suggestions.push(FillSuggestion {
                text: "Some($1)".into(),
                label: "Some(...)".into(),
                doc: "Some value".into(),
            });
        }
        "Result" => {
            suggestions.push(FillSuggestion {
                text: "Ok($1)".into(),
                label: "Ok(...)".into(),
                doc: "Success value".into(),
            });
            suggestions.push(FillSuggestion {
                text: "Err($1)".into(),
                label: "Err(...)".into(),
                doc: "Error value".into(),
            });
        }
        _ => {
            // Suggest default constructor
            suggestions.push(FillSuggestion {
                text: format!("{type_name}::new()"),
                label: format!("{type_name}::new()"),
                doc: format!("Create a new {type_name}"),
            });
        }
    }

    suggestions
}

// ═══════════════════════════════════════════════════════════════════════
// S31.4: Argument Completion
// ═══════════════════════════════════════════════════════════════════════

/// An argument completion suggestion.
#[derive(Debug, Clone)]
pub struct ArgCompletion {
    /// The suggested value.
    pub value: String,
    /// The type of the value.
    pub value_type: String,
    /// Source (variable name, literal, etc.).
    pub source: String,
    /// Score for ranking.
    pub score: u32,
}

/// Suggests arguments for a function parameter.
pub fn complete_argument(param_type: &str, scope_vars: &[(&str, &str)]) -> Vec<ArgCompletion> {
    let mut results: Vec<ArgCompletion> = scope_vars
        .iter()
        .filter(|(_, ty)| *ty == param_type)
        .map(|(name, ty)| ArgCompletion {
            value: (*name).into(),
            value_type: (*ty).into(),
            source: format!("local `{name}`"),
            score: 100,
        })
        .collect();

    // Add default constructors for common types
    match param_type {
        "bool" => {
            results.push(ArgCompletion {
                value: "true".into(),
                value_type: "bool".into(),
                source: "literal".into(),
                score: 50,
            });
            results.push(ArgCompletion {
                value: "false".into(),
                value_type: "bool".into(),
                source: "literal".into(),
                score: 50,
            });
        }
        "i32" | "i64" | "u32" | "u64" | "usize" => {
            results.push(ArgCompletion {
                value: "0".into(),
                value_type: param_type.into(),
                source: "literal".into(),
                score: 30,
            });
        }
        _ => {}
    }

    results.sort_by_key(|x| std::cmp::Reverse(x.score));
    results
}

// ═══════════════════════════════════════════════════════════════════════
// S31.5: Pattern Completion
// ═══════════════════════════════════════════════════════════════════════

/// A pattern suggestion for match arms.
#[derive(Debug, Clone)]
pub struct PatternSuggestion {
    /// The pattern text.
    pub pattern: String,
    /// Body placeholder.
    pub body: String,
    /// Whether this is the wildcard/default.
    pub is_wildcard: bool,
}

/// Generates exhaustive match patterns for an enum.
pub fn complete_patterns(
    enum_name: &str,
    variants: &[(&str, usize)], // (name, field_count)
) -> Vec<PatternSuggestion> {
    let mut patterns = Vec::new();

    for (variant, field_count) in variants {
        let pattern = if *field_count == 0 {
            format!("{enum_name}::{variant}")
        } else {
            let fields: Vec<String> = (0..*field_count).map(|i| format!("_{i}")).collect();
            format!("{}::{}({})", enum_name, variant, fields.join(", "))
        };

        patterns.push(PatternSuggestion {
            pattern,
            body: "todo!()".into(),
            is_wildcard: false,
        });
    }

    // Add wildcard arm
    patterns.push(PatternSuggestion {
        pattern: "_".into(),
        body: "todo!()".into(),
        is_wildcard: true,
    });

    patterns
}

// ═══════════════════════════════════════════════════════════════════════
// S31.6: Import Suggestions
// ═══════════════════════════════════════════════════════════════════════

/// An import suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSuggestion {
    /// The use statement to add.
    pub use_statement: String,
    /// The fully qualified name.
    pub fqn: String,
    /// The item kind.
    pub kind: ImportKind,
}

/// Kind of importable item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    /// A function.
    Function,
    /// A struct.
    Struct,
    /// An enum.
    Enum,
    /// A trait.
    Trait,
    /// A module.
    Module,
    /// A type alias.
    TypeAlias,
    /// A constant.
    Const,
}

/// Suggests imports for an unresolved name.
pub fn suggest_imports(
    name: &str,
    available: &[(&str, &str, ImportKind)], // (item_name, module_path, kind)
) -> Vec<ImportSuggestion> {
    available
        .iter()
        .filter(|(item_name, _, _)| *item_name == name)
        .map(|(item_name, module_path, kind)| ImportSuggestion {
            use_statement: format!("use {module_path}::{item_name}"),
            fqn: format!("{module_path}::{item_name}"),
            kind: *kind,
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S31.7: Postfix Completion
// ═══════════════════════════════════════════════════════════════════════

/// A postfix completion item.
#[derive(Debug, Clone)]
pub struct PostfixCompletion {
    /// Trigger (e.g., ".if", ".match").
    pub trigger: String,
    /// The generated code template.
    pub template: String,
    /// Description.
    pub description: String,
}

/// Generates postfix completions for an expression.
pub fn postfix_completions(expr: &str, expr_type: &str) -> Vec<PostfixCompletion> {
    let mut completions = Vec::new();

    // .if — for boolean expressions
    if expr_type == "bool" {
        completions.push(PostfixCompletion {
            trigger: ".if".into(),
            template: format!("if {expr} {{\n    $1\n}}"),
            description: "Wrap in if block".into(),
        });
        completions.push(PostfixCompletion {
            trigger: ".while".into(),
            template: format!("while {expr} {{\n    $1\n}}"),
            description: "Wrap in while loop".into(),
        });
        completions.push(PostfixCompletion {
            trigger: ".not".into(),
            template: format!("!{expr}"),
            description: "Negate boolean".into(),
        });
    }

    // .match — for any expression
    completions.push(PostfixCompletion {
        trigger: ".match".into(),
        template: format!("match {expr} {{\n    $1 => $2,\n}}"),
        description: "Wrap in match block".into(),
    });

    // .let — bind to variable
    completions.push(PostfixCompletion {
        trigger: ".let".into(),
        template: format!("let $1 = {expr}"),
        description: "Bind to variable".into(),
    });

    // .dbg — debug print
    completions.push(PostfixCompletion {
        trigger: ".dbg".into(),
        template: format!("dbg!({expr})"),
        description: "Debug print".into(),
    });

    // .some / .ok for wrapping
    if expr_type != "Option" && expr_type != "Result" {
        completions.push(PostfixCompletion {
            trigger: ".some".into(),
            template: format!("Some({expr})"),
            description: "Wrap in Some()".into(),
        });
        completions.push(PostfixCompletion {
            trigger: ".ok".into(),
            template: format!("Ok({expr})"),
            description: "Wrap in Ok()".into(),
        });
    }

    completions
}

// ═══════════════════════════════════════════════════════════════════════
// S31.8: Snippet Templates
// ═══════════════════════════════════════════════════════════════════════

/// A context-aware snippet template.
#[derive(Debug, Clone)]
pub struct SnippetTemplate {
    /// Trigger prefix.
    pub prefix: String,
    /// Snippet body (with tab stops).
    pub body: String,
    /// Description.
    pub description: String,
    /// When this snippet is available.
    pub context: SnippetContext,
}

/// Context in which a snippet is applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetContext {
    /// Top-level item position.
    TopLevel,
    /// Inside a function body.
    FunctionBody,
    /// Inside an impl block.
    ImplBlock,
    /// Anywhere.
    Any,
}

/// Returns built-in snippet templates.
pub fn builtin_snippets() -> Vec<SnippetTemplate> {
    vec![
        SnippetTemplate {
            prefix: "fn".into(),
            body: "fn ${1:name}(${2:params}) -> ${3:ReturnType} {\n    ${4:todo!()}\n}".into(),
            description: "Function definition".into(),
            context: SnippetContext::TopLevel,
        },
        SnippetTemplate {
            prefix: "for".into(),
            body: "for ${1:item} in ${2:iterable} {\n    ${3}\n}".into(),
            description: "For loop over iterable".into(),
            context: SnippetContext::FunctionBody,
        },
        SnippetTemplate {
            prefix: "test".into(),
            body: "@test\nfn ${1:test_name}() {\n    ${2:assert_eq!(1, 1)}\n}".into(),
            description: "Test function".into(),
            context: SnippetContext::TopLevel,
        },
        SnippetTemplate {
            prefix: "match".into(),
            body: "match ${1:expr} {\n    ${2:pattern} => ${3:body},\n    _ => ${4:todo!()},\n}".into(),
            description: "Match expression".into(),
            context: SnippetContext::FunctionBody,
        },
        SnippetTemplate {
            prefix: "struct".into(),
            body: "struct ${1:Name} {\n    ${2:field}: ${3:Type},\n}".into(),
            description: "Struct definition".into(),
            context: SnippetContext::TopLevel,
        },
        SnippetTemplate {
            prefix: "impl".into(),
            body: "impl ${1:Type} {\n    fn ${2:method}(&self) -> ${3:ReturnType} {\n        ${4:todo!()}\n    }\n}".into(),
            description: "Impl block".into(),
            context: SnippetContext::TopLevel,
        },
        SnippetTemplate {
            prefix: "err".into(),
            body: "match ${1:expr} {\n    Ok(val) => val,\n    Err(e) => return Err(e),\n}".into(),
            description: "Error handling boilerplate".into(),
            context: SnippetContext::FunctionBody,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// S31.9: Completion Ranking
// ═══════════════════════════════════════════════════════════════════════

/// Factors for ranking completion items.
#[derive(Debug, Clone)]
pub struct RankingFactors {
    /// Type match bonus (0-100).
    pub type_match: u32,
    /// Name similarity score (0-100).
    pub name_similarity: u32,
    /// Recency bonus (0-50).
    pub recency: u32,
    /// Locality bonus — same scope (0-30).
    pub locality: u32,
}

impl RankingFactors {
    /// Computes the total score.
    pub fn total_score(&self) -> u32 {
        self.type_match + self.name_similarity + self.recency + self.locality
    }
}

/// A ranked completion item.
#[derive(Debug, Clone)]
pub struct RankedCompletion {
    /// The completion text.
    pub text: String,
    /// Display label.
    pub label: String,
    /// Kind (function, variable, type, etc.).
    pub kind: String,
    /// Ranking factors.
    pub factors: RankingFactors,
}

/// Ranks and sorts completions by relevance.
pub fn rank_completions(items: &mut [RankedCompletion]) {
    items.sort_by(|a, b| {
        b.factors
            .total_score()
            .cmp(&a.factors.total_score())
            .then_with(|| a.label.cmp(&b.label))
    });
}

/// Computes name similarity using common prefix length.
pub fn name_similarity(prefix: &str, candidate: &str) -> u32 {
    if candidate.starts_with(prefix) {
        let ratio = prefix.len() as f64 / candidate.len().max(1) as f64;
        (ratio * 100.0) as u32
    } else {
        // Check for substring match
        if candidate.contains(prefix) {
            30
        } else {
            0
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S31.1 — Expected Type Analysis
    #[test]
    fn s31_1_expected_type_from_let() {
        let ctx = analyze_expected_type("    let x: i32 = ", 18, &[]);
        assert!(ctx.in_let_rhs);
        assert_eq!(ctx.expected_type, ExpectedType::Named("i32".into()));
    }

    #[test]
    fn s31_1_expected_type_generic() {
        let ctx = analyze_expected_type("    let v: Vec<i32> = ", 22, &[]);
        assert!(ctx.in_let_rhs);
        assert_eq!(
            ctx.expected_type,
            ExpectedType::Generic("Vec".into(), vec!["i32".into()])
        );
    }

    #[test]
    fn s31_1_expected_type_in_arg() {
        let ctx = analyze_expected_type("    foo(x, ", 11, &[]);
        assert!(ctx.in_argument);
        assert_eq!(ctx.argument_index, Some(1));
    }

    // S31.2 — Expression Synthesis
    #[test]
    fn s31_2_synthesize_option() {
        let expected = ExpectedType::Generic("Option".into(), vec!["i32".into()]);
        let exprs = synthesize_expressions(&expected, &[]);
        assert!(exprs.iter().any(|e| e.text == "None"));
        assert!(exprs.iter().any(|e| e.text == "Some($1)"));
    }

    #[test]
    fn s31_2_synthesize_from_scope() {
        let expected = ExpectedType::Named("i32".into());
        let scope = vec![("count", "i32"), ("name", "String")];
        let exprs = synthesize_expressions(&expected, &scope);
        assert!(exprs.iter().any(|e| e.text == "count"));
        assert!(!exprs.iter().any(|e| e.text == "name"));
    }

    // S31.3 — Fill-in-the-Blank
    #[test]
    fn s31_3_fill_vec() {
        let suggestions = fill_in_blank("Vec", &["i32"]);
        assert!(suggestions.iter().any(|s| s.text == "Vec::new()"));
        assert!(suggestions.iter().any(|s| s.text == "vec![]"));
        assert!(suggestions.len() >= 3);
    }

    #[test]
    fn s31_3_fill_option() {
        let suggestions = fill_in_blank("Option", &["String"]);
        assert!(suggestions.iter().any(|s| s.text == "None"));
        assert!(suggestions.iter().any(|s| s.text.contains("Some")));
    }

    // S31.4 — Argument Completion
    #[test]
    fn s31_4_arg_completion_by_type() {
        let scope = vec![("x", "i32"), ("y", "i32"), ("name", "String")];
        let completions = complete_argument("i32", &scope);
        assert!(completions.iter().any(|c| c.value == "x"));
        assert!(completions.iter().any(|c| c.value == "y"));
        assert!(!completions.iter().any(|c| c.value == "name"));
    }

    #[test]
    fn s31_4_arg_completion_bool_literals() {
        let completions = complete_argument("bool", &[]);
        assert!(completions.iter().any(|c| c.value == "true"));
        assert!(completions.iter().any(|c| c.value == "false"));
    }

    // S31.5 — Pattern Completion
    #[test]
    fn s31_5_enum_patterns() {
        let variants = vec![("Circle", 1), ("Rect", 2), ("Point", 0)];
        let patterns = complete_patterns("Shape", &variants);
        assert!(patterns.iter().any(|p| p.pattern.contains("Circle")));
        assert!(patterns.iter().any(|p| p.pattern.contains("Rect")));
        assert!(patterns.iter().any(|p| p.pattern == "_"));
    }

    #[test]
    fn s31_5_pattern_with_fields() {
        let variants = vec![("Some", 1), ("None", 0)];
        let patterns = complete_patterns("Option", &variants);
        let some = patterns
            .iter()
            .find(|p| p.pattern.contains("Some"))
            .unwrap();
        assert!(some.pattern.contains("_0"));
    }

    // S31.6 — Import Suggestions
    #[test]
    fn s31_6_suggest_import() {
        let available = vec![
            ("HashMap", "std::collections", ImportKind::Struct),
            ("HashSet", "std::collections", ImportKind::Struct),
        ];
        let suggestions = suggest_imports("HashMap", &available);
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0]
            .use_statement
            .contains("std::collections::HashMap"));
    }

    #[test]
    fn s31_6_no_import_found() {
        let available: Vec<(&str, &str, ImportKind)> = vec![];
        let suggestions = suggest_imports("NonExistent", &available);
        assert!(suggestions.is_empty());
    }

    // S31.7 — Postfix Completion
    #[test]
    fn s31_7_postfix_bool() {
        let completions = postfix_completions("x > 0", "bool");
        assert!(completions.iter().any(|c| c.trigger == ".if"));
        assert!(completions.iter().any(|c| c.trigger == ".while"));
        assert!(completions.iter().any(|c| c.trigger == ".not"));
    }

    #[test]
    fn s31_7_postfix_general() {
        let completions = postfix_completions("value", "i32");
        assert!(completions.iter().any(|c| c.trigger == ".match"));
        assert!(completions.iter().any(|c| c.trigger == ".let"));
        assert!(completions.iter().any(|c| c.trigger == ".dbg"));
    }

    // S31.8 — Snippet Templates
    #[test]
    fn s31_8_builtin_snippets() {
        let snippets = builtin_snippets();
        assert!(snippets.len() >= 5);
        assert!(snippets.iter().any(|s| s.prefix == "fn"));
        assert!(snippets.iter().any(|s| s.prefix == "for"));
        assert!(snippets.iter().any(|s| s.prefix == "test"));
    }

    // S31.9 — Completion Ranking
    #[test]
    fn s31_9_ranking_type_match_wins() {
        let mut items = vec![
            RankedCompletion {
                text: "foo".into(),
                label: "foo".into(),
                kind: "variable".into(),
                factors: RankingFactors {
                    type_match: 100,
                    name_similarity: 10,
                    recency: 0,
                    locality: 0,
                },
            },
            RankedCompletion {
                text: "bar".into(),
                label: "bar".into(),
                kind: "variable".into(),
                factors: RankingFactors {
                    type_match: 0,
                    name_similarity: 100,
                    recency: 50,
                    locality: 30,
                },
            },
        ];
        rank_completions(&mut items);
        assert_eq!(items[0].text, "bar"); // 180 > 110
    }

    #[test]
    fn s31_9_name_similarity() {
        assert_eq!(name_similarity("get", "get_name"), 37);
        assert_eq!(name_similarity("xyz", "abc"), 0);
        assert!(name_similarity("name", "get_name") > 0); // substring
    }

    // S31.10 — Integration
    #[test]
    fn s31_10_expected_type_display() {
        assert_eq!(ExpectedType::Named("i32".into()).to_string(), "i32");
        assert_eq!(
            ExpectedType::Generic("Vec".into(), vec!["i32".into()]).to_string(),
            "Vec<i32>"
        );
        assert_eq!(ExpectedType::Any.to_string(), "_");
    }
}
