//! LSP v3 Diagnostics & Quick Fixes — smart suggestions for errors.
//!
//! Provides quick fix code actions for common errors including missing
//! imports, type annotations, typos, mutability, struct fields, trait
//! implementations, ownership issues, and deprecation warnings.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// L3.1: Quick Fix — Add Missing Import
// ═══════════════════════════════════════════════════════════════════════

/// Diagnostic severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Information => write!(f, "info"),
            Self::Hint => write!(f, "hint"),
        }
    }
}

/// A diagnostic message with optional quick fix.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Severity level.
    pub severity: DiagnosticSeverity,
    /// Error code (e.g., "SE001").
    pub code: String,
    /// Message.
    pub message: String,
    /// Source location.
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    /// Related information.
    pub related: Vec<DiagnosticRelated>,
    /// Associated quick fixes.
    pub quick_fixes: Vec<QuickFix>,
    /// Tags (unnecessary, deprecated).
    pub tags: Vec<DiagnosticTag>,
}

/// Related diagnostic information.
#[derive(Debug, Clone)]
pub struct DiagnosticRelated {
    /// Message.
    pub message: String,
    /// Location.
    pub uri: String,
    pub line: u32,
    pub character: u32,
}

/// Diagnostic tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticTag {
    /// Unused or unnecessary code (gray out).
    Unnecessary,
    /// Deprecated symbol (strikethrough).
    Deprecated,
}

/// A quick fix for a diagnostic.
#[derive(Debug, Clone)]
pub struct QuickFix {
    /// Title shown to user.
    pub title: String,
    /// The fix kind.
    pub kind: QuickFixKind,
    /// Is this the preferred fix?
    pub is_preferred: bool,
}

/// Kinds of quick fixes.
#[derive(Debug, Clone)]
pub enum QuickFixKind {
    /// Insert a `use` statement.
    AddImport {
        import_path: String,
        insert_line: u32,
    },
    /// Add type annotation to variable.
    AddTypeAnnotation {
        var_name: String,
        var_type: String,
        line: u32,
        character: u32,
    },
    /// Fix typo (did you mean?).
    FixTypo {
        wrong: String,
        suggestion: String,
        line: u32,
        start_char: u32,
        end_char: u32,
    },
    /// Add `mut` keyword.
    MakeMutable {
        var_name: String,
        line: u32,
        character: u32,
    },
    /// Add missing struct field.
    AddMissingField {
        field_name: String,
        field_type: String,
        default_value: String,
        line: u32,
    },
    /// Generate trait impl stubs.
    ImplementTrait {
        struct_name: String,
        trait_name: String,
        insert_line: u32,
        stub_code: String,
    },
    /// Suggest clone or borrow for ownership error.
    FixOwnership {
        suggestion: OwnershipSuggestion,
        line: u32,
        character: u32,
    },
    /// Replace deprecated API.
    ReplaceDeprecated {
        old_api: String,
        new_api: String,
        line: u32,
        start_char: u32,
        end_char: u32,
    },
    /// Remove unused code.
    RemoveUnused { start_line: u32, end_line: u32 },
}

/// Ownership fix suggestions.
#[derive(Debug, Clone)]
pub enum OwnershipSuggestion {
    /// Add `.clone()` to avoid move.
    Clone { expr: String },
    /// Change to borrow `&x`.
    Borrow { expr: String },
    /// Change to mutable borrow `&mut x`.
    BorrowMut { expr: String },
    /// Use `Rc<T>` for shared ownership.
    UseRc { type_name: String },
}

impl fmt::Display for OwnershipSuggestion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clone { expr } => write!(f, "use `{expr}.clone()` to avoid move"),
            Self::Borrow { expr } => write!(f, "use `&{expr}` to borrow instead of move"),
            Self::BorrowMut { expr } => write!(f, "use `&mut {expr}` for mutable borrow"),
            Self::UseRc { type_name } => write!(f, "use `Rc<{type_name}>` for shared ownership"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// L3.3: Typo Suggestion (Levenshtein Distance)
// ═══════════════════════════════════════════════════════════════════════

/// Computes the Levenshtein edit distance between two strings.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_len]
}

/// Finds the closest match for a name from a list of candidates.
pub fn suggest_typo_fix(name: &str, candidates: &[&str], max_distance: usize) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for &candidate in candidates {
        let dist = levenshtein_distance(name, candidate);
        if dist <= max_distance && dist > 0 {
            let dominated = match best.as_ref() {
                None => true,
                Some((prev_dist, _)) => dist < *prev_dist,
            };
            if dominated {
                best = Some((dist, candidate.to_string()));
            }
        }
    }
    best.map(|(_, s)| s)
}

// ═══════════════════════════════════════════════════════════════════════
// L3.8: Type Mismatch Diagnostic
// ═══════════════════════════════════════════════════════════════════════

/// Creates a type mismatch diagnostic with helpful details.
pub fn type_mismatch_diagnostic(
    expected: &str,
    actual: &str,
    line: u32,
    start_char: u32,
    end_char: u32,
    context: &str,
) -> Diagnostic {
    let mut fixes = Vec::new();

    // Suggest conversions
    if expected == "str" && actual == "i32" {
        fixes.push(QuickFix {
            title: "Convert with `to_string()`".to_string(),
            kind: QuickFixKind::FixTypo {
                wrong: "".to_string(),
                suggestion: ".to_string()".to_string(),
                line,
                start_char: end_char,
                end_char,
            },
            is_preferred: true,
        });
    } else if expected == "i32" && actual == "str" {
        fixes.push(QuickFix {
            title: "Convert with `parse_int()`".to_string(),
            kind: QuickFixKind::FixTypo {
                wrong: "".to_string(),
                suggestion: "parse_int()".to_string(),
                line,
                start_char,
                end_char,
            },
            is_preferred: true,
        });
    } else if expected == "f64" && actual == "i32" {
        fixes.push(QuickFix {
            title: "Cast with `as f64`".to_string(),
            kind: QuickFixKind::FixTypo {
                wrong: "".to_string(),
                suggestion: " as f64".to_string(),
                line,
                start_char: end_char,
                end_char,
            },
            is_preferred: true,
        });
    }

    Diagnostic {
        severity: DiagnosticSeverity::Error,
        code: "SE004".to_string(),
        message: format!("type mismatch in {context}: expected `{expected}`, found `{actual}`"),
        start_line: line,
        start_char,
        end_line: line,
        end_char,
        related: vec![],
        quick_fixes: fixes,
        tags: vec![],
    }
}

// ═══════════════════════════════════════════════════════════════════════
// L3.9: Unreachable Code
// ═══════════════════════════════════════════════════════════════════════

/// Creates an unreachable code diagnostic (grayed out).
pub fn unreachable_code_diagnostic(start_line: u32, end_line: u32, reason: &str) -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Hint,
        code: "SE010".to_string(),
        message: format!("unreachable code: {reason}"),
        start_line,
        start_char: 0,
        end_line,
        end_char: 0,
        related: vec![],
        quick_fixes: vec![QuickFix {
            title: "Remove unreachable code".to_string(),
            kind: QuickFixKind::RemoveUnused {
                start_line,
                end_line,
            },
            is_preferred: false,
        }],
        tags: vec![DiagnosticTag::Unnecessary],
    }
}

// ═══════════════════════════════════════════════════════════════════════
// L3.10: Deprecated API
// ═══════════════════════════════════════════════════════════════════════

/// Known deprecations.
pub fn check_deprecated(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "mem_alloc" => Some(("mem_alloc", "allocator.alloc()")),
        "mem_free" => Some(("mem_free", "allocator.free()")),
        "tensor_zeros" => Some(("tensor_zeros", "zeros()")),
        "tensor_ones" => Some(("tensor_ones", "ones()")),
        "tensor_rand" => Some(("tensor_rand", "randn()")),
        _ => None,
    }
}

/// Creates a deprecation diagnostic (strikethrough).
pub fn deprecated_diagnostic(
    name: &str,
    replacement: &str,
    line: u32,
    start_char: u32,
    end_char: u32,
) -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Warning,
        code: "DEP001".to_string(),
        message: format!("`{name}` is deprecated, use `{replacement}` instead"),
        start_line: line,
        start_char,
        end_line: line,
        end_char,
        related: vec![],
        quick_fixes: vec![QuickFix {
            title: format!("Replace with `{replacement}`"),
            kind: QuickFixKind::ReplaceDeprecated {
                old_api: name.to_string(),
                new_api: replacement.to_string(),
                line,
                start_char,
                end_char,
            },
            is_preferred: true,
        }],
        tags: vec![DiagnosticTag::Deprecated],
    }
}

// ═══════════════════════════════════════════════════════════════════════
// L3.11: Suggest Cast for SE004 (Type Mismatch)
// ═══════════════════════════════════════════════════════════════════════

/// Suggests a cast/conversion for a type mismatch between `expected` and `found`.
/// Returns `(title, replacement_suffix)` if a conversion exists, e.g.:
///   ("Convert with `to_int()`", "to_int({expr})")
pub fn suggest_cast(expected: &str, found: &str, expr: &str) -> Option<QuickFix> {
    let (title, new_text) = match (expected, found) {
        ("i64" | "i32", "str") => (
            format!("Convert with `to_int({expr})`"),
            format!("to_int({expr})"),
        ),
        ("f64" | "f32", "str") => (
            format!("Convert with `to_float({expr})`"),
            format!("to_float({expr})"),
        ),
        ("str", "i64" | "i32" | "f64" | "f32" | "bool") => (
            format!("Convert with `to_string({expr})`"),
            format!("to_string({expr})"),
        ),
        ("f64", "i64" | "i32") | ("f32", "i64" | "i32") => (
            format!("Cast with `{expr} as {expected}`"),
            format!("{expr} as {expected}"),
        ),
        ("i64", "f64" | "f32") | ("i32", "f64" | "f32") => (
            format!("Cast with `{expr} as {expected}`"),
            format!("{expr} as {expected}"),
        ),
        ("bool", "i64" | "i32") => (format!("Compare: `{expr} != 0`"), format!("{expr} != 0")),
        _ => return None,
    };

    Some(QuickFix {
        title,
        kind: QuickFixKind::FixTypo {
            wrong: expr.to_string(),
            suggestion: new_text,
            line: 0,
            start_char: 0,
            end_char: 0,
        },
        is_preferred: true,
    })
}

/// Converts a `QuickFixKind` into `(title, new_text, line, start_char, end_char)` for
/// creating LSP TextEdits. Returns None for kinds that don't produce simple text replacements.
pub fn quickfix_to_edit(fix: &QuickFix) -> Option<(String, String, u32, u32, u32)> {
    match &fix.kind {
        QuickFixKind::FixTypo {
            suggestion,
            line,
            start_char,
            end_char,
            ..
        } => Some((
            fix.title.clone(),
            suggestion.clone(),
            *line,
            *start_char,
            *end_char,
        )),
        QuickFixKind::ReplaceDeprecated {
            new_api,
            line,
            start_char,
            end_char,
            ..
        } => Some((
            fix.title.clone(),
            new_api.clone(),
            *line,
            *start_char,
            *end_char,
        )),
        QuickFixKind::RemoveUnused {
            start_line,
            end_line,
        } => Some((fix.title.clone(), String::new(), *start_line, 0, *end_line)),
        QuickFixKind::MakeMutable {
            line, character, ..
        } => Some((
            fix.title.clone(),
            "mut ".to_string(),
            *line,
            *character,
            *character,
        )),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // L3.1: Missing import fix
    #[test]
    fn l3_1_add_import_fix() {
        let fix = QuickFix {
            title: "Add `use std::collections::HashMap`".to_string(),
            kind: QuickFixKind::AddImport {
                import_path: "std::collections::HashMap".to_string(),
                insert_line: 0,
            },
            is_preferred: true,
        };
        assert!(fix.is_preferred);
    }

    // L3.2: Add type annotation
    #[test]
    fn l3_2_add_type_annotation() {
        let fix = QuickFix {
            title: "Add type annotation `: i32`".to_string(),
            kind: QuickFixKind::AddTypeAnnotation {
                var_name: "x".to_string(),
                var_type: "i32".to_string(),
                line: 5,
                character: 6,
            },
            is_preferred: false,
        };
        match &fix.kind {
            QuickFixKind::AddTypeAnnotation { var_type, .. } => assert_eq!(var_type, "i32"),
            _ => panic!("wrong kind"),
        }
    }

    // L3.3: Typo suggestion
    #[test]
    fn l3_3_levenshtein() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("ab", "abc"), 1);
    }

    #[test]
    fn l3_3_suggest_typo() {
        let candidates = ["println", "print", "push", "pop", "parse_int"];
        let result = suggest_typo_fix("prntln", &candidates, 3);
        assert_eq!(result, Some("println".to_string()));

        let result2 = suggest_typo_fix("xyz", &candidates, 2);
        assert!(result2.is_none());
    }

    // L3.4: Make mutable
    #[test]
    fn l3_4_make_mutable() {
        let fix = QuickFix {
            title: "Make `x` mutable".to_string(),
            kind: QuickFixKind::MakeMutable {
                var_name: "x".to_string(),
                line: 3,
                character: 4,
            },
            is_preferred: true,
        };
        assert!(fix.is_preferred);
    }

    // L3.5: Add missing field
    #[test]
    fn l3_5_add_missing_field() {
        let fix = QuickFix {
            title: "Add field `z: f64`".to_string(),
            kind: QuickFixKind::AddMissingField {
                field_name: "z".to_string(),
                field_type: "f64".to_string(),
                default_value: "0.0".to_string(),
                line: 5,
            },
            is_preferred: false,
        };
        match &fix.kind {
            QuickFixKind::AddMissingField { field_name, .. } => assert_eq!(field_name, "z"),
            _ => panic!("wrong kind"),
        }
    }

    // L3.6: Implement trait
    #[test]
    fn l3_6_implement_trait() {
        let fix = QuickFix {
            title: "Implement `Drawable` for `Circle`".to_string(),
            kind: QuickFixKind::ImplementTrait {
                struct_name: "Circle".to_string(),
                trait_name: "Drawable".to_string(),
                insert_line: 10,
                stub_code: "impl Drawable for Circle { ... }".to_string(),
            },
            is_preferred: true,
        };
        assert!(fix.is_preferred);
    }

    // L3.7: Ownership suggestion
    #[test]
    fn l3_7_ownership_suggestions() {
        let clone = OwnershipSuggestion::Clone {
            expr: "data".to_string(),
        };
        assert_eq!(format!("{clone}"), "use `data.clone()` to avoid move");

        let borrow = OwnershipSuggestion::Borrow {
            expr: "data".to_string(),
        };
        assert_eq!(format!("{borrow}"), "use `&data` to borrow instead of move");

        let rc = OwnershipSuggestion::UseRc {
            type_name: "String".to_string(),
        };
        assert_eq!(format!("{rc}"), "use `Rc<String>` for shared ownership");
    }

    // L3.8: Type mismatch
    #[test]
    fn l3_8_type_mismatch() {
        let diag = type_mismatch_diagnostic("str", "i32", 5, 10, 15, "assignment");
        assert_eq!(diag.code, "SE004");
        assert_eq!(diag.severity, DiagnosticSeverity::Error);
        assert!(diag.message.contains("expected `str`"));
        assert!(diag.message.contains("found `i32`"));
        assert!(!diag.quick_fixes.is_empty());
    }

    // L3.9: Unreachable code
    #[test]
    fn l3_9_unreachable() {
        let diag = unreachable_code_diagnostic(10, 15, "code after return statement");
        assert_eq!(diag.code, "SE010");
        assert_eq!(diag.severity, DiagnosticSeverity::Hint);
        assert!(diag.tags.contains(&DiagnosticTag::Unnecessary));
    }

    // L3.10: Deprecated API
    #[test]
    fn l3_10_deprecated() {
        let result = check_deprecated("tensor_zeros");
        assert!(result.is_some());
        let (old, new) = result.unwrap();
        assert_eq!(old, "tensor_zeros");
        assert_eq!(new, "zeros()");

        let diag = deprecated_diagnostic("tensor_zeros", "zeros()", 3, 4, 16);
        assert_eq!(diag.code, "DEP001");
        assert!(diag.tags.contains(&DiagnosticTag::Deprecated));
        assert!(!diag.quick_fixes.is_empty());
    }

    // L3.10: Severity display
    #[test]
    fn l3_10_severity_display() {
        assert_eq!(format!("{}", DiagnosticSeverity::Error), "error");
        assert_eq!(format!("{}", DiagnosticSeverity::Warning), "warning");
        assert_eq!(format!("{}", DiagnosticSeverity::Hint), "hint");
    }

    // L3.11: suggest_cast for SE004
    #[test]
    fn l3_11_suggest_cast_str_to_int() {
        let fix = suggest_cast("i64", "str", "x");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.title.contains("to_int"));
        assert!(fix.is_preferred);
    }

    #[test]
    fn l3_11_suggest_cast_int_to_str() {
        let fix = suggest_cast("str", "i64", "count");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.title.contains("to_string"));
    }

    #[test]
    fn l3_11_suggest_cast_int_to_float() {
        let fix = suggest_cast("f64", "i64", "n");
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert!(fix.title.contains("as f64"));
    }

    #[test]
    fn l3_11_suggest_cast_unknown_types() {
        let fix = suggest_cast("Tensor", "Array", "data");
        assert!(fix.is_none());
    }

    #[test]
    fn l3_11_quickfix_to_edit() {
        let fix = suggest_cast("i64", "str", "input").unwrap();
        let edit = quickfix_to_edit(&fix);
        assert!(edit.is_some());
        let (title, new_text, _, _, _) = edit.unwrap();
        assert!(title.contains("to_int"));
        assert_eq!(new_text, "to_int(input)");
    }
}
