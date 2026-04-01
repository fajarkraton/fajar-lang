//! LSP completion, rename, inlay hints, and workspace symbols.
//!
//! Provides context-aware code intelligence features for IDE integration:
//! - Completion after `.`, `::`, and bare identifiers
//! - Rename symbol across a document
//! - Inlay type hints for `let` bindings
//! - Workspace-wide symbol search

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from LSP intelligence operations.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum LspError {
    /// The source position is out of bounds.
    #[error("position ({line}:{col}) is out of bounds")]
    PositionOutOfBounds {
        /// Line number (0-based).
        line: usize,
        /// Column number (0-based).
        col: usize,
    },

    /// Rename is not valid at this position.
    #[error("cannot rename: {reason}")]
    RenameInvalid {
        /// Description.
        reason: String,
    },

    /// Source parsing failed.
    #[error("parse error: {message}")]
    ParseFailed {
        /// Description.
        message: String,
    },

    /// No symbol found at position.
    #[error("no symbol at ({line}:{col})")]
    NoSymbolAtPosition {
        /// Line number.
        line: usize,
        /// Column number.
        col: usize,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Completion
// ═══════════════════════════════════════════════════════════════════════

/// What triggered the completion request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionTrigger {
    /// After `.` — list fields and methods.
    Dot,
    /// After `::` — list module items and associated functions.
    DoubleColon,
    /// After `<` — list generic type parameters.
    Angle,
    /// Default: list locals, params, builtins, imports.
    Default,
}

/// The kind of completion candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// A function or method.
    Function,
    /// A local variable or parameter.
    Variable,
    /// A struct type.
    Struct,
    /// An enum type.
    Enum,
    /// A struct field.
    Field,
    /// A module name.
    Module,
    /// A keyword.
    Keyword,
    /// A built-in function.
    Builtin,
}

/// A single completion candidate offered to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCandidate {
    /// Display label shown in the completion list.
    pub label: String,
    /// Kind of symbol.
    pub kind: CompletionKind,
    /// Optional detail (e.g., type signature).
    pub detail: Option<String>,
    /// Text to insert when accepted.
    pub insert_text: String,
}

/// Provides context-aware completion candidates.
#[derive(Debug)]
pub struct CompletionProvider {
    /// Built-in function names always available.
    builtins: Vec<CompletionCandidate>,
    /// Language keywords.
    keywords: Vec<CompletionCandidate>,
}

impl CompletionProvider {
    /// Creates a new completion provider with built-in entries.
    pub fn new() -> Self {
        Self {
            builtins: default_builtins(),
            keywords: default_keywords(),
        }
    }

    /// Returns completion candidates for the given source, position, and trigger.
    pub fn complete_at(
        &self,
        source: &str,
        line: usize,
        col: usize,
        trigger: CompletionTrigger,
    ) -> Vec<CompletionCandidate> {
        match trigger {
            CompletionTrigger::Dot => self.complete_dot(source, line, col),
            CompletionTrigger::DoubleColon => self.complete_double_colon(source, line),
            CompletionTrigger::Angle => self.complete_angle(),
            CompletionTrigger::Default => self.complete_default(source, line, col),
        }
    }

    /// Completes after `.` — fields and methods of the preceding expression.
    fn complete_dot(&self, source: &str, line: usize, col: usize) -> Vec<CompletionCandidate> {
        let prefix = self.extract_identifier_before(source, line, col);
        let structs = extract_struct_names(source);
        let mut results = Vec::new();

        // If the identifier matches a known struct variable, suggest fields
        let fields = extract_struct_fields(source, &prefix);
        for field in fields {
            results.push(CompletionCandidate {
                label: field.clone(),
                kind: CompletionKind::Field,
                detail: None,
                insert_text: field,
            });
        }

        // Always suggest common methods
        add_common_methods(&mut results, &structs);
        results
    }

    /// Completes after `::` — module items and associated functions.
    fn complete_double_colon(&self, source: &str, _line: usize) -> Vec<CompletionCandidate> {
        let mut results = Vec::new();
        // Suggest struct names as potential module/type prefixes
        for name in extract_struct_names(source) {
            results.push(CompletionCandidate {
                label: format!("{name}::new"),
                kind: CompletionKind::Function,
                detail: Some(format!("associated function of {name}")),
                insert_text: format!("{name}::new()"),
            });
        }
        results
    }

    /// Completes after `<` — generic type params.
    fn complete_angle(&self) -> Vec<CompletionCandidate> {
        // Suggest common type parameters
        vec![
            candidate("i32", CompletionKind::Keyword, "i32"),
            candidate("i64", CompletionKind::Keyword, "i64"),
            candidate("f64", CompletionKind::Keyword, "f64"),
            candidate("bool", CompletionKind::Keyword, "bool"),
            candidate("str", CompletionKind::Keyword, "str"),
            candidate("T", CompletionKind::Variable, "T"),
        ]
    }

    /// Default completion: locals, params, builtins, keywords.
    fn complete_default(
        &self,
        source: &str,
        _line: usize,
        _col: usize,
    ) -> Vec<CompletionCandidate> {
        let mut results = Vec::new();

        // Add local variable names
        for name in extract_local_names(source) {
            results.push(CompletionCandidate {
                label: name.clone(),
                kind: CompletionKind::Variable,
                detail: Some("local".to_string()),
                insert_text: name,
            });
        }

        // Add builtins
        results.extend(self.builtins.iter().cloned());
        // Add keywords
        results.extend(self.keywords.iter().cloned());
        results
    }

    /// Extracts the identifier just before the cursor position.
    fn extract_identifier_before(&self, source: &str, line: usize, col: usize) -> String {
        let target_line = match source.lines().nth(line) {
            Some(l) => l,
            None => return String::new(),
        };
        let before = if col <= target_line.len() {
            &target_line[..col]
        } else {
            target_line
        };
        extract_last_identifier(before)
    }
}

impl Default for CompletionProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a simple candidate.
fn candidate(label: &str, kind: CompletionKind, insert: &str) -> CompletionCandidate {
    CompletionCandidate {
        label: label.to_string(),
        kind,
        detail: None,
        insert_text: insert.to_string(),
    }
}

/// Returns default built-in function candidates.
fn default_builtins() -> Vec<CompletionCandidate> {
    let names = [
        ("print", "print(value)"),
        ("println", "println(value)"),
        ("len", "len(collection)"),
        ("type_of", "type_of(value)"),
        ("assert", "assert(condition)"),
        ("assert_eq", "assert_eq(a, b)"),
        ("panic", "panic(message)"),
        ("todo", "todo(message)"),
        ("dbg", "dbg(value)"),
    ];
    names
        .iter()
        .map(|(name, insert)| CompletionCandidate {
            label: name.to_string(),
            kind: CompletionKind::Builtin,
            detail: Some("builtin".to_string()),
            insert_text: insert.to_string(),
        })
        .collect()
}

/// Returns keyword candidates.
fn default_keywords() -> Vec<CompletionCandidate> {
    let kws = [
        "fn", "let", "mut", "if", "else", "while", "for", "in", "return", "struct", "enum", "impl",
        "trait", "match", "const", "use", "mod", "pub", "break", "continue", "loop",
        // V15 B3.6: Effect system keywords
        "effect", "handle", "with", "resume",
    ];
    kws.iter()
        .map(|kw| candidate(kw, CompletionKind::Keyword, kw))
        .collect()
}

/// Extracts struct names from source (simple regex-free scan).
fn extract_struct_names(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("struct ") {
            if let Some(name) = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
            {
                if !name.is_empty() {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

/// Extracts field names from a struct definition.
fn extract_struct_fields(source: &str, _var_name: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_struct = false;
    let mut brace_depth = 0;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("struct ") && trimmed.contains('{') {
            in_struct = true;
            brace_depth = 1;
            continue;
        }
        if in_struct {
            for ch in trimmed.chars() {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            in_struct = false;
                        }
                    }
                    _ => {}
                }
            }
            if in_struct {
                // Try to extract "field_name: type"
                if let Some(colon_pos) = trimmed.find(':') {
                    let field = trimmed[..colon_pos].trim();
                    if is_valid_identifier(field) {
                        fields.push(field.to_string());
                    }
                }
            }
        }
    }
    fields
}

/// Extracts local variable names from `let` bindings.
fn extract_local_names(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let rest = if let Some(r) = trimmed.strip_prefix("let mut ") {
            Some(r)
        } else {
            trimmed.strip_prefix("let ")
        };
        if let Some(rest) = rest {
            if let Some(name) = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
            {
                if !name.is_empty() && is_valid_identifier(name) {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

/// Extracts the last identifier before a position in a line.
fn extract_last_identifier(before: &str) -> String {
    let trimmed = before.trim_end_matches('.');
    let start = trimmed
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    trimmed[start..].to_string()
}

/// Checks if a string is a valid Fajar identifier.
fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let first = s.chars().next().unwrap_or('0');
    (first.is_alphabetic() || first == '_') && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Adds common methods to completion results.
fn add_common_methods(results: &mut Vec<CompletionCandidate>, _structs: &[String]) {
    let methods = [
        ("to_string", "fn to_string() -> str"),
        ("clone", "fn clone() -> Self"),
    ];
    for (name, sig) in &methods {
        results.push(CompletionCandidate {
            label: name.to_string(),
            kind: CompletionKind::Function,
            detail: Some(sig.to_string()),
            insert_text: format!("{name}()"),
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Rename
// ═══════════════════════════════════════════════════════════════════════

/// A source location (line, column, length).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Line number (0-based).
    pub line: usize,
    /// Column number (0-based).
    pub col: usize,
    /// Length of the symbol in characters.
    pub length: usize,
}

/// A text edit to apply to a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// Start line (0-based).
    pub start_line: usize,
    /// Start column (0-based).
    pub start_col: usize,
    /// End line (0-based).
    pub end_line: usize,
    /// End column (0-based).
    pub end_col: usize,
    /// Replacement text.
    pub new_text: String,
}

/// Provides rename-symbol functionality.
#[derive(Debug)]
pub struct RenameProvider;

impl RenameProvider {
    /// Creates a new rename provider.
    pub fn new() -> Self {
        Self
    }

    /// Finds all references to the symbol at the given position.
    pub fn find_all_references(
        &self,
        source: &str,
        line: usize,
        col: usize,
    ) -> Result<Vec<Location>, LspError> {
        let symbol = self.symbol_at(source, line, col)?;
        Ok(find_word_occurrences(source, &symbol))
    }

    /// Renames the symbol at the given position to `new_name`.
    pub fn rename_symbol(
        &self,
        source: &str,
        line: usize,
        col: usize,
        new_name: &str,
    ) -> Result<Vec<TextEdit>, LspError> {
        self.validate_new_name(new_name)?;
        let symbol = self.symbol_at(source, line, col)?;
        let locations = find_word_occurrences(source, &symbol);

        Ok(locations
            .into_iter()
            .map(|loc| TextEdit {
                start_line: loc.line,
                start_col: loc.col,
                end_line: loc.line,
                end_col: loc.col + loc.length,
                new_text: new_name.to_string(),
            })
            .collect())
    }

    /// Extracts the symbol name at the given position.
    fn symbol_at(&self, source: &str, line: usize, col: usize) -> Result<String, LspError> {
        let target_line = source
            .lines()
            .nth(line)
            .ok_or(LspError::PositionOutOfBounds { line, col })?;

        if col >= target_line.len() {
            return Err(LspError::PositionOutOfBounds { line, col });
        }

        extract_word_at(target_line, col).ok_or(LspError::NoSymbolAtPosition { line, col })
    }

    /// Validates that the new name is a legal identifier.
    fn validate_new_name(&self, name: &str) -> Result<(), LspError> {
        if !is_valid_identifier(name) {
            return Err(LspError::RenameInvalid {
                reason: format!("'{name}' is not a valid identifier"),
            });
        }
        Ok(())
    }
}

impl Default for RenameProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Extracts the word (identifier) surrounding the given column.
fn extract_word_at(line: &str, col: usize) -> Option<String> {
    let bytes = line.as_bytes();
    if col >= bytes.len() {
        return None;
    }

    let ch = bytes[col] as char;
    if !ch.is_alphanumeric() && ch != '_' {
        return None;
    }

    let start = (0..=col)
        .rev()
        .take_while(|&i| {
            let c = bytes[i] as char;
            c.is_alphanumeric() || c == '_'
        })
        .last()
        .unwrap_or(col);

    let end = (col..bytes.len())
        .take_while(|&i| {
            let c = bytes[i] as char;
            c.is_alphanumeric() || c == '_'
        })
        .last()
        .map(|i| i + 1)
        .unwrap_or(col + 1);

    Some(line[start..end].to_string())
}

/// Finds all whole-word occurrences of `word` in `source`.
fn find_word_occurrences(source: &str, word: &str) -> Vec<Location> {
    let mut results = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        let mut search_start = 0;
        while let Some(pos) = line[search_start..].find(word) {
            let col = search_start + pos;
            // Check whole-word boundary
            let before_ok = col == 0 || !is_ident_char(line.as_bytes()[col - 1] as char);
            let after_pos = col + word.len();
            let after_ok =
                after_pos >= line.len() || !is_ident_char(line.as_bytes()[after_pos] as char);

            if before_ok && after_ok {
                results.push(Location {
                    line: line_idx,
                    col,
                    length: word.len(),
                });
            }
            search_start = col + word.len();
        }
    }
    results
}

/// Checks if a character can be part of an identifier.
fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

// ═══════════════════════════════════════════════════════════════════════
// Inlay hints
// ═══════════════════════════════════════════════════════════════════════

/// The kind of inlay hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintKind {
    /// A type annotation hint (shown after variable name).
    TypeHint,
    /// A parameter name hint (shown before argument).
    ParameterHint,
}

/// An inlay hint to display in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHint {
    /// Line number (0-based).
    pub line: usize,
    /// Column number (0-based).
    pub col: usize,
    /// Display label.
    pub label: String,
    /// Kind of hint.
    pub kind: InlayHintKind,
}

/// Provides inlay type hints for `let` bindings without explicit types.
#[derive(Debug)]
pub struct InlayHintProvider;

impl InlayHintProvider {
    /// Creates a new inlay hint provider.
    pub fn new() -> Self {
        Self
    }

    /// Computes inlay hints for the given source.
    ///
    /// Generates type hints for `let` bindings that lack explicit
    /// type annotations, using simple heuristic inference.
    pub fn compute_inlay_hints(&self, source: &str) -> Vec<InlayHint> {
        let mut hints = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            if let Some(hint) = self.infer_let_binding(line, line_idx) {
                hints.push(hint);
            }
        }
        hints
    }

    /// Attempts to infer the type of a `let` binding on a single line.
    fn infer_let_binding(&self, line: &str, line_idx: usize) -> Option<InlayHint> {
        let trimmed = line.trim();
        let rest = trimmed
            .strip_prefix("let mut ")
            .or_else(|| trimmed.strip_prefix("let "))?;

        // Skip if there's already an explicit type annotation before `=`
        let eq_pos = rest.find('=')?;
        let before_eq = &rest[..eq_pos];
        if before_eq.contains(':') {
            return None; // explicit type, no hint needed
        }

        let name = before_eq.trim();
        if !is_valid_identifier(name) {
            return None;
        }

        let after_eq = rest[eq_pos + 1..].trim();
        let inferred = infer_type_from_expr(after_eq);

        // Position hint after the variable name
        let name_end = line.find(name).map(|i| i + name.len())?;

        Some(InlayHint {
            line: line_idx,
            col: name_end,
            label: format!(": {inferred}"),
            kind: InlayHintKind::TypeHint,
        })
    }
}

impl Default for InlayHintProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple heuristic type inference from an expression string.
fn infer_type_from_expr(expr: &str) -> String {
    let expr = expr.trim().trim_end_matches(';').trim();
    if expr == "true" || expr == "false" {
        return "bool".to_string();
    }
    if expr.starts_with('"') || expr.starts_with("f\"") {
        return "str".to_string();
    }
    if expr.contains('.') && expr.parse::<f64>().is_ok() {
        return "f64".to_string();
    }
    if expr.parse::<i64>().is_ok() {
        return "i64".to_string();
    }
    if expr.starts_with('[') {
        return "Array".to_string();
    }
    "unknown".to_string()
}

// ═══════════════════════════════════════════════════════════════════════
// Workspace symbols
// ═══════════════════════════════════════════════════════════════════════

/// The kind of workspace symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceSymbolKind {
    /// A function definition.
    Function,
    /// A struct definition.
    Struct,
    /// An enum definition.
    Enum,
    /// A trait definition.
    Trait,
    /// A constant.
    Constant,
    /// A module.
    Module,
}

/// A symbol found in the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbol {
    /// Symbol name.
    pub name: String,
    /// Symbol kind.
    pub kind: WorkspaceSymbolKind,
    /// File path (or "<source>" for inline).
    pub file: String,
    /// Line number (0-based).
    pub line: usize,
    /// Container name (e.g., struct for methods, module for items).
    pub container_name: Option<String>,
}

/// Provides workspace-wide symbol search.
#[derive(Debug)]
pub struct WorkspaceSymbolProvider;

impl WorkspaceSymbolProvider {
    /// Creates a new workspace symbol provider.
    pub fn new() -> Self {
        Self
    }

    /// Searches for symbols matching the query string (fuzzy match).
    pub fn search_symbols(&self, source: &str, query: &str) -> Vec<WorkspaceSymbol> {
        let all = self.extract_symbols(source);
        if query.is_empty() {
            return all;
        }
        let lower_query = query.to_lowercase();
        all.into_iter()
            .filter(|s| s.name.to_lowercase().contains(&lower_query))
            .collect()
    }

    /// Extracts all top-level symbols from source.
    fn extract_symbols(&self, source: &str) -> Vec<WorkspaceSymbol> {
        let mut symbols = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            self.try_extract_symbol(trimmed, line_idx, &mut symbols);
        }
        symbols
    }

    /// Tries to extract a symbol definition from a single line.
    fn try_extract_symbol(
        &self,
        trimmed: &str,
        line_idx: usize,
        symbols: &mut Vec<WorkspaceSymbol>,
    ) {
        // Remove leading `pub` if present
        let trimmed = trimmed.strip_prefix("pub ").unwrap_or(trimmed);

        if let Some(name) = extract_def_name(trimmed, "fn ") {
            symbols.push(make_symbol(name, WorkspaceSymbolKind::Function, line_idx));
        } else if let Some(name) = extract_def_name(trimmed, "struct ") {
            symbols.push(make_symbol(name, WorkspaceSymbolKind::Struct, line_idx));
        } else if let Some(name) = extract_def_name(trimmed, "enum ") {
            symbols.push(make_symbol(name, WorkspaceSymbolKind::Enum, line_idx));
        } else if let Some(name) = extract_def_name(trimmed, "trait ") {
            symbols.push(make_symbol(name, WorkspaceSymbolKind::Trait, line_idx));
        } else if let Some(name) = extract_def_name(trimmed, "const ") {
            symbols.push(make_symbol(name, WorkspaceSymbolKind::Constant, line_idx));
        } else if let Some(name) = extract_def_name(trimmed, "mod ") {
            symbols.push(make_symbol(name, WorkspaceSymbolKind::Module, line_idx));
        }
    }
}

impl Default for WorkspaceSymbolProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Extracts a definition name after a keyword prefix.
fn extract_def_name<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(prefix)?;
    rest.split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .filter(|s| !s.is_empty())
}

/// Creates a workspace symbol.
fn make_symbol(name: &str, kind: WorkspaceSymbolKind, line: usize) -> WorkspaceSymbol {
    WorkspaceSymbol {
        name: name.to_string(),
        kind,
        file: "<source>".to_string(),
        line,
        container_name: None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Top-level convenience function
// ═══════════════════════════════════════════════════════════════════════

/// Main entry point for completion at a source position.
///
/// Creates a temporary provider and returns candidates.
pub fn complete_at(
    source: &str,
    line: usize,
    col: usize,
    trigger: CompletionTrigger,
) -> Vec<CompletionCandidate> {
    let provider = CompletionProvider::new();
    provider.complete_at(source, line, col, trigger)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S27.1: CompletionProvider default completions
    #[test]
    fn completion_default_includes_builtins_and_keywords() {
        let source = "let x = 42\nlet y = x + 1\n";
        let results = complete_at(source, 1, 0, CompletionTrigger::Default);

        // Should include locals
        assert!(results.iter().any(|c| c.label == "x"));
        assert!(results.iter().any(|c| c.label == "y"));

        // Should include builtins
        assert!(results.iter().any(|c| c.label == "println"));
        assert!(results.iter().any(|c| c.label == "assert"));

        // Should include keywords
        assert!(results.iter().any(|c| c.label == "fn"));
        assert!(results.iter().any(|c| c.label == "let"));
    }

    // S27.2: Dot completion suggests fields
    #[test]
    fn completion_dot_shows_struct_fields() {
        let source = "struct Point {\n  x: f64,\n  y: f64\n}\nlet p = Point { x: 1.0, y: 2.0 }\np.";
        let results = complete_at(source, 5, 2, CompletionTrigger::Dot);

        // Should include fields from Point
        assert!(results.iter().any(|c| c.label == "x"));
        assert!(results.iter().any(|c| c.label == "y"));
        // Should include common methods
        assert!(results.iter().any(|c| c.label == "to_string"));
    }

    // S27.3: DoubleColon completion
    #[test]
    fn completion_double_colon_shows_associated() {
        let source = "struct Vec3 { x: f64, y: f64, z: f64 }\nVec3::";
        let results = complete_at(source, 1, 6, CompletionTrigger::DoubleColon);

        assert!(results.iter().any(|c| c.label.contains("Vec3::new")));
    }

    // S27.4: Angle completion
    #[test]
    fn completion_angle_shows_type_params() {
        let results = complete_at("fn foo<", 0, 7, CompletionTrigger::Angle);
        assert!(results.iter().any(|c| c.label == "i32"));
        assert!(results.iter().any(|c| c.label == "T"));
    }

    // S27.5: Rename finds all references
    #[test]
    fn rename_find_all_references() {
        let source = "let count = 0\ncount = count + 1\nprintln(count)\n";
        let rp = RenameProvider::new();
        let refs = rp.find_all_references(source, 0, 4).unwrap();

        // "count" appears 4 times across 3 lines
        assert_eq!(refs.len(), 4);
        assert_eq!(refs[0].line, 0);
        assert_eq!(refs[0].col, 4);
    }

    // S27.6: Rename produces correct edits
    #[test]
    fn rename_symbol_produces_edits() {
        let source = "let val = 10\nprintln(val)\n";
        let rp = RenameProvider::new();
        let edits = rp.rename_symbol(source, 0, 4, "value").unwrap();

        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].new_text, "value");
        assert_eq!(edits[1].new_text, "value");
    }

    // S27.7: Rename invalid name rejected
    #[test]
    fn rename_invalid_name_rejected() {
        let source = "let x = 1\n";
        let rp = RenameProvider::new();
        assert!(rp.rename_symbol(source, 0, 4, "123bad").is_err());
        assert!(rp.rename_symbol(source, 0, 4, "").is_err());
    }

    // S27.8: Inlay type hints
    #[test]
    fn inlay_hints_for_let_bindings() {
        let source = "let x = 42\nlet name = \"hello\"\nlet flag = true\nlet y: i32 = 10\n";
        let provider = InlayHintProvider::new();
        let hints = provider.compute_inlay_hints(source);

        // Lines 0, 1, 2 should get hints; line 3 has explicit type
        assert_eq!(hints.len(), 3);
        assert!(hints[0].label.contains("i64"));
        assert!(hints[1].label.contains("str"));
        assert!(hints[2].label.contains("bool"));
    }

    // S27.9: Workspace symbol search
    #[test]
    fn workspace_symbol_search() {
        let source = "fn add(a: i32, b: i32) -> i32 { a + b }\nstruct Point { x: f64 }\nenum Color { Red, Blue }\ntrait Display { fn show() }\nconst MAX: i32 = 100\n";
        let provider = WorkspaceSymbolProvider::new();

        let all = provider.search_symbols(source, "");
        assert_eq!(all.len(), 5);

        let fns = provider.search_symbols(source, "add");
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].kind, WorkspaceSymbolKind::Function);

        let structs = provider.search_symbols(source, "Point");
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].kind, WorkspaceSymbolKind::Struct);
    }

    // S27.10: No symbol at position
    #[test]
    fn rename_no_symbol_at_position() {
        let source = "let x = 42\n";
        let rp = RenameProvider::new();
        // Column 3 is a space
        assert!(rp.find_all_references(source, 0, 3).is_err());
    }
}
