//! Parser error recovery for Fajar Lang.
//!
//! Provides strategies for recovering from syntax errors to continue
//! parsing and provide better diagnostics. Includes partial AST support,
//! cascade filtering, and "did you mean?" suggestions via Levenshtein
//! distance.

use std::collections::HashSet;
use std::fmt;

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors specific to the recovery subsystem.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum RecoveryError {
    /// Recovery failed — could not find a synchronisation point.
    #[error("recovery failed at {line}:{col}: {reason}")]
    SyncFailed {
        /// Line number where recovery was attempted.
        line: u32,
        /// Column number where recovery was attempted.
        col: u32,
        /// Human-readable description.
        reason: String,
    },

    /// Too many cascading errors — stopped collecting.
    #[error("cascade limit reached: {count} errors suppressed")]
    CascadeOverflow {
        /// Number of suppressed errors.
        count: usize,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Recovery strategies
// ═══════════════════════════════════════════════════════════════════════

/// Strategy the parser should use to recover from a given error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryStrategy {
    /// Skip tokens until a semicolon or newline statement boundary.
    SkipToSemicolon,
    /// Skip tokens until a matching closing brace `}`.
    SkipToBrace,
    /// Skip tokens until a keyword that starts a new statement.
    SkipToKeyword,
    /// Insert a synthetic token (e.g., a missing semicolon).
    InsertToken,
    /// Delete the unexpected token and retry.
    DeleteToken,
}

impl fmt::Display for RecoveryStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SkipToSemicolon => write!(f, "SkipToSemicolon"),
            Self::SkipToBrace => write!(f, "SkipToBrace"),
            Self::SkipToKeyword => write!(f, "SkipToKeyword"),
            Self::InsertToken => write!(f, "InsertToken"),
            Self::DeleteToken => write!(f, "DeleteToken"),
        }
    }
}

/// Returns true if `kw` is a keyword that begins a new statement/item.
fn is_statement_keyword(kw: &str) -> bool {
    matches!(
        kw,
        "let"
            | "mut"
            | "fn"
            | "struct"
            | "enum"
            | "impl"
            | "trait"
            | "if"
            | "while"
            | "for"
            | "return"
            | "use"
            | "mod"
            | "const"
            | "type"
            | "pub"
            | "match"
            | "loop"
    )
}

// ═══════════════════════════════════════════════════════════════════════
// ErrorRecovery — main driver
// ═══════════════════════════════════════════════════════════════════════

/// Accumulated context for error recovery across a parse session.
///
/// Tracks which strategies were applied and how many tokens were
/// consumed so the parser can decide when to give up.
#[derive(Debug, Clone)]
pub struct ErrorRecovery {
    /// Maximum number of consecutive recovery attempts before aborting.
    max_attempts: usize,
    /// Number of recovery attempts so far.
    attempts: usize,
    /// Strategies applied (for diagnostics).
    applied: Vec<RecoveryStrategy>,
}

impl ErrorRecovery {
    /// Creates a new recovery context.
    ///
    /// `max_attempts` limits how many times the parser will try to
    /// recover before giving up entirely.
    pub fn new(max_attempts: usize) -> Self {
        Self {
            max_attempts,
            attempts: 0,
            applied: Vec::new(),
        }
    }

    /// Returns a default recovery context with limit 32.
    pub fn default_limit() -> Self {
        Self::new(32)
    }

    /// Returns the number of recovery attempts performed.
    pub fn attempt_count(&self) -> usize {
        self.attempts
    }

    /// Returns a slice of strategies that were applied.
    pub fn applied_strategies(&self) -> &[RecoveryStrategy] {
        &self.applied
    }

    /// Returns `true` if recovery is still allowed.
    pub fn can_recover(&self) -> bool {
        self.attempts < self.max_attempts
    }

    /// Records a recovery attempt and returns the recommended strategy
    /// for the given token text.
    ///
    /// Returns `Err` if the limit has been reached.
    pub fn attempt(&mut self, current_token: &str) -> Result<RecoveryStrategy, RecoveryError> {
        if !self.can_recover() {
            return Err(RecoveryError::CascadeOverflow {
                count: self.attempts,
            });
        }
        self.attempts += 1;

        let strategy = choose_strategy(current_token);
        self.applied.push(strategy);
        Ok(strategy)
    }

    /// Resets the recovery context (e.g., after a successful parse).
    pub fn reset(&mut self) {
        self.attempts = 0;
        self.applied.clear();
    }
}

/// Heuristically picks a recovery strategy based on the current token.
fn choose_strategy(token: &str) -> RecoveryStrategy {
    match token {
        "}" => RecoveryStrategy::SkipToBrace,
        ";" => RecoveryStrategy::SkipToSemicolon,
        t if is_statement_keyword(t) => RecoveryStrategy::SkipToKeyword,
        _ => RecoveryStrategy::SkipToSemicolon,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PartialAst — AST with attached errors
// ═══════════════════════════════════════════════════════════════════════

/// A node in a partial AST that may contain errors.
///
/// Used by IDE tooling (LSP) to provide completions/diagnostics
/// even when the source has syntax errors.
#[derive(Debug, Clone, PartialEq)]
pub struct PartialNode {
    /// Kind of node (function, struct, let-binding, etc.).
    pub kind: PartialKind,
    /// Name of the item, if recoverable.
    pub name: Option<String>,
    /// Byte offset where the node starts.
    pub start: usize,
    /// Byte offset where the node ends (best-effort).
    pub end: usize,
    /// Errors encountered while parsing this node.
    pub errors: Vec<String>,
    /// Child nodes (e.g., body of a function).
    pub children: Vec<PartialNode>,
}

/// Classification of a partial AST node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartialKind {
    /// Top-level program.
    Program,
    /// Function definition (possibly incomplete).
    Function,
    /// Struct definition.
    Struct,
    /// Enum definition.
    Enum,
    /// Let binding.
    LetBinding,
    /// Expression statement.
    ExprStmt,
    /// Error placeholder — completely unrecoverable fragment.
    Error,
}

impl fmt::Display for PartialKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program => write!(f, "Program"),
            Self::Function => write!(f, "Function"),
            Self::Struct => write!(f, "Struct"),
            Self::Enum => write!(f, "Enum"),
            Self::LetBinding => write!(f, "LetBinding"),
            Self::ExprStmt => write!(f, "ExprStmt"),
            Self::Error => write!(f, "Error"),
        }
    }
}

/// A partial AST — the result of parsing a file with errors.
///
/// Contains both successfully-parsed nodes and error placeholders.
#[derive(Debug, Clone)]
pub struct PartialAst {
    /// Root nodes of the partial AST.
    pub nodes: Vec<PartialNode>,
    /// Total number of errors encountered.
    pub error_count: usize,
}

impl PartialAst {
    /// Creates an empty partial AST.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            error_count: 0,
        }
    }

    /// Adds a successfully-parsed node.
    pub fn push_node(&mut self, node: PartialNode) {
        self.nodes.push(node);
    }

    /// Adds an error placeholder spanning `start..end`.
    pub fn push_error(&mut self, start: usize, end: usize, msg: String) {
        self.error_count += 1;
        self.nodes.push(PartialNode {
            kind: PartialKind::Error,
            name: None,
            start,
            end,
            errors: vec![msg],
            children: Vec::new(),
        });
    }

    /// Returns true if the partial AST contains any error nodes.
    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }

    /// Returns all named items (functions, structs, enums) for IDE use.
    pub fn named_items(&self) -> Vec<(&str, PartialKind)> {
        self.nodes
            .iter()
            .filter_map(|n| n.name.as_deref().map(|name| (name, n.kind)))
            .collect()
    }
}

impl Default for PartialAst {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CascadeFilter — suppress secondary errors
// ═══════════════════════════════════════════════════════════════════════

/// Filters cascading errors to avoid showing noise caused by
/// a single root-cause parse failure.
///
/// Tracks error locations and suppresses errors that are "too close"
/// (within `proximity` bytes) to an already-reported error.
#[derive(Debug, Clone)]
pub struct CascadeFilter {
    /// Byte offsets of already-reported errors.
    reported: HashSet<usize>,
    /// Minimum byte distance between reported errors.
    proximity: usize,
    /// Number of errors suppressed.
    suppressed_count: usize,
}

impl CascadeFilter {
    /// Creates a new cascade filter.
    ///
    /// `proximity` is the minimum byte distance between errors;
    /// errors closer than this to an existing one are suppressed.
    pub fn new(proximity: usize) -> Self {
        Self {
            reported: HashSet::new(),
            proximity,
            suppressed_count: 0,
        }
    }

    /// Returns a default filter with proximity 20 bytes.
    pub fn default_proximity() -> Self {
        Self::new(20)
    }

    /// Returns the number of suppressed errors.
    pub fn suppressed_count(&self) -> usize {
        self.suppressed_count
    }

    /// Returns the number of reported (non-suppressed) errors.
    pub fn reported_count(&self) -> usize {
        self.reported.len()
    }

    /// Checks whether an error at `offset` should be reported.
    ///
    /// Returns `true` if the error is far enough from all
    /// previously-reported errors; returns `false` (suppressed)
    /// otherwise.
    pub fn should_report(&mut self, offset: usize) -> bool {
        let dominated = self
            .reported
            .iter()
            .any(|&existing| offset.abs_diff(existing) < self.proximity);

        if dominated {
            self.suppressed_count += 1;
            false
        } else {
            self.reported.insert(offset);
            true
        }
    }

    /// Resets the filter state.
    pub fn reset(&mut self) {
        self.reported.clear();
        self.suppressed_count = 0;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DidYouMean — Levenshtein distance suggestion engine
// ═══════════════════════════════════════════════════════════════════════

/// Computes the Levenshtein edit distance between two strings.
///
/// Uses a standard dynamic programming approach with O(m*n) time
/// and O(min(m, n)) space.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    // Use two-row optimisation.
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

/// Suggests the most similar candidate to `name` from `candidates`.
///
/// Returns `None` if no candidate is within `max_distance` edits.
pub fn suggest_similar(name: &str, candidates: &[&str], max_distance: usize) -> Option<String> {
    let mut best: Option<(usize, &str)> = None;

    for &cand in candidates {
        let dist = levenshtein_distance(name, cand);
        if dist <= max_distance {
            match best {
                None => best = Some((dist, cand)),
                Some((d, _)) if dist < d => best = Some((dist, cand)),
                _ => {}
            }
        }
    }

    best.map(|(_, s)| s.to_string())
}

/// Suggests similar candidates, returning up to `limit` matches
/// sorted by distance (ascending).
pub fn suggest_similar_top(
    name: &str,
    candidates: &[&str],
    max_distance: usize,
    limit: usize,
) -> Vec<String> {
    let mut scored: Vec<(usize, &str)> = candidates
        .iter()
        .filter_map(|&c| {
            let d = levenshtein_distance(name, c);
            if d <= max_distance {
                Some((d, c))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by_key(|&(d, _)| d);
    scored
        .into_iter()
        .take(limit)
        .map(|(_, s)| s.to_string())
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// MissingSemicolon — detection heuristic
// ═══════════════════════════════════════════════════════════════════════

/// Result of missing-semicolon analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct MissingSemicolon {
    /// Line number where the semicolon is probably missing.
    pub line: u32,
    /// Column at the end of the previous token (insertion point).
    pub col: u32,
    /// The token that triggered the heuristic.
    pub next_token: String,
}

/// Checks whether a parse failure looks like a missing semicolon.
///
/// Heuristic: if the previous token ends a valid expression and the
/// current token starts a new statement, a semicolon is likely missing.
///
/// Note: Fajar Lang does NOT require semicolons — this is for
/// detecting cases where a newline was expected but tokens ran together.
pub fn detect_missing_semicolon(
    prev_token: &str,
    current_token: &str,
    line: u32,
    col: u32,
) -> Option<MissingSemicolon> {
    let prev_ends_expr = matches!(prev_token, ")" | "]" | "}" | "true" | "false" | "null")
        || prev_token.chars().all(|c| c.is_alphanumeric() || c == '_');

    let curr_starts_stmt = is_statement_keyword(current_token);

    if prev_ends_expr && curr_starts_stmt {
        Some(MissingSemicolon {
            line,
            col,
            next_token: current_token.to_string(),
        })
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// UnclosedDelimiter — detection with origin tracking
// ═══════════════════════════════════════════════════════════════════════

/// Tracks an opening delimiter that has not yet been closed.
#[derive(Debug, Clone, PartialEq)]
pub struct UnclosedDelimiter {
    /// The opening delimiter character.
    pub open: char,
    /// The expected closing delimiter.
    pub expected_close: char,
    /// Line where the delimiter was opened.
    pub open_line: u32,
    /// Column where the delimiter was opened.
    pub open_col: u32,
}

impl fmt::Display for UnclosedDelimiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unclosed '{}' opened at {}:{}, expected '{}'",
            self.open, self.open_line, self.open_col, self.expected_close
        )
    }
}

/// Delimiter tracker for detecting mismatched / unclosed brackets.
///
/// Push opening delimiters as you encounter them; pop when the
/// matching close is found. At the end, `unclosed()` returns any
/// remaining openers.
#[derive(Debug, Clone)]
pub struct DelimiterTracker {
    /// Stack of currently-open delimiters.
    stack: Vec<UnclosedDelimiter>,
}

impl DelimiterTracker {
    /// Creates a new empty tracker.
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Returns the expected closing character for an opener.
    fn matching_close(open: char) -> Option<char> {
        match open {
            '(' => Some(')'),
            '[' => Some(']'),
            '{' => Some('}'),
            '<' => Some('>'),
            _ => None,
        }
    }

    /// Pushes an opening delimiter onto the stack.
    pub fn push_open(&mut self, open: char, line: u32, col: u32) {
        if let Some(expected_close) = Self::matching_close(open) {
            self.stack.push(UnclosedDelimiter {
                open,
                expected_close,
                open_line: line,
                open_col: col,
            });
        }
    }

    /// Pops the matching opening delimiter for `close`.
    ///
    /// Returns `Ok(())` if matched, `Err` with a description if
    /// the close is mismatched or there is nothing to close.
    pub fn push_close(&mut self, close: char) -> Result<(), String> {
        match self.stack.last() {
            Some(top) if top.expected_close == close => {
                self.stack.pop();
                Ok(())
            }
            Some(top) => Err(format!(
                "expected '{}' to close '{}' at {}:{}, found '{}'",
                top.expected_close, top.open, top.open_line, top.open_col, close
            )),
            None => Err(format!("unexpected closing delimiter '{close}'")),
        }
    }

    /// Returns all unclosed delimiters (outermost first).
    pub fn unclosed(&self) -> &[UnclosedDelimiter] {
        &self.stack
    }

    /// Returns true if the delimiter stack is balanced.
    pub fn is_balanced(&self) -> bool {
        self.stack.is_empty()
    }

    /// Resets the tracker.
    pub fn reset(&mut self) {
        self.stack.clear();
    }
}

impl Default for DelimiterTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TypeMismatchContext — rich error context for type errors
// ═══════════════════════════════════════════════════════════════════════

/// Extended context attached to a type-mismatch diagnostic.
///
/// Used by both the analyzer and IDE to provide actionable hints.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeMismatchContext {
    /// The type that was expected.
    pub expected: String,
    /// The type that was found.
    pub found: String,
    /// Where the expectation comes from.
    pub reason: MismatchReason,
    /// Optional suggestion (e.g., "use `as i64`").
    pub suggestion: Option<String>,
}

/// Why a particular type was expected.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MismatchReason {
    /// Return type declared in function signature.
    ReturnType,
    /// Assignment `let x: T = expr`.
    LetBinding,
    /// Function argument at position N.
    Argument {
        /// Zero-based argument index.
        position: usize,
    },
    /// Binary operator requires matching types.
    BinaryOp {
        /// The operator symbol.
        operator: String,
    },
    /// If-else branches must agree.
    IfElseBranch,
    /// Match arms must agree.
    MatchArm,
}

impl fmt::Display for MismatchReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReturnType => write!(f, "return type"),
            Self::LetBinding => write!(f, "let binding"),
            Self::Argument { position } => {
                write!(f, "argument at position {position}")
            }
            Self::BinaryOp { operator } => {
                write!(f, "binary operator '{operator}'")
            }
            Self::IfElseBranch => write!(f, "if-else branch types"),
            Self::MatchArm => write!(f, "match arm types"),
        }
    }
}

impl TypeMismatchContext {
    /// Creates a new type-mismatch context.
    pub fn new(
        expected: impl Into<String>,
        found: impl Into<String>,
        reason: MismatchReason,
    ) -> Self {
        Self {
            expected: expected.into(),
            found: found.into(),
            reason,
            suggestion: None,
        }
    }

    /// Attaches a suggestion to the context.
    pub fn with_suggestion(mut self, hint: impl Into<String>) -> Self {
        self.suggestion = Some(hint.into());
        self
    }

    /// Formats the mismatch as a diagnostic message.
    pub fn message(&self) -> String {
        let base = format!(
            "expected `{}`, found `{}` ({})",
            self.expected, self.found, self.reason
        );
        match &self.suggestion {
            Some(hint) => format!("{base}; hint: {hint}"),
            None => base,
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
    fn s23_1_recovery_strategy_display() {
        assert_eq!(
            RecoveryStrategy::SkipToSemicolon.to_string(),
            "SkipToSemicolon"
        );
        assert_eq!(RecoveryStrategy::SkipToBrace.to_string(), "SkipToBrace");
        assert_eq!(RecoveryStrategy::InsertToken.to_string(), "InsertToken");
    }

    #[test]
    fn s23_2_error_recovery_limits() {
        let mut er = ErrorRecovery::new(3);
        assert!(er.can_recover());
        assert_eq!(er.attempt_count(), 0);

        er.attempt("+").unwrap();
        er.attempt("*").unwrap();
        er.attempt("-").unwrap();
        assert!(!er.can_recover());

        let result = er.attempt("x");
        assert!(result.is_err());
        assert_eq!(er.attempt_count(), 3);
    }

    #[test]
    fn s23_3_recovery_chooses_strategy() {
        let mut er = ErrorRecovery::new(10);
        assert_eq!(er.attempt("}").unwrap(), RecoveryStrategy::SkipToBrace);
        assert_eq!(er.attempt(";").unwrap(), RecoveryStrategy::SkipToSemicolon);
        assert_eq!(er.attempt("fn").unwrap(), RecoveryStrategy::SkipToKeyword);
        assert_eq!(er.attempt("42").unwrap(), RecoveryStrategy::SkipToSemicolon);
    }

    #[test]
    fn s23_4_partial_ast_error_tracking() {
        let mut ast = PartialAst::new();
        assert!(!ast.has_errors());

        ast.push_node(PartialNode {
            kind: PartialKind::Function,
            name: Some("main".to_string()),
            start: 0,
            end: 50,
            errors: Vec::new(),
            children: Vec::new(),
        });
        assert!(!ast.has_errors());

        ast.push_error(55, 60, "unexpected token".to_string());
        assert!(ast.has_errors());
        assert_eq!(ast.error_count, 1);

        let named = ast.named_items();
        assert_eq!(named.len(), 1);
        assert_eq!(named[0].0, "main");
        assert_eq!(named[0].1, PartialKind::Function);
    }

    #[test]
    fn s23_5_cascade_filter_suppresses_nearby() {
        let mut cf = CascadeFilter::new(10);

        assert!(cf.should_report(0));
        assert!(cf.should_report(50));
        // Offset 5 is within 10 bytes of offset 0 — suppress.
        assert!(!cf.should_report(5));
        // Offset 55 is within 10 bytes of offset 50 — suppress.
        assert!(!cf.should_report(55));

        assert_eq!(cf.reported_count(), 2);
        assert_eq!(cf.suppressed_count(), 2);
    }

    #[test]
    fn s23_6_levenshtein_exact_match() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
        assert_eq!(levenshtein_distance("", ""), 0);
    }

    #[test]
    fn s23_7_levenshtein_edits() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("", "xyz"), 3);
        assert_eq!(levenshtein_distance("fahr", "fajar"), 2);
    }

    #[test]
    fn s23_8_suggest_similar_finds_best() {
        let candidates = &["println", "print", "panic", "parse"];
        let suggestion = suggest_similar("prnt", candidates, 2);
        assert_eq!(suggestion, Some("print".to_string()));

        let no_match = suggest_similar("zzzzz", candidates, 2);
        assert!(no_match.is_none());
    }

    #[test]
    fn s23_9_missing_semicolon_detection() {
        let result = detect_missing_semicolon("x", "let", 5, 10);
        assert!(result.is_some());
        let ms = result.unwrap();
        assert_eq!(ms.line, 5);
        assert_eq!(ms.next_token, "let");

        // Two keywords in a row — not a missing semicolon heuristic.
        let none = detect_missing_semicolon("+", "fn", 1, 1);
        assert!(none.is_none());
    }

    #[test]
    fn s23_10_unclosed_delimiter_tracking() {
        let mut dt = DelimiterTracker::new();
        dt.push_open('(', 1, 5);
        dt.push_open('{', 2, 1);
        assert!(!dt.is_balanced());
        assert_eq!(dt.unclosed().len(), 2);

        assert!(dt.push_close('}').is_ok());
        assert!(dt.push_close(')').is_ok());
        assert!(dt.is_balanced());

        // Mismatch
        dt.push_open('[', 3, 1);
        let err = dt.push_close(')');
        assert!(err.is_err());
    }
}
