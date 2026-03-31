//! Automatic Binding Generator — `fj bindgen` for C, C++, Python, Rust.
//!
//! Sprint E7: 10 tasks covering CLI config, header parsing (C/C++/Python/Rust),
//! binding customization, doc preservation, incremental regeneration, and
//! safety annotations.
//!
//! All parsing is simulated — extracts declarations from simple patterns in
//! string input rather than invoking real libclang / rustc / python.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// E7.1: `fj bindgen` CLI — BindgenConfig
// ═══════════════════════════════════════════════════════════════════════

/// Source language for binding generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindgenLanguage {
    /// C header files (.h).
    C,
    /// C++ header files (.h, .hpp).
    Cpp,
    /// Python stub files (.pyi).
    Python,
    /// Rust source files (.rs) / crate metadata.
    Rust,
}

impl fmt::Display for BindgenLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::C => write!(f, "C"),
            Self::Cpp => write!(f, "C++"),
            Self::Python => write!(f, "Python"),
            Self::Rust => write!(f, "Rust"),
        }
    }
}

/// Top-level configuration for `fj bindgen`.
#[derive(Debug, Clone)]
pub struct BindgenConfig {
    /// Path to the source header / stub / crate.
    pub source_path: String,
    /// Source language.
    pub language: BindgenLanguage,
    /// Output path for the generated `.fj` bindings.
    pub output_path: String,
    /// Optional TOML-based customization.
    pub config: Option<BindgenToml>,
}

impl BindgenConfig {
    /// Create a new config with defaults.
    pub fn new(source_path: &str, language: BindgenLanguage, output_path: &str) -> Self {
        Self {
            source_path: source_path.to_string(),
            language,
            output_path: output_path.to_string(),
            config: None,
        }
    }

    /// Attach a customization config.
    pub fn with_config(mut self, config: BindgenToml) -> Self {
        self.config = Some(config);
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E7.2–E7.5: Foreign Item Model (shared across all parsers)
// ═══════════════════════════════════════════════════════════════════════

/// A parameter extracted from a foreign declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignParam {
    /// Parameter name.
    pub name: String,
    /// Type as a string (source language representation).
    pub param_type: String,
}

/// A field in a foreign struct / class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignField {
    /// Field name.
    pub name: String,
    /// Field type (source language representation).
    pub field_type: String,
}

/// An enum variant extracted from foreign source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignVariant {
    /// Variant name.
    pub name: String,
    /// Optional explicit value.
    pub value: Option<i64>,
}

/// A foreign declaration extracted by any of the parsers.
#[derive(Debug, Clone)]
pub enum ForeignItem {
    /// A function or method.
    Function {
        name: String,
        params: Vec<ForeignParam>,
        return_type: Option<String>,
        doc: Option<String>,
        is_unsafe: bool,
    },
    /// A struct or record type.
    Struct {
        name: String,
        fields: Vec<ForeignField>,
        doc: Option<String>,
    },
    /// An enum.
    Enum {
        name: String,
        variants: Vec<ForeignVariant>,
        doc: Option<String>,
    },
    /// A type alias / typedef.
    TypeAlias {
        name: String,
        target: String,
        doc: Option<String>,
    },
    /// A C++ / Python class.
    Class {
        name: String,
        methods: Vec<ForeignItem>,
        fields: Vec<ForeignField>,
        base_classes: Vec<String>,
        namespace: Option<String>,
        doc: Option<String>,
    },
    /// A Rust trait.
    Trait {
        name: String,
        methods: Vec<ForeignItem>,
        doc: Option<String>,
    },
}

impl ForeignItem {
    /// Return the item's name regardless of variant.
    pub fn name(&self) -> &str {
        match self {
            Self::Function { name, .. }
            | Self::Struct { name, .. }
            | Self::Enum { name, .. }
            | Self::TypeAlias { name, .. }
            | Self::Class { name, .. }
            | Self::Trait { name, .. } => name,
        }
    }

    /// Return the doc comment if present.
    pub fn doc(&self) -> Option<&str> {
        match self {
            Self::Function { doc, .. }
            | Self::Struct { doc, .. }
            | Self::Enum { doc, .. }
            | Self::TypeAlias { doc, .. }
            | Self::Class { doc, .. }
            | Self::Trait { doc, .. } => doc.as_deref(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E7.2: C Header Parsing — CHeaderParser
// ═══════════════════════════════════════════════════════════════════════

/// Simulated C header parser. Extracts functions, typedefs, structs,
/// and enums from simple C-like patterns in string input.
#[derive(Debug)]
pub struct CHeaderParser {
    /// Include paths for resolving `#include`.
    pub include_paths: Vec<String>,
}

impl Default for CHeaderParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CHeaderParser {
    /// Create a new parser.
    pub fn new() -> Self {
        Self {
            include_paths: Vec::new(),
        }
    }

    /// Add an include path.
    pub fn add_include_path(&mut self, path: &str) {
        self.include_paths.push(path.to_string());
    }

    /// Parse a C header string and extract foreign items.
    ///
    /// Recognizes simple patterns:
    /// - `int foo(int a, float b);` — function
    /// - `typedef int myint;` — type alias
    /// - `struct Foo { int x; float y; };` — struct
    /// - `enum Color { RED, GREEN = 2, BLUE };` — enum
    /// - `/* doc comment */` or `// doc comment` preceding a declaration
    pub fn parse(&self, source: &str) -> Vec<ForeignItem> {
        let mut items = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Skip empty lines and preprocessor directives.
            if line.is_empty() || line.starts_with('#') {
                i += 1;
                continue;
            }

            // Collect preceding doc comment.
            let doc = self.extract_c_doc(&lines, i);

            // Typedef.
            if line.starts_with("typedef ") {
                if let Some(item) = self.parse_c_typedef(line, &doc) {
                    items.push(item);
                }
                i += 1;
                continue;
            }

            // Struct.
            if line.starts_with("struct ") && line.contains('{') {
                let block = self.collect_block(&lines, i);
                if let Some(item) = self.parse_c_struct(&block, &doc) {
                    items.push(item);
                }
                i += block.lines().count().max(1);
                continue;
            }

            // Enum.
            if line.starts_with("enum ") && line.contains('{') {
                let block = self.collect_block(&lines, i);
                if let Some(item) = self.parse_c_enum(&block, &doc) {
                    items.push(item);
                }
                i += block.lines().count().max(1);
                continue;
            }

            // Function declaration: `<type> <name>(<params>);`
            if line.contains('(') && line.ends_with(';') && !line.starts_with("//") {
                if let Some(item) = self.parse_c_function(line, &doc) {
                    items.push(item);
                }
            }

            i += 1;
        }

        items
    }

    /// Extract a preceding doc comment (line above the current index).
    fn extract_c_doc(&self, lines: &[&str], idx: usize) -> Option<String> {
        if idx == 0 {
            return None;
        }
        let prev = lines[idx - 1].trim();
        if prev.starts_with("//") {
            Some(prev.trim_start_matches("//").trim().to_string())
        } else if prev.starts_with("/*") && prev.ends_with("*/") {
            let inner = prev
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim();
            Some(inner.to_string())
        } else {
            None
        }
    }

    /// Parse a C function declaration: `int foo(int a, float b);`
    fn parse_c_function(&self, line: &str, doc: &Option<String>) -> Option<ForeignItem> {
        let line = line.trim().trim_end_matches(';').trim();
        let paren_pos = line.find('(')?;
        let close_paren = line.rfind(')')?;

        let prefix = &line[..paren_pos];
        let params_str = &line[paren_pos + 1..close_paren];

        // Split prefix into return_type + name.
        let parts: Vec<&str> = prefix.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }
        let name = parts.last()?.trim_start_matches('*').to_string();
        let return_type = parts[..parts.len() - 1].join(" ");

        let params = self.parse_c_params(params_str);

        let ret = if return_type == "void" {
            None
        } else {
            Some(return_type)
        };

        Some(ForeignItem::Function {
            name,
            params,
            return_type: ret,
            doc: doc.clone(),
            is_unsafe: true, // All C FFI is unsafe.
        })
    }

    /// Parse a C typedef: `typedef int myint;`
    fn parse_c_typedef(&self, line: &str, doc: &Option<String>) -> Option<ForeignItem> {
        let line = line
            .trim()
            .trim_start_matches("typedef ")
            .trim_end_matches(';')
            .trim();
        let parts: Vec<&str> = line.rsplitn(2, ' ').collect();
        if parts.len() < 2 {
            return None;
        }
        Some(ForeignItem::TypeAlias {
            name: parts[0].to_string(),
            target: parts[1].to_string(),
            doc: doc.clone(),
        })
    }

    /// Parse a C struct block.
    fn parse_c_struct(&self, block: &str, doc: &Option<String>) -> Option<ForeignItem> {
        let first_line = block.lines().next()?.trim();
        let name = first_line
            .trim_start_matches("struct ")
            .split(|c: char| c == '{' || c.is_whitespace())
            .next()?
            .trim()
            .to_string();
        if name.is_empty() {
            return None;
        }

        let mut fields = Vec::new();
        for line in block.lines().skip(1) {
            let line = line.trim().trim_end_matches(';').trim();
            if line.is_empty() || line == "}" || line == "};" {
                continue;
            }
            let parts: Vec<&str> = line.rsplitn(2, ' ').collect();
            if parts.len() == 2 {
                fields.push(ForeignField {
                    name: parts[0].to_string(),
                    field_type: parts[1].to_string(),
                });
            }
        }

        Some(ForeignItem::Struct {
            name,
            fields,
            doc: doc.clone(),
        })
    }

    /// Parse a C enum block.
    fn parse_c_enum(&self, block: &str, doc: &Option<String>) -> Option<ForeignItem> {
        let first_line = block.lines().next()?.trim();
        let name = first_line
            .trim_start_matches("enum ")
            .split(|c: char| c == '{' || c.is_whitespace())
            .next()?
            .trim()
            .to_string();
        if name.is_empty() {
            return None;
        }

        let mut variants = Vec::new();
        for line in block.lines().skip(1) {
            let line = line.trim().trim_end_matches(',').trim();
            if line.is_empty() || line.starts_with('}') {
                continue;
            }
            if line.contains('=') {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                let vname = parts[0].trim().to_string();
                let val = parts[1].trim().parse::<i64>().ok();
                variants.push(ForeignVariant {
                    name: vname,
                    value: val,
                });
            } else {
                variants.push(ForeignVariant {
                    name: line.to_string(),
                    value: None,
                });
            }
        }

        Some(ForeignItem::Enum {
            name,
            variants,
            doc: doc.clone(),
        })
    }

    /// Parse a C parameter list: `"int a, float b"` -> Vec<ForeignParam>.
    fn parse_c_params(&self, params_str: &str) -> Vec<ForeignParam> {
        let params_str = params_str.trim();
        if params_str.is_empty() || params_str == "void" {
            return Vec::new();
        }
        params_str
            .split(',')
            .filter_map(|p| {
                let p = p.trim();
                let parts: Vec<&str> = p.rsplitn(2, ' ').collect();
                if parts.len() == 2 {
                    Some(ForeignParam {
                        name: parts[0].trim_start_matches('*').to_string(),
                        param_type: parts[1].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Collect lines until a closing `};` to form a block.
    fn collect_block(&self, lines: &[&str], start: usize) -> String {
        let mut result = String::new();
        for line in &lines[start..] {
            result.push_str(line);
            result.push('\n');
            if line.trim().starts_with('}') {
                break;
            }
        }
        result
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E7.3: C++ Header Parsing — CppHeaderParser
// ═══════════════════════════════════════════════════════════════════════

/// Simulated C++ header parser. Extends C parsing with classes, namespaces,
/// and template declarations.
#[derive(Debug)]
pub struct CppHeaderParser {
    /// Inner C parser for basic declarations.
    pub c_parser: CHeaderParser,
}

impl Default for CppHeaderParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CppHeaderParser {
    /// Create a new C++ header parser.
    pub fn new() -> Self {
        Self {
            c_parser: CHeaderParser::new(),
        }
    }

    /// Parse a C++ header string. Handles classes, namespaces, and templates
    /// in addition to basic C declarations.
    pub fn parse(&self, source: &str) -> Vec<ForeignItem> {
        let mut items = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                i += 1;
                continue;
            }

            let doc = self.c_parser.extract_c_doc(&lines, i);

            // Namespace block.
            if line.starts_with("namespace ") && line.contains('{') {
                let ns_name = line
                    .trim_start_matches("namespace ")
                    .split(|c: char| c == '{' || c.is_whitespace())
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();

                // Collect items inside namespace (simplified: one-level deep).
                let block = self.c_parser.collect_block(&lines, i);
                let inner_lines: Vec<&str> = block.lines().skip(1).collect();
                let inner = inner_lines.join("\n");
                let inner_items = self.parse(&inner);
                for item in inner_items {
                    // Prefix names with namespace.
                    items.push(self.prefix_namespace(&ns_name, item));
                }
                i += block.lines().count().max(1);
                continue;
            }

            // Class declaration.
            if (line.starts_with("class ") || line.starts_with("struct "))
                && line.contains('{')
                && !line.contains("typedef")
            {
                let block = self.c_parser.collect_block(&lines, i);
                if let Some(item) = self.parse_cpp_class(&block, &doc) {
                    items.push(item);
                }
                i += block.lines().count().max(1);
                continue;
            }

            // Template declaration: `template<typename T>` followed by a function or class.
            if line.starts_with("template") {
                // Skip the template line, parse next line as a regular declaration.
                i += 1;
                continue;
            }

            // Fall through to C-style parsing for functions, typedefs, enums.
            if line.contains('(') && line.ends_with(';') {
                if let Some(item) = self.c_parser.parse_c_function(line, &doc) {
                    items.push(item);
                }
            } else if line.starts_with("typedef ") {
                if let Some(item) = self.c_parser.parse_c_typedef(line, &doc) {
                    items.push(item);
                }
            } else if line.starts_with("enum ") && line.contains('{') {
                let block = self.c_parser.collect_block(&lines, i);
                if let Some(item) = self.c_parser.parse_c_enum(&block, &doc) {
                    items.push(item);
                }
                i += block.lines().count().max(1);
                continue;
            }

            i += 1;
        }

        items
    }

    /// Parse a C++ class block into a `ForeignItem::Class`.
    fn parse_cpp_class(&self, block: &str, doc: &Option<String>) -> Option<ForeignItem> {
        let first_line = block.lines().next()?.trim();

        // Determine if `class` or `struct`.
        let is_class = first_line.starts_with("class ");
        let prefix = if is_class { "class " } else { "struct " };

        let after_keyword = first_line.trim_start_matches(prefix);
        // Handle base classes: `class Foo : public Bar {`
        let (name_part, bases_part) = if after_keyword.contains(':') {
            let parts: Vec<&str> = after_keyword.splitn(2, ':').collect();
            (parts[0].trim(), Some(parts[1]))
        } else {
            (after_keyword.split('{').next().unwrap_or("").trim(), None)
        };

        let name = name_part.to_string();
        if name.is_empty() {
            return None;
        }

        let base_classes: Vec<String> = bases_part
            .map(|b| {
                b.split(',')
                    .map(|s| {
                        s.trim()
                            .trim_start_matches("public ")
                            .trim_start_matches("protected ")
                            .trim_start_matches("private ")
                            .split('{')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string()
                    })
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let mut methods = Vec::new();
        let mut fields = Vec::new();

        for line in block.lines().skip(1) {
            let line = line.trim();
            if line.is_empty()
                || line.starts_with('}')
                || line.starts_with("public:")
                || line.starts_with("private:")
                || line.starts_with("protected:")
            {
                continue;
            }

            // Method: has `(` and ends with `;`.
            if line.contains('(') && line.ends_with(';') {
                if let Some(item) = self.c_parser.parse_c_function(line, &None) {
                    methods.push(item);
                }
            } else {
                // Field.
                let field_line = line.trim_end_matches(';').trim();
                let parts: Vec<&str> = field_line.rsplitn(2, ' ').collect();
                if parts.len() == 2 {
                    fields.push(ForeignField {
                        name: parts[0].to_string(),
                        field_type: parts[1].to_string(),
                    });
                }
            }
        }

        Some(ForeignItem::Class {
            name,
            methods,
            fields,
            base_classes,
            namespace: None,
            doc: doc.clone(),
        })
    }

    /// Prefix a foreign item's name with a namespace.
    fn prefix_namespace(&self, ns: &str, item: ForeignItem) -> ForeignItem {
        match item {
            ForeignItem::Function {
                name,
                params,
                return_type,
                doc,
                is_unsafe,
            } => ForeignItem::Function {
                name: format!("{}::{}", ns, name),
                params,
                return_type,
                doc,
                is_unsafe,
            },
            ForeignItem::Class {
                name,
                methods,
                fields,
                base_classes,
                doc,
                ..
            } => ForeignItem::Class {
                name: format!("{}::{}", ns, name),
                methods,
                fields,
                base_classes,
                namespace: Some(ns.to_string()),
                doc,
            },
            ForeignItem::Struct { name, fields, doc } => ForeignItem::Struct {
                name: format!("{}::{}", ns, name),
                fields,
                doc,
            },
            ForeignItem::Enum {
                name,
                variants,
                doc,
            } => ForeignItem::Enum {
                name: format!("{}::{}", ns, name),
                variants,
                doc,
            },
            ForeignItem::TypeAlias { name, target, doc } => ForeignItem::TypeAlias {
                name: format!("{}::{}", ns, name),
                target,
                doc,
            },
            other => other,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E7.4: Python Stub Parsing — PythonStubParser
// ═══════════════════════════════════════════════════════════════════════

/// Simulated Python `.pyi` stub parser. Extracts typed function signatures
/// and class definitions.
#[derive(Debug)]
pub struct PythonStubParser;

impl Default for PythonStubParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonStubParser {
    /// Create a new Python stub parser.
    pub fn new() -> Self {
        Self
    }

    /// Parse a `.pyi` stub string.
    ///
    /// Recognizes:
    /// - `def foo(a: int, b: float) -> str: ...`
    /// - `class Foo:` followed by indented methods
    /// - `# doc comment` preceding a declaration
    pub fn parse(&self, source: &str) -> Vec<ForeignItem> {
        let mut items = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with("import ") || trimmed.starts_with("from ")
            {
                i += 1;
                continue;
            }

            let doc = self.extract_py_doc(&lines, i);

            // Top-level function: `def foo(a: int) -> int: ...`
            if trimmed.starts_with("def ") && !line.starts_with(' ') && !line.starts_with('\t') {
                if let Some(item) = self.parse_py_function(trimmed, &doc) {
                    items.push(item);
                }
                i += 1;
                continue;
            }

            // Class: `class Foo:` or `class Foo(Base):`
            if trimmed.starts_with("class ") && !line.starts_with(' ') && !line.starts_with('\t') {
                let class_item = self.parse_py_class(&lines, i, &doc);
                let advance = class_item.1;
                items.push(class_item.0);
                i += advance;
                continue;
            }

            i += 1;
        }

        items
    }

    /// Extract preceding `# comment` as doc.
    fn extract_py_doc(&self, lines: &[&str], idx: usize) -> Option<String> {
        if idx == 0 {
            return None;
        }
        let prev = lines[idx - 1].trim();
        if prev.starts_with('#') {
            Some(prev.trim_start_matches('#').trim().to_string())
        } else {
            None
        }
    }

    /// Parse a Python function: `def foo(a: int, b: float) -> str: ...`
    fn parse_py_function(&self, line: &str, doc: &Option<String>) -> Option<ForeignItem> {
        let line = line.trim_start_matches("def ").trim();
        let paren_pos = line.find('(')?;
        let close_paren = line.find(')')?;

        let name = line[..paren_pos].trim().to_string();
        let params_str = &line[paren_pos + 1..close_paren];

        let params = self.parse_py_params(params_str);

        // Return type: `-> str: ...`
        let return_type = if let Some(arrow_pos) = line.find("->") {
            let after_arrow = line[arrow_pos + 2..].trim();
            let ret = after_arrow.split(':').next().unwrap_or("").trim();
            if ret.is_empty() || ret == "None" {
                None
            } else {
                Some(ret.to_string())
            }
        } else {
            None
        };

        Some(ForeignItem::Function {
            name,
            params,
            return_type,
            doc: doc.clone(),
            is_unsafe: false,
        })
    }

    /// Parse Python parameters: `"a: int, b: float"`.
    fn parse_py_params(&self, params_str: &str) -> Vec<ForeignParam> {
        let params_str = params_str.trim();
        if params_str.is_empty() {
            return Vec::new();
        }
        params_str
            .split(',')
            .filter_map(|p| {
                let p = p.trim();
                if p == "self" || p == "cls" {
                    return None;
                }
                if let Some(colon_pos) = p.find(':') {
                    let name = p[..colon_pos].trim().to_string();
                    let param_type = p[colon_pos + 1..].trim().to_string();
                    Some(ForeignParam { name, param_type })
                } else {
                    Some(ForeignParam {
                        name: p.to_string(),
                        param_type: "Any".to_string(),
                    })
                }
            })
            .collect()
    }

    /// Parse a Python class starting at index `i`. Returns (item, lines_consumed).
    fn parse_py_class(
        &self,
        lines: &[&str],
        start: usize,
        doc: &Option<String>,
    ) -> (ForeignItem, usize) {
        let header = lines[start].trim();
        let after_class = header.trim_start_matches("class ").trim();

        // Name and base classes.
        let (name, base_classes) = if let Some(paren_pos) = after_class.find('(') {
            let close = after_class.find(')').unwrap_or(after_class.len());
            let n = after_class[..paren_pos].trim().to_string();
            let bases: Vec<String> = after_class[paren_pos + 1..close]
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            (n, bases)
        } else {
            let n = after_class
                .trim_end_matches(':')
                .trim()
                .to_string();
            (n, Vec::new())
        };

        let mut methods = Vec::new();
        let mut fields = Vec::new();
        let mut consumed = 1;

        // Collect indented body lines.
        for line in &lines[start + 1..] {
            let raw = *line;
            if raw.trim().is_empty() {
                consumed += 1;
                continue;
            }
            // Stop if not indented (new top-level declaration).
            if !raw.starts_with(' ') && !raw.starts_with('\t') {
                break;
            }
            consumed += 1;

            let trimmed = raw.trim();

            // Method.
            if trimmed.starts_with("def ") {
                if let Some(item) = self.parse_py_function(trimmed, &None) {
                    methods.push(item);
                }
            }
            // Field annotation: `name: type`
            else if trimmed.contains(':') && !trimmed.starts_with('#') && !trimmed.contains("...") {
                let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                if parts.len() == 2 {
                    fields.push(ForeignField {
                        name: parts[0].trim().to_string(),
                        field_type: parts[1].trim().to_string(),
                    });
                }
            }
        }

        let item = ForeignItem::Class {
            name,
            methods,
            fields,
            base_classes,
            namespace: None,
            doc: doc.clone(),
        };

        (item, consumed)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E7.5: Rust Crate Binding — RustCrateParser
// ═══════════════════════════════════════════════════════════════════════

/// Simulated Rust source parser. Extracts `pub fn`, `pub struct`, `pub enum`,
/// and `pub trait` declarations from Rust source text.
#[derive(Debug)]
pub struct RustCrateParser;

impl Default for RustCrateParser {
    fn default() -> Self {
        Self::new()
    }
}

impl RustCrateParser {
    /// Create a new Rust crate parser.
    pub fn new() -> Self {
        Self
    }

    /// Parse Rust source and extract public items.
    ///
    /// Recognizes:
    /// - `pub fn foo(a: i32) -> String { ... }`
    /// - `pub struct Foo { pub x: i32, pub y: f64 }`
    /// - `pub enum Color { Red, Green, Blue }`
    /// - `pub trait MyTrait { fn method(&self); }`
    /// - `/// doc comment`
    pub fn parse(&self, source: &str) -> Vec<ForeignItem> {
        let mut items = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            if line.is_empty() || line.starts_with("use ") || line.starts_with("mod ") {
                i += 1;
                continue;
            }

            let doc = self.extract_rust_doc(&lines, i);

            // pub fn
            if line.starts_with("pub fn ") {
                if let Some(item) = self.parse_rust_fn(line, &doc) {
                    items.push(item);
                }
                // Skip body lines if opening brace is on same line.
                if line.contains('{') {
                    i += self.skip_block(&lines, i);
                } else {
                    i += 1;
                }
                continue;
            }

            // pub struct
            if line.starts_with("pub struct ") && line.contains('{') {
                let block = self.collect_rust_block(&lines, i);
                if let Some(item) = self.parse_rust_struct(&block, &doc) {
                    items.push(item);
                }
                i += block.lines().count().max(1);
                continue;
            }

            // pub enum
            if line.starts_with("pub enum ") && line.contains('{') {
                let block = self.collect_rust_block(&lines, i);
                if let Some(item) = self.parse_rust_enum(&block, &doc) {
                    items.push(item);
                }
                i += block.lines().count().max(1);
                continue;
            }

            // pub trait
            if line.starts_with("pub trait ") && line.contains('{') {
                let block = self.collect_rust_block(&lines, i);
                if let Some(item) = self.parse_rust_trait(&block, &doc) {
                    items.push(item);
                }
                i += block.lines().count().max(1);
                continue;
            }

            // pub type alias
            if line.starts_with("pub type ") && line.contains('=') {
                if let Some(item) = self.parse_rust_type_alias(line, &doc) {
                    items.push(item);
                }
                i += 1;
                continue;
            }

            i += 1;
        }

        items
    }

    /// Extract `///` doc comments above a declaration.
    fn extract_rust_doc(&self, lines: &[&str], idx: usize) -> Option<String> {
        let mut doc_lines = Vec::new();
        let mut j = idx;
        while j > 0 {
            j -= 1;
            let prev = lines[j].trim();
            if prev.starts_with("///") {
                doc_lines.push(prev.trim_start_matches("///").trim().to_string());
            } else {
                break;
            }
        }
        if doc_lines.is_empty() {
            None
        } else {
            doc_lines.reverse();
            Some(doc_lines.join("\n"))
        }
    }

    /// Parse `pub fn foo(a: i32, b: &str) -> Result<(), Error>`.
    fn parse_rust_fn(&self, line: &str, doc: &Option<String>) -> Option<ForeignItem> {
        let line = line.trim_start_matches("pub fn ").trim();
        let paren_pos = line.find('(')?;
        let close_paren = line.find(')')?;

        let name = line[..paren_pos].trim().to_string();
        let params_str = &line[paren_pos + 1..close_paren];

        let params: Vec<ForeignParam> = params_str
            .split(',')
            .filter_map(|p| {
                let p = p.trim();
                if p.is_empty() || p == "&self" || p == "&mut self" || p == "self" {
                    return None;
                }
                p.find(':').map(|colon| ForeignParam {
                    name: p[..colon].trim().to_string(),
                    param_type: p[colon + 1..].trim().to_string(),
                })
            })
            .collect();

        let return_type = if let Some(arrow_pos) = line.find("->") {
            let after = line[arrow_pos + 2..].trim();
            let ret = after
                .split(['{', ';'])
                .next()
                .unwrap_or("")
                .trim();
            if ret.is_empty() || ret == "()" {
                None
            } else {
                Some(ret.to_string())
            }
        } else {
            None
        };

        Some(ForeignItem::Function {
            name,
            params,
            return_type,
            doc: doc.clone(),
            is_unsafe: false,
        })
    }

    /// Parse `pub struct Foo { pub x: i32, ... }`.
    fn parse_rust_struct(&self, block: &str, doc: &Option<String>) -> Option<ForeignItem> {
        let first_line = block.lines().next()?.trim();
        let name = first_line
            .trim_start_matches("pub struct ")
            .split(|c: char| c == '{' || c == '<' || c.is_whitespace())
            .next()?
            .trim()
            .to_string();
        if name.is_empty() {
            return None;
        }

        let mut fields = Vec::new();
        for line in block.lines().skip(1) {
            let line = line.trim().trim_end_matches(',').trim();
            if line.is_empty() || line == "}" {
                continue;
            }
            let line = line.trim_start_matches("pub ");
            if let Some(colon) = line.find(':') {
                fields.push(ForeignField {
                    name: line[..colon].trim().to_string(),
                    field_type: line[colon + 1..].trim().to_string(),
                });
            }
        }

        Some(ForeignItem::Struct {
            name,
            fields,
            doc: doc.clone(),
        })
    }

    /// Parse `pub enum Color { Red, Green(i32), Blue }`.
    fn parse_rust_enum(&self, block: &str, doc: &Option<String>) -> Option<ForeignItem> {
        let first_line = block.lines().next()?.trim();
        let name = first_line
            .trim_start_matches("pub enum ")
            .split(|c: char| c == '{' || c == '<' || c.is_whitespace())
            .next()?
            .trim()
            .to_string();
        if name.is_empty() {
            return None;
        }

        let mut variants = Vec::new();
        for line in block.lines().skip(1) {
            let line = line.trim().trim_end_matches(',').trim();
            if line.is_empty() || line == "}" {
                continue;
            }
            // Strip payload info for variant name.
            let vname = line
                .split(['(', '{'])
                .next()
                .unwrap_or(line)
                .trim()
                .to_string();
            if !vname.is_empty() {
                variants.push(ForeignVariant {
                    name: vname,
                    value: None,
                });
            }
        }

        Some(ForeignItem::Enum {
            name,
            variants,
            doc: doc.clone(),
        })
    }

    /// Parse `pub trait MyTrait { fn method(&self) -> i32; }`.
    fn parse_rust_trait(&self, block: &str, doc: &Option<String>) -> Option<ForeignItem> {
        let first_line = block.lines().next()?.trim();
        let name = first_line
            .trim_start_matches("pub trait ")
            .split(|c: char| c == '{' || c == '<' || c == ':' || c.is_whitespace())
            .next()?
            .trim()
            .to_string();
        if name.is_empty() {
            return None;
        }

        let mut methods = Vec::new();
        for line in block.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() || line == "}" {
                continue;
            }
            if line.starts_with("fn ") {
                // Treat as `pub fn` for parsing.
                let prefixed = format!("pub {}", line);
                if let Some(item) = self.parse_rust_fn(&prefixed, &None) {
                    methods.push(item);
                }
            }
        }

        Some(ForeignItem::Trait {
            name,
            methods,
            doc: doc.clone(),
        })
    }

    /// Parse `pub type Alias = Target;`.
    fn parse_rust_type_alias(&self, line: &str, doc: &Option<String>) -> Option<ForeignItem> {
        let line = line
            .trim_start_matches("pub type ")
            .trim_end_matches(';')
            .trim();
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() < 2 {
            return None;
        }
        Some(ForeignItem::TypeAlias {
            name: parts[0].trim().to_string(),
            target: parts[1].trim().to_string(),
            doc: doc.clone(),
        })
    }

    /// Skip a brace-delimited block starting at `start`.
    fn skip_block(&self, lines: &[&str], start: usize) -> usize {
        let mut depth = 0i32;
        let mut count = 0;
        for line in &lines[start..] {
            for ch in line.chars() {
                if ch == '{' {
                    depth += 1;
                }
                if ch == '}' {
                    depth -= 1;
                }
            }
            count += 1;
            if depth <= 0 && count > 0 {
                break;
            }
        }
        count
    }

    /// Collect a brace-delimited block into a single string.
    fn collect_rust_block(&self, lines: &[&str], start: usize) -> String {
        let mut result = String::new();
        let mut depth = 0i32;
        for line in &lines[start..] {
            result.push_str(line);
            result.push('\n');
            for ch in line.chars() {
                if ch == '{' {
                    depth += 1;
                }
                if ch == '}' {
                    depth -= 1;
                }
            }
            if depth <= 0 && !result.is_empty() {
                break;
            }
        }
        result
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E7.6: Binding Customization — BindgenToml
// ═══════════════════════════════════════════════════════════════════════

/// Rename rule for generated bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameRule {
    /// Convert to `snake_case`.
    SnakeCase,
    /// Convert to `PascalCase`.
    PascalCase,
    /// Add a prefix.
    Prefix(String),
    /// Add a suffix.
    Suffix(String),
    /// Direct rename: old -> new.
    Exact(String, String),
}

/// Customization config for binding generation, typically loaded from
/// `bindgen.toml`.
#[derive(Debug, Clone, Default)]
pub struct BindgenToml {
    /// Glob patterns for items to skip (e.g., `"__*"` skips internal items).
    pub skip_patterns: Vec<String>,
    /// Type overrides: foreign type -> Fajar type.
    pub type_overrides: HashMap<String, String>,
    /// Rename rules applied to generated names.
    pub rename_rules: Vec<RenameRule>,
    /// Additional raw Fajar source to prepend to output.
    pub preamble: Option<String>,
    /// Whether to generate safe wrappers for unsafe FFI.
    pub generate_safe_wrappers: bool,
}

impl BindgenToml {
    /// Create an empty config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a skip pattern.
    pub fn skip(mut self, pattern: &str) -> Self {
        self.skip_patterns.push(pattern.to_string());
        self
    }

    /// Add a type override.
    pub fn type_override(mut self, foreign: &str, fajar: &str) -> Self {
        self.type_overrides
            .insert(foreign.to_string(), fajar.to_string());
        self
    }

    /// Add a rename rule.
    pub fn rename(mut self, rule: RenameRule) -> Self {
        self.rename_rules.push(rule);
        self
    }

    /// Enable safe wrapper generation.
    pub fn with_safe_wrappers(mut self) -> Self {
        self.generate_safe_wrappers = true;
        self
    }

    /// Check whether a name matches any skip pattern.
    pub fn should_skip(&self, name: &str) -> bool {
        for pattern in &self.skip_patterns {
            if pattern.ends_with('*') {
                let prefix = &pattern[..pattern.len() - 1];
                if name.starts_with(prefix) {
                    return true;
                }
            } else if let Some(suffix) = pattern.strip_prefix('*') {
                if name.ends_with(suffix) {
                    return true;
                }
            } else if name == pattern {
                return true;
            }
        }
        false
    }

    /// Apply type overrides to a type string.
    pub fn apply_type_override(&self, ty: &str) -> String {
        self.type_overrides
            .get(ty)
            .cloned()
            .unwrap_or_else(|| ty.to_string())
    }

    /// Apply rename rules to a name.
    pub fn apply_rename(&self, name: &str) -> String {
        let mut result = name.to_string();
        for rule in &self.rename_rules {
            result = match rule {
                RenameRule::SnakeCase => to_snake_case(&result),
                RenameRule::PascalCase => to_pascal_case(&result),
                RenameRule::Prefix(p) => format!("{}{}", p, result),
                RenameRule::Suffix(s) => format!("{}{}", result, s),
                RenameRule::Exact(old, new) => {
                    if result == *old {
                        new.clone()
                    } else {
                        result
                    }
                }
            };
        }
        result
    }
}

/// Convert a string to `snake_case`.
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            let prev = s.chars().nth(i - 1);
            if prev.is_some_and(|c| c.is_lowercase()) {
                result.push('_');
            }
        }
        result.push(ch.to_lowercase().next().unwrap_or(ch));
    }
    result
}

/// Convert a string to `PascalCase`.
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let mut r = c.to_uppercase().to_string();
                    r.extend(chars);
                    r
                }
            }
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// E7.7: Doc Preservation + E7.9: Safety Annotations
// ═══════════════════════════════════════════════════════════════════════

/// A single generated binding (one Fajar source code fragment).
#[derive(Debug, Clone)]
pub struct GeneratedBinding {
    /// The original foreign item this was generated from.
    pub source_item_name: String,
    /// The generated Fajar Lang source code.
    pub fajar_source: String,
    /// Whether this binding includes a safe wrapper.
    pub has_safe_wrapper: bool,
}

/// Statistics about a binding generation run.
#[derive(Debug, Clone, Default)]
pub struct BindgenStats {
    /// Number of functions generated.
    pub functions: usize,
    /// Number of structs generated.
    pub structs: usize,
    /// Number of enums generated.
    pub enums: usize,
    /// Number of type aliases generated.
    pub type_aliases: usize,
    /// Number of classes generated.
    pub classes: usize,
    /// Number of traits generated.
    pub traits: usize,
    /// Number of items skipped (by skip_patterns).
    pub skipped: usize,
    /// Number of items with preserved docs.
    pub docs_preserved: usize,
    /// Number of safe wrappers generated.
    pub safe_wrappers: usize,
}

impl BindgenStats {
    /// Total items generated.
    pub fn total(&self) -> usize {
        self.functions + self.structs + self.enums + self.type_aliases + self.classes + self.traits
    }
}

impl fmt::Display for BindgenStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Generated {} bindings ({} fn, {} struct, {} enum, {} alias, {} class, {} trait), \
             {} skipped, {} docs preserved, {} safe wrappers",
            self.total(),
            self.functions,
            self.structs,
            self.enums,
            self.type_aliases,
            self.classes,
            self.traits,
            self.skipped,
            self.docs_preserved,
            self.safe_wrappers,
        )
    }
}

/// Result of a binding generation run.
#[derive(Debug, Clone)]
pub struct BindgenResult {
    /// Generated bindings.
    pub bindings: Vec<GeneratedBinding>,
    /// Statistics.
    pub stats: BindgenStats,
}

// ═══════════════════════════════════════════════════════════════════════
// E7.8: Incremental Regeneration — ChangeDetector
// ═══════════════════════════════════════════════════════════════════════

/// Hash-based change detection for incremental binding regeneration.
#[derive(Debug, Clone, Default)]
pub struct ChangeDetector {
    /// Previously computed hashes: file path -> content hash.
    previous_hashes: HashMap<String, u64>,
    /// Current hashes: file path -> content hash.
    current_hashes: HashMap<String, u64>,
}

impl ChangeDetector {
    /// Create a new change detector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load previous hashes (e.g., from a cache file).
    pub fn load_previous(&mut self, hashes: HashMap<String, u64>) {
        self.previous_hashes = hashes;
    }

    /// Record the current hash for a file path.
    pub fn record(&mut self, path: &str, content: &str) {
        let hash = simple_hash(content);
        self.current_hashes.insert(path.to_string(), hash);
    }

    /// Check whether a file has changed since the previous run.
    pub fn has_changed(&self, path: &str) -> bool {
        match (
            self.previous_hashes.get(path),
            self.current_hashes.get(path),
        ) {
            (Some(prev), Some(curr)) => prev != curr,
            (None, Some(_)) => true,  // New file.
            (Some(_), None) => true,  // Deleted or not yet recorded.
            (None, None) => true,     // Unknown — assume changed.
        }
    }

    /// Get all changed file paths.
    pub fn changed_files(&self) -> Vec<String> {
        let mut changed = Vec::new();
        for path in self.current_hashes.keys() {
            if self.has_changed(path) {
                changed.push(path.clone());
            }
        }
        changed.sort();
        changed
    }

    /// Get all unchanged file paths.
    pub fn unchanged_files(&self) -> Vec<String> {
        let mut unchanged = Vec::new();
        for path in self.current_hashes.keys() {
            if !self.has_changed(path) {
                unchanged.push(path.clone());
            }
        }
        unchanged.sort();
        unchanged
    }

    /// Return current hashes for serialization.
    pub fn current_hashes(&self) -> &HashMap<String, u64> {
        &self.current_hashes
    }
}

/// Simple non-cryptographic hash (FNV-1a variant) for change detection.
fn simple_hash(data: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in data.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ═══════════════════════════════════════════════════════════════════════
// Core: Binding Code Generator
// ═══════════════════════════════════════════════════════════════════════

/// The main binding generator. Takes parsed `ForeignItem`s and produces
/// Fajar Lang source code.
#[derive(Debug)]
pub struct BindingGenerator {
    /// Customization config.
    config: Option<BindgenToml>,
    /// Language of the source items.
    language: BindgenLanguage,
}

impl BindingGenerator {
    /// Create a new generator for the given language.
    pub fn new(language: BindgenLanguage, config: Option<BindgenToml>) -> Self {
        Self { config, language }
    }

    /// Generate bindings for a list of foreign items.
    pub fn generate(&self, items: &[ForeignItem]) -> BindgenResult {
        let mut bindings = Vec::new();
        let mut stats = BindgenStats::default();

        for item in items {
            let name = item.name();

            // E7.6: Check skip patterns.
            if let Some(cfg) = &self.config {
                if cfg.should_skip(name) {
                    stats.skipped += 1;
                    continue;
                }
            }

            // E7.7: Track doc preservation.
            if item.doc().is_some() {
                stats.docs_preserved += 1;
            }

            match item {
                ForeignItem::Function { .. } => {
                    let binding = self.gen_function(item);
                    stats.functions += 1;
                    if binding.has_safe_wrapper {
                        stats.safe_wrappers += 1;
                    }
                    bindings.push(binding);
                }
                ForeignItem::Struct { .. } => {
                    bindings.push(self.gen_struct(item));
                    stats.structs += 1;
                }
                ForeignItem::Enum { .. } => {
                    bindings.push(self.gen_enum(item));
                    stats.enums += 1;
                }
                ForeignItem::TypeAlias { .. } => {
                    bindings.push(self.gen_type_alias(item));
                    stats.type_aliases += 1;
                }
                ForeignItem::Class { .. } => {
                    let class_bindings = self.gen_class(item);
                    stats.classes += 1;
                    stats.functions += class_bindings
                        .iter()
                        .filter(|b| b.source_item_name.contains("::"))
                        .count();
                    bindings.extend(class_bindings);
                }
                ForeignItem::Trait { .. } => {
                    bindings.push(self.gen_trait(item));
                    stats.traits += 1;
                }
            }
        }

        BindgenResult { bindings, stats }
    }

    /// Generate the complete Fajar source file from a result.
    pub fn emit(&self, result: &BindgenResult) -> String {
        let mut output = String::new();

        // Preamble.
        output.push_str(&format!(
            "// Auto-generated by `fj bindgen` from {} source\n",
            self.language
        ));
        output.push_str("// Do not edit manually — regenerate with `fj bindgen`\n\n");

        if let Some(cfg) = &self.config {
            if let Some(preamble) = &cfg.preamble {
                output.push_str(preamble);
                output.push_str("\n\n");
            }
        }

        for binding in &result.bindings {
            output.push_str(&binding.fajar_source);
            output.push_str("\n\n");
        }

        output
    }

    /// Generate binding for a function.
    fn gen_function(&self, item: &ForeignItem) -> GeneratedBinding {
        if let ForeignItem::Function {
            name,
            params,
            return_type,
            doc,
            is_unsafe,
        } = item
        {
            let fj_name = self.apply_name_transforms(name);
            let mut code = String::new();

            // E7.7: Preserve doc comment.
            if let Some(d) = doc {
                for line in d.lines() {
                    code.push_str(&format!("/// {}\n", line));
                }
            }

            // E7.9: Mark unsafe FFI calls.
            let params_str = params
                .iter()
                .map(|p| {
                    let ty = self.map_type(&p.param_type);
                    format!("{}: {}", p.name, ty)
                })
                .collect::<Vec<_>>()
                .join(", ");

            let ret_str = match return_type {
                Some(t) => format!(" -> {}", self.map_type(t)),
                None => String::new(),
            };

            if *is_unsafe {
                // Generate @unsafe extern and optionally a safe wrapper.
                code.push_str(&format!(
                    "@unsafe @ffi\nextern fn {}({}){}\n",
                    fj_name, params_str, ret_str
                ));

                let generate_wrapper = self
                    .config
                    .as_ref()
                    .is_some_and(|c| c.generate_safe_wrappers);

                if generate_wrapper {
                    code.push('\n');
                    code.push_str(&format!("/// Safe wrapper for `{}`.\n", fj_name));
                    let safe_name = format!("{}_safe", fj_name);
                    let call_params = params
                        .iter()
                        .map(|p| p.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ");
                    code.push_str(&format!(
                        "@safe\nfn {}({}){} {{\n    @unsafe {{ {}({}) }}\n}}\n",
                        safe_name, params_str, ret_str, fj_name, call_params,
                    ));

                    return GeneratedBinding {
                        source_item_name: name.clone(),
                        fajar_source: code,
                        has_safe_wrapper: true,
                    };
                }
            } else {
                code.push_str(&format!("@ffi\nextern fn {}({}){}\n", fj_name, params_str, ret_str));
            }

            GeneratedBinding {
                source_item_name: name.clone(),
                fajar_source: code,
                has_safe_wrapper: false,
            }
        } else {
            unreachable!("gen_function called with non-function item")
        }
    }

    /// Generate binding for a struct.
    fn gen_struct(&self, item: &ForeignItem) -> GeneratedBinding {
        if let ForeignItem::Struct { name, fields, doc } = item {
            let fj_name = self.apply_name_transforms(name);
            let mut code = String::new();

            if let Some(d) = doc {
                for line in d.lines() {
                    code.push_str(&format!("/// {}\n", line));
                }
            }

            code.push_str(&format!("struct {} {{\n", fj_name));
            for field in fields {
                let ty = self.map_type(&field.field_type);
                code.push_str(&format!("    {}: {},\n", field.name, ty));
            }
            code.push_str("}\n");

            GeneratedBinding {
                source_item_name: name.clone(),
                fajar_source: code,
                has_safe_wrapper: false,
            }
        } else {
            unreachable!("gen_struct called with non-struct item")
        }
    }

    /// Generate binding for an enum.
    fn gen_enum(&self, item: &ForeignItem) -> GeneratedBinding {
        if let ForeignItem::Enum {
            name,
            variants,
            doc,
        } = item
        {
            let fj_name = self.apply_name_transforms(name);
            let mut code = String::new();

            if let Some(d) = doc {
                for line in d.lines() {
                    code.push_str(&format!("/// {}\n", line));
                }
            }

            code.push_str(&format!("enum {} {{\n", fj_name));
            for variant in variants {
                match variant.value {
                    Some(v) => code.push_str(&format!("    {} = {},\n", variant.name, v)),
                    None => code.push_str(&format!("    {},\n", variant.name)),
                }
            }
            code.push_str("}\n");

            GeneratedBinding {
                source_item_name: name.clone(),
                fajar_source: code,
                has_safe_wrapper: false,
            }
        } else {
            unreachable!("gen_enum called with non-enum item")
        }
    }

    /// Generate binding for a type alias.
    fn gen_type_alias(&self, item: &ForeignItem) -> GeneratedBinding {
        if let ForeignItem::TypeAlias { name, target, doc } = item {
            let fj_name = self.apply_name_transforms(name);
            let fj_target = self.map_type(target);
            let mut code = String::new();

            if let Some(d) = doc {
                for line in d.lines() {
                    code.push_str(&format!("/// {}\n", line));
                }
            }

            code.push_str(&format!("type {} = {}\n", fj_name, fj_target));

            GeneratedBinding {
                source_item_name: name.clone(),
                fajar_source: code,
                has_safe_wrapper: false,
            }
        } else {
            unreachable!("gen_type_alias called with non-alias item")
        }
    }

    /// Generate bindings for a class (struct + method externs).
    fn gen_class(&self, item: &ForeignItem) -> Vec<GeneratedBinding> {
        if let ForeignItem::Class {
            name,
            methods,
            fields,
            doc,
            ..
        } = item
        {
            let mut results = Vec::new();
            let fj_name = self.apply_name_transforms(name);

            // Struct for the class.
            let mut code = String::new();
            if let Some(d) = doc {
                for line in d.lines() {
                    code.push_str(&format!("/// {}\n", line));
                }
            }
            code.push_str(&format!("struct {} {{\n", fj_name));
            for field in fields {
                let ty = self.map_type(&field.field_type);
                code.push_str(&format!("    {}: {},\n", field.name, ty));
            }
            code.push_str("}\n");

            results.push(GeneratedBinding {
                source_item_name: name.clone(),
                fajar_source: code,
                has_safe_wrapper: false,
            });

            // Extern functions for each method.
            for method in methods {
                if let ForeignItem::Function {
                    name: mname,
                    params,
                    return_type,
                    doc: mdoc,
                    is_unsafe,
                } = method
                {
                    let full_name = format!("{}::{}", fj_name, mname);
                    let qualified = ForeignItem::Function {
                        name: full_name,
                        params: params.clone(),
                        return_type: return_type.clone(),
                        doc: mdoc.clone(),
                        is_unsafe: *is_unsafe,
                    };
                    results.push(self.gen_function(&qualified));
                }
            }

            results
        } else {
            unreachable!("gen_class called with non-class item")
        }
    }

    /// Generate binding for a trait.
    fn gen_trait(&self, item: &ForeignItem) -> GeneratedBinding {
        if let ForeignItem::Trait { name, methods, doc } = item {
            let fj_name = self.apply_name_transforms(name);
            let mut code = String::new();

            if let Some(d) = doc {
                for line in d.lines() {
                    code.push_str(&format!("/// {}\n", line));
                }
            }

            code.push_str(&format!("trait {} {{\n", fj_name));
            for method in methods {
                if let ForeignItem::Function {
                    name: mname,
                    params,
                    return_type,
                    ..
                } = method
                {
                    let params_str = params
                        .iter()
                        .map(|p| {
                            let ty = self.map_type(&p.param_type);
                            format!("{}: {}", p.name, ty)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");

                    let ret_str = match return_type {
                        Some(t) => format!(" -> {}", self.map_type(t)),
                        None => String::new(),
                    };

                    code.push_str(&format!("    fn {}({}){}\n", mname, params_str, ret_str));
                }
            }
            code.push_str("}\n");

            GeneratedBinding {
                source_item_name: name.clone(),
                fajar_source: code,
                has_safe_wrapper: false,
            }
        } else {
            unreachable!("gen_trait called with non-trait item")
        }
    }

    /// Apply name transforms (rename rules from config).
    fn apply_name_transforms(&self, name: &str) -> String {
        if let Some(cfg) = &self.config {
            cfg.apply_rename(name)
        } else {
            name.to_string()
        }
    }

    /// Map a foreign type to a Fajar type.
    fn map_type(&self, foreign_type: &str) -> String {
        // Check overrides first.
        if let Some(cfg) = &self.config {
            let overridden = cfg.apply_type_override(foreign_type);
            if overridden != foreign_type {
                return overridden;
            }
        }

        // Standard mappings. When a type string is shared across languages
        // (e.g. C "int" vs Python "int"), we pick the most common mapping.
        // Per-language overrides should go in BindgenToml::type_overrides.
        match foreign_type {
            // C / C++ integer types.
            "int" | "int32_t" => "i32".to_string(),
            "unsigned int" | "uint32_t" => "u32".to_string(),
            "long" | "long long" | "int64_t" => "i64".to_string(),
            "unsigned long" | "unsigned long long" | "uint64_t" => "u64".to_string(),
            "short" | "int16_t" => "i16".to_string(),
            "unsigned short" | "uint16_t" => "u16".to_string(),
            "char" | "int8_t" => "i8".to_string(),
            "unsigned char" | "uint8_t" => "u8".to_string(),
            "float" | "f32" => "f32".to_string(),
            "double" | "f64" => "f64".to_string(),
            "bool" | "_Bool" => "bool".to_string(),
            "void" | "()" | "None" => "void".to_string(),
            "size_t" | "usize" => "usize".to_string(),
            "ssize_t" | "ptrdiff_t" | "isize" => "isize".to_string(),
            "char*" | "const char*" | "str" | "String" | "&str" => "str".to_string(),
            "void*" | "const void*" => "ptr".to_string(),

            // Python collection types.
            "bytes" | "Vec<u8>" => "[u8]".to_string(),
            "list" | "List" => "Array".to_string(),
            "dict" | "Dict" => "Map".to_string(),

            // Rust integer types that map directly.
            "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128" => {
                foreign_type.to_string()
            }

            // Unknown — pass through.
            other => other.to_string(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Top-level API: run_bindgen
// ═══════════════════════════════════════════════════════════════════════

/// Run the complete binding generation pipeline for the given config and
/// source content. Returns the generated result with stats.
///
/// This is the entry point that `fj bindgen` would call.
pub fn run_bindgen(config: &BindgenConfig, source: &str) -> BindgenResult {
    let items = match config.language {
        BindgenLanguage::C => CHeaderParser::new().parse(source),
        BindgenLanguage::Cpp => CppHeaderParser::new().parse(source),
        BindgenLanguage::Python => PythonStubParser::new().parse(source),
        BindgenLanguage::Rust => RustCrateParser::new().parse(source),
    };

    let generator = BindingGenerator::new(config.language, config.config.clone());
    generator.generate(&items)
}

// ═══════════════════════════════════════════════════════════════════════
// E7.10: Tests (15+)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // E7.1: BindgenConfig tests
    // ------------------------------------------------------------------

    #[test]
    fn e7_1_bindgen_config_creation() {
        let cfg = BindgenConfig::new("mylib.h", BindgenLanguage::C, "bindings.fj");
        assert_eq!(cfg.source_path, "mylib.h");
        assert_eq!(cfg.language, BindgenLanguage::C);
        assert_eq!(cfg.output_path, "bindings.fj");
        assert!(cfg.config.is_none());
    }

    #[test]
    fn e7_1_bindgen_config_with_toml() {
        let toml = BindgenToml::new().skip("__*");
        let cfg = BindgenConfig::new("lib.h", BindgenLanguage::C, "out.fj").with_config(toml);
        assert!(cfg.config.is_some());
        assert_eq!(cfg.config.as_ref().unwrap().skip_patterns.len(), 1);
    }

    // ------------------------------------------------------------------
    // E7.2: C header parsing
    // ------------------------------------------------------------------

    #[test]
    fn e7_2_parse_c_function() {
        let header = "int add(int a, float b);\n";
        let items = CHeaderParser::new().parse(header);
        assert_eq!(items.len(), 1);
        if let ForeignItem::Function {
            name,
            params,
            return_type,
            is_unsafe,
            ..
        } = &items[0]
        {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "a");
            assert_eq!(params[0].param_type, "int");
            assert_eq!(params[1].name, "b");
            assert_eq!(params[1].param_type, "float");
            assert_eq!(return_type.as_deref(), Some("int"));
            assert!(*is_unsafe);
        } else {
            panic!("Expected ForeignItem::Function");
        }
    }

    #[test]
    fn e7_2_parse_c_struct() {
        let header = "struct Point {\n    int x;\n    float y;\n};\n";
        let items = CHeaderParser::new().parse(header);
        assert_eq!(items.len(), 1);
        if let ForeignItem::Struct { name, fields, .. } = &items[0] {
            assert_eq!(name, "Point");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].field_type, "int");
            assert_eq!(fields[1].name, "y");
            assert_eq!(fields[1].field_type, "float");
        } else {
            panic!("Expected ForeignItem::Struct");
        }
    }

    #[test]
    fn e7_2_parse_c_enum() {
        let header = "enum Color {\n    RED,\n    GREEN = 2,\n    BLUE\n};\n";
        let items = CHeaderParser::new().parse(header);
        assert_eq!(items.len(), 1);
        if let ForeignItem::Enum {
            name, variants, ..
        } = &items[0]
        {
            assert_eq!(name, "Color");
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name, "RED");
            assert!(variants[0].value.is_none());
            assert_eq!(variants[1].name, "GREEN");
            assert_eq!(variants[1].value, Some(2));
            assert_eq!(variants[2].name, "BLUE");
        } else {
            panic!("Expected ForeignItem::Enum");
        }
    }

    #[test]
    fn e7_2_parse_c_typedef() {
        let header = "typedef unsigned int uint;\n";
        let items = CHeaderParser::new().parse(header);
        assert_eq!(items.len(), 1);
        if let ForeignItem::TypeAlias { name, target, .. } = &items[0] {
            assert_eq!(name, "uint");
            assert_eq!(target, "unsigned int");
        } else {
            panic!("Expected ForeignItem::TypeAlias");
        }
    }

    // ------------------------------------------------------------------
    // E7.3: C++ header parsing
    // ------------------------------------------------------------------

    #[test]
    fn e7_3_parse_cpp_class() {
        let header = "class Widget : public Base {\npublic:\n    int width;\n    void draw(int x);\n};\n";
        let items = CppHeaderParser::new().parse(header);
        assert!(!items.is_empty());
        if let ForeignItem::Class {
            name,
            fields,
            methods,
            base_classes,
            ..
        } = &items[0]
        {
            assert_eq!(name, "Widget");
            assert_eq!(base_classes, &["Base"]);
            assert!(!fields.is_empty());
            assert!(!methods.is_empty());
        } else {
            panic!("Expected ForeignItem::Class, got {:?}", items[0]);
        }
    }

    #[test]
    fn e7_3_parse_cpp_namespace() {
        let header = "namespace math {\nint add(int a, int b);\n}\n";
        let items = CppHeaderParser::new().parse(header);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name(), "math::add");
    }

    // ------------------------------------------------------------------
    // E7.4: Python stub parsing
    // ------------------------------------------------------------------

    #[test]
    fn e7_4_parse_python_function() {
        let stub = "def greet(name: str, count: int) -> str: ...\n";
        let items = PythonStubParser::new().parse(stub);
        assert_eq!(items.len(), 1);
        if let ForeignItem::Function {
            name,
            params,
            return_type,
            is_unsafe,
            ..
        } = &items[0]
        {
            assert_eq!(name, "greet");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "name");
            assert_eq!(params[0].param_type, "str");
            assert_eq!(return_type.as_deref(), Some("str"));
            assert!(!is_unsafe); // Python FFI is not raw-unsafe.
        } else {
            panic!("Expected ForeignItem::Function");
        }
    }

    #[test]
    fn e7_4_parse_python_class() {
        let stub = "class Vector(Base):\n    x: float\n    y: float\n    def length(self) -> float: ...\n";
        let items = PythonStubParser::new().parse(stub);
        assert_eq!(items.len(), 1);
        if let ForeignItem::Class {
            name,
            fields,
            methods,
            base_classes,
            ..
        } = &items[0]
        {
            assert_eq!(name, "Vector");
            assert_eq!(base_classes, &["Base"]);
            assert_eq!(fields.len(), 2);
            assert_eq!(methods.len(), 1);
        } else {
            panic!("Expected ForeignItem::Class");
        }
    }

    // ------------------------------------------------------------------
    // E7.5: Rust crate parsing
    // ------------------------------------------------------------------

    #[test]
    fn e7_5_parse_rust_pub_fn() {
        let src = "/// Add two numbers.\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
        let items = RustCrateParser::new().parse(src);
        assert_eq!(items.len(), 1);
        if let ForeignItem::Function {
            name,
            params,
            return_type,
            doc,
            ..
        } = &items[0]
        {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 2);
            assert_eq!(return_type.as_deref(), Some("i32"));
            assert_eq!(doc.as_deref(), Some("Add two numbers."));
        } else {
            panic!("Expected ForeignItem::Function");
        }
    }

    #[test]
    fn e7_5_parse_rust_struct_and_enum() {
        let src = "pub struct Point {\n    pub x: f64,\n    pub y: f64,\n}\n\npub enum Shape {\n    Circle,\n    Rect,\n}\n";
        let items = RustCrateParser::new().parse(src);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name(), "Point");
        assert_eq!(items[1].name(), "Shape");
    }

    #[test]
    fn e7_5_parse_rust_trait() {
        let src = "pub trait Drawable {\n    fn draw(&self) -> bool;\n    fn area(&self, scale: f64) -> f64;\n}\n";
        let items = RustCrateParser::new().parse(src);
        assert_eq!(items.len(), 1);
        if let ForeignItem::Trait { name, methods, .. } = &items[0] {
            assert_eq!(name, "Drawable");
            assert_eq!(methods.len(), 2);
        } else {
            panic!("Expected ForeignItem::Trait");
        }
    }

    // ------------------------------------------------------------------
    // E7.6: Binding customization
    // ------------------------------------------------------------------

    #[test]
    fn e7_6_skip_patterns() {
        let toml = BindgenToml::new().skip("__*").skip("internal_*");
        assert!(toml.should_skip("__init"));
        assert!(toml.should_skip("internal_helper"));
        assert!(!toml.should_skip("public_api"));
    }

    #[test]
    fn e7_6_type_overrides_and_rename() {
        let toml = BindgenToml::new()
            .type_override("GLint", "i32")
            .rename(RenameRule::Prefix("ffi_".to_string()));

        assert_eq!(toml.apply_type_override("GLint"), "i32");
        assert_eq!(toml.apply_type_override("float"), "float");
        assert_eq!(toml.apply_rename("create_window"), "ffi_create_window");
    }

    #[test]
    fn e7_6_rename_snake_case_and_pascal_case() {
        let snake = BindgenToml::new().rename(RenameRule::SnakeCase);
        assert_eq!(snake.apply_rename("MyStruct"), "my_struct");

        let pascal = BindgenToml::new().rename(RenameRule::PascalCase);
        assert_eq!(pascal.apply_rename("my_func"), "MyFunc");
    }

    // ------------------------------------------------------------------
    // E7.7: Doc preservation
    // ------------------------------------------------------------------

    #[test]
    fn e7_7_doc_preserved_in_generated_code() {
        let header = "// Compute the sum.\nint sum(int a, int b);\n";
        let items = CHeaderParser::new().parse(header);
        assert_eq!(items[0].doc(), Some("Compute the sum."));

        let generator = BindingGenerator::new(BindgenLanguage::C, None);
        let result = generator.generate(&items);
        assert!(result.bindings[0].fajar_source.contains("/// Compute the sum."));
        assert_eq!(result.stats.docs_preserved, 1);
    }

    // ------------------------------------------------------------------
    // E7.8: Incremental regeneration
    // ------------------------------------------------------------------

    #[test]
    fn e7_8_change_detector_unchanged() {
        let mut detector = ChangeDetector::new();
        let mut prev = HashMap::new();
        prev.insert("lib.h".to_string(), simple_hash("int foo();"));
        detector.load_previous(prev);

        detector.record("lib.h", "int foo();");
        assert!(!detector.has_changed("lib.h"));
        assert!(detector.unchanged_files().contains(&"lib.h".to_string()));
    }

    #[test]
    fn e7_8_change_detector_changed() {
        let mut detector = ChangeDetector::new();
        let mut prev = HashMap::new();
        prev.insert("lib.h".to_string(), simple_hash("int foo();"));
        detector.load_previous(prev);

        detector.record("lib.h", "int foo(int x);");
        assert!(detector.has_changed("lib.h"));
        assert!(detector.changed_files().contains(&"lib.h".to_string()));
    }

    #[test]
    fn e7_8_change_detector_new_file() {
        let mut detector = ChangeDetector::new();
        detector.load_previous(HashMap::new());
        detector.record("new.h", "void bar();");
        assert!(detector.has_changed("new.h"));
    }

    // ------------------------------------------------------------------
    // E7.9: Safety annotations
    // ------------------------------------------------------------------

    #[test]
    fn e7_9_unsafe_ffi_annotation() {
        let header = "void* malloc(size_t size);\n";
        let items = CHeaderParser::new().parse(header);
        let generator = BindingGenerator::new(BindgenLanguage::C, None);
        let result = generator.generate(&items);
        assert!(result.bindings[0].fajar_source.contains("@unsafe @ffi"));
    }

    #[test]
    fn e7_9_safe_wrapper_generation() {
        let header = "int compute(int x);\n";
        let items = CHeaderParser::new().parse(header);
        let toml = BindgenToml::new().with_safe_wrappers();
        let generator = BindingGenerator::new(BindgenLanguage::C, Some(toml));
        let result = generator.generate(&items);

        assert!(result.bindings[0].has_safe_wrapper);
        assert!(result.bindings[0].fajar_source.contains("@safe"));
        assert!(result.bindings[0].fajar_source.contains("compute_safe"));
        assert_eq!(result.stats.safe_wrappers, 1);
    }

    // ------------------------------------------------------------------
    // Integration: run_bindgen end-to-end
    // ------------------------------------------------------------------

    #[test]
    fn e7_10_end_to_end_c_bindgen() {
        let source = "\
typedef unsigned int uint32;\n\
// A 2D point.\n\
struct Point {\n\
    int x;\n\
    int y;\n\
};\n\
\n\
enum Direction {\n\
    UP,\n\
    DOWN = 1,\n\
    LEFT,\n\
    RIGHT\n\
};\n\
\n\
// Create a point.\n\
Point* create_point(int x, int y);\n\
void destroy_point(Point* p);\n\
";

        let config = BindgenConfig::new("geometry.h", BindgenLanguage::C, "geometry.fj");
        let result = run_bindgen(&config, source);

        assert_eq!(result.stats.type_aliases, 1);
        assert_eq!(result.stats.structs, 1);
        assert_eq!(result.stats.enums, 1);
        assert_eq!(result.stats.functions, 2);
        assert!(result.stats.docs_preserved >= 1);
        assert!(result.stats.total() >= 5);
    }

    #[test]
    fn e7_10_end_to_end_python_bindgen() {
        let source = "\
# Math utilities.\n\
def add(a: int, b: int) -> int: ...\n\
def mul(a: float, b: float) -> float: ...\n\
\n\
class Calculator:\n\
    value: float\n\
    def reset(self) -> None: ...\n\
";

        let config = BindgenConfig::new("math.pyi", BindgenLanguage::Python, "math.fj");
        let result = run_bindgen(&config, source);

        // 2 top-level functions + 1 class (with 1 method counted as function).
        assert_eq!(result.stats.functions, 3);
        assert_eq!(result.stats.classes, 1);
        assert!(result.stats.total() >= 4);
    }

    #[test]
    fn e7_10_emit_produces_valid_header() {
        let source = "int add(int a, int b);\n";
        let config = BindgenConfig::new("lib.h", BindgenLanguage::C, "lib.fj");
        let result = run_bindgen(&config, source);
        let generator = BindingGenerator::new(BindgenLanguage::C, None);
        let output = generator.emit(&result);

        assert!(output.contains("Auto-generated by `fj bindgen` from C source"));
        assert!(output.contains("extern fn add"));
    }

    #[test]
    fn e7_10_skip_pattern_filters_items() {
        let source = "int __internal_fn(void);\nint public_fn(int x);\n";
        let toml = BindgenToml::new().skip("__*");
        let config =
            BindgenConfig::new("lib.h", BindgenLanguage::C, "lib.fj").with_config(toml);
        let result = run_bindgen(&config, source);

        assert_eq!(result.stats.functions, 1);
        assert_eq!(result.stats.skipped, 1);
        assert_eq!(result.bindings[0].source_item_name, "public_fn");
    }

    #[test]
    fn e7_10_language_display() {
        assert_eq!(format!("{}", BindgenLanguage::C), "C");
        assert_eq!(format!("{}", BindgenLanguage::Cpp), "C++");
        assert_eq!(format!("{}", BindgenLanguage::Python), "Python");
        assert_eq!(format!("{}", BindgenLanguage::Rust), "Rust");
    }

    #[test]
    fn e7_10_bindgen_stats_display() {
        let stats = BindgenStats {
            functions: 5,
            structs: 2,
            enums: 1,
            type_aliases: 0,
            classes: 0,
            traits: 1,
            skipped: 3,
            docs_preserved: 4,
            safe_wrappers: 2,
        };
        let display = format!("{}", stats);
        assert!(display.contains("9 bindings"));
        assert!(display.contains("3 skipped"));
        assert!(display.contains("4 docs preserved"));
    }

    #[test]
    fn e7_10_foreign_item_name_and_doc_accessors() {
        let item = ForeignItem::Function {
            name: "foo".to_string(),
            params: vec![],
            return_type: None,
            doc: Some("A function.".to_string()),
            is_unsafe: false,
        };
        assert_eq!(item.name(), "foo");
        assert_eq!(item.doc(), Some("A function."));

        let item2 = ForeignItem::Struct {
            name: "Bar".to_string(),
            fields: vec![],
            doc: None,
        };
        assert_eq!(item2.name(), "Bar");
        assert!(item2.doc().is_none());
    }
}
