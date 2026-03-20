//! Documentation generator for Fajar Lang.
//!
//! Generates HTML/JSON documentation from `.fj` source files
//! and `///` doc comments. Supports Markdown rendering, cross-references,
//! and fuzzy search across documented items.

use std::collections::HashMap;
use std::fmt;

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors that can occur during documentation generation.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum DocGenError {
    /// Source file could not be read.
    #[error("cannot read source '{path}': {reason}")]
    ReadError {
        /// File path.
        path: String,
        /// Reason for failure.
        reason: String,
    },

    /// Invalid doc comment syntax.
    #[error("malformed doc comment at line {line}: {reason}")]
    MalformedComment {
        /// Line number.
        line: u32,
        /// Description.
        reason: String,
    },

    /// Cross-reference target not found.
    #[error("unresolved cross-reference: [{target}]")]
    UnresolvedRef {
        /// The reference target that could not be found.
        target: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// DocItem — a single documented entity
// ═══════════════════════════════════════════════════════════════════════

/// The kind of a documented item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocItemKind {
    /// A function or method.
    Function,
    /// A struct definition.
    Struct,
    /// An enum definition.
    Enum,
    /// A trait definition.
    Trait,
    /// A constant.
    Const,
    /// A type alias.
    TypeAlias,
}

impl fmt::Display for DocItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Function => write!(f, "function"),
            Self::Struct => write!(f, "struct"),
            Self::Enum => write!(f, "enum"),
            Self::Trait => write!(f, "trait"),
            Self::Const => write!(f, "const"),
            Self::TypeAlias => write!(f, "type"),
        }
    }
}

/// A single documented item extracted from source.
#[derive(Debug, Clone, PartialEq)]
pub struct DocItem {
    /// Name of the item.
    pub name: String,
    /// Kind (function, struct, enum, trait, etc.).
    pub kind: DocItemKind,
    /// The doc comment text (raw Markdown).
    pub doc_text: String,
    /// The signature line (e.g., `fn add(a: i32, b: i32) -> i32`).
    pub signature: String,
    /// Line number in the source file.
    pub line: u32,
}

impl DocItem {
    /// Creates a new documented item.
    pub fn new(
        name: impl Into<String>,
        kind: DocItemKind,
        doc_text: impl Into<String>,
        signature: impl Into<String>,
        line: u32,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            doc_text: doc_text.into(),
            signature: signature.into(),
            line,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DocModule — a module's documentation
// ═══════════════════════════════════════════════════════════════════════

/// Documentation for a single module (typically one `.fj` file).
#[derive(Debug, Clone, PartialEq)]
pub struct DocModule {
    /// Module name.
    pub name: String,
    /// Module-level doc comment.
    pub module_doc: String,
    /// Items in this module.
    pub items: Vec<DocItem>,
    /// Submodule documentation.
    pub submodules: Vec<DocModule>,
}

impl DocModule {
    /// Creates a new module with the given name and doc text.
    pub fn new(name: impl Into<String>, module_doc: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            module_doc: module_doc.into(),
            items: Vec::new(),
            submodules: Vec::new(),
        }
    }

    /// Adds an item to the module.
    pub fn add_item(&mut self, item: DocItem) {
        self.items.push(item);
    }

    /// Adds a submodule.
    pub fn add_submodule(&mut self, submodule: DocModule) {
        self.submodules.push(submodule);
    }

    /// Returns the total number of items (including submodule items).
    pub fn total_items(&self) -> usize {
        let own = self.items.len();
        let sub: usize = self.submodules.iter().map(|m| m.total_items()).sum();
        own + sub
    }
}

// ═══════════════════════════════════════════════════════════════════════
// parse_doc_comments — extract doc items from source text
// ═══════════════════════════════════════════════════════════════════════

/// Extracts documented items from Fajar Lang source text.
///
/// Looks for `///` doc comments immediately preceding `fn`, `struct`,
/// `enum`, `trait`, `const`, or `type` declarations.
pub fn parse_doc_comments(source: &str) -> Vec<DocItem> {
    let lines: Vec<&str> = source.lines().collect();
    let mut items = Vec::new();
    let mut doc_buf = String::new();
    let mut doc_start_line: u32 = 0;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if let Some(comment) = trimmed.strip_prefix("///") {
            if doc_buf.is_empty() {
                doc_start_line = (i + 1) as u32;
            }
            if !doc_buf.is_empty() {
                doc_buf.push('\n');
            }
            doc_buf.push_str(comment.trim());
            continue;
        }

        if !doc_buf.is_empty() {
            if let Some(item) = try_parse_declaration(trimmed, &doc_buf, doc_start_line) {
                items.push(item);
            }
            doc_buf.clear();
        }
    }

    items
}

/// Tries to parse a declaration line and attach doc text.
fn try_parse_declaration(line: &str, doc_text: &str, line_num: u32) -> Option<DocItem> {
    let (kind, name) = detect_item_kind(line)?;
    Some(DocItem::new(name, kind, doc_text, line, line_num))
}

/// Detects the kind and name of a declaration line.
fn detect_item_kind(line: &str) -> Option<(DocItemKind, String)> {
    // Strip leading `pub` if present.
    let line = line.strip_prefix("pub ").unwrap_or(line);

    if let Some(rest) = line.strip_prefix("fn ") {
        let name = extract_name(rest);
        return Some((DocItemKind::Function, name));
    }
    if let Some(rest) = line.strip_prefix("struct ") {
        let name = extract_name(rest);
        return Some((DocItemKind::Struct, name));
    }
    if let Some(rest) = line.strip_prefix("enum ") {
        let name = extract_name(rest);
        return Some((DocItemKind::Enum, name));
    }
    if let Some(rest) = line.strip_prefix("trait ") {
        let name = extract_name(rest);
        return Some((DocItemKind::Trait, name));
    }
    if let Some(rest) = line.strip_prefix("const ") {
        let name = extract_name(rest);
        return Some((DocItemKind::Const, name));
    }
    if let Some(rest) = line.strip_prefix("type ") {
        let name = extract_name(rest);
        return Some((DocItemKind::TypeAlias, name));
    }
    None
}

/// Extracts the identifier name from a declaration fragment.
fn extract_name(rest: &str) -> String {
    rest.split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("")
        .to_string()
}

// ═══════════════════════════════════════════════════════════════════════
// Markdown rendering — subset for doc output
// ═══════════════════════════════════════════════════════════════════════

/// Converts a subset of Markdown to HTML.
///
/// Supports: headings (`#`), bold (`**`), italic (`*`), code (`` ` ``),
/// and paragraphs (blank-line separated). This is intentionally
/// minimal — a full CommonMark parser is not needed for doc comments.
pub fn render_markdown(text: &str) -> String {
    let mut html = String::new();
    let mut in_paragraph = false;

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if in_paragraph {
                html.push_str("</p>\n");
                in_paragraph = false;
            }
            continue;
        }

        if let Some(heading) = parse_heading(trimmed) {
            if in_paragraph {
                html.push_str("</p>\n");
                in_paragraph = false;
            }
            html.push_str(&heading);
            html.push('\n');
            continue;
        }

        let inline_rendered = render_inline(trimmed);
        if !in_paragraph {
            html.push_str("<p>");
            in_paragraph = true;
        } else {
            html.push(' ');
        }
        html.push_str(&inline_rendered);
    }

    if in_paragraph {
        html.push_str("</p>\n");
    }

    html
}

/// Parses a heading line (e.g., `# Title` -> `<h1>Title</h1>`).
fn parse_heading(line: &str) -> Option<String> {
    let level = line.chars().take_while(|&c| c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = line[level..].trim();
    if rest.is_empty() {
        return None;
    }
    Some(format!("<h{level}>{rest}</h{level}>"))
}

/// Renders inline Markdown: `**bold**`, `*italic*`, and `` `code` ``.
fn render_inline(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 16);
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some(end) = find_closing_double_star(&chars, i + 2) {
                result.push_str("<strong>");
                let inner: String = chars[i + 2..end].iter().collect();
                result.push_str(&inner);
                result.push_str("</strong>");
                i = end + 2;
                continue;
            }
        }

        if chars[i] == '*' && (i + 1 < len) && chars[i + 1] != '*' {
            if let Some(end) = find_closing_char(&chars, i + 1, '*') {
                result.push_str("<em>");
                let inner: String = chars[i + 1..end].iter().collect();
                result.push_str(&inner);
                result.push_str("</em>");
                i = end + 1;
                continue;
            }
        }

        if chars[i] == '`' {
            if let Some(end) = find_closing_char(&chars, i + 1, '`') {
                result.push_str("<code>");
                let inner: String = chars[i + 1..end].iter().collect();
                result.push_str(&inner);
                result.push_str("</code>");
                i = end + 1;
                continue;
            }
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Finds the index of a closing `**` starting from `start`.
fn find_closing_double_star(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == '*' && chars[i + 1] == '*' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Finds the index of a closing character starting from `start`.
fn find_closing_char(chars: &[char], start: usize, closing: char) -> Option<usize> {
    chars.iter().position(|&c| c == closing).and_then(|pos| {
        if pos >= start {
            Some(pos)
        } else {
            // Search from `start`.
            chars[start..]
                .iter()
                .position(|&c| c == closing)
                .map(|p| p + start)
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════
// CrossReference — resolve doc links
// ═══════════════════════════════════════════════════════════════════════

/// Resolves `[`TypeName`]` cross-references in doc text.
///
/// Maintains a registry of known item names and replaces references
/// with HTML links.
#[derive(Debug, Clone)]
pub struct CrossReference {
    /// Map from item name to its anchor (URL fragment).
    registry: HashMap<String, String>,
}

impl CrossReference {
    /// Creates a new cross-reference resolver.
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
        }
    }

    /// Registers an item that can be linked to.
    pub fn register(&mut self, name: &str, anchor: &str) {
        self.registry.insert(name.to_string(), anchor.to_string());
    }

    /// Populates the registry from a list of doc modules.
    pub fn register_modules(&mut self, modules: &[DocModule]) {
        for module in modules {
            for item in &module.items {
                let anchor = format!("{}.{}", module.name, item.name.to_lowercase());
                self.register(&item.name, &anchor);
            }
            self.register_modules(&module.submodules);
        }
    }

    /// Resolves `[`Name`]` references in text, replacing them with
    /// HTML links. Unresolved references are left as-is.
    pub fn resolve(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut remaining = text;

        while let Some(start) = remaining.find('[') {
            result.push_str(&remaining[..start]);
            let after_bracket = &remaining[start + 1..];

            if let Some(end) = after_bracket.find(']') {
                let ref_name = &after_bracket[..end];
                if let Some(anchor) = self.registry.get(ref_name) {
                    result.push_str(&format!("<a href=\"#{anchor}\">{ref_name}</a>"));
                } else {
                    // Leave unresolved references as-is.
                    result.push('[');
                    result.push_str(ref_name);
                    result.push(']');
                }
                remaining = &after_bracket[end + 1..];
            } else {
                result.push('[');
                remaining = after_bracket;
            }
        }

        result.push_str(remaining);
        result
    }

    /// Returns the number of registered items.
    pub fn len(&self) -> usize {
        self.registry.len()
    }

    /// Returns `true` if no items are registered.
    pub fn is_empty(&self) -> bool {
        self.registry.is_empty()
    }
}

impl Default for CrossReference {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DocSearch — fuzzy search over documented items
// ═══════════════════════════════════════════════════════════════════════

/// A search hit with relevance score.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    /// Module name.
    pub module: String,
    /// Item name.
    pub item_name: String,
    /// Item kind.
    pub kind: DocItemKind,
    /// Relevance score (lower is better).
    pub score: usize,
}

/// Searches documented items by name substring (case-insensitive).
///
/// Returns matches sorted by relevance (exact prefix > substring).
pub fn search_docs(modules: &[DocModule], query: &str) -> Vec<SearchHit> {
    let query_lower = query.to_lowercase();
    let mut hits = Vec::new();

    collect_search_hits(modules, &query_lower, &mut hits);

    hits.sort_by_key(|h| h.score);
    hits
}

/// Recursively collects search hits from modules.
fn collect_search_hits(modules: &[DocModule], query: &str, hits: &mut Vec<SearchHit>) {
    for module in modules {
        for item in &module.items {
            let name_lower = item.name.to_lowercase();
            if let Some(score) = fuzzy_score(&name_lower, query) {
                hits.push(SearchHit {
                    module: module.name.clone(),
                    item_name: item.name.clone(),
                    kind: item.kind,
                    score,
                });
            }
        }
        collect_search_hits(&module.submodules, query, hits);
    }
}

/// Returns a relevance score if `name` matches `query`.
///
/// Scoring: 0 = exact match, 1 = prefix match, 2 = substring match.
/// Returns `None` if there is no match.
fn fuzzy_score(name: &str, query: &str) -> Option<usize> {
    if name == query {
        Some(0)
    } else if name.starts_with(query) {
        Some(1)
    } else if name.contains(query) {
        Some(2)
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DocOutput — output format selection
// ═══════════════════════════════════════════════════════════════════════

/// Output format for generated documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocOutput {
    /// HTML output.
    Html,
    /// JSON output (for tooling integration).
    Json,
}

impl fmt::Display for DocOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Html => write!(f, "html"),
            Self::Json => write!(f, "json"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// HTML generation
// ═══════════════════════════════════════════════════════════════════════

/// Generates an HTML documentation page from modules.
pub fn generate_html(modules: &[DocModule]) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
    html.push_str("  <meta charset=\"utf-8\">\n");
    html.push_str("  <title>Fajar Lang Documentation</title>\n");
    html.push_str("  <style>\n");
    html.push_str("    body { font-family: sans-serif; margin: 2em; }\n");
    html.push_str("    .item { margin: 1em 0; }\n");
    html.push_str("    .signature { font-family: monospace; ");
    html.push_str("background: #f4f4f4; padding: 0.3em; }\n");
    html.push_str("    .kind { color: #666; font-size: 0.9em; }\n");
    html.push_str("  </style>\n");
    html.push_str("</head>\n<body>\n");
    html.push_str("<h1>Fajar Lang Documentation</h1>\n");

    for module in modules {
        render_module_html(module, &mut html, 2);
    }

    html.push_str("</body>\n</html>\n");
    html
}

/// Renders a single module to HTML.
fn render_module_html(module: &DocModule, html: &mut String, level: u8) {
    html.push_str(&format!("<h{level}>Module: {}</h{level}>\n", module.name));

    if !module.module_doc.is_empty() {
        html.push_str("<p>");
        html.push_str(&module.module_doc);
        html.push_str("</p>\n");
    }

    for item in &module.items {
        html.push_str("<div class=\"item\">\n");
        html.push_str(&format!("  <span class=\"kind\">{}</span> ", item.kind));
        html.push_str(&format!("<strong>{}</strong>\n", item.name));
        html.push_str(&format!(
            "  <div class=\"signature\">{}</div>\n",
            item.signature
        ));
        if !item.doc_text.is_empty() {
            html.push_str("  <div class=\"doc\">");
            html.push_str(&render_markdown(&item.doc_text));
            html.push_str("</div>\n");
        }
        html.push_str("</div>\n");
    }

    let sub_level = level.saturating_add(1).min(6);
    for sub in &module.submodules {
        render_module_html(sub, html, sub_level);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// JSON generation
// ═══════════════════════════════════════════════════════════════════════

/// Generates a JSON documentation structure from modules.
///
/// Each module is an object with `name`, `doc`, `items` (array),
/// and `submodules` (array). Uses manual JSON formatting.
pub fn generate_json(modules: &[DocModule]) -> String {
    let mut out = String::new();
    write_json_modules(modules, &mut out, 0);
    out
}

/// Writes a JSON array of modules to the output string.
fn write_json_modules(modules: &[DocModule], out: &mut String, indent: usize) {
    let pad = "  ".repeat(indent);
    let pad1 = "  ".repeat(indent + 1);
    let pad2 = "  ".repeat(indent + 2);

    out.push_str(&format!("{pad}[\n"));
    for (i, m) in modules.iter().enumerate() {
        out.push_str(&format!("{pad1}{{\n"));
        out.push_str(&format!("{pad2}\"name\": \"{}\",\n", json_escape(&m.name)));
        out.push_str(&format!(
            "{pad2}\"doc\": \"{}\",\n",
            json_escape(&m.module_doc)
        ));
        write_json_items(&m.items, out, indent + 2);
        out.push_str(&format!("{pad2}\"submodules\": "));
        if m.submodules.is_empty() {
            out.push_str("[]\n");
        } else {
            out.push('\n');
            write_json_modules(&m.submodules, out, indent + 2);
        }
        out.push_str(&format!("{pad1}}}"));
        if i + 1 < modules.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{pad}]\n"));
}

/// Writes the `"items"` array for a module.
fn write_json_items(items: &[DocItem], out: &mut String, indent: usize) {
    let pad = "  ".repeat(indent);
    let pad1 = "  ".repeat(indent + 1);

    out.push_str(&format!("{pad}\"items\": [\n"));
    for (i, item) in items.iter().enumerate() {
        out.push_str(&format!("{pad1}{{"));
        out.push_str(&format!("\"name\": \"{}\", ", json_escape(&item.name)));
        out.push_str(&format!("\"kind\": \"{}\", ", item.kind));
        out.push_str(&format!("\"doc\": \"{}\", ", json_escape(&item.doc_text)));
        out.push_str(&format!(
            "\"signature\": \"{}\", ",
            json_escape(&item.signature)
        ));
        out.push_str(&format!("\"line\": {}", item.line));
        out.push('}');
        if i + 1 < items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{pad}],\n"));
}

/// Escapes a string for JSON output.
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ═══════════════════════════════════════════════════════════════════════
// DocGenerator — high-level orchestrator
// ═══════════════════════════════════════════════════════════════════════

/// High-level documentation generator.
///
/// Walks source text entries, extracts doc comments, and produces
/// either HTML or JSON output.
#[derive(Debug)]
pub struct DocGenerator {
    /// Collected modules.
    modules: Vec<DocModule>,
    /// Cross-reference resolver.
    cross_ref: CrossReference,
    /// Output format.
    output_format: DocOutput,
}

impl DocGenerator {
    /// Creates a new generator for the given output format.
    pub fn new(output_format: DocOutput) -> Self {
        Self {
            modules: Vec::new(),
            cross_ref: CrossReference::new(),
            output_format,
        }
    }

    /// Adds a source file's documentation.
    ///
    /// `module_name` is the module path (e.g., "std::math").
    /// `source` is the full `.fj` source text.
    pub fn add_source(&mut self, module_name: &str, source: &str) {
        let items = parse_doc_comments(source);
        let module_doc = extract_module_doc(source);
        let mut module = DocModule::new(module_name, module_doc);
        for item in items {
            module.add_item(item);
        }
        self.modules.push(module);
    }

    /// Builds cross-references from all added modules.
    pub fn build_cross_refs(&mut self) {
        self.cross_ref.register_modules(&self.modules);
    }

    /// Generates the final output.
    pub fn generate(&self) -> String {
        match self.output_format {
            DocOutput::Html => generate_html(&self.modules),
            DocOutput::Json => generate_json(&self.modules),
        }
    }

    /// Returns the number of modules added.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Returns the total number of documented items.
    pub fn total_items(&self) -> usize {
        self.modules.iter().map(|m| m.total_items()).sum()
    }
}

/// Extracts the module-level doc comment (lines starting with `//!`).
fn extract_module_doc(source: &str) -> String {
    let mut doc = String::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(comment) = trimmed.strip_prefix("//!") {
            if !doc.is_empty() {
                doc.push('\n');
            }
            doc.push_str(comment.trim());
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            break;
        }
    }
    doc
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s25_1_parse_doc_comments_basic() {
        let source = r#"
/// Adds two numbers.
fn add(a: i32, b: i32) -> i32 { a + b }

/// A point in 2D space.
struct Point { x: f64, y: f64 }
"#;
        let items = parse_doc_comments(source);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "add");
        assert_eq!(items[0].kind, DocItemKind::Function);
        assert_eq!(items[0].doc_text, "Adds two numbers.");
        assert_eq!(items[1].name, "Point");
        assert_eq!(items[1].kind, DocItemKind::Struct);
    }

    #[test]
    fn s25_2_parse_doc_comments_multiline() {
        let source = r#"
/// First line.
/// Second line.
fn multi() -> i32 { 0 }
"#;
        let items = parse_doc_comments(source);
        assert_eq!(items.len(), 1);
        assert!(items[0].doc_text.contains("First line."));
        assert!(items[0].doc_text.contains("Second line."));
    }

    #[test]
    fn s25_3_parse_doc_pub_items() {
        let source = "/// Public function.\npub fn exported() -> bool { true }\n";
        let items = parse_doc_comments(source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "exported");
        assert_eq!(items[0].kind, DocItemKind::Function);
    }

    #[test]
    fn s25_4_render_markdown_headings() {
        let md = "# Title\nSome text.\n## Sub";
        let html = render_markdown(md);
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<h2>Sub</h2>"));
        assert!(html.contains("<p>Some text.</p>"));
    }

    #[test]
    fn s25_5_render_markdown_inline() {
        let md = "Use **bold** and *italic* and `code`.";
        let html = render_markdown(md);
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn s25_6_cross_reference_resolve() {
        let mut xref = CrossReference::new();
        xref.register("Point", "math.point");
        xref.register("Vec", "collections.vec");

        let resolved = xref.resolve("See [Point] and [Vec] for details.");
        assert!(resolved.contains("<a href=\"#math.point\">Point</a>"));
        assert!(resolved.contains("<a href=\"#collections.vec\">Vec</a>"));

        let unresolved = xref.resolve("See [Unknown] for details.");
        assert!(unresolved.contains("[Unknown]"));
    }

    #[test]
    fn s25_7_search_docs_fuzzy() {
        let modules = vec![DocModule {
            name: "math".to_string(),
            module_doc: String::new(),
            items: vec![
                DocItem::new("add", DocItemKind::Function, "Add", "fn add()", 1),
                DocItem::new("subtract", DocItemKind::Function, "Sub", "fn subtract()", 5),
                DocItem::new(
                    "add_float",
                    DocItemKind::Function,
                    "Add f",
                    "fn add_float()",
                    10,
                ),
            ],
            submodules: Vec::new(),
        }];

        let hits = search_docs(&modules, "add");
        assert_eq!(hits.len(), 2); // "add" exact + "add_float" prefix
        assert_eq!(hits[0].item_name, "add");
        assert_eq!(hits[0].score, 0);
    }

    #[test]
    fn s25_8_generate_html_structure() {
        let modules = vec![DocModule {
            name: "core".to_string(),
            module_doc: "Core module.".to_string(),
            items: vec![DocItem::new(
                "main",
                DocItemKind::Function,
                "Entry point.",
                "fn main() -> i64",
                1,
            )],
            submodules: Vec::new(),
        }];

        let html = generate_html(&modules);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Module: core"));
        assert!(html.contains("main"));
        assert!(html.contains("fn main() -> i64"));
    }

    #[test]
    fn s25_9_generate_json_structure() {
        let modules = vec![DocModule {
            name: "io".to_string(),
            module_doc: "I/O module.".to_string(),
            items: vec![DocItem::new(
                "println",
                DocItemKind::Function,
                "Print with newline.",
                "fn println(s: str)",
                1,
            )],
            submodules: Vec::new(),
        }];

        let json = generate_json(&modules);
        assert!(json.contains("\"name\": \"io\""));
        assert!(json.contains("\"name\": \"println\""));
        assert!(json.contains("\"kind\": \"function\""));
    }

    #[test]
    fn s25_10_doc_generator_end_to_end() {
        let source = r#"//! Math utilities.

/// Adds two integers.
fn add(a: i32, b: i32) -> i32 { a + b }

/// A 2D vector.
struct Vec2 { x: f64, y: f64 }
"#;
        let mut doc_gen = DocGenerator::new(DocOutput::Html);
        doc_gen.add_source("math", source);
        doc_gen.build_cross_refs();

        assert_eq!(doc_gen.module_count(), 1);
        assert_eq!(doc_gen.total_items(), 2);

        let html = doc_gen.generate();
        assert!(html.contains("Math utilities."));
        assert!(html.contains("add"));
        assert!(html.contains("Vec2"));
    }
}
