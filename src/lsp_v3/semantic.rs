//! LSP v3 Semantic Analysis — tokens, navigation, hierarchy, hints, lens.
//!
//! Provides rich semantic information for IDE features including
//! go-to-definition, find references, call hierarchy, inlay hints,
//! and code lens annotations.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// L1.1: Semantic Tokens (24 types, 8 modifiers)
// ═══════════════════════════════════════════════════════════════════════

/// Semantic token types as defined by LSP 3.17.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticTokenType {
    Namespace,
    Type,
    Class,
    Enum,
    Interface,
    Struct,
    TypeParameter,
    Parameter,
    Variable,
    Property,
    EnumMember,
    Event,
    Function,
    Method,
    Macro,
    Keyword,
    Modifier,
    Comment,
    String,
    Number,
    Regexp,
    Operator,
    Decorator,
    Label,
}

impl SemanticTokenType {
    /// Returns the LSP legend index for this token type.
    pub fn index(self) -> u32 {
        self as u32
    }

    /// Returns all token types for the legend.
    pub fn legend() -> Vec<&'static str> {
        vec![
            "namespace",
            "type",
            "class",
            "enum",
            "interface",
            "struct",
            "typeParameter",
            "parameter",
            "variable",
            "property",
            "enumMember",
            "event",
            "function",
            "method",
            "macro",
            "keyword",
            "modifier",
            "comment",
            "string",
            "number",
            "regexp",
            "operator",
            "decorator",
            "label",
        ]
    }
}

/// Semantic token modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticTokenModifier {
    Declaration,
    Definition,
    Readonly,
    Static,
    Deprecated,
    Abstract,
    Async,
    Modification,
}

impl SemanticTokenModifier {
    /// Returns the bitmask for this modifier.
    pub fn bitmask(self) -> u32 {
        1 << (self as u32)
    }

    /// Returns all modifier names for the legend.
    pub fn legend() -> Vec<&'static str> {
        vec![
            "declaration",
            "definition",
            "readonly",
            "static",
            "deprecated",
            "abstract",
            "async",
            "modification",
        ]
    }
}

/// A single semantic token in the encoded delta format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticToken {
    /// Delta line from previous token.
    pub delta_line: u32,
    /// Delta start character from previous token on same line.
    pub delta_start: u32,
    /// Token length.
    pub length: u32,
    /// Token type index.
    pub token_type: u32,
    /// Token modifier bitmask.
    pub token_modifiers: u32,
}

/// Encodes semantic tokens from absolute positions to delta format.
pub fn encode_semantic_tokens(tokens: &[AbsoluteToken]) -> Vec<SemanticToken> {
    let mut result = Vec::with_capacity(tokens.len());
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;

    for tok in tokens {
        let delta_line = tok.line - prev_line;
        let delta_start = if delta_line == 0 {
            tok.start - prev_start
        } else {
            tok.start
        };
        result.push(SemanticToken {
            delta_line,
            delta_start,
            length: tok.length,
            token_type: tok.token_type,
            token_modifiers: tok.modifiers,
        });
        prev_line = tok.line;
        prev_start = tok.start;
    }
    result
}

/// Absolute-position semantic token (before delta encoding).
#[derive(Debug, Clone)]
pub struct AbsoluteToken {
    /// Line number (0-based).
    pub line: u32,
    /// Start character (0-based).
    pub start: u32,
    /// Token length.
    pub length: u32,
    /// Token type (index into legend).
    pub token_type: u32,
    /// Modifier bitmask.
    pub modifiers: u32,
}

// ═══════════════════════════════════════════════════════════════════════
// L1.2-L1.3: Go-to-Definition & Find References
// ═══════════════════════════════════════════════════════════════════════

/// A source location (file + position).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    /// File path (URI).
    pub uri: String,
    /// Line number (0-based).
    pub line: u32,
    /// Column (0-based, UTF-16 offset).
    pub character: u32,
    /// End line.
    pub end_line: u32,
    /// End column.
    pub end_character: u32,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.uri, self.line + 1, self.character + 1)
    }
}

/// Symbol definition information.
#[derive(Debug, Clone)]
pub struct DefinitionInfo {
    /// Symbol name.
    pub name: String,
    /// Symbol kind.
    pub kind: SymbolKind,
    /// Definition location.
    pub location: Location,
    /// Documentation string.
    pub documentation: Option<String>,
    /// Type signature.
    pub signature: Option<String>,
}

/// Find-references result.
#[derive(Debug, Clone)]
pub struct ReferenceResult {
    /// Definition location.
    pub definition: Location,
    /// All reference locations (including definition if requested).
    pub references: Vec<Location>,
    /// Whether the definition is included in references.
    pub include_definition: bool,
}

/// Symbol kinds for navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Method,
    Variable,
    Constant,
    Struct,
    Enum,
    EnumVariant,
    Trait,
    TraitMethod,
    Module,
    Field,
    Parameter,
    TypeAlias,
    Macro,
    Label,
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Function => write!(f, "function"),
            Self::Method => write!(f, "method"),
            Self::Variable => write!(f, "variable"),
            Self::Constant => write!(f, "constant"),
            Self::Struct => write!(f, "struct"),
            Self::Enum => write!(f, "enum"),
            Self::EnumVariant => write!(f, "enum variant"),
            Self::Trait => write!(f, "trait"),
            Self::TraitMethod => write!(f, "trait method"),
            Self::Module => write!(f, "module"),
            Self::Field => write!(f, "field"),
            Self::Parameter => write!(f, "parameter"),
            Self::TypeAlias => write!(f, "type alias"),
            Self::Macro => write!(f, "macro"),
            Self::Label => write!(f, "label"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// L1.4-L1.5: Go-to-Implementation & Type Hierarchy
// ═══════════════════════════════════════════════════════════════════════

/// Implementation lookup result.
#[derive(Debug, Clone)]
pub struct ImplementationResult {
    /// Trait or type being implemented.
    pub target: DefinitionInfo,
    /// All implementations found.
    pub implementations: Vec<Location>,
}

/// Type hierarchy item.
#[derive(Debug, Clone)]
pub struct TypeHierarchyItem {
    /// Type name.
    pub name: String,
    /// Kind (struct, enum, trait).
    pub kind: SymbolKind,
    /// Location.
    pub location: Location,
    /// Supertypes (traits this type implements).
    pub supertypes: Vec<String>,
    /// Subtypes (types that implement this trait).
    pub subtypes: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// L1.6: Call Hierarchy
// ═══════════════════════════════════════════════════════════════════════

/// Call hierarchy item.
#[derive(Debug, Clone)]
pub struct CallHierarchyItem {
    /// Function name.
    pub name: String,
    /// Kind.
    pub kind: SymbolKind,
    /// Location of the function definition.
    pub location: Location,
    /// Detail (e.g., full signature).
    pub detail: Option<String>,
}

/// An incoming call (who calls this function).
#[derive(Debug, Clone)]
pub struct IncomingCall {
    /// The caller.
    pub from: CallHierarchyItem,
    /// Ranges within the caller where the call occurs.
    pub from_ranges: Vec<Location>,
}

/// An outgoing call (what this function calls).
#[derive(Debug, Clone)]
pub struct OutgoingCall {
    /// The callee.
    pub to: CallHierarchyItem,
    /// Ranges where the call occurs within this function.
    pub from_ranges: Vec<Location>,
}

// ═══════════════════════════════════════════════════════════════════════
// L1.7-L1.8: Workspace & Document Symbols
// ═══════════════════════════════════════════════════════════════════════

/// Document symbol (outline view).
#[derive(Debug, Clone)]
pub struct DocumentSymbol {
    /// Symbol name.
    pub name: String,
    /// Detail (e.g., type signature).
    pub detail: Option<String>,
    /// Kind.
    pub kind: SymbolKind,
    /// Full range of the symbol.
    pub range: Location,
    /// Range of the symbol's name.
    pub selection_range: Location,
    /// Children (e.g., methods inside impl).
    pub children: Vec<DocumentSymbol>,
}

/// Workspace symbol (cross-file search).
#[derive(Debug, Clone)]
pub struct WorkspaceSymbol {
    /// Symbol name.
    pub name: String,
    /// Kind.
    pub kind: SymbolKind,
    /// Location.
    pub location: Location,
    /// Container name (e.g., module or struct).
    pub container: Option<String>,
}

/// Searches workspace symbols matching a query.
pub fn search_workspace_symbols<'a>(
    symbols: &'a [WorkspaceSymbol],
    query: &str,
) -> Vec<&'a WorkspaceSymbol> {
    if query.is_empty() {
        return symbols.iter().collect();
    }
    let query_lower = query.to_lowercase();
    symbols
        .iter()
        .filter(|s| {
            let name_lower = s.name.to_lowercase();
            // Fuzzy match: all query chars appear in order
            let mut qi = 0;
            let query_chars: Vec<char> = query_lower.chars().collect();
            for ch in name_lower.chars() {
                if qi < query_chars.len() && ch == query_chars[qi] {
                    qi += 1;
                }
            }
            qi == query_chars.len()
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// L1.9-L1.10: Import Suggestions & Unused Detection
// ═══════════════════════════════════════════════════════════════════════

/// An import suggestion.
#[derive(Debug, Clone)]
pub struct ImportSuggestion {
    /// The unresolved name.
    pub name: String,
    /// Suggested import path.
    pub import_path: String,
    /// Module where the symbol is defined.
    pub source_module: String,
    /// Kind of the imported symbol.
    pub kind: SymbolKind,
}

/// An unused import diagnostic.
#[derive(Debug, Clone)]
pub struct UnusedImport {
    /// The import path.
    pub path: String,
    /// Location of the use statement.
    pub location: Location,
    /// Whether it's completely unused or just a partial unused path.
    pub fully_unused: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// L1.11-L1.12: Hover & Parameter Info
// ═══════════════════════════════════════════════════════════════════════

/// Hover information.
#[derive(Debug, Clone)]
pub struct HoverInfo {
    /// Markdown content.
    pub contents: String,
    /// Range of the hovered token.
    pub range: Option<Location>,
}

/// Generates hover markdown for a definition.
pub fn hover_markdown(def: &DefinitionInfo) -> String {
    let mut md = String::new();
    md.push_str("```fajar\n");
    if let Some(ref sig) = def.signature {
        md.push_str(sig);
    } else {
        md.push_str(&format!("{} {}", def.kind, def.name));
    }
    md.push_str("\n```\n");
    if let Some(ref doc) = def.documentation {
        md.push('\n');
        md.push_str(doc);
    }
    md
}

/// Parameter information for signature help.
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    /// Parameter label (name + type).
    pub label: String,
    /// Documentation for this parameter.
    pub documentation: Option<String>,
}

/// Signature help for a function call.
#[derive(Debug, Clone)]
pub struct SignatureHelp {
    /// Function signatures (may have overloads).
    pub signatures: Vec<SignatureInfo>,
    /// Active signature index.
    pub active_signature: u32,
    /// Active parameter index.
    pub active_parameter: u32,
}

/// A single function signature.
#[derive(Debug, Clone)]
pub struct SignatureInfo {
    /// Full signature label.
    pub label: String,
    /// Documentation.
    pub documentation: Option<String>,
    /// Parameters.
    pub parameters: Vec<ParameterInfo>,
}

// ═══════════════════════════════════════════════════════════════════════
// L1.13-L1.14: Inlay Hints
// ═══════════════════════════════════════════════════════════════════════

/// Inlay hint kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintKind {
    /// Type annotation hint (`: i32`).
    Type,
    /// Parameter name hint (`name:`).
    Parameter,
    /// Chained method return type.
    ChainingHint,
}

/// An inlay hint to display in the editor.
#[derive(Debug, Clone)]
pub struct InlayHint {
    /// Position to place the hint.
    pub line: u32,
    /// Character offset.
    pub character: u32,
    /// Hint label text.
    pub label: String,
    /// Hint kind.
    pub kind: InlayHintKind,
    /// Whether padding should be added before the hint.
    pub padding_left: bool,
    /// Whether padding should be added after the hint.
    pub padding_right: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// L1.15-L1.17: Code Lens
// ═══════════════════════════════════════════════════════════════════════

/// A code lens annotation.
#[derive(Debug, Clone)]
pub struct CodeLens {
    /// Location of the lens.
    pub range: Location,
    /// Display text.
    pub title: String,
    /// Command to execute when clicked.
    pub command: Option<String>,
    /// Kind of lens.
    pub kind: CodeLensKind,
}

/// Code lens kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLensKind {
    /// Test count for a function.
    TestCount(u32),
    /// Reference count.
    ReferenceCount(u32),
    /// Implementation count for a trait.
    ImplCount(u32),
    /// "Run" button for main/test functions.
    Run,
    /// "Debug" button.
    Debug,
}

impl fmt::Display for CodeLensKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TestCount(n) => write!(f, "{n} test{}", if *n == 1 { "" } else { "s" }),
            Self::ReferenceCount(n) => write!(f, "{n} reference{}", if *n == 1 { "" } else { "s" }),
            Self::ImplCount(n) => write!(f, "{n} implementation{}", if *n == 1 { "" } else { "s" }),
            Self::Run => write!(f, "Run"),
            Self::Debug => write!(f, "Debug"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// L1.18-L1.20: Folding, Breadcrumbs, Semantic Folding
// ═══════════════════════════════════════════════════════════════════════

/// A folding range.
#[derive(Debug, Clone)]
pub struct FoldingRange {
    /// Start line (0-based).
    pub start_line: u32,
    /// End line (0-based).
    pub end_line: u32,
    /// Kind.
    pub kind: FoldingRangeKind,
}

/// Folding range kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldingRangeKind {
    /// Comment block.
    Comment,
    /// Import block.
    Imports,
    /// Code region (function body, block, etc.).
    Region,
}

/// Breadcrumb navigation item.
#[derive(Debug, Clone)]
pub struct BreadcrumbItem {
    /// Name.
    pub name: String,
    /// Kind.
    pub kind: SymbolKind,
    /// Range.
    pub range: Location,
}

/// Computes breadcrumbs for a cursor position.
pub fn breadcrumbs_at(
    symbols: &[DocumentSymbol],
    line: u32,
    character: u32,
) -> Vec<BreadcrumbItem> {
    let mut result = Vec::new();
    fn walk(
        symbols: &[DocumentSymbol],
        line: u32,
        _character: u32,
        path: &mut Vec<BreadcrumbItem>,
    ) -> bool {
        for sym in symbols {
            if line >= sym.range.line && line <= sym.range.end_line {
                path.push(BreadcrumbItem {
                    name: sym.name.clone(),
                    kind: sym.kind,
                    range: sym.range.clone(),
                });
                if !sym.children.is_empty() {
                    walk(&sym.children, line, _character, path);
                }
                return true;
            }
        }
        false
    }
    walk(symbols, line, character, &mut result);
    result
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // L1.1: Semantic token legend
    #[test]
    fn l1_1_semantic_token_legend() {
        let types = SemanticTokenType::legend();
        assert_eq!(types.len(), 24);
        assert_eq!(types[0], "namespace");
        assert_eq!(types[12], "function");
    }

    #[test]
    fn l1_1_semantic_modifier_legend() {
        let mods = SemanticTokenModifier::legend();
        assert_eq!(mods.len(), 8);
        assert_eq!(mods[0], "declaration");
        assert_eq!(SemanticTokenModifier::Declaration.bitmask(), 1);
        assert_eq!(SemanticTokenModifier::Readonly.bitmask(), 4);
    }

    #[test]
    fn l1_1_encode_semantic_tokens() {
        let tokens = vec![
            AbsoluteToken {
                line: 0,
                start: 0,
                length: 2,
                token_type: 15,
                modifiers: 0,
            },
            AbsoluteToken {
                line: 0,
                start: 3,
                length: 4,
                token_type: 12,
                modifiers: 1,
            },
            AbsoluteToken {
                line: 1,
                start: 4,
                length: 3,
                token_type: 8,
                modifiers: 0,
            },
        ];
        let encoded = encode_semantic_tokens(&tokens);
        assert_eq!(encoded.len(), 3);
        assert_eq!(encoded[0].delta_line, 0);
        assert_eq!(encoded[0].delta_start, 0);
        assert_eq!(encoded[1].delta_line, 0);
        assert_eq!(encoded[1].delta_start, 3);
        assert_eq!(encoded[2].delta_line, 1);
        assert_eq!(encoded[2].delta_start, 4);
    }

    // L1.2: Go-to-definition
    #[test]
    fn l1_2_definition_info() {
        let def = DefinitionInfo {
            name: "fibonacci".to_string(),
            kind: SymbolKind::Function,
            location: Location {
                uri: "file:///main.fj".to_string(),
                line: 10,
                character: 3,
                end_line: 10,
                end_character: 12,
            },
            documentation: Some("Compute fibonacci number".to_string()),
            signature: Some("fn fibonacci(n: i32) -> i32".to_string()),
        };
        assert_eq!(def.kind, SymbolKind::Function);
        assert!(def.signature.as_ref().unwrap().contains("fibonacci"));
    }

    // L1.3: Find references
    #[test]
    fn l1_3_reference_result() {
        let refs = ReferenceResult {
            definition: Location {
                uri: "file:///main.fj".to_string(),
                line: 0,
                character: 3,
                end_line: 0,
                end_character: 12,
            },
            references: vec![
                Location {
                    uri: "file:///main.fj".to_string(),
                    line: 5,
                    character: 4,
                    end_line: 5,
                    end_character: 13,
                },
                Location {
                    uri: "file:///test.fj".to_string(),
                    line: 2,
                    character: 8,
                    end_line: 2,
                    end_character: 17,
                },
            ],
            include_definition: false,
        };
        assert_eq!(refs.references.len(), 2);
    }

    // L1.4: Go-to-implementation
    #[test]
    fn l1_4_implementation_result() {
        let result = ImplementationResult {
            target: DefinitionInfo {
                name: "Drawable".to_string(),
                kind: SymbolKind::Trait,
                location: Location {
                    uri: "file:///lib.fj".to_string(),
                    line: 0,
                    character: 0,
                    end_line: 5,
                    end_character: 0,
                },
                documentation: None,
                signature: Some("trait Drawable".to_string()),
            },
            implementations: vec![
                Location {
                    uri: "file:///circle.fj".to_string(),
                    line: 3,
                    character: 0,
                    end_line: 10,
                    end_character: 0,
                },
                Location {
                    uri: "file:///rect.fj".to_string(),
                    line: 5,
                    character: 0,
                    end_line: 12,
                    end_character: 0,
                },
            ],
        };
        assert_eq!(result.implementations.len(), 2);
    }

    // L1.5: Type hierarchy
    #[test]
    fn l1_5_type_hierarchy() {
        let item = TypeHierarchyItem {
            name: "Circle".to_string(),
            kind: SymbolKind::Struct,
            location: Location {
                uri: "file:///shapes.fj".to_string(),
                line: 0,
                character: 0,
                end_line: 0,
                end_character: 0,
            },
            supertypes: vec!["Drawable".to_string(), "Debug".to_string()],
            subtypes: vec![],
        };
        assert_eq!(item.supertypes.len(), 2);
        assert!(item.supertypes.contains(&"Drawable".to_string()));
    }

    // L1.6: Call hierarchy
    #[test]
    fn l1_6_call_hierarchy() {
        let item = CallHierarchyItem {
            name: "process".to_string(),
            kind: SymbolKind::Function,
            location: Location {
                uri: "file:///main.fj".to_string(),
                line: 10,
                character: 0,
                end_line: 20,
                end_character: 0,
            },
            detail: Some("fn process(data: [i32]) -> Result<i32, str>".to_string()),
        };
        let incoming = IncomingCall {
            from: item.clone(),
            from_ranges: vec![Location {
                uri: "file:///main.fj".to_string(),
                line: 15,
                character: 4,
                end_line: 15,
                end_character: 11,
            }],
        };
        assert_eq!(incoming.from_ranges.len(), 1);
    }

    // L1.7: Workspace symbol search
    #[test]
    fn l1_7_workspace_symbol_search() {
        let symbols = vec![
            WorkspaceSymbol {
                name: "fibonacci".to_string(),
                kind: SymbolKind::Function,
                location: Location {
                    uri: "a".to_string(),
                    line: 0,
                    character: 0,
                    end_line: 0,
                    end_character: 0,
                },
                container: None,
            },
            WorkspaceSymbol {
                name: "factorial".to_string(),
                kind: SymbolKind::Function,
                location: Location {
                    uri: "b".to_string(),
                    line: 0,
                    character: 0,
                    end_line: 0,
                    end_character: 0,
                },
                container: None,
            },
            WorkspaceSymbol {
                name: "Point".to_string(),
                kind: SymbolKind::Struct,
                location: Location {
                    uri: "c".to_string(),
                    line: 0,
                    character: 0,
                    end_line: 0,
                    end_character: 0,
                },
                container: None,
            },
        ];
        let results = search_workspace_symbols(&symbols, "fib");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "fibonacci");

        let results2 = search_workspace_symbols(&symbols, "f");
        assert_eq!(results2.len(), 2); // fibonacci, factorial

        let results3 = search_workspace_symbols(&symbols, "");
        assert_eq!(results3.len(), 3); // all
    }

    // L1.8: Document symbols
    #[test]
    fn l1_8_document_symbol_children() {
        let sym = DocumentSymbol {
            name: "Point".to_string(),
            detail: Some("struct Point".to_string()),
            kind: SymbolKind::Struct,
            range: Location {
                uri: "".to_string(),
                line: 0,
                character: 0,
                end_line: 10,
                end_character: 0,
            },
            selection_range: Location {
                uri: "".to_string(),
                line: 0,
                character: 7,
                end_line: 0,
                end_character: 12,
            },
            children: vec![
                DocumentSymbol {
                    name: "x".to_string(),
                    detail: Some("f64".to_string()),
                    kind: SymbolKind::Field,
                    range: Location {
                        uri: "".to_string(),
                        line: 1,
                        character: 4,
                        end_line: 1,
                        end_character: 10,
                    },
                    selection_range: Location {
                        uri: "".to_string(),
                        line: 1,
                        character: 4,
                        end_line: 1,
                        end_character: 5,
                    },
                    children: vec![],
                },
                DocumentSymbol {
                    name: "y".to_string(),
                    detail: Some("f64".to_string()),
                    kind: SymbolKind::Field,
                    range: Location {
                        uri: "".to_string(),
                        line: 2,
                        character: 4,
                        end_line: 2,
                        end_character: 10,
                    },
                    selection_range: Location {
                        uri: "".to_string(),
                        line: 2,
                        character: 4,
                        end_line: 2,
                        end_character: 5,
                    },
                    children: vec![],
                },
            ],
        };
        assert_eq!(sym.children.len(), 2);
    }

    // L1.11: Hover
    #[test]
    fn l1_11_hover_markdown() {
        let def = DefinitionInfo {
            name: "sqrt".to_string(),
            kind: SymbolKind::Function,
            location: Location {
                uri: "".to_string(),
                line: 0,
                character: 0,
                end_line: 0,
                end_character: 0,
            },
            documentation: Some("Returns the square root.".to_string()),
            signature: Some("fn sqrt(x: f64) -> f64".to_string()),
        };
        let md = hover_markdown(&def);
        assert!(md.contains("```fajar"));
        assert!(md.contains("fn sqrt(x: f64) -> f64"));
        assert!(md.contains("square root"));
    }

    // L1.13: Inlay hints
    #[test]
    fn l1_13_inlay_hint_kinds() {
        let hint = InlayHint {
            line: 5,
            character: 10,
            label: ": i32".to_string(),
            kind: InlayHintKind::Type,
            padding_left: true,
            padding_right: false,
        };
        assert_eq!(hint.kind, InlayHintKind::Type);
        assert!(hint.padding_left);
    }

    // L1.15: Code lens
    #[test]
    fn l1_15_code_lens_display() {
        assert_eq!(format!("{}", CodeLensKind::TestCount(3)), "3 tests");
        assert_eq!(format!("{}", CodeLensKind::TestCount(1)), "1 test");
        assert_eq!(
            format!("{}", CodeLensKind::ReferenceCount(5)),
            "5 references"
        );
        assert_eq!(
            format!("{}", CodeLensKind::ImplCount(2)),
            "2 implementations"
        );
        assert_eq!(format!("{}", CodeLensKind::Run), "Run");
    }

    // L1.18: Folding ranges
    #[test]
    fn l1_18_folding_range() {
        let range = FoldingRange {
            start_line: 5,
            end_line: 20,
            kind: FoldingRangeKind::Region,
        };
        assert_eq!(range.kind, FoldingRangeKind::Region);
    }

    // L1.19: Breadcrumbs
    #[test]
    fn l1_19_breadcrumbs() {
        let symbols = vec![DocumentSymbol {
            name: "main".to_string(),
            detail: None,
            kind: SymbolKind::Function,
            range: Location {
                uri: "".to_string(),
                line: 0,
                character: 0,
                end_line: 10,
                end_character: 0,
            },
            selection_range: Location {
                uri: "".to_string(),
                line: 0,
                character: 3,
                end_line: 0,
                end_character: 7,
            },
            children: vec![],
        }];
        let crumbs = breadcrumbs_at(&symbols, 5, 0);
        assert_eq!(crumbs.len(), 1);
        assert_eq!(crumbs[0].name, "main");
    }

    // L1.12: Signature help
    #[test]
    fn l1_12_signature_help() {
        let help = SignatureHelp {
            signatures: vec![SignatureInfo {
                label: "fn add(a: i32, b: i32) -> i32".to_string(),
                documentation: Some("Add two numbers".to_string()),
                parameters: vec![
                    ParameterInfo {
                        label: "a: i32".to_string(),
                        documentation: None,
                    },
                    ParameterInfo {
                        label: "b: i32".to_string(),
                        documentation: None,
                    },
                ],
            }],
            active_signature: 0,
            active_parameter: 1,
        };
        assert_eq!(help.signatures[0].parameters.len(), 2);
        assert_eq!(help.active_parameter, 1);
    }

    // L1.20: Location display
    #[test]
    fn l1_20_location_display() {
        let loc = Location {
            uri: "file:///main.fj".to_string(),
            line: 9,
            character: 3,
            end_line: 9,
            end_character: 12,
        };
        assert_eq!(format!("{loc}"), "file:///main.fj:10:4");
    }
}
