//! Diagnostic System — error/warning/note message formatting with source spans,
//! color output simulation, multi-line span rendering, fix suggestions
//! ("did you mean..."), error code catalog, diagnostic sink with severity filtering.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S9.1: Source Snippets & Span Rendering
// ═══════════════════════════════════════════════════════════════════════

/// A source span (file, line, column, length).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceSpan {
    /// File path.
    pub file: String,
    /// Starting line (1-based).
    pub line: usize,
    /// Starting column (1-based).
    pub col: usize,
    /// Length in characters.
    pub len: usize,
    /// Ending line (for multi-line spans).
    pub end_line: usize,
    /// Ending column.
    pub end_col: usize,
}

impl SourceSpan {
    /// Creates a single-line span.
    pub fn new(file: &str, line: usize, col: usize, len: usize) -> Self {
        Self {
            file: file.into(),
            line,
            col,
            len,
            end_line: line,
            end_col: col + len,
        }
    }

    /// Creates a multi-line span.
    pub fn multi_line(
        file: &str,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Self {
        Self {
            file: file.into(),
            line: start_line,
            col: start_col,
            len: 0,
            end_line,
            end_col,
        }
    }

    /// Whether this span covers multiple lines.
    pub fn is_multiline(&self) -> bool {
        self.end_line > self.line
    }
}

impl fmt::Display for SourceSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_multiline() {
            write!(
                f,
                "{}:{}:{}-{}:{}",
                self.file, self.line, self.col, self.end_line, self.end_col
            )
        } else {
            write!(f, "{}:{}:{}", self.file, self.line, self.col)
        }
    }
}

/// Renders a source snippet with underline markers.
pub fn render_snippet(source: &str, span: &SourceSpan) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut output = Vec::new();

    if span.line == 0 || span.line > lines.len() {
        return String::new();
    }

    let line_idx = span.line - 1;
    let line_content = lines[line_idx];

    // Line number gutter width
    let gutter_width = format!("{}", span.end_line).len();

    // Separator line
    output.push(format!("{:>gutter_width$} |", ""));

    // Source line
    output.push(format!("{:>gutter_width$} | {}", span.line, line_content));

    // Underline
    let col_offset = if span.col > 0 { span.col - 1 } else { 0 };
    let underline_len = if span.len > 0 { span.len } else { 1 };
    let spaces = " ".repeat(col_offset);
    let carets = "^".repeat(underline_len);
    output.push(format!("{:>gutter_width$} | {spaces}{carets}", ""));

    // For multi-line spans, show additional lines
    if span.is_multiline() {
        for extra_line in (span.line + 1)..=span.end_line.min(lines.len()) {
            let extra_idx = extra_line - 1;
            if extra_idx < lines.len() {
                output.push(format!(
                    "{:>gutter_width$} | {}",
                    extra_line, lines[extra_idx]
                ));
            }
        }
    }

    output.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// S9.2: Error Codes & Severity
// ═══════════════════════════════════════════════════════════════════════

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// Informational note.
    Note,
    /// Warning (does not block compilation).
    Warning,
    /// Error (blocks compilation).
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Note => write!(f, "note"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

/// Error code catalog entry.
#[derive(Debug, Clone)]
pub struct ErrorCodeEntry {
    /// Error code (e.g., "SE004").
    pub code: String,
    /// Short description.
    pub short: String,
    /// Long explanation.
    pub explanation: String,
    /// Category.
    pub category: String,
}

/// Error code catalog.
#[derive(Debug, Clone, Default)]
pub struct ErrorCatalog {
    /// Code -> entry mapping.
    entries: HashMap<String, ErrorCodeEntry>,
}

impl ErrorCatalog {
    /// Creates a new catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a catalog with standard Fajar Lang error codes.
    pub fn standard() -> Self {
        let mut catalog = Self::new();
        catalog.register(ErrorCodeEntry {
            code: "SE001".into(),
            short: "undefined variable".into(),
            explanation: "The variable referenced has not been declared in the current scope or any parent scope.".into(),
            category: "semantic".into(),
        });
        catalog.register(ErrorCodeEntry {
            code: "SE004".into(),
            short: "type mismatch".into(),
            explanation:
                "The types of two values do not match where they are expected to be the same."
                    .into(),
            category: "semantic".into(),
        });
        catalog.register(ErrorCodeEntry {
            code: "SE009".into(),
            short: "unused variable".into(),
            explanation: "A variable is declared but never used. Prefix with _ to suppress.".into(),
            category: "semantic".into(),
        });
        catalog.register(ErrorCodeEntry {
            code: "PE001".into(),
            short: "unexpected token".into(),
            explanation: "The parser encountered a token that does not fit the expected grammar."
                .into(),
            category: "parse".into(),
        });
        catalog.register(ErrorCodeEntry {
            code: "KE001".into(),
            short: "heap allocation in @kernel".into(),
            explanation: "Heap allocation is forbidden in @kernel context. Use stack allocation or static buffers.".into(),
            category: "kernel".into(),
        });
        catalog.register(ErrorCodeEntry {
            code: "DE001".into(),
            short: "raw pointer in @device".into(),
            explanation: "Raw pointer operations are forbidden in @device context for safety."
                .into(),
            category: "device".into(),
        });
        catalog.register(ErrorCodeEntry {
            code: "ME001".into(),
            short: "use after move".into(),
            explanation: "The value has been moved and can no longer be used. Clone it if you need to keep a copy.".into(),
            category: "memory".into(),
        });
        catalog.register(ErrorCodeEntry {
            code: "LE001".into(),
            short: "invalid character".into(),
            explanation:
                "The lexer encountered a character that is not valid in Fajar Lang source code."
                    .into(),
            category: "lex".into(),
        });
        catalog
    }

    /// Registers an error code entry.
    pub fn register(&mut self, entry: ErrorCodeEntry) {
        self.entries.insert(entry.code.clone(), entry);
    }

    /// Looks up an error code.
    pub fn lookup(&self, code: &str) -> Option<&ErrorCodeEntry> {
        self.entries.get(code)
    }

    /// Returns the explanation for `--explain CODE`.
    pub fn explain(&self, code: &str) -> String {
        match self.entries.get(code) {
            Some(entry) => format!(
                "{} [{}]: {}\n\n{}",
                entry.code, entry.category, entry.short, entry.explanation
            ),
            None => format!("unknown error code: {code}"),
        }
    }

    /// Returns the number of registered error codes.
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.3: Suggestions ("Did you mean...?")
// ═══════════════════════════════════════════════════════════════════════

/// A fix suggestion attached to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Suggestion text (e.g., "did you mean `println`?").
    pub message: String,
    /// Suggested replacement text.
    pub replacement: Option<String>,
    /// Span to replace (if applicable).
    pub span: Option<SourceSpan>,
}

/// Computes the edit distance (Levenshtein) between two strings.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    let mut dp = vec![vec![0usize; b_len + 1]; a_len + 1];

    for (i, row) in dp.iter_mut().enumerate().take(a_len + 1) {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate().take(b_len + 1) {
        *val = j;
    }

    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[a_len][b_len]
}

/// Finds the closest match to `name` from a list of candidates.
pub fn closest_match<'a>(
    name: &str,
    candidates: &[&'a str],
    max_distance: usize,
) -> Option<&'a str> {
    let mut best: Option<(&str, usize)> = None;
    for &candidate in candidates {
        let dist = edit_distance(name, candidate);
        if dist <= max_distance {
            if best.is_none_or(|(_, d)| dist < d) {
                best = Some((candidate, dist));
            }
        }
    }
    best.map(|(s, _)| s)
}

/// Creates a "did you mean..." suggestion.
pub fn suggest_similar(name: &str, candidates: &[&str]) -> Option<Suggestion> {
    closest_match(name, candidates, 2).map(|match_| Suggestion {
        message: format!("did you mean `{match_}`?"),
        replacement: Some(match_.into()),
        span: None,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S9.4: Diagnostic Message
// ═══════════════════════════════════════════════════════════════════════

/// A complete diagnostic message.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Severity level.
    pub severity: Severity,
    /// Error code (e.g., "SE004").
    pub code: String,
    /// Primary message.
    pub message: String,
    /// Source span.
    pub span: Option<SourceSpan>,
    /// Attached suggestions.
    pub suggestions: Vec<Suggestion>,
    /// Attached notes.
    pub notes: Vec<String>,
}

impl Diagnostic {
    /// Creates a new error diagnostic.
    pub fn error(code: &str, message: &str) -> Self {
        Self {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
            span: None,
            suggestions: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Creates a new warning diagnostic.
    pub fn warning(code: &str, message: &str) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
            span: None,
            suggestions: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Creates a new note diagnostic.
    pub fn note(message: &str) -> Self {
        Self {
            severity: Severity::Note,
            code: String::new(),
            message: message.into(),
            span: None,
            suggestions: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Attaches a source span.
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// Attaches a suggestion.
    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Attaches a note.
    pub fn with_note(mut self, note: &str) -> Self {
        self.notes.push(note.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // severity[code]: message
        if self.code.is_empty() {
            write!(f, "{}: {}", self.severity, self.message)?;
        } else {
            write!(f, "{}[{}]: {}", self.severity, self.code, self.message)?;
        }

        // --> file:line:col
        if let Some(span) = &self.span {
            write!(f, "\n  --> {span}")?;
        }

        // Suggestions
        for suggestion in &self.suggestions {
            write!(f, "\n  help: {}", suggestion.message)?;
        }

        // Notes
        for note in &self.notes {
            write!(f, "\n  note: {note}")?;
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.5: Color Output Simulation
// ═══════════════════════════════════════════════════════════════════════

/// ANSI color codes for terminal output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiColor {
    Red,
    Yellow,
    Blue,
    Green,
    Cyan,
    Magenta,
    White,
    Bold,
    Reset,
}

impl AnsiColor {
    /// Returns the ANSI escape code.
    pub fn code(&self) -> &'static str {
        match self {
            AnsiColor::Red => "\x1b[31m",
            AnsiColor::Yellow => "\x1b[33m",
            AnsiColor::Blue => "\x1b[34m",
            AnsiColor::Green => "\x1b[32m",
            AnsiColor::Cyan => "\x1b[36m",
            AnsiColor::Magenta => "\x1b[35m",
            AnsiColor::White => "\x1b[37m",
            AnsiColor::Bold => "\x1b[1m",
            AnsiColor::Reset => "\x1b[0m",
        }
    }
}

/// Wraps text in ANSI color codes.
pub fn colorize(text: &str, color: AnsiColor) -> String {
    format!("{}{}{}", color.code(), text, AnsiColor::Reset.code())
}

/// Returns the color for a severity level.
pub fn severity_color(severity: Severity) -> AnsiColor {
    match severity {
        Severity::Error => AnsiColor::Red,
        Severity::Warning => AnsiColor::Yellow,
        Severity::Note => AnsiColor::Blue,
    }
}

/// Renders a diagnostic with ANSI colors.
pub fn render_colored(diag: &Diagnostic) -> String {
    let color = severity_color(diag.severity);
    let mut parts = Vec::new();

    // Header: bold severity[code]: message
    let header = if diag.code.is_empty() {
        format!(
            "{}{}{}: {}",
            AnsiColor::Bold.code(),
            colorize(&diag.severity.to_string(), color),
            AnsiColor::Reset.code(),
            diag.message
        )
    } else {
        format!(
            "{}{}[{}]{}: {}",
            AnsiColor::Bold.code(),
            colorize(&diag.severity.to_string(), color),
            diag.code,
            AnsiColor::Reset.code(),
            diag.message
        )
    };
    parts.push(header);

    // Location
    if let Some(span) = &diag.span {
        parts.push(format!("  {} {}", colorize("-->", AnsiColor::Cyan), span));
    }

    // Suggestions in green
    for suggestion in &diag.suggestions {
        parts.push(format!(
            "  {}: {}",
            colorize("help", AnsiColor::Green),
            suggestion.message
        ));
    }

    // Notes in blue
    for note in &diag.notes {
        parts.push(format!("  {}: {note}", colorize("note", AnsiColor::Blue)));
    }

    parts.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// S9.6 / S9.7: Diagnostic Sink with Severity Filtering
// ═══════════════════════════════════════════════════════════════════════

/// A diagnostic sink that collects and filters diagnostics.
#[derive(Debug, Clone)]
pub struct DiagnosticSink {
    /// All collected diagnostics.
    diagnostics: Vec<Diagnostic>,
    /// Minimum severity to collect.
    pub min_severity: Severity,
    /// Maximum number of errors before stopping.
    pub max_errors: usize,
    /// Whether to use colored output.
    pub use_color: bool,
}

impl DiagnosticSink {
    /// Creates a new diagnostic sink.
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            min_severity: Severity::Note,
            max_errors: 100,
            use_color: false,
        }
    }

    /// Emits a diagnostic.
    pub fn emit(&mut self, diag: Diagnostic) {
        if diag.severity >= self.min_severity {
            self.diagnostics.push(diag);
        }
    }

    /// Returns all collected diagnostics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns errors only.
    pub fn errors(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect()
    }

    /// Returns warnings only.
    pub fn warnings(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect()
    }

    /// Returns the error count.
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    /// Returns the warning count.
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }

    /// Whether the error limit has been reached.
    pub fn limit_reached(&self) -> bool {
        self.error_count() >= self.max_errors
    }

    /// Returns true if any errors were collected.
    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }

    /// Clears all diagnostics.
    pub fn clear(&mut self) {
        self.diagnostics.clear();
    }

    /// Renders all diagnostics as a string.
    pub fn render(&self) -> String {
        let mut lines = Vec::new();
        for diag in &self.diagnostics {
            if self.use_color {
                lines.push(render_colored(diag));
            } else {
                lines.push(diag.to_string());
            }
        }

        // Summary
        let errors = self.error_count();
        let warnings = self.warning_count();
        if errors > 0 || warnings > 0 {
            lines.push(format!(
                "\n{} error(s), {} warning(s) emitted",
                errors, warnings
            ));
        }

        lines.join("\n\n")
    }
}

impl Default for DiagnosticSink {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.8 / S9.9: JSON Error Output & Help Messages
// ═══════════════════════════════════════════════════════════════════════

/// Renders diagnostics as JSON for IDE consumption.
pub fn render_json(diagnostics: &[Diagnostic]) -> String {
    let mut entries = Vec::new();
    for diag in diagnostics {
        let span_json = if let Some(span) = &diag.span {
            format!(
                r#","file":"{}","line":{},"col":{},"end_line":{},"end_col":{}"#,
                span.file, span.line, span.col, span.end_line, span.end_col
            )
        } else {
            String::new()
        };

        let suggestions_json: Vec<String> = diag
            .suggestions
            .iter()
            .map(|s| {
                format!(
                    r#"{{"message":"{}","replacement":{}}}"#,
                    escape_json(&s.message),
                    s.replacement
                        .as_ref()
                        .map(|r| format!("\"{}\"", escape_json(r)))
                        .unwrap_or_else(|| "null".into())
                )
            })
            .collect();

        entries.push(format!(
            r#"{{"severity":"{}","code":"{}","message":"{}"{},
  "suggestions":[{}],
  "notes":[{}]}}"#,
            diag.severity,
            escape_json(&diag.code),
            escape_json(&diag.message),
            span_json,
            suggestions_json.join(","),
            diag.notes
                .iter()
                .map(|n| format!("\"{}\"", escape_json(n)))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }

    format!("[{}]", entries.join(",\n"))
}

/// Simple JSON string escaping.
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S9.1 — Source Snippets
    #[test]
    fn s9_1_source_span_single_line() {
        let span = SourceSpan::new("test.fj", 5, 10, 3);
        assert!(!span.is_multiline());
        assert!(span.to_string().contains("test.fj:5:10"));
    }

    #[test]
    fn s9_1_source_span_multiline() {
        let span = SourceSpan::multi_line("test.fj", 5, 1, 8, 10);
        assert!(span.is_multiline());
        assert!(span.to_string().contains("5:1-8:10"));
    }

    #[test]
    fn s9_1_render_snippet() {
        let source = "fn main() {\n    let x = 42\n    println(x)\n}";
        let span = SourceSpan::new("test.fj", 2, 9, 2);
        let rendered = render_snippet(source, &span);
        assert!(rendered.contains("let x = 42"));
        assert!(rendered.contains("^^"));
    }

    // S9.2 — Error Codes
    #[test]
    fn s9_2_error_catalog() {
        let catalog = ErrorCatalog::standard();
        assert!(catalog.count() >= 8);
        assert!(catalog.lookup("SE004").is_some());
        assert!(catalog.lookup("PE001").is_some());
        assert!(catalog.lookup("INVALID").is_none());
    }

    #[test]
    fn s9_2_explain_error() {
        let catalog = ErrorCatalog::standard();
        let explanation = catalog.explain("SE004");
        assert!(explanation.contains("type mismatch"));
        assert!(explanation.contains("semantic"));

        let unknown = catalog.explain("XX999");
        assert!(unknown.contains("unknown error code"));
    }

    #[test]
    fn s9_2_severity_display() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Note.to_string(), "note");
    }

    // S9.3 — Suggestions
    #[test]
    fn s9_3_edit_distance() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("hello", "hello"), 0);
        assert_eq!(edit_distance("abc", "def"), 3);
        assert_eq!(edit_distance("", "abc"), 3);
    }

    #[test]
    fn s9_3_closest_match() {
        let candidates = ["println", "print", "parse_int", "len"];
        assert_eq!(closest_match("printl", &candidates, 2), Some("println"));
        assert_eq!(closest_match("xyz", &candidates, 2), None);
    }

    #[test]
    fn s9_3_suggest_similar() {
        let candidates = ["println", "print", "parse_int"];
        let suggestion = suggest_similar("printl", &candidates);
        assert!(suggestion.is_some());
        let s = suggestion.unwrap();
        assert!(s.message.contains("did you mean"));
        assert!(s.message.contains("println"));
    }

    // S9.4 — Diagnostic Messages
    #[test]
    fn s9_4_diagnostic_error() {
        let diag = Diagnostic::error("SE004", "type mismatch: expected i32, found bool")
            .with_span(SourceSpan::new("test.fj", 10, 5, 4))
            .with_note("expected due to return type");
        let display = diag.to_string();
        assert!(display.contains("error[SE004]"));
        assert!(display.contains("type mismatch"));
        assert!(display.contains("test.fj:10:5"));
        assert!(display.contains("note:"));
    }

    #[test]
    fn s9_4_diagnostic_warning() {
        let diag =
            Diagnostic::warning("SE009", "unused variable: `x`").with_suggestion(Suggestion {
                message: "prefix with `_` to suppress".into(),
                replacement: Some("_x".into()),
                span: None,
            });
        let display = diag.to_string();
        assert!(display.contains("warning[SE009]"));
        assert!(display.contains("help:"));
    }

    #[test]
    fn s9_4_diagnostic_note() {
        let diag = Diagnostic::note("variable was moved here");
        let display = diag.to_string();
        assert!(display.contains("note:"));
        assert!(!display.contains("[")); // no error code
    }

    // S9.5 — Color Output
    #[test]
    fn s9_5_colorize() {
        let colored = colorize("error", AnsiColor::Red);
        assert!(colored.contains("\x1b[31m"));
        assert!(colored.contains("\x1b[0m"));
        assert!(colored.contains("error"));
    }

    #[test]
    fn s9_5_severity_color() {
        assert_eq!(severity_color(Severity::Error), AnsiColor::Red);
        assert_eq!(severity_color(Severity::Warning), AnsiColor::Yellow);
        assert_eq!(severity_color(Severity::Note), AnsiColor::Blue);
    }

    #[test]
    fn s9_5_render_colored() {
        let diag = Diagnostic::error("SE004", "type mismatch");
        let rendered = render_colored(&diag);
        assert!(rendered.contains("\x1b[")); // has ANSI codes
        assert!(rendered.contains("type mismatch"));
    }

    // S9.6 — Diagnostic Sink
    #[test]
    fn s9_6_sink_collect() {
        let mut sink = DiagnosticSink::new();
        sink.emit(Diagnostic::error("SE004", "type mismatch"));
        sink.emit(Diagnostic::warning("SE009", "unused variable"));
        sink.emit(Diagnostic::note("defined here"));
        assert_eq!(sink.error_count(), 1);
        assert_eq!(sink.warning_count(), 1);
        assert!(sink.has_errors());
        assert_eq!(sink.diagnostics().len(), 3);
    }

    #[test]
    fn s9_6_sink_filter_by_severity() {
        let mut sink = DiagnosticSink::new();
        sink.min_severity = Severity::Warning;
        sink.emit(Diagnostic::note("this should be filtered"));
        sink.emit(Diagnostic::warning("W001", "this passes"));
        assert_eq!(sink.diagnostics().len(), 1);
    }

    #[test]
    fn s9_6_sink_limit() {
        let mut sink = DiagnosticSink::new();
        sink.max_errors = 2;
        sink.emit(Diagnostic::error("E1", "first"));
        sink.emit(Diagnostic::error("E2", "second"));
        assert!(sink.limit_reached());
    }

    // S9.7 — Warnings
    #[test]
    fn s9_7_warnings_separate() {
        let mut sink = DiagnosticSink::new();
        sink.emit(Diagnostic::error("SE004", "error"));
        sink.emit(Diagnostic::warning("SE009", "warning"));
        assert_eq!(sink.errors().len(), 1);
        assert_eq!(sink.warnings().len(), 1);
    }

    // S9.8 — Help Messages
    #[test]
    fn s9_8_help_via_catalog() {
        let catalog = ErrorCatalog::standard();
        let help = catalog.explain("ME001");
        assert!(help.contains("use after move"));
        assert!(help.contains("Clone"));
    }

    // S9.9 — JSON Output
    #[test]
    fn s9_9_json_output() {
        let diagnostics = vec![
            Diagnostic::error("SE004", "type mismatch")
                .with_span(SourceSpan::new("test.fj", 5, 10, 3)),
        ];
        let json = render_json(&diagnostics);
        assert!(json.contains("\"severity\":\"error\""));
        assert!(json.contains("\"code\":\"SE004\""));
        assert!(json.contains("\"line\":5"));
    }

    #[test]
    fn s9_9_json_escape() {
        let diag = Diagnostic::error("E1", "expected \"i32\"");
        let json = render_json(&[diag]);
        assert!(json.contains("\\\"i32\\\""));
    }

    // S9.10 — Full Render
    #[test]
    fn s9_10_sink_render() {
        let mut sink = DiagnosticSink::new();
        sink.emit(Diagnostic::error("SE004", "type mismatch"));
        sink.emit(Diagnostic::warning("SE009", "unused variable"));
        let rendered = sink.render();
        assert!(rendered.contains("error[SE004]"));
        assert!(rendered.contains("warning[SE009]"));
        assert!(rendered.contains("1 error(s), 1 warning(s)"));
    }

    #[test]
    fn s9_10_sink_clear() {
        let mut sink = DiagnosticSink::new();
        sink.emit(Diagnostic::error("E1", "error"));
        assert!(sink.has_errors());
        sink.clear();
        assert!(!sink.has_errors());
        assert_eq!(sink.diagnostics().len(), 0);
    }

    #[test]
    fn s9_10_colored_render() {
        let mut sink = DiagnosticSink::new();
        sink.use_color = true;
        sink.emit(Diagnostic::error("SE004", "type mismatch"));
        let rendered = sink.render();
        assert!(rendered.contains("\x1b["));
    }

    #[test]
    fn s9_10_ansi_color_codes() {
        assert_eq!(AnsiColor::Red.code(), "\x1b[31m");
        assert_eq!(AnsiColor::Bold.code(), "\x1b[1m");
        assert_eq!(AnsiColor::Reset.code(), "\x1b[0m");
        assert_eq!(AnsiColor::Green.code(), "\x1b[32m");
    }
}
