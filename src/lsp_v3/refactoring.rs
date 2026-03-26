//! LSP v3 Refactoring — rename, extract, inline, code actions.
//!
//! Provides IDE refactoring operations including rename symbol,
//! extract function/variable, inline, import organization, and
//! structural code transformations.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// L2.1: Rename Symbol (cross-file)
// ═══════════════════════════════════════════════════════════════════════

/// A text edit at a specific location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// Start line (0-based).
    pub start_line: u32,
    /// Start character.
    pub start_char: u32,
    /// End line.
    pub end_line: u32,
    /// End character.
    pub end_char: u32,
    /// New text.
    pub new_text: String,
}

/// A workspace edit (edits across multiple files).
#[derive(Debug, Clone, Default)]
pub struct WorkspaceEdit {
    /// Map from file URI to list of edits.
    pub changes: HashMap<String, Vec<TextEdit>>,
}

impl WorkspaceEdit {
    /// Creates a new empty workspace edit.
    pub fn new() -> Self {
        Self { changes: HashMap::new() }
    }

    /// Adds an edit for a file.
    pub fn add_edit(&mut self, uri: &str, edit: TextEdit) {
        self.changes
            .entry(uri.to_string())
            .or_default()
            .push(edit);
    }

    /// Total number of edits across all files.
    pub fn edit_count(&self) -> usize {
        self.changes.values().map(|v| v.len()).sum()
    }

    /// Number of files affected.
    pub fn file_count(&self) -> usize {
        self.changes.len()
    }
}

/// Rename preparation result.
#[derive(Debug, Clone)]
pub struct PrepareRenameResult {
    /// Range of the symbol to rename.
    pub range_start_line: u32,
    pub range_start_char: u32,
    pub range_end_line: u32,
    pub range_end_char: u32,
    /// Current symbol name (placeholder for rename input).
    pub placeholder: String,
}

/// Validates a new name for a rename operation.
pub fn validate_rename(new_name: &str) -> Result<(), String> {
    if new_name.is_empty() {
        return Err("name cannot be empty".to_string());
    }
    if new_name.starts_with(|c: char| c.is_ascii_digit()) {
        return Err("name cannot start with a digit".to_string());
    }
    let valid = new_name.chars().all(|c| c.is_alphanumeric() || c == '_');
    if !valid {
        return Err("name must contain only alphanumeric characters and underscores".to_string());
    }
    // Check against keywords
    let keywords = [
        "fn", "let", "mut", "const", "struct", "enum", "impl", "trait", "type",
        "if", "else", "match", "while", "for", "in", "return", "break", "continue",
        "loop", "use", "mod", "pub", "extern", "as", "true", "false", "null",
    ];
    if keywords.contains(&new_name) {
        return Err(format!("'{new_name}' is a reserved keyword"));
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// L2.2-L2.3: Extract Function / Extract Variable
// ═══════════════════════════════════════════════════════════════════════

/// Extract function parameters.
#[derive(Debug, Clone)]
pub struct ExtractFunctionParams {
    /// Selected code range.
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    /// Proposed function name.
    pub function_name: String,
    /// Variables read from outer scope (become parameters).
    pub captured_reads: Vec<CapturedVariable>,
    /// Variables written (become return value or &mut params).
    pub captured_writes: Vec<CapturedVariable>,
}

/// A variable captured by an extracted function.
#[derive(Debug, Clone)]
pub struct CapturedVariable {
    /// Variable name.
    pub name: String,
    /// Variable type.
    pub var_type: String,
    /// Whether the variable is mutably captured.
    pub is_mutable: bool,
}

/// Generates the extracted function and call site.
pub fn generate_extracted_function(
    source_code: &str,
    params: &ExtractFunctionParams,
) -> (String, String) {
    // Build parameter list
    let param_list: Vec<String> = params.captured_reads.iter().map(|v| {
        if v.is_mutable {
            format!("{}: &mut {}", v.name, v.var_type)
        } else {
            format!("{}: {}", v.name, v.var_type)
        }
    }).collect();

    // Build return type
    let return_type = if params.captured_writes.is_empty() {
        String::new()
    } else if params.captured_writes.len() == 1 {
        format!(" -> {}", params.captured_writes[0].var_type)
    } else {
        let types: Vec<&str> = params.captured_writes.iter().map(|v| v.var_type.as_str()).collect();
        format!(" -> ({})", types.join(", "))
    };

    // Extract the selected code
    let lines: Vec<&str> = source_code.lines().collect();
    let mut extracted = String::new();
    for i in params.start_line..=params.end_line {
        if (i as usize) < lines.len() {
            extracted.push_str("    ");
            extracted.push_str(lines[i as usize].trim());
            extracted.push('\n');
        }
    }

    // Return value
    if !params.captured_writes.is_empty() {
        let names: Vec<&str> = params.captured_writes.iter().map(|v| v.name.as_str()).collect();
        if names.len() == 1 {
            extracted.push_str(&format!("    {}\n", names[0]));
        } else {
            extracted.push_str(&format!("    ({})\n", names.join(", ")));
        }
    }

    // Function definition
    let func_def = format!(
        "fn {}({}){} {{\n{}}}\n",
        params.function_name,
        param_list.join(", "),
        return_type,
        extracted,
    );

    // Call site
    let args: Vec<&str> = params.captured_reads.iter().map(|v| v.name.as_str()).collect();
    let call = if params.captured_writes.is_empty() {
        format!("{}({})", params.function_name, args.join(", "))
    } else if params.captured_writes.len() == 1 {
        format!("let {} = {}({})", params.captured_writes[0].name, params.function_name, args.join(", "))
    } else {
        let names: Vec<&str> = params.captured_writes.iter().map(|v| v.name.as_str()).collect();
        format!("let ({}) = {}({})", names.join(", "), params.function_name, args.join(", "))
    };

    (func_def, call)
}

/// Extract variable: replace expression with a named variable.
#[derive(Debug, Clone)]
pub struct ExtractVariableParams {
    /// Expression range.
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    /// Proposed variable name.
    pub variable_name: String,
    /// Inferred type (if available).
    pub inferred_type: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// L2.4-L2.5: Inline Variable / Inline Function
// ═══════════════════════════════════════════════════════════════════════

/// Inline variable: replace all uses of a variable with its initializer.
#[derive(Debug, Clone)]
pub struct InlineVariableResult {
    /// The variable's initializer expression.
    pub initializer: String,
    /// All locations where the variable is used.
    pub use_sites: Vec<TextEdit>,
    /// The declaration to remove.
    pub declaration_removal: TextEdit,
}

/// Inline function: replace call site with function body.
#[derive(Debug, Clone)]
pub struct InlineFunctionResult {
    /// The inlined code (function body with args substituted).
    pub inlined_code: String,
    /// Edit to replace the call.
    pub call_replacement: TextEdit,
}

// ═══════════════════════════════════════════════════════════════════════
// L2.6-L2.8: Code Transformations
// ═══════════════════════════════════════════════════════════════════════

/// Code action kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeActionKind {
    /// Quick fix for a diagnostic.
    QuickFix,
    /// Refactoring action.
    Refactor,
    /// Refactor → extract.
    RefactorExtract,
    /// Refactor → inline.
    RefactorInline,
    /// Refactor → rewrite.
    RefactorRewrite,
    /// Source action (format, organize imports).
    Source,
    /// Source → organize imports.
    SourceOrganizeImports,
}

impl fmt::Display for CodeActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QuickFix => write!(f, "quickfix"),
            Self::Refactor => write!(f, "refactor"),
            Self::RefactorExtract => write!(f, "refactor.extract"),
            Self::RefactorInline => write!(f, "refactor.inline"),
            Self::RefactorRewrite => write!(f, "refactor.rewrite"),
            Self::Source => write!(f, "source"),
            Self::SourceOrganizeImports => write!(f, "source.organizeImports"),
        }
    }
}

/// A code action offered to the user.
#[derive(Debug, Clone)]
pub struct CodeAction {
    /// Action title.
    pub title: String,
    /// Action kind.
    pub kind: CodeActionKind,
    /// Workspace edits to apply.
    pub edit: Option<WorkspaceEdit>,
    /// Whether this is the preferred action.
    pub is_preferred: bool,
    /// Diagnostic this action fixes (if QuickFix).
    pub diagnostic_id: Option<String>,
}

/// Convert if-else chain to match expression.
pub fn if_else_to_match(condition_var: &str, branches: &[(String, String)], else_body: &str) -> String {
    let mut result = format!("match {condition_var} {{\n");
    for (pattern, body) in branches {
        result.push_str(&format!("    {pattern} => {body},\n"));
    }
    result.push_str(&format!("    _ => {else_body},\n"));
    result.push_str("}\n");
    result
}

/// Convert match to if-else chain.
pub fn match_to_if_else(match_var: &str, arms: &[(String, String)]) -> String {
    let mut result = String::new();
    for (i, (pattern, body)) in arms.iter().enumerate() {
        if pattern == "_" {
            result.push_str(&format!("else {{ {body} }}"));
        } else if i == 0 {
            result.push_str(&format!("if {match_var} == {pattern} {{ {body} }}\n"));
        } else {
            result.push_str(&format!("else if {match_var} == {pattern} {{ {body} }}\n"));
        }
    }
    result
}

// ═══════════════════════════════════════════════════════════════════════
// L2.9-L2.10: Generate Trait Impl & Constructor
// ═══════════════════════════════════════════════════════════════════════

/// Generates stub implementations for a trait.
pub fn generate_trait_impl(
    struct_name: &str,
    trait_name: &str,
    methods: &[TraitMethodStub],
) -> String {
    let mut result = format!("impl {trait_name} for {struct_name} {{\n");
    for method in methods {
        let params: Vec<String> = method.params.iter()
            .map(|(name, ty)| format!("{name}: {ty}"))
            .collect();
        let ret = method.return_type.as_deref().map(|t| format!(" -> {t}")).unwrap_or_default();
        result.push_str(&format!("    fn {}({}){} {{\n", method.name, params.join(", "), ret));
        result.push_str("        todo!()\n");
        result.push_str("    }\n\n");
    }
    result.push_str("}\n");
    result
}

/// A trait method stub.
#[derive(Debug, Clone)]
pub struct TraitMethodStub {
    /// Method name.
    pub name: String,
    /// Parameters (name, type).
    pub params: Vec<(String, String)>,
    /// Return type.
    pub return_type: Option<String>,
}

/// Generates a constructor for a struct.
pub fn generate_constructor(struct_name: &str, fields: &[(String, String)]) -> String {
    let params: Vec<String> = fields.iter()
        .map(|(name, ty)| format!("{name}: {ty}"))
        .collect();
    let assignments: Vec<String> = fields.iter()
        .map(|(name, _)| format!("        {name}: {name}"))
        .collect();
    format!(
        "    fn new({}) -> {struct_name} {{\n        {struct_name} {{\n{}\n        }}\n    }}\n",
        params.join(", "),
        assignments.join(",\n"),
    )
}

// ═══════════════════════════════════════════════════════════════════════
// L2.11-L2.13: Type Annotation, Imports
// ═══════════════════════════════════════════════════════════════════════

/// Add type annotation to a variable.
pub fn add_type_annotation(var_name: &str, inferred_type: &str) -> String {
    format!("{var_name}: {inferred_type}")
}

/// Organize imports: sort and group.
pub fn organize_imports(imports: &[String]) -> Vec<String> {
    let mut std_imports = Vec::new();
    let mut external_imports = Vec::new();
    let mut local_imports = Vec::new();

    for imp in imports {
        if imp.starts_with("use std::") {
            std_imports.push(imp.clone());
        } else if imp.starts_with("use crate::") || imp.starts_with("use super::") {
            local_imports.push(imp.clone());
        } else {
            external_imports.push(imp.clone());
        }
    }

    std_imports.sort();
    external_imports.sort();
    local_imports.sort();

    let mut result = Vec::new();
    if !std_imports.is_empty() {
        result.extend(std_imports);
        result.push(String::new()); // blank line separator
    }
    if !external_imports.is_empty() {
        result.extend(external_imports);
        result.push(String::new());
    }
    result.extend(local_imports);
    result
}

// ═══════════════════════════════════════════════════════════════════════
// L2.14-L2.18: Wrap, Convert, Generate Doc
// ═══════════════════════════════════════════════════════════════════════

/// Wrap expression in Some() or Ok().
pub fn wrap_in_some(expr: &str) -> String { format!("Some({expr})") }
pub fn wrap_in_ok(expr: &str) -> String { format!("Ok({expr})") }
pub fn wrap_in_err(expr: &str) -> String { format!("Err({expr})") }
pub fn add_question_mark(expr: &str) -> String { format!("{expr}?") }

/// Convert string literal to f-string.
pub fn to_fstring(literal: &str) -> String {
    format!("f{literal}")
}

/// Generate documentation comment for a function.
pub fn generate_doc_comment(
    name: &str,
    params: &[(String, String)],
    return_type: Option<&str>,
) -> String {
    let mut doc = format!("/// {name}\n");
    doc.push_str("///\n");
    for (pname, ptype) in params {
        doc.push_str(&format!("/// * `{pname}` — {ptype}\n"));
    }
    if let Some(ret) = return_type {
        doc.push_str("///\n");
        doc.push_str(&format!("/// Returns: {ret}\n"));
    }
    doc
}

// ═══════════════════════════════════════════════════════════════════════
// L2.19-L2.20: Move to File, Refactoring Preview
// ═══════════════════════════════════════════════════════════════════════

/// Move item to a new file.
#[derive(Debug, Clone)]
pub struct MoveToFileResult {
    /// New file path.
    pub new_file_path: String,
    /// Content for the new file.
    pub new_file_content: String,
    /// Edit to remove from original file.
    pub removal_edit: TextEdit,
    /// Use statement to add in original file.
    pub use_statement: String,
}

/// Preview of a refactoring operation (diff).
#[derive(Debug, Clone)]
pub struct RefactoringPreview {
    /// Description.
    pub title: String,
    /// Files affected.
    pub affected_files: Vec<String>,
    /// Before/after diffs per file.
    pub diffs: Vec<FileDiff>,
}

/// A before/after diff for a single file.
#[derive(Debug, Clone)]
pub struct FileDiff {
    /// File URI.
    pub uri: String,
    /// Hunks of changes.
    pub hunks: Vec<DiffHunk>,
}

/// A diff hunk.
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// Start line in old file.
    pub old_start: u32,
    /// Number of lines in old file.
    pub old_count: u32,
    /// Start line in new file.
    pub new_start: u32,
    /// Number of lines in new file.
    pub new_count: u32,
    /// Diff lines (prefixed with +/-/space).
    pub lines: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // L2.1: Rename validation
    #[test]
    fn l2_1_validate_rename_valid() {
        assert!(validate_rename("new_name").is_ok());
        assert!(validate_rename("x").is_ok());
        assert!(validate_rename("myVar123").is_ok());
    }

    #[test]
    fn l2_1_validate_rename_invalid() {
        assert!(validate_rename("").is_err());
        assert!(validate_rename("123abc").is_err());
        assert!(validate_rename("my-name").is_err());
        assert!(validate_rename("fn").is_err());
        assert!(validate_rename("let").is_err());
    }

    #[test]
    fn l2_1_workspace_edit() {
        let mut edit = WorkspaceEdit::new();
        edit.add_edit("file:///a.fj", TextEdit {
            start_line: 0, start_char: 3, end_line: 0, end_char: 6,
            new_text: "new_fn".to_string(),
        });
        edit.add_edit("file:///a.fj", TextEdit {
            start_line: 5, start_char: 4, end_line: 5, end_char: 7,
            new_text: "new_fn".to_string(),
        });
        edit.add_edit("file:///b.fj", TextEdit {
            start_line: 2, start_char: 8, end_line: 2, end_char: 11,
            new_text: "new_fn".to_string(),
        });
        assert_eq!(edit.edit_count(), 3);
        assert_eq!(edit.file_count(), 2);
    }

    // L2.2: Extract function
    #[test]
    fn l2_2_extract_function() {
        let params = ExtractFunctionParams {
            start_line: 5, start_char: 0, end_line: 8, end_char: 0,
            function_name: "compute_sum".to_string(),
            captured_reads: vec![
                CapturedVariable { name: "a".to_string(), var_type: "i32".to_string(), is_mutable: false },
                CapturedVariable { name: "b".to_string(), var_type: "i32".to_string(), is_mutable: false },
            ],
            captured_writes: vec![
                CapturedVariable { name: "result".to_string(), var_type: "i32".to_string(), is_mutable: false },
            ],
        };
        let source = "line0\nline1\nline2\nline3\nline4\nlet result = a + b\nresult = result * 2\nresult";
        let (func, call) = generate_extracted_function(source, &params);
        assert!(func.contains("fn compute_sum(a: i32, b: i32) -> i32"));
        assert!(call.contains("compute_sum(a, b)"));
    }

    // L2.6: If-else to match
    #[test]
    fn l2_6_if_else_to_match() {
        let result = if_else_to_match("x", &[
            ("1".to_string(), "\"one\"".to_string()),
            ("2".to_string(), "\"two\"".to_string()),
        ], "\"other\"");
        assert!(result.contains("match x {"));
        assert!(result.contains("1 => \"one\""));
        assert!(result.contains("_ => \"other\""));
    }

    // L2.7: Match to if-else
    #[test]
    fn l2_7_match_to_if_else() {
        let result = match_to_if_else("x", &[
            ("1".to_string(), "println(\"one\")".to_string()),
            ("_".to_string(), "println(\"other\")".to_string()),
        ]);
        assert!(result.contains("if x == 1"));
        assert!(result.contains("else { println(\"other\") }"));
    }

    // L2.9: Generate trait impl
    #[test]
    fn l2_9_generate_trait_impl() {
        let stubs = vec![
            TraitMethodStub {
                name: "draw".to_string(),
                params: vec![("self".to_string(), "Self".to_string())],
                return_type: Some("str".to_string()),
            },
            TraitMethodStub {
                name: "area".to_string(),
                params: vec![("self".to_string(), "Self".to_string())],
                return_type: Some("f64".to_string()),
            },
        ];
        let result = generate_trait_impl("Circle", "Drawable", &stubs);
        assert!(result.contains("impl Drawable for Circle {"));
        assert!(result.contains("fn draw(self: Self) -> str"));
        assert!(result.contains("todo!()"));
    }

    // L2.10: Generate constructor
    #[test]
    fn l2_10_generate_constructor() {
        let fields = vec![
            ("x".to_string(), "f64".to_string()),
            ("y".to_string(), "f64".to_string()),
        ];
        let result = generate_constructor("Point", &fields);
        assert!(result.contains("fn new(x: f64, y: f64) -> Point"));
        assert!(result.contains("x: x"));
        assert!(result.contains("y: y"));
    }

    // L2.12: Organize imports
    #[test]
    fn l2_12_organize_imports() {
        let imports = vec![
            "use crate::parser".to_string(),
            "use std::collections::HashMap".to_string(),
            "use fj_math::linear".to_string(),
            "use std::fmt".to_string(),
            "use crate::lexer".to_string(),
        ];
        let organized = organize_imports(&imports);
        // std first, then external, then local
        assert!(organized[0].contains("std::collections"));
        assert!(organized[1].contains("std::fmt"));
    }

    // L2.14: Wrap expressions
    #[test]
    fn l2_14_wrap_expressions() {
        assert_eq!(wrap_in_some("42"), "Some(42)");
        assert_eq!(wrap_in_ok("data"), "Ok(data)");
        assert_eq!(wrap_in_err("\"fail\""), "Err(\"fail\")");
        assert_eq!(add_question_mark("parse_int(s)"), "parse_int(s)?");
    }

    // L2.16: Generate doc comment
    #[test]
    fn l2_16_generate_doc() {
        let doc = generate_doc_comment("add", &[
            ("a".to_string(), "i32".to_string()),
            ("b".to_string(), "i32".to_string()),
        ], Some("i32"));
        assert!(doc.contains("/// add"));
        assert!(doc.contains("/// * `a` — i32"));
        assert!(doc.contains("/// Returns: i32"));
    }

    // L2.15: F-string conversion
    #[test]
    fn l2_15_to_fstring() {
        assert_eq!(to_fstring("\"hello {name}\""), "f\"hello {name}\"");
    }

    // L2.6: Code action kinds
    #[test]
    fn l2_6_code_action_kinds() {
        assert_eq!(format!("{}", CodeActionKind::QuickFix), "quickfix");
        assert_eq!(format!("{}", CodeActionKind::RefactorExtract), "refactor.extract");
        assert_eq!(format!("{}", CodeActionKind::SourceOrganizeImports), "source.organizeImports");
    }
}
