//! Advanced LSP features for Fajar Lang.
//!
//! Provides comprehensive language intelligence beyond basic completion/rename:
//! - **SymbolIndex** -- Cross-file symbol resolution and lookup
//! - **ReferencesFinder** -- Find all references to a symbol (definition, read, write)
//! - **CodeActionProvider** -- Automated code transformations (quick fixes, refactors)
//! - **SemanticTokenizer** -- Full semantic syntax highlighting
//! - **SignatureHelper** -- Function signature help with active parameter tracking
//! - **CallHierarchyProvider** -- Incoming/outgoing call hierarchy navigation

use std::collections::HashMap;

use thiserror::Error;

// ============================================================================
// Error types
// ============================================================================

/// Errors from advanced LSP operations.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum AdvancedLspError {
    /// The requested symbol was not found.
    #[error("symbol not found: {name}")]
    SymbolNotFound {
        /// The symbol name.
        name: String,
    },

    /// The source position is out of bounds.
    #[error("position ({line}:{col}) is out of bounds")]
    PositionOutOfBounds {
        /// Line number (0-based).
        line: usize,
        /// Column number (0-based).
        col: usize,
    },

    /// The function was not found in the call graph.
    #[error("function not found in call graph: {name}")]
    FunctionNotFound {
        /// The function name.
        name: String,
    },

    /// The code action is not applicable.
    #[error("code action not applicable: {reason}")]
    ActionNotApplicable {
        /// Explanation.
        reason: String,
    },
}

// ============================================================================
// 1. SymbolIndex -- Cross-file symbol resolution
// ============================================================================

/// The kind of a symbol definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    /// A function definition.
    Function,
    /// A variable binding (let).
    Variable,
    /// A constant binding.
    Constant,
    /// A struct definition.
    Struct,
    /// An enum definition.
    Enum,
    /// A trait definition.
    Trait,
    /// An impl block.
    Impl,
    /// A type alias.
    TypeAlias,
    /// A module declaration.
    Module,
    /// A struct field.
    Field,
    /// A method (fn inside impl/trait).
    Method,
    /// A function parameter.
    Parameter,
}

/// A symbol definition extracted from source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDef {
    /// The symbol name.
    pub name: String,
    /// The kind of symbol.
    pub kind: SymbolKind,
    /// Start byte offset in the source.
    pub span_start: usize,
    /// End byte offset in the source.
    pub span_end: usize,
    /// Optional doc comment preceding the definition.
    pub doc_comment: Option<String>,
    /// Parent scope name (e.g., struct name for fields, impl name for methods).
    pub parent_scope: Option<String>,
}

/// Cross-file symbol index supporting lookup and iteration.
#[derive(Debug, Clone)]
pub struct SymbolIndex {
    /// All indexed symbols.
    symbols: Vec<SymbolDef>,
    /// Name-to-indices map for fast lookup.
    name_map: HashMap<String, Vec<usize>>,
}

impl SymbolIndex {
    /// Builds a symbol index by scanning source code for definitions.
    pub fn build_from_source(source: &str) -> Self {
        let mut symbols = Vec::new();
        let mut name_map: HashMap<String, Vec<usize>> = HashMap::new();
        let mut current_scope: Option<String> = None;
        let mut brace_depth: i32 = 0;
        let mut scope_start_depth: i32 = 0;
        let mut doc_comment: Option<String> = None;
        let mut byte_offset: usize = 0;

        for line in source.lines() {
            let trimmed = line.trim();
            Self::collect_doc_comment(trimmed, &mut doc_comment);
            Self::extract_line_symbols(
                trimmed,
                byte_offset,
                &mut symbols,
                &current_scope,
                &mut doc_comment,
            );
            Self::track_scope(
                trimmed,
                &mut brace_depth,
                &mut scope_start_depth,
                &mut current_scope,
                &symbols,
            );
            byte_offset += line.len() + 1; // +1 for newline
        }

        for (idx, sym) in symbols.iter().enumerate() {
            name_map.entry(sym.name.clone()).or_default().push(idx);
        }

        Self { symbols, name_map }
    }

    /// Looks up all symbols with the given name.
    pub fn lookup(&self, name: &str) -> Vec<&SymbolDef> {
        self.name_map
            .get(name)
            .map(|indices| indices.iter().map(|&i| &self.symbols[i]).collect())
            .unwrap_or_default()
    }

    /// Returns all indexed symbols.
    pub fn all_symbols(&self) -> Vec<&SymbolDef> {
        self.symbols.iter().collect()
    }

    /// Collects `///` doc comments, accumulating consecutive lines.
    fn collect_doc_comment(trimmed: &str, doc_comment: &mut Option<String>) {
        if let Some(rest) = trimmed.strip_prefix("///") {
            let text = rest.trim();
            match doc_comment {
                Some(existing) => {
                    existing.push('\n');
                    existing.push_str(text);
                }
                None => *doc_comment = Some(text.to_string()),
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            // Non-comment, non-empty line: doc comment only applies
            // if immediately followed by a definition (handled in extract)
        }
    }

    /// Extracts symbol definitions from a single line.
    fn extract_line_symbols(
        trimmed: &str,
        byte_offset: usize,
        symbols: &mut Vec<SymbolDef>,
        current_scope: &Option<String>,
        doc_comment: &mut Option<String>,
    ) {
        let clean = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
        let doc = doc_comment.take();

        if let Some(sym) = try_extract_fn(clean, byte_offset, &doc, current_scope) {
            symbols.push(sym);
        } else if let Some(sym) =
            try_extract_keyword(clean, "struct ", SymbolKind::Struct, byte_offset, &doc)
        {
            symbols.push(sym);
        } else if let Some(sym) =
            try_extract_keyword(clean, "enum ", SymbolKind::Enum, byte_offset, &doc)
        {
            symbols.push(sym);
        } else if let Some(sym) =
            try_extract_keyword(clean, "trait ", SymbolKind::Trait, byte_offset, &doc)
        {
            symbols.push(sym);
        } else if let Some(sym) =
            try_extract_keyword(clean, "impl ", SymbolKind::Impl, byte_offset, &doc)
        {
            symbols.push(sym);
        } else if let Some(sym) =
            try_extract_keyword(clean, "type ", SymbolKind::TypeAlias, byte_offset, &doc)
        {
            symbols.push(sym);
        } else if let Some(sym) =
            try_extract_keyword(clean, "mod ", SymbolKind::Module, byte_offset, &doc)
        {
            symbols.push(sym);
        } else if let Some(sym) = try_extract_const(clean, byte_offset, &doc) {
            symbols.push(sym);
        } else if let Some(sym) = try_extract_let(clean, byte_offset, &doc, current_scope) {
            symbols.push(sym);
        } else {
            // No definition found -- keep doc_comment for next line
            *doc_comment = doc;
        }
    }

    /// Tracks brace-delimited scope for parent_scope assignment.
    fn track_scope(
        trimmed: &str,
        brace_depth: &mut i32,
        scope_start_depth: &mut i32,
        current_scope: &mut Option<String>,
        symbols: &[SymbolDef],
    ) {
        for ch in trimmed.chars() {
            match ch {
                '{' => {
                    if *brace_depth == 0 {
                        *scope_start_depth = 0;
                        // Check if this opens a struct/impl/trait block
                        *current_scope = Self::detect_scope_name(trimmed, symbols);
                    }
                    *brace_depth += 1;
                }
                '}' => {
                    *brace_depth -= 1;
                    if *brace_depth <= *scope_start_depth && current_scope.is_some() {
                        *current_scope = None;
                    }
                }
                _ => {}
            }
        }
    }

    /// Detects if a line opens a named scope (struct, impl, trait).
    fn detect_scope_name(trimmed: &str, symbols: &[SymbolDef]) -> Option<String> {
        let clean = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
        for prefix in &["struct ", "impl ", "trait ", "enum "] {
            if let Some(rest) = clean.strip_prefix(prefix) {
                let name = rest
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
        // Also check if any of the last symbols was a struct/impl/trait
        if let Some(last) = symbols.last() {
            if matches!(
                last.kind,
                SymbolKind::Struct | SymbolKind::Impl | SymbolKind::Trait | SymbolKind::Enum
            ) {
                return Some(last.name.clone());
            }
        }
        None
    }
}

/// Tries to extract a function or method definition.
fn try_extract_fn(
    clean: &str,
    byte_offset: usize,
    doc: &Option<String>,
    current_scope: &Option<String>,
) -> Option<SymbolDef> {
    let rest = clean.strip_prefix("fn ")?;
    let name = extract_ident(rest)?;
    let kind = if current_scope.is_some() {
        SymbolKind::Method
    } else {
        SymbolKind::Function
    };
    Some(SymbolDef {
        name: name.to_string(),
        kind,
        span_start: byte_offset,
        span_end: byte_offset + clean.len(),
        doc_comment: doc.clone(),
        parent_scope: current_scope.clone(),
    })
}

/// Tries to extract a keyword-based definition (struct, enum, trait, impl, type, mod).
fn try_extract_keyword(
    clean: &str,
    prefix: &str,
    kind: SymbolKind,
    byte_offset: usize,
    doc: &Option<String>,
) -> Option<SymbolDef> {
    let rest = clean.strip_prefix(prefix)?;
    let name = extract_ident(rest)?;
    Some(SymbolDef {
        name: name.to_string(),
        kind,
        span_start: byte_offset,
        span_end: byte_offset + clean.len(),
        doc_comment: doc.clone(),
        parent_scope: None,
    })
}

/// Tries to extract a `const` definition.
fn try_extract_const(clean: &str, byte_offset: usize, doc: &Option<String>) -> Option<SymbolDef> {
    let rest = clean.strip_prefix("const ")?;
    let name = extract_ident(rest)?;
    Some(SymbolDef {
        name: name.to_string(),
        kind: SymbolKind::Constant,
        span_start: byte_offset,
        span_end: byte_offset + clean.len(),
        doc_comment: doc.clone(),
        parent_scope: None,
    })
}

/// Tries to extract a `let` / `let mut` variable definition.
fn try_extract_let(
    clean: &str,
    byte_offset: usize,
    doc: &Option<String>,
    current_scope: &Option<String>,
) -> Option<SymbolDef> {
    let rest = clean
        .strip_prefix("let mut ")
        .or_else(|| clean.strip_prefix("let "))?;
    let name = extract_ident(rest)?;
    Some(SymbolDef {
        name: name.to_string(),
        kind: SymbolKind::Variable,
        span_start: byte_offset,
        span_end: byte_offset + clean.len(),
        doc_comment: doc.clone(),
        parent_scope: current_scope.clone(),
    })
}

/// Extracts the first valid identifier from text.
fn extract_ident(text: &str) -> Option<&str> {
    let name = text
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .filter(|s| !s.is_empty())?;
    let first = name.chars().next()?;
    if first.is_alphabetic() || first == '_' {
        Some(name)
    } else {
        None
    }
}

// ============================================================================
// 2. ReferencesFinder -- Find all references to a symbol
// ============================================================================

/// The kind of reference to a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    /// The symbol's definition site.
    Definition,
    /// A read access (usage in expression context).
    Read,
    /// A write access (assignment target).
    Write,
    /// An import reference (use statement).
    Import,
}

/// A single reference to a symbol in source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// Line number (0-based).
    pub line: usize,
    /// Start column (0-based).
    pub col_start: usize,
    /// End column (exclusive, 0-based).
    pub col_end: usize,
    /// The kind of reference.
    pub kind: ReferenceKind,
}

/// Finds all references to a given symbol in source code.
pub struct ReferencesFinder;

impl ReferencesFinder {
    /// Creates a new references finder.
    pub fn new() -> Self {
        Self
    }

    /// Finds all references to `symbol_name` in `source`.
    ///
    /// `definition_line` is the line where the symbol is defined (0-based),
    /// used to distinguish the definition from other occurrences.
    pub fn find_references(
        &self,
        source: &str,
        symbol_name: &str,
        definition_line: usize,
    ) -> Vec<Reference> {
        let mut refs = Vec::new();
        for (line_idx, line_text) in source.lines().enumerate() {
            self.find_in_line(line_text, line_idx, symbol_name, definition_line, &mut refs);
        }
        refs
    }

    /// Finds whole-word occurrences in a single line, classifying each.
    fn find_in_line(
        &self,
        line_text: &str,
        line_idx: usize,
        symbol_name: &str,
        definition_line: usize,
        refs: &mut Vec<Reference>,
    ) {
        let mut search_from = 0;
        while let Some(pos) = line_text[search_from..].find(symbol_name) {
            let col = search_from + pos;
            if is_whole_word(line_text, col, symbol_name.len()) {
                let kind =
                    classify_reference(line_text, col, symbol_name, line_idx, definition_line);
                refs.push(Reference {
                    line: line_idx,
                    col_start: col,
                    col_end: col + symbol_name.len(),
                    kind,
                });
            }
            search_from = col + symbol_name.len();
        }
    }
}

impl Default for ReferencesFinder {
    fn default() -> Self {
        Self::new()
    }
}

/// Checks if the match at `col` with `len` is a whole-word match.
fn is_whole_word(line: &str, col: usize, len: usize) -> bool {
    let bytes = line.as_bytes();
    let before_ok = col == 0 || !is_ident_byte(bytes[col - 1]);
    let after_pos = col + len;
    let after_ok = after_pos >= bytes.len() || !is_ident_byte(bytes[after_pos]);
    before_ok && after_ok
}

/// Checks if a byte is part of an identifier.
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Classifies a reference as Definition, Write, Import, or Read.
fn classify_reference(
    line_text: &str,
    col: usize,
    symbol_name: &str,
    line_idx: usize,
    definition_line: usize,
) -> ReferenceKind {
    let trimmed = line_text.trim();
    // Check import
    if trimmed.starts_with("use ") {
        return ReferenceKind::Import;
    }
    // Check definition
    if line_idx == definition_line && is_definition_site(trimmed, symbol_name) {
        return ReferenceKind::Definition;
    }
    // Check write (assignment target)
    if is_write_site(line_text, col, symbol_name) {
        return ReferenceKind::Write;
    }
    ReferenceKind::Read
}

/// Checks if the line defines the symbol (let/const/fn/struct/enum/trait).
fn is_definition_site(trimmed: &str, name: &str) -> bool {
    let clean = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
    let prefixes = [
        "let mut ", "let ", "const ", "fn ", "struct ", "enum ", "trait ", "impl ", "type ", "mod ",
    ];
    for prefix in &prefixes {
        if let Some(rest) = clean.strip_prefix(prefix) {
            if let Some(ident) = extract_ident(rest) {
                if ident == name {
                    return true;
                }
            }
        }
    }
    false
}

/// Checks if symbol at `col` is on the left-hand side of an assignment.
fn is_write_site(line_text: &str, col: usize, symbol_name: &str) -> bool {
    let after = &line_text[col + symbol_name.len()..];
    let after_trimmed = after.trim_start();
    // Direct assignment: `x = ...` (but not `==`)
    if let Some(rest) = after_trimmed.strip_prefix('=') {
        if !rest.starts_with('=') {
            return true;
        }
    }
    // Compound assignment: `x += ...`, `x -= ...`, etc.
    for op in &["+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>="] {
        if after_trimmed.starts_with(op) {
            return true;
        }
    }
    false
}

// ============================================================================
// 3. CodeActionProvider -- Automated code transformations
// ============================================================================

/// The kind of code action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeActionKind {
    /// A quick fix for a diagnostic.
    QuickFix,
    /// A general refactoring.
    Refactor,
    /// Extract code into a new scope/function.
    RefactorExtract,
    /// Inline a definition.
    RefactorInline,
}

/// A text edit to apply to a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionTextEdit {
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

/// A code action that transforms source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeAction {
    /// Human-readable title.
    pub title: String,
    /// The kind of action.
    pub kind: CodeActionKind,
    /// Edits to apply.
    pub edits: Vec<CodeActionTextEdit>,
}

/// Provides code actions (quick fixes and refactorings).
pub struct CodeActionProvider;

impl CodeActionProvider {
    /// Creates a new code action provider.
    pub fn new() -> Self {
        Self
    }

    /// Returns applicable code actions for the given line.
    pub fn actions_for_line(&self, source: &str, line: usize) -> Vec<CodeAction> {
        let lines: Vec<&str> = source.lines().collect();
        if line >= lines.len() {
            return Vec::new();
        }
        let line_text = lines[line];
        let mut actions = Vec::new();

        self.try_make_mutable(&mut actions, line_text, line);
        self.try_add_type_annotation(&mut actions, line_text, line);
        self.try_inline_variable(&mut actions, source, line_text, line);
        self.try_convert_if_to_match(&mut actions, lines.as_slice(), line);
        self.try_add_missing_import(&mut actions, source, line_text, line);
        self.try_add_missing_fields(&mut actions, source, line_text, line);

        actions
    }

    /// Returns an "extract function" action for the given line range.
    pub fn extract_function(
        &self,
        source: &str,
        start_line: usize,
        end_line: usize,
        fn_name: &str,
    ) -> Result<CodeAction, AdvancedLspError> {
        let lines: Vec<&str> = source.lines().collect();
        if start_line >= lines.len() || end_line >= lines.len() || start_line > end_line {
            return Err(AdvancedLspError::ActionNotApplicable {
                reason: "invalid line range".to_string(),
            });
        }
        let body: Vec<&str> = lines[start_line..=end_line].to_vec();
        let indented_body = body
            .iter()
            .map(|l| format!("    {}", l.trim()))
            .collect::<Vec<_>>()
            .join("\n");
        let new_fn = format!("fn {fn_name}() {{\n{indented_body}\n}}");
        let call_text = format!("{fn_name}()");

        Ok(CodeAction {
            title: format!("Extract function '{fn_name}'"),
            kind: CodeActionKind::RefactorExtract,
            edits: vec![
                CodeActionTextEdit {
                    start_line,
                    start_col: 0,
                    end_line,
                    end_col: lines[end_line].len(),
                    new_text: call_text,
                },
                CodeActionTextEdit {
                    start_line: lines.len(),
                    start_col: 0,
                    end_line: lines.len(),
                    end_col: 0,
                    new_text: format!("\n{new_fn}\n"),
                },
            ],
        })
    }

    /// Suggests adding `mut` to an immutable let binding.
    fn try_make_mutable(&self, actions: &mut Vec<CodeAction>, line_text: &str, line: usize) {
        let trimmed = line_text.trim();
        if trimmed.starts_with("let ") && !trimmed.starts_with("let mut ") {
            if let Some(col) = line_text.find("let ") {
                actions.push(CodeAction {
                    title: "Make variable mutable".to_string(),
                    kind: CodeActionKind::QuickFix,
                    edits: vec![CodeActionTextEdit {
                        start_line: line,
                        start_col: col,
                        end_line: line,
                        end_col: col + 4, // "let "
                        new_text: "let mut ".to_string(),
                    }],
                });
            }
        }
    }

    /// Suggests adding a type annotation to a let binding without one.
    fn try_add_type_annotation(&self, actions: &mut Vec<CodeAction>, line_text: &str, line: usize) {
        let trimmed = line_text.trim();
        let rest = trimmed
            .strip_prefix("let mut ")
            .or_else(|| trimmed.strip_prefix("let "));
        let rest = match rest {
            Some(r) => r,
            None => return,
        };
        // Already has type annotation?
        let eq_pos = match rest.find('=') {
            Some(p) => p,
            None => return,
        };
        let before_eq = &rest[..eq_pos];
        if before_eq.contains(':') {
            return;
        }
        let name = before_eq.trim();
        if name.is_empty() || name.contains(' ') {
            return;
        }
        let after_eq = rest[eq_pos + 1..].trim().trim_end_matches(';').trim();
        let inferred = infer_simple_type(after_eq);

        // Find position of name end in original line
        if let Some(name_pos) = line_text.find(name) {
            let insert_col = name_pos + name.len();
            actions.push(CodeAction {
                title: format!("Add type annotation: {inferred}"),
                kind: CodeActionKind::QuickFix,
                edits: vec![CodeActionTextEdit {
                    start_line: line,
                    start_col: insert_col,
                    end_line: line,
                    end_col: insert_col,
                    new_text: format!(": {inferred}"),
                }],
            });
        }
    }

    /// Suggests inlining a variable with a simple value.
    fn try_inline_variable(
        &self,
        actions: &mut Vec<CodeAction>,
        source: &str,
        line_text: &str,
        line: usize,
    ) {
        let trimmed = line_text.trim();
        let rest = trimmed
            .strip_prefix("let mut ")
            .or_else(|| trimmed.strip_prefix("let "));
        let rest = match rest {
            Some(r) => r,
            None => return,
        };
        let eq_pos = match rest.find('=') {
            Some(p) => p,
            None => return,
        };
        let before_eq = rest[..eq_pos].trim();
        // Strip type annotation for the name
        let name = before_eq.split(':').next().unwrap_or("").trim();
        if name.is_empty() || name.contains(' ') {
            return;
        }
        let value = rest[eq_pos + 1..].trim().trim_end_matches(';').trim();
        if value.is_empty() {
            return;
        }

        // Count usages -- only suggest if used at least once elsewhere
        let usage_count = count_whole_word_occurrences(source, name);
        if usage_count > 1 {
            actions.push(CodeAction {
                title: format!("Inline variable '{name}'"),
                kind: CodeActionKind::RefactorInline,
                edits: build_inline_edits(source, name, value, line),
            });
        }
    }

    /// Suggests converting an if/else chain to a match expression.
    fn try_convert_if_to_match(&self, actions: &mut Vec<CodeAction>, lines: &[&str], line: usize) {
        let trimmed = lines[line].trim();
        // Simple pattern: `if x == val { ... } else if x == val2 { ... }`
        if !trimmed.starts_with("if ") || !trimmed.contains("==") {
            return;
        }
        // Extract the compared variable
        let after_if = trimmed.strip_prefix("if ").unwrap_or("");
        let eq_pos = match after_if.find("==") {
            Some(p) => p,
            None => return,
        };
        let var_name = after_if[..eq_pos].trim();
        if var_name.is_empty() {
            return;
        }
        actions.push(CodeAction {
            title: format!("Convert if/else to match on '{var_name}'"),
            kind: CodeActionKind::Refactor,
            edits: vec![], // Placeholder: full conversion needs multi-line analysis
        });
    }

    /// Suggests adding a `use` import for an identifier that looks unresolved.
    fn try_add_missing_import(
        &self,
        actions: &mut Vec<CodeAction>,
        source: &str,
        line_text: &str,
        line: usize,
    ) {
        let trimmed = line_text.trim();
        // Detect `ModName::func()` pattern where no `use ModName` exists
        if let Some(colon_pos) = trimmed.find("::") {
            let before = trimmed[..colon_pos].trim();
            let mod_name = before
                .rsplit(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if mod_name.is_empty() {
                return;
            }
            let has_import = source
                .lines()
                .any(|l| l.trim().starts_with("use ") && l.contains(mod_name));
            if !has_import {
                actions.push(CodeAction {
                    title: format!("Add missing import for '{mod_name}'"),
                    kind: CodeActionKind::QuickFix,
                    edits: vec![CodeActionTextEdit {
                        start_line: 0,
                        start_col: 0,
                        end_line: 0,
                        end_col: 0,
                        new_text: format!("use {mod_name}\n"),
                    }],
                });
            }
        }
        // Suppress unused variable warning for line parameter
        let _ = line;
    }

    /// Suggests adding missing fields to a struct literal.
    fn try_add_missing_fields(
        &self,
        actions: &mut Vec<CodeAction>,
        source: &str,
        line_text: &str,
        line: usize,
    ) {
        let trimmed = line_text.trim();
        // Pattern: `StructName { field1: val, ... }`
        if !trimmed.contains('{') || trimmed.starts_with("struct ") {
            return;
        }
        let brace_pos = match trimmed.find('{') {
            Some(p) => p,
            None => return,
        };
        let struct_name = trimmed[..brace_pos].trim();
        let struct_name = struct_name
            .rsplit(|c: char| !c.is_alphanumeric() && c != '_')
            .next()
            .unwrap_or("");
        if struct_name.is_empty() || !struct_name.chars().next().unwrap_or('_').is_uppercase() {
            return;
        }
        let all_fields = find_struct_fields(source, struct_name);
        let present_fields = extract_present_fields(trimmed);
        let missing: Vec<&String> = all_fields
            .iter()
            .filter(|f| !present_fields.contains(&f.as_str()))
            .collect();
        if missing.is_empty() {
            return;
        }
        let fill_text = missing
            .iter()
            .map(|f| format!("{f}: todo!()"))
            .collect::<Vec<_>>()
            .join(", ");
        actions.push(CodeAction {
            title: format!(
                "Add missing fields: {}",
                missing
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            kind: CodeActionKind::QuickFix,
            edits: vec![CodeActionTextEdit {
                start_line: line,
                start_col: trimmed.len().saturating_sub(1), // before closing }
                end_line: line,
                end_col: trimmed.len().saturating_sub(1),
                new_text: format!(", {fill_text}"),
            }],
        });
    }
}

impl Default for CodeActionProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Infers a simple type from an expression string.
fn infer_simple_type(expr: &str) -> String {
    if expr == "true" || expr == "false" {
        return "bool".to_string();
    }
    if expr.starts_with('"') {
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

/// Counts whole-word occurrences of a name in source.
fn count_whole_word_occurrences(source: &str, name: &str) -> usize {
    let mut count = 0;
    for line in source.lines() {
        let mut from = 0;
        while let Some(pos) = line[from..].find(name) {
            let col = from + pos;
            if is_whole_word(line, col, name.len()) {
                count += 1;
            }
            from = col + name.len();
        }
    }
    count
}

/// Builds inline edits: remove definition line, replace usages with value.
fn build_inline_edits(
    source: &str,
    name: &str,
    value: &str,
    def_line: usize,
) -> Vec<CodeActionTextEdit> {
    let mut edits = Vec::new();
    // Remove the definition line
    let lines: Vec<&str> = source.lines().collect();
    if def_line < lines.len() {
        edits.push(CodeActionTextEdit {
            start_line: def_line,
            start_col: 0,
            end_line: def_line,
            end_col: lines[def_line].len(),
            new_text: String::new(),
        });
    }
    // Replace each usage with the value
    for (line_idx, line_text) in source.lines().enumerate() {
        if line_idx == def_line {
            continue;
        }
        let mut from = 0;
        while let Some(pos) = line_text[from..].find(name) {
            let col = from + pos;
            if is_whole_word(line_text, col, name.len()) {
                edits.push(CodeActionTextEdit {
                    start_line: line_idx,
                    start_col: col,
                    end_line: line_idx,
                    end_col: col + name.len(),
                    new_text: value.to_string(),
                });
            }
            from = col + name.len();
        }
    }
    edits
}

/// Finds struct field names from a struct definition in source.
fn find_struct_fields(source: &str, struct_name: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_target = false;
    let mut brace_depth = 0;

    for line in source.lines() {
        let trimmed = line.trim();
        let clean = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
        if let Some(rest) = clean.strip_prefix("struct ") {
            let name = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if name == struct_name {
                in_target = true;
                brace_depth = 0;
                // Count braces on this line
                for ch in trimmed.chars() {
                    match ch {
                        '{' => brace_depth += 1,
                        '}' => brace_depth -= 1,
                        _ => {}
                    }
                }
                continue;
            }
        }
        if in_target {
            for ch in trimmed.chars() {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            in_target = false;
                        }
                    }
                    _ => {}
                }
            }
            if in_target {
                if let Some(colon_pos) = trimmed.find(':') {
                    let field = trimmed[..colon_pos].trim();
                    let field = field.strip_prefix("pub ").unwrap_or(field);
                    if !field.is_empty()
                        && field.chars().next().unwrap_or('0').is_alphabetic()
                        && field.chars().all(|c| c.is_alphanumeric() || c == '_')
                    {
                        fields.push(field.to_string());
                    }
                }
            }
        }
    }
    fields
}

/// Extracts field names present in a struct literal line.
fn extract_present_fields(line: &str) -> Vec<&str> {
    let brace_start = match line.find('{') {
        Some(p) => p + 1,
        None => return Vec::new(),
    };
    let brace_end = line.rfind('}').unwrap_or(line.len());
    let inner = &line[brace_start..brace_end];
    inner
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            if let Some(colon_pos) = part.find(':') {
                let field = part[..colon_pos].trim();
                if !field.is_empty() {
                    Some(field)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

// ============================================================================
// 4. SemanticTokenizer -- Full semantic highlighting
// ============================================================================

/// Semantic token types for syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticTokenType {
    /// A function name.
    Function,
    /// A variable name.
    Variable,
    /// A function parameter.
    Parameter,
    /// A type name (primitive or alias).
    Type,
    /// A struct name.
    Struct,
    /// An enum name.
    Enum,
    /// An enum variant.
    EnumMember,
    /// A struct field or property.
    Property,
    /// A language keyword.
    Keyword,
    /// A numeric literal.
    Number,
    /// A string literal.
    String,
    /// A comment.
    Comment,
    /// An operator.
    Operator,
    /// A macro invocation.
    Macro,
    /// A namespace/module name.
    Namespace,
    /// A decorator/annotation (@kernel, @device, etc.).
    Decorator,
}

/// Modifier flags for semantic tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemanticTokenModifier {
    /// Raw bitfield value.
    bits: u16,
}

impl SemanticTokenModifier {
    /// No modifiers.
    pub const NONE: Self = Self { bits: 0 };
    /// Declaration site.
    pub const DECLARATION: Self = Self { bits: 1 };
    /// Definition site.
    pub const DEFINITION: Self = Self { bits: 2 };
    /// Readonly/immutable.
    pub const READONLY: Self = Self { bits: 4 };
    /// Static item.
    pub const STATIC: Self = Self { bits: 8 };
    /// Deprecated item.
    pub const DEPRECATED: Self = Self { bits: 16 };
    /// Async function.
    pub const ASYNC: Self = Self { bits: 32 };
    /// Mutable variable.
    pub const MUTABLE: Self = Self { bits: 64 };
    /// Default library item.
    pub const DEFAULT_LIBRARY: Self = Self { bits: 128 };

    /// Combines two modifier sets.
    pub fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    /// Tests if a modifier flag is set.
    pub fn contains(self, flag: Self) -> bool {
        (self.bits & flag.bits) == flag.bits
    }

    /// Returns the raw bits.
    pub fn raw(self) -> u16 {
        self.bits
    }
}

/// A single semantic token in source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticToken {
    /// Line number (0-based).
    pub line: usize,
    /// Start character (0-based column).
    pub start_char: usize,
    /// Length in characters.
    pub length: usize,
    /// The semantic token type.
    pub token_type: SemanticTokenType,
    /// Modifier flags.
    pub modifiers: SemanticTokenModifier,
}

/// Produces semantic tokens for full syntax highlighting.
pub struct SemanticTokenizer;

impl SemanticTokenizer {
    /// Creates a new semantic tokenizer.
    pub fn new() -> Self {
        Self
    }

    /// Tokenizes source code into semantic tokens.
    pub fn tokenize(&self, source: &str) -> Vec<SemanticToken> {
        let mut tokens = Vec::new();
        for (line_idx, line) in source.lines().enumerate() {
            self.tokenize_line(line, line_idx, &mut tokens);
        }
        tokens
    }

    /// Tokenizes a single line into semantic tokens.
    fn tokenize_line(&self, line: &str, line_idx: usize, tokens: &mut Vec<SemanticToken>) {
        let trimmed = line.trim();
        // Comments
        if trimmed.starts_with("//") {
            if let Some(pos) = line.find("//") {
                tokens.push(SemanticToken {
                    line: line_idx,
                    start_char: pos,
                    length: line.len() - pos,
                    token_type: SemanticTokenType::Comment,
                    modifiers: SemanticTokenModifier::NONE,
                });
            }
            return;
        }
        self.tokenize_annotations(line, line_idx, tokens);
        self.tokenize_strings(line, line_idx, tokens);
        self.tokenize_keywords_and_idents(line, line_idx, tokens);
        self.tokenize_numbers(line, line_idx, tokens);
    }

    /// Tokenizes `@annotation` decorators.
    fn tokenize_annotations(&self, line: &str, line_idx: usize, tokens: &mut Vec<SemanticToken>) {
        let annotations = ["@kernel", "@device", "@safe", "@unsafe", "@ffi"];
        for ann in &annotations {
            let mut from = 0;
            while let Some(pos) = line[from..].find(ann) {
                let col = from + pos;
                tokens.push(SemanticToken {
                    line: line_idx,
                    start_char: col,
                    length: ann.len(),
                    token_type: SemanticTokenType::Decorator,
                    modifiers: SemanticTokenModifier::NONE,
                });
                from = col + ann.len();
            }
        }
    }

    /// Tokenizes string literals.
    fn tokenize_strings(&self, line: &str, line_idx: usize, tokens: &mut Vec<SemanticToken>) {
        let mut in_string = false;
        let mut start = 0;
        for (i, ch) in line.char_indices() {
            if ch == '"' {
                if in_string {
                    tokens.push(SemanticToken {
                        line: line_idx,
                        start_char: start,
                        length: i - start + 1,
                        token_type: SemanticTokenType::String,
                        modifiers: SemanticTokenModifier::NONE,
                    });
                    in_string = false;
                } else {
                    in_string = true;
                    start = i;
                }
            }
        }
    }

    /// Tokenizes keywords, function names, and identifiers.
    fn tokenize_keywords_and_idents(
        &self,
        line: &str,
        line_idx: usize,
        tokens: &mut Vec<SemanticToken>,
    ) {
        let keywords = [
            "fn", "let", "mut", "if", "else", "match", "while", "for", "in", "return", "struct",
            "enum", "impl", "trait", "const", "use", "mod", "pub", "break", "continue", "loop",
            "type", "true", "false", "null", "as", "extern",
        ];

        let mut i = 0;
        let bytes = line.as_bytes();
        while i < bytes.len() {
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &line[start..i];
                let token = self.classify_word(word, line, start, line_idx, &keywords);
                tokens.push(token);
            } else {
                i += 1;
            }
        }
    }

    /// Classifies a word as a keyword, function, type, or variable.
    fn classify_word(
        &self,
        word: &str,
        line: &str,
        start: usize,
        line_idx: usize,
        keywords: &[&str],
    ) -> SemanticToken {
        let mut modifiers = SemanticTokenModifier::NONE;
        let token_type = if keywords.contains(&word) {
            SemanticTokenType::Keyword
        } else if is_followed_by(line, start + word.len(), '(') {
            // Function call or definition
            let trimmed = line.trim();
            if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                modifiers =
                    SemanticTokenModifier::DECLARATION.union(SemanticTokenModifier::DEFINITION);
            }
            SemanticTokenType::Function
        } else if word.chars().next().unwrap_or('_').is_uppercase() {
            // Capitalized: struct, enum, or type
            SemanticTokenType::Type
        } else {
            // Check if this is a mutable variable
            if is_preceded_by_mut(line, start) {
                modifiers = SemanticTokenModifier::MUTABLE;
            }
            SemanticTokenType::Variable
        };

        SemanticToken {
            line: line_idx,
            start_char: start,
            length: word.len(),
            token_type,
            modifiers,
        }
    }

    /// Tokenizes numeric literals.
    fn tokenize_numbers(&self, line: &str, line_idx: usize, tokens: &mut Vec<SemanticToken>) {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i].is_ascii_digit() {
                // Don't tokenize digits inside identifiers
                if i > 0 && (bytes[i - 1].is_ascii_alphabetic() || bytes[i - 1] == b'_') {
                    i += 1;
                    continue;
                }
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'_')
                {
                    i += 1;
                }
                tokens.push(SemanticToken {
                    line: line_idx,
                    start_char: start,
                    length: i - start,
                    token_type: SemanticTokenType::Number,
                    modifiers: SemanticTokenModifier::NONE,
                });
            } else {
                i += 1;
            }
        }
    }
}

impl Default for SemanticTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Checks if position `pos` in `line` is followed by character `ch`.
fn is_followed_by(line: &str, pos: usize, ch: char) -> bool {
    if pos >= line.len() {
        return false;
    }
    line[pos..].trim_start().starts_with(ch)
}

/// Checks if the word at `start` is preceded by `mut ` keyword.
fn is_preceded_by_mut(line: &str, start: usize) -> bool {
    if start >= 4 {
        line[..start].ends_with("mut ")
    } else {
        false
    }
}

// ============================================================================
// 5. SignatureHelper -- Function signature help
// ============================================================================

/// Information about a single function parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterInfo {
    /// The parameter label (e.g., "a: i32").
    pub label: String,
    /// Optional documentation for this parameter.
    pub doc: Option<String>,
}

/// Complete signature information for a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureInfo {
    /// The full function signature label.
    pub label: String,
    /// Optional documentation.
    pub doc: Option<String>,
    /// Individual parameter info.
    pub parameters: Vec<ParameterInfo>,
}

/// The active signature and parameter at the cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSignature {
    /// The function signature.
    pub signature: SignatureInfo,
    /// Index of the active parameter (0-based).
    pub active_parameter: usize,
}

/// Provides function signature help at cursor position.
pub struct SignatureHelper {
    /// Known function signatures.
    signatures: HashMap<String, SignatureInfo>,
}

impl SignatureHelper {
    /// Creates a new signature helper with built-in signatures.
    pub fn new() -> Self {
        let mut signatures = HashMap::new();
        Self::register_builtins(&mut signatures);
        Self { signatures }
    }

    /// Returns signature help at the given cursor position.
    pub fn get_signature(&self, source: &str, line: usize, col: usize) -> Option<ActiveSignature> {
        let target_line = source.lines().nth(line)?;
        let before_cursor = if col <= target_line.len() {
            &target_line[..col]
        } else {
            target_line
        };
        let (fn_name, active_param) = parse_call_context(before_cursor)?;

        // Look up in known signatures, then in source-defined functions
        let sig = self
            .signatures
            .get(&fn_name)
            .cloned()
            .or_else(|| extract_fn_signature(source, &fn_name))?;

        Some(ActiveSignature {
            signature: sig,
            active_parameter: active_param,
        })
    }

    /// Registers a custom function signature.
    pub fn register_signature(&mut self, name: &str, sig: SignatureInfo) {
        self.signatures.insert(name.to_string(), sig);
    }

    /// Registers all built-in function signatures.
    fn register_builtins(sigs: &mut HashMap<String, SignatureInfo>) {
        Self::register_io_builtins(sigs);
        Self::register_assert_builtins(sigs);
        Self::register_collection_builtins(sigs);
        Self::register_tensor_builtins(sigs);
        Self::register_math_builtins(sigs);
    }

    /// Registers I/O built-in signatures.
    fn register_io_builtins(sigs: &mut HashMap<String, SignatureInfo>) {
        sigs.insert(
            "print".to_string(),
            make_sig(
                "print(value: any)",
                "Prints a value without newline.",
                &["value: any"],
            ),
        );
        sigs.insert(
            "println".to_string(),
            make_sig(
                "println(value: any)",
                "Prints a value with newline.",
                &["value: any"],
            ),
        );
        sigs.insert(
            "eprintln".to_string(),
            make_sig(
                "eprintln(value: any)",
                "Prints to stderr with newline.",
                &["value: any"],
            ),
        );
        sigs.insert(
            "dbg".to_string(),
            make_sig("dbg(value: any)", "Debug-prints a value.", &["value: any"]),
        );
    }

    /// Registers assertion built-in signatures.
    fn register_assert_builtins(sigs: &mut HashMap<String, SignatureInfo>) {
        sigs.insert(
            "assert".to_string(),
            make_sig(
                "assert(condition: bool)",
                "Asserts a condition is true.",
                &["condition: bool"],
            ),
        );
        sigs.insert(
            "assert_eq".to_string(),
            make_sig(
                "assert_eq(left: any, right: any)",
                "Asserts two values are equal.",
                &["left: any", "right: any"],
            ),
        );
        sigs.insert(
            "panic".to_string(),
            make_sig(
                "panic(message: str)",
                "Aborts with a message.",
                &["message: str"],
            ),
        );
        sigs.insert(
            "todo".to_string(),
            make_sig(
                "todo(message: str)",
                "Marks unfinished code.",
                &["message: str"],
            ),
        );
    }

    /// Registers collection built-in signatures.
    fn register_collection_builtins(sigs: &mut HashMap<String, SignatureInfo>) {
        sigs.insert(
            "len".to_string(),
            make_sig(
                "len(collection: any) -> usize",
                "Returns the length.",
                &["collection: any"],
            ),
        );
        sigs.insert(
            "type_of".to_string(),
            make_sig(
                "type_of(value: any) -> str",
                "Returns the type name.",
                &["value: any"],
            ),
        );
        sigs.insert(
            "push".to_string(),
            make_sig(
                "push(array: Array, value: any)",
                "Appends a value to an array.",
                &["array: Array", "value: any"],
            ),
        );
    }

    /// Registers tensor built-in signatures.
    fn register_tensor_builtins(sigs: &mut HashMap<String, SignatureInfo>) {
        sigs.insert(
            "zeros".to_string(),
            make_sig(
                "zeros(rows: usize, cols: usize) -> Tensor",
                "Creates a zero-filled tensor.",
                &["rows: usize", "cols: usize"],
            ),
        );
        sigs.insert(
            "ones".to_string(),
            make_sig(
                "ones(rows: usize, cols: usize) -> Tensor",
                "Creates a ones-filled tensor.",
                &["rows: usize", "cols: usize"],
            ),
        );
        sigs.insert(
            "randn".to_string(),
            make_sig(
                "randn(rows: usize, cols: usize) -> Tensor",
                "Creates a random normal tensor.",
                &["rows: usize", "cols: usize"],
            ),
        );
        sigs.insert(
            "matmul".to_string(),
            make_sig(
                "matmul(a: Tensor, b: Tensor) -> Tensor",
                "Matrix multiplication.",
                &["a: Tensor", "b: Tensor"],
            ),
        );
    }

    /// Registers math built-in signatures.
    fn register_math_builtins(sigs: &mut HashMap<String, SignatureInfo>) {
        sigs.insert(
            "abs".to_string(),
            make_sig(
                "abs(x: number) -> number",
                "Absolute value.",
                &["x: number"],
            ),
        );
        sigs.insert(
            "sqrt".to_string(),
            make_sig("sqrt(x: f64) -> f64", "Square root.", &["x: f64"]),
        );
        sigs.insert(
            "pow".to_string(),
            make_sig(
                "pow(base: f64, exp: f64) -> f64",
                "Power.",
                &["base: f64", "exp: f64"],
            ),
        );
    }
}

impl Default for SignatureHelper {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a `SignatureInfo` from label, doc, and parameter labels.
fn make_sig(label: &str, doc: &str, params: &[&str]) -> SignatureInfo {
    SignatureInfo {
        label: label.to_string(),
        doc: Some(doc.to_string()),
        parameters: params
            .iter()
            .map(|p| ParameterInfo {
                label: p.to_string(),
                doc: None,
            })
            .collect(),
    }
}

/// Parses the function call context before the cursor.
///
/// Returns `(function_name, active_parameter_index)`.
fn parse_call_context(before_cursor: &str) -> Option<(String, usize)> {
    // Walk backwards to find the matching `(`
    let mut paren_depth: usize = 0;
    let mut comma_count = 0;
    let bytes = before_cursor.as_bytes();

    for i in (0..bytes.len()).rev() {
        match bytes[i] {
            b')' => paren_depth += 1,
            b'(' => {
                if paren_depth > 0 {
                    paren_depth -= 1;
                } else {
                    // Extract function name before this paren
                    let before_paren = &before_cursor[..i];
                    let name = extract_trailing_ident(before_paren)?;
                    return Some((name, comma_count));
                }
            }
            b',' if paren_depth == 0 => comma_count += 1,
            _ => {}
        }
    }
    None
}

/// Extracts the trailing identifier from a string.
fn extract_trailing_ident(s: &str) -> Option<String> {
    let trimmed = s.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let end = trimmed.len();
    let start = (0..end)
        .rev()
        .take_while(|&i| bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
        .last()?;
    let ident = &trimmed[start..end];
    if ident.is_empty() || ident.chars().next()?.is_ascii_digit() {
        return None;
    }
    Some(ident.to_string())
}

/// Extracts a function signature from source code definitions.
fn extract_fn_signature(source: &str, fn_name: &str) -> Option<SignatureInfo> {
    for line in source.lines() {
        let trimmed = line.trim();
        let clean = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
        if let Some(rest) = clean.strip_prefix("fn ") {
            let name = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if name == fn_name {
                return Some(build_sig_from_line(clean));
            }
        }
    }
    None
}

/// Builds a `SignatureInfo` from a function definition line.
fn build_sig_from_line(line: &str) -> SignatureInfo {
    let params = extract_params_from_line(line);
    SignatureInfo {
        label: line.to_string(),
        doc: None,
        parameters: params
            .iter()
            .map(|p| ParameterInfo {
                label: p.to_string(),
                doc: None,
            })
            .collect(),
    }
}

/// Extracts parameter strings from a function definition line.
fn extract_params_from_line(line: &str) -> Vec<String> {
    let open = match line.find('(') {
        Some(p) => p + 1,
        None => return Vec::new(),
    };
    let close = match line.find(')') {
        Some(p) => p,
        None => return Vec::new(),
    };
    let params_str = &line[open..close];
    if params_str.trim().is_empty() {
        return Vec::new();
    }
    params_str
        .split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

// ============================================================================
// 6. CallHierarchyProvider -- Call hierarchy navigation
// ============================================================================

/// An item in the call hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallItem {
    /// The function name.
    pub name: String,
    /// The symbol kind.
    pub kind: SymbolKind,
    /// The starting line of the function definition.
    pub range_start_line: usize,
    /// The ending line of the function definition.
    pub range_end_line: usize,
}

/// An edge in the call graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallEdge {
    /// The calling function.
    pub from: CallItem,
    /// The called function.
    pub to: CallItem,
    /// The line where the call occurs.
    pub call_line: usize,
}

/// Provides call hierarchy (incoming/outgoing calls) for functions.
pub struct CallHierarchyProvider {
    /// Maps function name to (start_line, end_line).
    fn_ranges: HashMap<String, (usize, usize)>,
    /// Maps function name to list of (called_fn_name, call_line).
    outgoing: HashMap<String, Vec<(String, usize)>>,
}

impl CallHierarchyProvider {
    /// Creates a new provider by building the call graph from source.
    pub fn build_from_source(source: &str) -> Self {
        let fn_ranges = find_function_ranges(source);
        let outgoing = build_call_graph(source, &fn_ranges);
        Self {
            fn_ranges,
            outgoing,
        }
    }

    /// Returns all functions that call the given function.
    pub fn incoming_calls(&self, fn_name: &str) -> Result<Vec<CallEdge>, AdvancedLspError> {
        let to_item = self.make_call_item(fn_name)?;
        let mut edges = Vec::new();
        for (caller, calls) in &self.outgoing {
            for (callee, call_line) in calls {
                if callee == fn_name {
                    if let Ok(from_item) = self.make_call_item(caller) {
                        edges.push(CallEdge {
                            from: from_item,
                            to: to_item.clone(),
                            call_line: *call_line,
                        });
                    }
                }
            }
        }
        Ok(edges)
    }

    /// Returns all functions called by the given function.
    pub fn outgoing_calls(&self, fn_name: &str) -> Result<Vec<CallEdge>, AdvancedLspError> {
        let from_item = self.make_call_item(fn_name)?;
        let calls = self.outgoing.get(fn_name).cloned().unwrap_or_default();
        let mut edges = Vec::new();
        for (callee, call_line) in calls {
            if let Ok(to_item) = self.make_call_item(&callee) {
                edges.push(CallEdge {
                    from: from_item.clone(),
                    to: to_item,
                    call_line,
                });
            }
        }
        Ok(edges)
    }

    /// Creates a `CallItem` for the given function name.
    fn make_call_item(&self, name: &str) -> Result<CallItem, AdvancedLspError> {
        let &(start, end) =
            self.fn_ranges
                .get(name)
                .ok_or_else(|| AdvancedLspError::FunctionNotFound {
                    name: name.to_string(),
                })?;
        Ok(CallItem {
            name: name.to_string(),
            kind: SymbolKind::Function,
            range_start_line: start,
            range_end_line: end,
        })
    }
}

/// Finds all function definitions and their line ranges.
fn find_function_ranges(source: &str) -> HashMap<String, (usize, usize)> {
    let mut ranges = HashMap::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        let clean = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
        if let Some(rest) = clean.strip_prefix("fn ") {
            let name = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if !name.is_empty() {
                let start = i;
                let end = find_block_end(&lines, i);
                ranges.insert(name.to_string(), (start, end));
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    ranges
}

/// Finds the end line of a brace-delimited block starting at `start`.
fn find_block_end(lines: &[&str], start: usize) -> usize {
    let mut depth: i32 = 0;
    let mut found_open = false;
    for (i, line) in lines.iter().enumerate().skip(start) {
        for ch in line.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    found_open = true;
                }
                '}' => {
                    depth -= 1;
                    if found_open && depth == 0 {
                        return i;
                    }
                }
                _ => {}
            }
        }
    }
    // No closing brace found: single-line function or malformed
    start
}

/// Builds the outgoing call graph for all functions.
fn build_call_graph(
    source: &str,
    fn_ranges: &HashMap<String, (usize, usize)>,
) -> HashMap<String, Vec<(String, usize)>> {
    let lines: Vec<&str> = source.lines().collect();
    let mut graph: HashMap<String, Vec<(String, usize)>> = HashMap::new();

    for (fn_name, &(start, end)) in fn_ranges {
        let mut calls = Vec::new();
        for (line_idx, line) in lines
            .iter()
            .enumerate()
            .take(end.min(lines.len().saturating_sub(1)) + 1)
            .skip(start)
        {
            find_calls_in_line(line, line_idx, fn_ranges, fn_name, &mut calls);
        }
        graph.insert(fn_name.clone(), calls);
    }
    graph
}

/// Finds function calls in a single line of a function body.
fn find_calls_in_line(
    line: &str,
    line_idx: usize,
    fn_ranges: &HashMap<String, (usize, usize)>,
    self_name: &str,
    calls: &mut Vec<(String, usize)>,
) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &line[start..i];
            // Check if followed by `(` and is a known function
            if is_followed_by(line, i, '(') && word != self_name && fn_ranges.contains_key(word) {
                calls.push((word.to_string(), line_idx));
            }
            // Also handle `Mod::func(` pattern
            if line[i..].trim_start().starts_with("::") {
                let after_colons = line[i..].trim_start().strip_prefix("::").unwrap_or("");
                let method_name: String = after_colons
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !method_name.is_empty() && fn_ranges.contains_key(&method_name) {
                    calls.push((method_name, line_idx));
                }
            }
        } else if bytes[i] == b'.' {
            // Method call: `obj.method(`
            i += 1;
            if i < bytes.len() && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let method = &line[start..i];
                if is_followed_by(line, i, '(') && fn_ranges.contains_key(method) {
                    calls.push((method.to_string(), line_idx));
                }
            }
        } else {
            i += 1;
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ====================================================================
    // S13 -- SymbolIndex tests (s13_1 through s13_10)
    // ====================================================================

    #[test]
    fn s13_1_symbol_index_finds_function() {
        let source = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let index = SymbolIndex::build_from_source(source);
        let syms = index.lookup("add");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, SymbolKind::Function);
    }

    #[test]
    fn s13_2_symbol_index_finds_struct() {
        let source = "struct Point { x: f64, y: f64 }";
        let index = SymbolIndex::build_from_source(source);
        let syms = index.lookup("Point");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn s13_3_symbol_index_finds_enum_and_trait() {
        let source = "enum Color { Red, Blue }\ntrait Display { fn show() }";
        let index = SymbolIndex::build_from_source(source);
        assert_eq!(index.lookup("Color").len(), 1);
        assert_eq!(index.lookup("Color")[0].kind, SymbolKind::Enum);
        assert_eq!(index.lookup("Display").len(), 1);
        assert_eq!(index.lookup("Display")[0].kind, SymbolKind::Trait);
    }

    #[test]
    fn s13_4_symbol_index_finds_variable_and_const() {
        let source = "let x = 42\nconst MAX: i32 = 100";
        let index = SymbolIndex::build_from_source(source);
        assert_eq!(index.lookup("x")[0].kind, SymbolKind::Variable);
        assert_eq!(index.lookup("MAX")[0].kind, SymbolKind::Constant);
    }

    #[test]
    fn s13_5_symbol_index_all_symbols() {
        let source = "fn foo() {}\nstruct Bar {}\nlet z = 0\nmod utils\ntype Alias = i32";
        let index = SymbolIndex::build_from_source(source);
        let all = index.all_symbols();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn s13_6_symbol_index_pub_prefix_stripped() {
        let source = "pub fn serve() {}\npub struct Config {}";
        let index = SymbolIndex::build_from_source(source);
        assert_eq!(index.lookup("serve").len(), 1);
        assert_eq!(index.lookup("Config").len(), 1);
    }

    #[test]
    fn s13_7_symbol_index_impl_detected() {
        let source = "impl Point {\n    fn new() -> Point { Point { x: 0.0, y: 0.0 } }\n}";
        let index = SymbolIndex::build_from_source(source);
        let impl_syms = index.lookup("Point");
        assert!(!impl_syms.is_empty());
    }

    #[test]
    fn s13_8_symbol_index_doc_comment_captured() {
        let source = "/// Adds two numbers.\nfn add(a: i32, b: i32) -> i32 { a + b }";
        let index = SymbolIndex::build_from_source(source);
        let syms = index.lookup("add");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].doc_comment.as_deref(), Some("Adds two numbers."));
    }

    #[test]
    fn s13_9_symbol_index_method_has_parent_scope() {
        let source = "impl Widget {\n    fn draw() {}\n}";
        let index = SymbolIndex::build_from_source(source);
        let methods = index.lookup("draw");
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].kind, SymbolKind::Method);
        assert_eq!(methods[0].parent_scope.as_deref(), Some("Widget"));
    }

    #[test]
    fn s13_10_symbol_index_empty_source() {
        let index = SymbolIndex::build_from_source("");
        assert!(index.all_symbols().is_empty());
        assert!(index.lookup("anything").is_empty());
    }

    // ====================================================================
    // S14 -- ReferencesFinder + CodeAction tests (s14_1 through s14_10)
    // ====================================================================

    #[test]
    fn s14_1_references_finds_definition() {
        let source = "let count = 0\ncount = count + 1";
        let finder = ReferencesFinder::new();
        let refs = finder.find_references(source, "count", 0);
        assert!(refs.iter().any(|r| r.kind == ReferenceKind::Definition));
    }

    #[test]
    fn s14_2_references_finds_reads_and_writes() {
        let source = "let x = 0\nx = 10\nprintln(x)";
        let finder = ReferencesFinder::new();
        let refs = finder.find_references(source, "x", 0);
        assert!(refs.iter().any(|r| r.kind == ReferenceKind::Write));
        assert!(refs.iter().any(|r| r.kind == ReferenceKind::Read));
    }

    #[test]
    fn s14_3_references_whole_word_only() {
        let source = "let count = 0\nlet counter = 1\ncount = 5";
        let finder = ReferencesFinder::new();
        let refs = finder.find_references(source, "count", 0);
        // "count" should not match inside "counter"
        assert_eq!(refs.len(), 2); // definition + write
    }

    #[test]
    fn s14_4_references_import_kind() {
        let source = "use math\nlet x = math::sqrt(4.0)";
        let finder = ReferencesFinder::new();
        let refs = finder.find_references(source, "math", 0);
        assert!(refs.iter().any(|r| r.kind == ReferenceKind::Import));
    }

    #[test]
    fn s14_5_references_no_matches() {
        let source = "let a = 1\nlet b = 2";
        let finder = ReferencesFinder::new();
        let refs = finder.find_references(source, "nonexistent", 0);
        assert!(refs.is_empty());
    }

    #[test]
    fn s14_6_code_action_make_mutable() {
        let source = "let x = 42";
        let provider = CodeActionProvider::new();
        let actions = provider.actions_for_line(source, 0);
        assert!(actions.iter().any(|a| a.title == "Make variable mutable"));
    }

    #[test]
    fn s14_7_code_action_add_type_annotation() {
        let source = "let x = 42";
        let provider = CodeActionProvider::new();
        let actions = provider.actions_for_line(source, 0);
        assert!(actions
            .iter()
            .any(|a| a.title.contains("Add type annotation")));
    }

    #[test]
    fn s14_8_code_action_no_mut_for_already_mut() {
        let source = "let mut x = 42";
        let provider = CodeActionProvider::new();
        let actions = provider.actions_for_line(source, 0);
        assert!(!actions.iter().any(|a| a.title == "Make variable mutable"));
    }

    #[test]
    fn s14_9_code_action_extract_function() {
        let source = "fn main() {\n    let a = 1\n    let b = 2\n    println(a + b)\n}";
        let provider = CodeActionProvider::new();
        let result = provider.extract_function(source, 1, 2, "setup");
        assert!(result.is_ok());
        let action = result.unwrap();
        assert_eq!(action.kind, CodeActionKind::RefactorExtract);
        assert!(action.title.contains("setup"));
    }

    #[test]
    fn s14_10_code_action_add_missing_import() {
        let source = "let x = Math::sqrt(4.0)";
        let provider = CodeActionProvider::new();
        let actions = provider.actions_for_line(source, 0);
        assert!(actions
            .iter()
            .any(|a| a.title.contains("Add missing import")));
    }

    // ====================================================================
    // S15 -- SemanticTokenizer tests (s15_1 through s15_10)
    // ====================================================================

    #[test]
    fn s15_1_semantic_tokenizer_comments() {
        let source = "// This is a comment";
        let tokenizer = SemanticTokenizer::new();
        let tokens = tokenizer.tokenize(source);
        assert!(tokens
            .iter()
            .any(|t| t.token_type == SemanticTokenType::Comment));
    }

    #[test]
    fn s15_2_semantic_tokenizer_keywords() {
        let source = "let x = 42";
        let tokenizer = SemanticTokenizer::new();
        let tokens = tokenizer.tokenize(source);
        assert!(tokens
            .iter()
            .any(|t| t.token_type == SemanticTokenType::Keyword && t.start_char == 0));
    }

    #[test]
    fn s15_3_semantic_tokenizer_function_declaration() {
        let source = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let tokenizer = SemanticTokenizer::new();
        let tokens = tokenizer.tokenize(source);
        let fn_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type == SemanticTokenType::Function)
            .collect();
        assert!(!fn_tokens.is_empty());
        // "add" should have declaration modifier
        assert!(fn_tokens[0]
            .modifiers
            .contains(SemanticTokenModifier::DECLARATION));
    }

    #[test]
    fn s15_4_semantic_tokenizer_string_literals() {
        let source = "let name = \"hello world\"";
        let tokenizer = SemanticTokenizer::new();
        let tokens = tokenizer.tokenize(source);
        assert!(tokens
            .iter()
            .any(|t| t.token_type == SemanticTokenType::String));
    }

    #[test]
    fn s15_5_semantic_tokenizer_numbers() {
        let source = "let x = 42";
        let tokenizer = SemanticTokenizer::new();
        let tokens = tokenizer.tokenize(source);
        assert!(tokens
            .iter()
            .any(|t| t.token_type == SemanticTokenType::Number));
    }

    #[test]
    fn s15_6_semantic_tokenizer_type_names() {
        let source = "let p: Point = Point { x: 0.0 }";
        let tokenizer = SemanticTokenizer::new();
        let tokens = tokenizer.tokenize(source);
        assert!(tokens
            .iter()
            .any(|t| t.token_type == SemanticTokenType::Type));
    }

    #[test]
    fn s15_7_semantic_tokenizer_annotations() {
        let source = "@kernel fn setup_page_table() {}";
        let tokenizer = SemanticTokenizer::new();
        let tokens = tokenizer.tokenize(source);
        assert!(tokens
            .iter()
            .any(|t| t.token_type == SemanticTokenType::Decorator));
    }

    #[test]
    fn s15_8_semantic_tokenizer_mutable_variable() {
        let source = "let mut counter = 0";
        let tokenizer = SemanticTokenizer::new();
        let tokens = tokenizer.tokenize(source);
        let mut_vars: Vec<_> = tokens
            .iter()
            .filter(|t| t.modifiers.contains(SemanticTokenModifier::MUTABLE))
            .collect();
        assert!(!mut_vars.is_empty());
    }

    #[test]
    fn s15_9_semantic_tokenizer_empty_source() {
        let tokenizer = SemanticTokenizer::new();
        let tokens = tokenizer.tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn s15_10_semantic_tokenizer_multiline() {
        let source = "fn foo() {\n    let x = 42\n    return x\n}";
        let tokenizer = SemanticTokenizer::new();
        let tokens = tokenizer.tokenize(source);
        // Should have tokens on multiple lines
        let lines: std::collections::HashSet<usize> = tokens.iter().map(|t| t.line).collect();
        assert!(lines.len() >= 3);
    }

    // ====================================================================
    // S16 -- SignatureHelper + CallHierarchy tests (s16_1 through s16_10)
    // ====================================================================

    #[test]
    fn s16_1_signature_helper_builtin_println() {
        let source = "println(";
        let helper = SignatureHelper::new();
        let sig = helper.get_signature(source, 0, 8);
        assert!(sig.is_some());
        let active = sig.unwrap();
        assert!(active.signature.label.contains("println"));
        assert_eq!(active.active_parameter, 0);
    }

    #[test]
    fn s16_2_signature_helper_active_parameter() {
        let source = "assert_eq(a, ";
        let helper = SignatureHelper::new();
        let sig = helper.get_signature(source, 0, 13);
        assert!(sig.is_some());
        assert_eq!(sig.unwrap().active_parameter, 1);
    }

    #[test]
    fn s16_3_signature_helper_user_defined_fn() {
        let source = "fn greet(name: str, loud: bool) { println(name) }\ngreet(";
        let helper = SignatureHelper::new();
        let sig = helper.get_signature(source, 1, 6);
        assert!(sig.is_some());
        let active = sig.unwrap();
        assert_eq!(active.signature.parameters.len(), 2);
    }

    #[test]
    fn s16_4_signature_helper_no_context() {
        let source = "let x = 42";
        let helper = SignatureHelper::new();
        let sig = helper.get_signature(source, 0, 10);
        assert!(sig.is_none());
    }

    #[test]
    fn s16_5_signature_helper_tensor_builtin() {
        let source = "zeros(3, ";
        let helper = SignatureHelper::new();
        let sig = helper.get_signature(source, 0, 9);
        assert!(sig.is_some());
        let active = sig.unwrap();
        assert!(active.signature.label.contains("zeros"));
        assert_eq!(active.active_parameter, 1);
    }

    #[test]
    fn s16_6_call_hierarchy_outgoing() {
        let source = "fn helper() { }\nfn main() {\n    helper()\n}";
        let provider = CallHierarchyProvider::build_from_source(source);
        let outgoing = provider.outgoing_calls("main").unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].to.name, "helper");
    }

    #[test]
    fn s16_7_call_hierarchy_incoming() {
        let source = "fn helper() { }\nfn main() {\n    helper()\n}";
        let provider = CallHierarchyProvider::build_from_source(source);
        let incoming = provider.incoming_calls("helper").unwrap();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].from.name, "main");
    }

    #[test]
    fn s16_8_call_hierarchy_no_calls() {
        let source = "fn lonely() { }";
        let provider = CallHierarchyProvider::build_from_source(source);
        let outgoing = provider.outgoing_calls("lonely").unwrap();
        assert!(outgoing.is_empty());
    }

    #[test]
    fn s16_9_call_hierarchy_unknown_function() {
        let source = "fn main() { }";
        let provider = CallHierarchyProvider::build_from_source(source);
        let result = provider.outgoing_calls("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn s16_10_call_hierarchy_chain() {
        let source = "fn a() { }\nfn b() {\n    a()\n}\nfn c() {\n    b()\n}";
        let provider = CallHierarchyProvider::build_from_source(source);

        let outgoing_c = provider.outgoing_calls("c").unwrap();
        assert_eq!(outgoing_c.len(), 1);
        assert_eq!(outgoing_c[0].to.name, "b");

        let outgoing_b = provider.outgoing_calls("b").unwrap();
        assert_eq!(outgoing_b.len(), 1);
        assert_eq!(outgoing_b[0].to.name, "a");

        let incoming_a = provider.incoming_calls("a").unwrap();
        assert_eq!(incoming_a.len(), 1);
        assert_eq!(incoming_a[0].from.name, "b");
    }
}
