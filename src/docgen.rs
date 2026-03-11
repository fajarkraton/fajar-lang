//! Documentation generator for Fajar Lang.
//!
//! Parses `///` doc comments from AST and generates HTML documentation.
//! Entry point: [`generate_docs`] takes a `Program` and produces an HTML string.

use crate::parser::ast::{
    ConstDef, EnumDef, FnDef, ImplBlock, Item, Program, StructDef, TraitDef, TypeExpr, UnionDef,
};

/// A documented item extracted from the AST.
#[derive(Debug)]
pub struct DocItem {
    /// Item kind (function, struct, enum, trait, const, impl, union).
    pub kind: DocItemKind,
    /// Item name.
    pub name: String,
    /// Doc comment content (may contain markdown).
    pub doc: String,
    /// Signature string (e.g., `fn add(a: i64, b: i64) -> i64`).
    pub signature: String,
    /// Whether the item is public.
    pub is_pub: bool,
}

/// The kind of documented item.
#[derive(Debug, Clone, PartialEq)]
pub enum DocItemKind {
    /// A function.
    Function,
    /// A struct.
    Struct,
    /// An enum.
    Enum,
    /// A trait.
    Trait,
    /// A constant.
    Const,
    /// An impl block.
    Impl,
    /// A union.
    Union,
}

impl DocItemKind {
    /// Returns a CSS class name for this kind.
    pub fn css_class(&self) -> &'static str {
        match self {
            DocItemKind::Function => "fn",
            DocItemKind::Struct => "struct",
            DocItemKind::Enum => "enum",
            DocItemKind::Trait => "trait",
            DocItemKind::Const => "const",
            DocItemKind::Impl => "impl",
            DocItemKind::Union => "union",
        }
    }

    /// Returns a display label for this kind.
    pub fn label(&self) -> &'static str {
        match self {
            DocItemKind::Function => "Function",
            DocItemKind::Struct => "Struct",
            DocItemKind::Enum => "Enum",
            DocItemKind::Trait => "Trait",
            DocItemKind::Const => "Constant",
            DocItemKind::Impl => "Implementation",
            DocItemKind::Union => "Union",
        }
    }
}

/// Extracts all documented items from a program's AST.
pub fn extract_doc_items(program: &Program) -> Vec<DocItem> {
    let mut items = Vec::new();

    for item in &program.items {
        match item {
            Item::FnDef(fndef) => {
                if let Some(ref doc) = fndef.doc_comment {
                    items.push(DocItem {
                        kind: DocItemKind::Function,
                        name: fndef.name.clone(),
                        doc: doc.clone(),
                        signature: fn_signature(fndef),
                        is_pub: fndef.is_pub,
                    });
                }
            }
            Item::StructDef(sd) => {
                if let Some(ref doc) = sd.doc_comment {
                    items.push(DocItem {
                        kind: DocItemKind::Struct,
                        name: sd.name.clone(),
                        doc: doc.clone(),
                        signature: struct_signature(sd),
                        is_pub: sd.is_pub,
                    });
                }
            }
            Item::EnumDef(ed) => {
                if let Some(ref doc) = ed.doc_comment {
                    items.push(DocItem {
                        kind: DocItemKind::Enum,
                        name: ed.name.clone(),
                        doc: doc.clone(),
                        signature: enum_signature(ed),
                        is_pub: ed.is_pub,
                    });
                }
            }
            Item::TraitDef(td) => {
                if let Some(ref doc) = td.doc_comment {
                    items.push(DocItem {
                        kind: DocItemKind::Trait,
                        name: td.name.clone(),
                        doc: doc.clone(),
                        signature: trait_signature(td),
                        is_pub: td.is_pub,
                    });
                }
            }
            Item::ConstDef(cd) => {
                if let Some(ref doc) = cd.doc_comment {
                    items.push(DocItem {
                        kind: DocItemKind::Const,
                        name: cd.name.clone(),
                        doc: doc.clone(),
                        signature: const_signature(cd),
                        is_pub: cd.is_pub,
                    });
                }
            }
            Item::ImplBlock(ib) => {
                if let Some(ref doc) = ib.doc_comment {
                    let name = if let Some(ref trait_name) = ib.trait_name {
                        format!("{} for {}", trait_name, ib.target_type)
                    } else {
                        ib.target_type.clone()
                    };
                    items.push(DocItem {
                        kind: DocItemKind::Impl,
                        name,
                        doc: doc.clone(),
                        signature: impl_signature(ib),
                        is_pub: true,
                    });
                }
            }
            Item::UnionDef(ud) => {
                if let Some(ref doc) = ud.doc_comment {
                    items.push(DocItem {
                        kind: DocItemKind::Union,
                        name: ud.name.clone(),
                        doc: doc.clone(),
                        signature: union_signature(ud),
                        is_pub: ud.is_pub,
                    });
                }
            }
            _ => {}
        }
    }

    items
}

/// Extracts doc test code blocks from doc comments.
///
/// Returns a vector of (item_name, code_block) pairs.
pub fn extract_doc_tests(program: &Program) -> Vec<(String, String)> {
    let mut tests = Vec::new();
    let items = extract_doc_items(program);

    for item in &items {
        let mut in_code_block = false;
        let mut code = String::new();

        for line in item.doc.lines() {
            let trimmed = line.trim();
            if trimmed == "```" || trimmed == "```fajar" || trimmed == "```fj" {
                if in_code_block {
                    // End of code block
                    if !code.trim().is_empty() {
                        tests.push((item.name.clone(), code.trim().to_string()));
                    }
                    code.clear();
                    in_code_block = false;
                } else {
                    in_code_block = true;
                }
            } else if in_code_block {
                code.push_str(line);
                code.push('\n');
            }
        }
    }

    tests
}

/// Formats a type expression as a string.
fn type_to_string(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Simple { name, .. } => name.clone(),
        TypeExpr::Generic { name, args, .. } => {
            let args_str: Vec<String> = args.iter().map(type_to_string).collect();
            format!("{}<{}>", name, args_str.join(", "))
        }
        TypeExpr::Array { element, size, .. } => {
            format!("[{}; {}]", type_to_string(element), size)
        }
        TypeExpr::Slice { element, .. } => {
            format!("[{}]", type_to_string(element))
        }
        TypeExpr::Tuple { elements, .. } => {
            let parts: Vec<String> = elements.iter().map(type_to_string).collect();
            format!("({})", parts.join(", "))
        }
        TypeExpr::Reference {
            lifetime,
            mutable,
            inner,
            ..
        } => {
            let lt_str = lifetime
                .as_ref()
                .map(|lt| format!("'{lt} "))
                .unwrap_or_default();
            if *mutable {
                format!("&{lt_str}mut {}", type_to_string(inner))
            } else {
                format!("&{lt_str}{}", type_to_string(inner))
            }
        }
        TypeExpr::Pointer { mutable, inner, .. } => {
            if *mutable {
                format!("*mut {}", type_to_string(inner))
            } else {
                format!("*const {}", type_to_string(inner))
            }
        }
        TypeExpr::Fn {
            params,
            return_type,
            ..
        } => {
            let params_str: Vec<String> = params.iter().map(type_to_string).collect();
            format!(
                "fn({}) -> {}",
                params_str.join(", "),
                type_to_string(return_type)
            )
        }
        TypeExpr::Tensor {
            element_type, dims, ..
        } => {
            let dims_str: Vec<String> = dims
                .iter()
                .map(|d| match d {
                    Some(n) => n.to_string(),
                    None => "*".to_string(),
                })
                .collect();
            format!(
                "Tensor<{}>[{}]",
                type_to_string(element_type),
                dims_str.join(", ")
            )
        }
        TypeExpr::DynTrait { trait_name, .. } => format!("dyn {trait_name}"),
        _ => "...".to_string(),
    }
}

/// Generates a function signature string.
fn fn_signature(f: &FnDef) -> String {
    let vis = if f.is_pub { "pub " } else { "" };
    let async_kw = if f.is_async { "async " } else { "" };
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, type_to_string(&p.ty)))
        .collect();
    let ret = match &f.return_type {
        Some(ty) => format!(" -> {}", type_to_string(ty)),
        None => String::new(),
    };
    let generics = if f.generic_params.is_empty() {
        String::new()
    } else {
        let gp: Vec<String> = f.generic_params.iter().map(|g| g.name.clone()).collect();
        format!("<{}>", gp.join(", "))
    };
    format!(
        "{}{}fn {}{}({}){}",
        vis,
        async_kw,
        f.name,
        generics,
        params.join(", "),
        ret
    )
}

/// Generates a struct signature string.
fn struct_signature(s: &StructDef) -> String {
    let vis = if s.is_pub { "pub " } else { "" };
    let generics = if s.generic_params.is_empty() {
        String::new()
    } else {
        let gp: Vec<String> = s.generic_params.iter().map(|g| g.name.clone()).collect();
        format!("<{}>", gp.join(", "))
    };
    let fields: Vec<String> = s
        .fields
        .iter()
        .map(|f| format!("{}: {}", f.name, type_to_string(&f.ty)))
        .collect();
    format!(
        "{}struct {}{} {{ {} }}",
        vis,
        s.name,
        generics,
        fields.join(", ")
    )
}

/// Generates an enum signature string.
fn enum_signature(e: &EnumDef) -> String {
    let vis = if e.is_pub { "pub " } else { "" };
    let generics = if e.generic_params.is_empty() {
        String::new()
    } else {
        let gp: Vec<String> = e.generic_params.iter().map(|g| g.name.clone()).collect();
        format!("<{}>", gp.join(", "))
    };
    let variants: Vec<String> = e
        .variants
        .iter()
        .map(|v| {
            if v.fields.is_empty() {
                v.name.clone()
            } else {
                let fields: Vec<String> = v.fields.iter().map(type_to_string).collect();
                format!("{}({})", v.name, fields.join(", "))
            }
        })
        .collect();
    format!(
        "{}enum {}{} {{ {} }}",
        vis,
        e.name,
        generics,
        variants.join(", ")
    )
}

/// Generates a trait signature string.
fn trait_signature(t: &TraitDef) -> String {
    let vis = if t.is_pub { "pub " } else { "" };
    let generics = if t.generic_params.is_empty() {
        String::new()
    } else {
        let gp: Vec<String> = t.generic_params.iter().map(|g| g.name.clone()).collect();
        format!("<{}>", gp.join(", "))
    };
    let methods: Vec<String> = t
        .methods
        .iter()
        .map(|m| format!("fn {}(..)", m.name))
        .collect();
    format!(
        "{}trait {}{} {{ {} }}",
        vis,
        t.name,
        generics,
        methods.join("; ")
    )
}

/// Generates a const signature string.
fn const_signature(c: &ConstDef) -> String {
    let vis = if c.is_pub { "pub " } else { "" };
    format!("{}const {}: {}", vis, c.name, type_to_string(&c.ty))
}

/// Generates an impl block signature string.
fn impl_signature(ib: &ImplBlock) -> String {
    if let Some(ref trait_name) = ib.trait_name {
        format!("impl {} for {}", trait_name, ib.target_type)
    } else {
        format!("impl {}", ib.target_type)
    }
}

/// Generates a union signature string.
fn union_signature(u: &UnionDef) -> String {
    let vis = if u.is_pub { "pub " } else { "" };
    let fields: Vec<String> = u
        .fields
        .iter()
        .map(|f| format!("{}: {}", f.name, type_to_string(&f.ty)))
        .collect();
    format!("{}union {} {{ {} }}", vis, u.name, fields.join(", "))
}

/// Renders markdown-like doc comment content to HTML.
///
/// Supports: headings (#), code blocks (```), inline code (`), bold (**), italic (*),
/// unordered lists (- item), and cross-references ([`TypeName`]).
pub fn render_markdown(input: &str) -> String {
    let mut html = String::new();
    let mut in_code_block = false;
    let mut in_list = false;
    let lines: Vec<&str> = input.lines().collect();

    for line in &lines {
        let trimmed = line.trim();

        // Code blocks
        if trimmed.starts_with("```") {
            if in_code_block {
                html.push_str("</code></pre>\n");
                in_code_block = false;
            } else {
                if in_list {
                    html.push_str("</ul>\n");
                    in_list = false;
                }
                html.push_str("<pre><code>");
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            html.push_str(&html_escape(trimmed));
            html.push('\n');
            continue;
        }

        // Empty line
        if trimmed.is_empty() {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            continue;
        }

        // Headings
        if let Some(heading) = trimmed.strip_prefix("## ") {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            html.push_str(&format!("<h4>{}</h4>\n", render_inline(heading)));
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix("# ") {
            if in_list {
                html.push_str("</ul>\n");
                in_list = false;
            }
            html.push_str(&format!("<h3>{}</h3>\n", render_inline(heading)));
            continue;
        }

        // Unordered list
        if let Some(item_text) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            if !in_list {
                html.push_str("<ul>\n");
                in_list = true;
            }
            html.push_str(&format!("<li>{}</li>\n", render_inline(item_text)));
            continue;
        }

        // Paragraph
        if in_list {
            html.push_str("</ul>\n");
            in_list = false;
        }
        html.push_str(&format!("<p>{}</p>\n", render_inline(trimmed)));
    }

    if in_code_block {
        html.push_str("</code></pre>\n");
    }
    if in_list {
        html.push_str("</ul>\n");
    }

    html
}

/// Renders inline markdown: backtick code, bold, italic, cross-references.
fn render_inline(input: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Cross-reference: [`TypeName`]
        if i + 2 < chars.len() && chars[i] == '[' && chars[i + 1] == '`' {
            if let Some(end) = find_pattern(&chars, i + 2, '`', ']') {
                let name: String = chars[i + 2..end].iter().collect();
                result.push_str(&format!(
                    "<a href=\"#{name}\" class=\"cross-ref\"><code>{name}</code></a>"
                ));
                i = end + 2; // skip `]
                continue;
            }
        }

        // Inline code: `code`
        if chars[i] == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '`') {
                let code: String = chars[i + 1..i + 1 + end].iter().collect();
                result.push_str(&format!("<code>{}</code>", html_escape(&code)));
                i = i + 2 + end;
                continue;
            }
        }

        // Bold: **text**
        if i + 3 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some(end) = find_double_star(&chars, i + 2) {
                let text: String = chars[i + 2..end].iter().collect();
                result.push_str(&format!("<strong>{}</strong>", html_escape(&text)));
                i = end + 2;
                continue;
            }
        }

        // Italic: *text*
        if chars[i] == '*' && (i + 1 < chars.len() && chars[i + 1] != '*') {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '*') {
                let text: String = chars[i + 1..i + 1 + end].iter().collect();
                result.push_str(&format!("<em>{}</em>", html_escape(&text)));
                i = i + 2 + end;
                continue;
            }
        }

        // Regular character
        match chars[i] {
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '&' => result.push_str("&amp;"),
            c => result.push(c),
        }
        i += 1;
    }

    result
}

/// Finds the position of `end_char` followed by `after_char` starting from `start`.
fn find_pattern(chars: &[char], start: usize, end_char: char, after_char: char) -> Option<usize> {
    (start..chars.len().saturating_sub(1))
        .find(|&i| chars[i] == end_char && chars[i + 1] == after_char)
}

/// Finds the position of `**` starting from `start`.
fn find_double_star(chars: &[char], start: usize) -> Option<usize> {
    (start..chars.len().saturating_sub(1)).find(|&i| chars[i] == '*' && chars[i + 1] == '*')
}

/// Escapes HTML special characters.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Generates complete HTML documentation for a program.
pub fn generate_html(module_name: &str, items: &[DocItem]) -> String {
    let mut html = String::new();

    // HTML header
    html.push_str(&format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{module_name} — Fajar Lang Documentation</title>
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 900px; margin: 0 auto; padding: 2rem; line-height: 1.6; color: #333; background: #fafafa; }}
h1 {{ color: #1a1a2e; border-bottom: 2px solid #e94560; padding-bottom: 0.5rem; }}
h2 {{ color: #16213e; margin-top: 2rem; }}
h3 {{ color: #0f3460; }}
.item {{ background: #fff; border: 1px solid #e0e0e0; border-radius: 8px; padding: 1.5rem; margin: 1rem 0; }}
.item-header {{ display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.5rem; }}
.badge {{ display: inline-block; padding: 0.15rem 0.5rem; border-radius: 4px; font-size: 0.75rem; font-weight: bold; text-transform: uppercase; color: #fff; }}
.badge.fn {{ background: #4361ee; }}
.badge.struct {{ background: #3a86ff; }}
.badge.enum {{ background: #8338ec; }}
.badge.trait {{ background: #ff006e; }}
.badge.const {{ background: #fb5607; }}
.badge.impl {{ background: #606c38; }}
.badge.union {{ background: #bc6c25; }}
.signature {{ font-family: 'JetBrains Mono', 'Fira Code', monospace; background: #f4f4f9; padding: 0.75rem 1rem; border-radius: 4px; border-left: 3px solid #4361ee; margin: 0.5rem 0; overflow-x: auto; white-space: pre-wrap; }}
.doc {{ margin-top: 0.75rem; }}
.doc code {{ background: #f0f0f5; padding: 0.1rem 0.3rem; border-radius: 3px; font-family: 'JetBrains Mono', 'Fira Code', monospace; font-size: 0.9em; }}
.doc pre {{ background: #2d2d2d; color: #f8f8f2; padding: 1rem; border-radius: 6px; overflow-x: auto; }}
.doc pre code {{ background: none; color: inherit; padding: 0; }}
.cross-ref {{ color: #4361ee; text-decoration: none; }}
.cross-ref:hover {{ text-decoration: underline; }}
.index {{ background: #fff; border: 1px solid #e0e0e0; border-radius: 8px; padding: 1.5rem; margin: 1rem 0; }}
.index ul {{ column-count: 2; }}
.index li {{ margin: 0.25rem 0; }}
.index a {{ color: #16213e; text-decoration: none; }}
.index a:hover {{ color: #4361ee; text-decoration: underline; }}
.pub-badge {{ color: #2d6a4f; font-size: 0.8em; font-weight: bold; }}
footer {{ margin-top: 3rem; padding-top: 1rem; border-top: 1px solid #e0e0e0; color: #888; font-size: 0.85rem; text-align: center; }}
</style>
</head>
<body>
<h1>{module_name}</h1>
"#
    ));

    // Module index
    html.push_str("<div class=\"index\">\n<h2>Index</h2>\n");

    let kinds = [
        (DocItemKind::Function, "Functions"),
        (DocItemKind::Struct, "Structs"),
        (DocItemKind::Enum, "Enums"),
        (DocItemKind::Trait, "Traits"),
        (DocItemKind::Const, "Constants"),
        (DocItemKind::Union, "Unions"),
        (DocItemKind::Impl, "Implementations"),
    ];

    for (kind, label) in &kinds {
        let matching: Vec<&DocItem> = items.iter().filter(|i| i.kind == *kind).collect();
        if !matching.is_empty() {
            html.push_str(&format!("<h3>{label}</h3>\n<ul>\n"));
            for item in &matching {
                let pub_badge = if item.is_pub {
                    "<span class=\"pub-badge\">pub </span>"
                } else {
                    ""
                };
                html.push_str(&format!(
                    "<li>{pub_badge}<a href=\"#{}\">{}</a></li>\n",
                    item.name, item.name
                ));
            }
            html.push_str("</ul>\n");
        }
    }
    html.push_str("</div>\n");

    // Item details
    for item in items {
        html.push_str(&format!("<div class=\"item\" id=\"{}\">\n", item.name));
        html.push_str("<div class=\"item-header\">\n");
        html.push_str(&format!(
            "<span class=\"badge {}\">{}</span>\n",
            item.kind.css_class(),
            item.kind.label()
        ));
        html.push_str(&format!("<h2>{}</h2>\n", item.name));
        html.push_str("</div>\n");

        html.push_str(&format!(
            "<div class=\"signature\">{}</div>\n",
            html_escape(&item.signature)
        ));

        html.push_str("<div class=\"doc\">\n");
        html.push_str(&render_markdown(&item.doc));
        html.push_str("</div>\n");

        html.push_str("</div>\n");
    }

    // Footer
    html.push_str(
        "<footer>Generated by <code>fj doc</code> — Fajar Lang Documentation Generator</footer>\n",
    );
    html.push_str("</body>\n</html>\n");

    html
}

/// Generates HTML documentation for a program.
///
/// Returns the HTML string, or an empty string if no documented items are found.
pub fn generate_docs(module_name: &str, program: &Program) -> String {
    let items = extract_doc_items(program);
    if items.is_empty() {
        return String::new();
    }
    generate_html(module_name, &items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_markdown_heading() {
        let result = render_markdown("# Hello");
        assert!(result.contains("<h3>Hello</h3>"));
    }

    #[test]
    fn render_markdown_code_block() {
        let result = render_markdown("```\nlet x = 42\n```");
        assert!(result.contains("<pre><code>"));
        assert!(result.contains("let x = 42"));
    }

    #[test]
    fn render_markdown_inline_code() {
        let result = render_markdown("Use `foo()` here");
        assert!(result.contains("<code>foo()</code>"));
    }

    #[test]
    fn render_markdown_bold() {
        let result = render_markdown("This is **bold** text");
        assert!(result.contains("<strong>bold</strong>"));
    }

    #[test]
    fn render_markdown_list() {
        let result = render_markdown("- item one\n- item two");
        assert!(result.contains("<ul>"));
        assert!(result.contains("<li>item one</li>"));
        assert!(result.contains("<li>item two</li>"));
    }

    #[test]
    fn render_markdown_cross_reference() {
        let result = render_markdown("See [`Point`] for details");
        assert!(result.contains("<a href=\"#Point\""));
        assert!(result.contains("cross-ref"));
    }

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(
            html_escape("<div>&\"test\"</div>"),
            "&lt;div&gt;&amp;&quot;test&quot;&lt;/div&gt;"
        );
    }

    #[test]
    fn generate_html_produces_valid_structure() {
        let items = vec![DocItem {
            kind: DocItemKind::Function,
            name: "add".to_string(),
            doc: "Adds two numbers.".to_string(),
            signature: "fn add(a: i64, b: i64) -> i64".to_string(),
            is_pub: true,
        }];
        let html = generate_html("test_module", &items);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("test_module"));
        assert!(html.contains("add"));
        assert!(html.contains("Adds two numbers."));
        assert!(html.contains("fn add(a: i64, b: i64) -&gt; i64"));
    }
}
