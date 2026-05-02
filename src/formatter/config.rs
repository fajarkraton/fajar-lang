//! Formatter configuration for Fajar Lang.
//!
//! Configurable formatting options for `fj fmt`, including line width,
//! brace style, trailing commas, import sorting, and expression wrapping.
//! Configuration can be loaded from TOML or constructed programmatically.

use std::fmt;

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors that can occur when loading formatter configuration.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum ConfigError {
    /// TOML parsing failed.
    #[error("formatter config parse error: {0}")]
    ParseError(String),

    /// A configuration value is out of range.
    #[error("invalid config value for '{key}': {reason}")]
    InvalidValue {
        /// The configuration key.
        key: String,
        /// Why the value is invalid.
        reason: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Trailing comma policy
// ═══════════════════════════════════════════════════════════════════════

/// Policy for trailing commas in multi-line lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrailingComma {
    /// Always add a trailing comma in multi-line contexts.
    Always,
    /// Never add trailing commas.
    Never,
    /// Add trailing comma only when the list is multi-line and
    /// already has a trailing comma (preserves author intent).
    Consistent,
}

impl fmt::Display for TrailingComma {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Always => write!(f, "always"),
            Self::Never => write!(f, "never"),
            Self::Consistent => write!(f, "consistent"),
        }
    }
}

impl TrailingComma {
    /// Parses a trailing-comma policy from a string.
    pub fn from_str_config(s: &str) -> Result<Self, ConfigError> {
        match s.to_lowercase().as_str() {
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            "consistent" => Ok(Self::Consistent),
            _ => Err(ConfigError::InvalidValue {
                key: "trailing_comma".to_string(),
                reason: format!("expected always/never/consistent, got '{s}'"),
            }),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Brace style
// ═══════════════════════════════════════════════════════════════════════

/// Brace placement style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BraceStyle {
    /// Opening brace on the same line (K&R / Fajar Lang default).
    SameLine,
    /// Opening brace on the next line (Allman).
    NextLine,
}

impl fmt::Display for BraceStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SameLine => write!(f, "same_line"),
            Self::NextLine => write!(f, "next_line"),
        }
    }
}

impl BraceStyle {
    /// Parses a brace style from a string.
    pub fn from_str_config(s: &str) -> Result<Self, ConfigError> {
        match s.to_lowercase().as_str() {
            "same_line" | "sameline" | "kr" | "k&r" => Ok(Self::SameLine),
            "next_line" | "nextline" | "allman" => Ok(Self::NextLine),
            _ => Err(ConfigError::InvalidValue {
                key: "brace_style".to_string(),
                reason: format!("expected same_line/next_line, got '{s}'"),
            }),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Import sort order
// ═══════════════════════════════════════════════════════════════════════

/// How `use` imports are sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportSortOrder {
    /// Alphabetical by full path.
    Alphabetical,
    /// Grouped: stdlib first, then external, then local — each sorted.
    GroupedBySource,
}

impl fmt::Display for ImportSortOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alphabetical => write!(f, "alphabetical"),
            Self::GroupedBySource => write!(f, "grouped"),
        }
    }
}

impl ImportSortOrder {
    /// Parses an import sort order from a string.
    pub fn from_str_config(s: &str) -> Result<Self, ConfigError> {
        match s.to_lowercase().as_str() {
            "alphabetical" | "alpha" => Ok(Self::Alphabetical),
            "grouped" | "grouped_by_source" => Ok(Self::GroupedBySource),
            _ => Err(ConfigError::InvalidValue {
                key: "import_sort".to_string(),
                reason: format!("expected alphabetical/grouped, got '{s}'"),
            }),
        }
    }

    /// Sorts a list of import paths according to this order.
    pub fn sort_imports(&self, imports: &mut [String]) {
        match self {
            Self::Alphabetical => imports.sort(),
            Self::GroupedBySource => {
                imports.sort_by(|a, b| {
                    let ga = import_group(a);
                    let gb = import_group(b);
                    ga.cmp(&gb).then_with(|| a.cmp(b))
                });
            }
        }
    }
}

/// Returns the sorting group for an import path.
///
/// 0 = stdlib (`std::`), 1 = external crate, 2 = local (`crate::`/`super::`).
fn import_group(path: &str) -> u8 {
    if path.starts_with("std::") {
        0
    } else if path.starts_with("crate::") || path.starts_with("super::") {
        2
    } else {
        1
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Comment preservation
// ═══════════════════════════════════════════════════════════════════════

/// Rules for how comments are preserved during formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommentPreservation {
    /// Keep comments exactly where the user put them.
    Preserve,
    /// Move inline comments to the line above if they exceed width.
    NormalizeInline,
    /// Strip all comments (useful for minification).
    Strip,
}

impl fmt::Display for CommentPreservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preserve => write!(f, "preserve"),
            Self::NormalizeInline => write!(f, "normalize_inline"),
            Self::Strip => write!(f, "strip"),
        }
    }
}

impl CommentPreservation {
    /// Parses a comment preservation mode from a string.
    pub fn from_str_config(s: &str) -> Result<Self, ConfigError> {
        match s.to_lowercase().as_str() {
            "preserve" => Ok(Self::Preserve),
            "normalize_inline" | "normalize" => Ok(Self::NormalizeInline),
            "strip" => Ok(Self::Strip),
            _ => Err(ConfigError::InvalidValue {
                key: "comment_preservation".to_string(),
                reason: format!("expected preserve/normalize_inline/strip, got '{s}'"),
            }),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Expression wrapping
// ═══════════════════════════════════════════════════════════════════════

/// How long expressions are wrapped.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionWrapper {
    /// Maximum column width before wrapping.
    pub max_width: usize,
    /// Operators where line breaks are preferred.
    pub break_before_operators: Vec<String>,
}

impl ExpressionWrapper {
    /// Creates a new expression wrapper with the given width.
    pub fn new(max_width: usize) -> Self {
        Self {
            max_width,
            break_before_operators: vec![
                "&&".to_string(),
                "||".to_string(),
                "+".to_string(),
                "|>".to_string(),
            ],
        }
    }

    /// Returns `true` if a line of `length` exceeds the max width.
    pub fn should_wrap(&self, length: usize) -> bool {
        length > self.max_width
    }

    /// Returns `true` if `op` is a preferred break point.
    pub fn is_break_operator(&self, op: &str) -> bool {
        self.break_before_operators.iter().any(|o| o == op)
    }
}

impl Default for ExpressionWrapper {
    fn default() -> Self {
        Self::new(100)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Signature wrapping
// ═══════════════════════════════════════════════════════════════════════

/// Rules for wrapping function signatures.
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureWrapper {
    /// Max width before putting each parameter on its own line.
    pub max_width: usize,
    /// Indent continuation lines by this many spaces.
    pub continuation_indent: usize,
}

impl SignatureWrapper {
    /// Creates a new signature wrapper.
    pub fn new(max_width: usize, continuation_indent: usize) -> Self {
        Self {
            max_width,
            continuation_indent,
        }
    }

    /// Returns `true` if a signature of `length` should be wrapped.
    pub fn should_wrap(&self, length: usize) -> bool {
        length > self.max_width
    }

    /// Returns the indentation string for continuation lines.
    pub fn indent_str(&self) -> String {
        " ".repeat(self.continuation_indent)
    }
}

impl Default for SignatureWrapper {
    fn default() -> Self {
        Self::new(100, 4)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FormatterConfig — main configuration
// ═══════════════════════════════════════════════════════════════════════

/// Complete formatter configuration.
///
/// Controls all aspects of `fj fmt` output: indentation, width,
/// braces, commas, imports, comments, and wrapping.
#[derive(Debug, Clone, PartialEq)]
pub struct FormatterConfig {
    /// Maximum line width (default 100).
    pub max_width: usize,
    /// Number of spaces per indentation level (default 4).
    pub indent_size: usize,
    /// Trailing comma policy.
    pub trailing_comma: TrailingComma,
    /// Brace placement style.
    pub brace_style: BraceStyle,
    /// Import sorting order.
    pub import_sort: ImportSortOrder,
    /// Comment handling rules.
    pub comment_preservation: CommentPreservation,
    /// Expression wrapping configuration.
    pub expression_wrapper: ExpressionWrapper,
    /// Function signature wrapping configuration.
    pub signature_wrapper: SignatureWrapper,
    /// Maximum number of consecutive blank lines (default 1).
    pub max_blank_lines: usize,
    /// Whether to ensure a trailing newline (default true).
    pub trailing_newline: bool,
}

impl FormatterConfig {
    /// Returns the indentation string for a given depth.
    pub fn indent_str(&self, depth: usize) -> String {
        " ".repeat(self.indent_size * depth)
    }

    /// Returns `true` if a line of `length` exceeds the max width.
    pub fn exceeds_width(&self, length: usize) -> bool {
        length > self.max_width
    }

    /// Parses a `FormatterConfig` from a TOML string.
    ///
    /// Unknown keys are silently ignored. Missing keys use defaults.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if the TOML is malformed or a value
    /// is out of range.
    pub fn from_toml(config_str: &str) -> Result<Self, ConfigError> {
        let table: toml::Table = config_str
            .parse()
            .map_err(|e: toml::de::Error| ConfigError::ParseError(e.to_string()))?;

        let mut cfg = Self::default();

        if let Some(v) = table.get("max_width") {
            cfg.max_width = parse_usize_field(v, "max_width", 40, 200)?;
        }
        if let Some(v) = table.get("indent_size") {
            cfg.indent_size = parse_usize_field(v, "indent_size", 1, 8)?;
        }
        if let Some(v) = table.get("max_blank_lines") {
            cfg.max_blank_lines = parse_usize_field(v, "max_blank_lines", 0, 5)?;
        }
        if let Some(v) = table.get("trailing_newline") {
            cfg.trailing_newline = v.as_bool().ok_or_else(|| ConfigError::InvalidValue {
                key: "trailing_newline".to_string(),
                reason: "expected boolean".to_string(),
            })?;
        }
        if let Some(v) = table.get("trailing_comma") {
            let s = v.as_str().ok_or_else(|| ConfigError::InvalidValue {
                key: "trailing_comma".to_string(),
                reason: "expected string".to_string(),
            })?;
            cfg.trailing_comma = TrailingComma::from_str_config(s)?;
        }
        if let Some(v) = table.get("brace_style") {
            let s = v.as_str().ok_or_else(|| ConfigError::InvalidValue {
                key: "brace_style".to_string(),
                reason: "expected string".to_string(),
            })?;
            cfg.brace_style = BraceStyle::from_str_config(s)?;
        }
        if let Some(v) = table.get("import_sort") {
            let s = v.as_str().ok_or_else(|| ConfigError::InvalidValue {
                key: "import_sort".to_string(),
                reason: "expected string".to_string(),
            })?;
            cfg.import_sort = ImportSortOrder::from_str_config(s)?;
        }
        if let Some(v) = table.get("comment_preservation") {
            let s = v.as_str().ok_or_else(|| ConfigError::InvalidValue {
                key: "comment_preservation".to_string(),
                reason: "expected string".to_string(),
            })?;
            cfg.comment_preservation = CommentPreservation::from_str_config(s)?;
        }

        Ok(cfg)
    }
}

/// Parses a TOML value as a bounded usize.
fn parse_usize_field(
    value: &toml::Value,
    key: &str,
    min: usize,
    max: usize,
) -> Result<usize, ConfigError> {
    let n = value
        .as_integer()
        .ok_or_else(|| ConfigError::InvalidValue {
            key: key.to_string(),
            reason: "expected integer".to_string(),
        })? as usize;

    if n < min || n > max {
        return Err(ConfigError::InvalidValue {
            key: key.to_string(),
            reason: format!("must be between {min} and {max}, got {n}"),
        });
    }
    Ok(n)
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            max_width: 100,
            indent_size: 4,
            trailing_comma: TrailingComma::Consistent,
            brace_style: BraceStyle::SameLine,
            import_sort: ImportSortOrder::GroupedBySource,
            comment_preservation: CommentPreservation::Preserve,
            expression_wrapper: ExpressionWrapper::default(),
            signature_wrapper: SignatureWrapper::default(),
            max_blank_lines: 1,
            trailing_newline: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s24_1_default_config_values() {
        let cfg = FormatterConfig::default();
        assert_eq!(cfg.max_width, 100);
        assert_eq!(cfg.indent_size, 4);
        assert_eq!(cfg.trailing_comma, TrailingComma::Consistent);
        assert_eq!(cfg.brace_style, BraceStyle::SameLine);
        assert_eq!(cfg.import_sort, ImportSortOrder::GroupedBySource);
        assert!(cfg.trailing_newline);
        assert_eq!(cfg.max_blank_lines, 1);
    }

    #[test]
    fn s24_2_indent_str_depth() {
        let cfg = FormatterConfig::default();
        assert_eq!(cfg.indent_str(0), "");
        assert_eq!(cfg.indent_str(1), "    ");
        assert_eq!(cfg.indent_str(2), "        ");

        let cfg2 = FormatterConfig {
            indent_size: 2,
            ..Default::default()
        };
        assert_eq!(cfg2.indent_str(3), "      ");
    }

    #[test]
    fn s24_3_trailing_comma_parse() {
        assert_eq!(
            TrailingComma::from_str_config("always").unwrap(),
            TrailingComma::Always
        );
        assert_eq!(
            TrailingComma::from_str_config("Never").unwrap(),
            TrailingComma::Never
        );
        assert!(TrailingComma::from_str_config("sometimes").is_err());
    }

    #[test]
    fn s24_4_brace_style_parse() {
        assert_eq!(
            BraceStyle::from_str_config("same_line").unwrap(),
            BraceStyle::SameLine
        );
        assert_eq!(
            BraceStyle::from_str_config("allman").unwrap(),
            BraceStyle::NextLine
        );
        assert_eq!(
            BraceStyle::from_str_config("K&R").unwrap(),
            BraceStyle::SameLine
        );
        assert!(BraceStyle::from_str_config("gnu").is_err());
    }

    #[test]
    fn s24_5_import_sort_alphabetical() {
        let order = ImportSortOrder::Alphabetical;
        let mut imports = vec![
            "crate::parser".to_string(),
            "std::io".to_string(),
            "ndarray::Array2".to_string(),
        ];
        order.sort_imports(&mut imports);
        assert_eq!(imports[0], "crate::parser");
        assert_eq!(imports[1], "ndarray::Array2");
        assert_eq!(imports[2], "std::io");
    }

    #[test]
    fn s24_6_import_sort_grouped() {
        let order = ImportSortOrder::GroupedBySource;
        let mut imports = vec![
            "crate::parser".to_string(),
            "std::io".to_string(),
            "ndarray::Array2".to_string(),
            "std::collections".to_string(),
            "super::ast".to_string(),
        ];
        order.sort_imports(&mut imports);
        // Group 0: std
        assert_eq!(imports[0], "std::collections");
        assert_eq!(imports[1], "std::io");
        // Group 1: external
        assert_eq!(imports[2], "ndarray::Array2");
        // Group 2: local
        assert_eq!(imports[3], "crate::parser");
        assert_eq!(imports[4], "super::ast");
    }

    #[test]
    fn s24_7_expression_wrapper() {
        let ew = ExpressionWrapper::new(80);
        assert!(ew.should_wrap(81));
        assert!(!ew.should_wrap(80));
        assert!(ew.is_break_operator("&&"));
        assert!(ew.is_break_operator("|>"));
        assert!(!ew.is_break_operator("*"));
    }

    #[test]
    fn s24_8_signature_wrapper() {
        let sw = SignatureWrapper::new(80, 8);
        assert!(sw.should_wrap(90));
        assert!(!sw.should_wrap(70));
        assert_eq!(sw.indent_str(), "        ");
    }

    #[test]
    fn s24_9_from_toml_full() {
        let toml_str = r#"
max_width = 120
indent_size = 2
trailing_comma = "always"
brace_style = "next_line"
import_sort = "alphabetical"
trailing_newline = false
max_blank_lines = 2
"#;
        let cfg = FormatterConfig::from_toml(toml_str).unwrap();
        assert_eq!(cfg.max_width, 120);
        assert_eq!(cfg.indent_size, 2);
        assert_eq!(cfg.trailing_comma, TrailingComma::Always);
        assert_eq!(cfg.brace_style, BraceStyle::NextLine);
        assert_eq!(cfg.import_sort, ImportSortOrder::Alphabetical);
        assert!(!cfg.trailing_newline);
        assert_eq!(cfg.max_blank_lines, 2);
    }

    #[test]
    fn s24_10_from_toml_errors() {
        // Malformed TOML.
        let result = FormatterConfig::from_toml("[broken");
        assert!(result.is_err());

        // Out-of-range value.
        let result = FormatterConfig::from_toml("indent_size = 99");
        assert!(result.is_err());

        // Invalid enum value.
        let result = FormatterConfig::from_toml("trailing_comma = \"sometimes\"");
        assert!(result.is_err());
    }
}
