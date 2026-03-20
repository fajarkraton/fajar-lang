//! Error formatting, suggestions, and loan timeline visualization for Polonius.
//!
//! Provides rich diagnostics for borrow checker errors including:
//! - Error templates with source span highlights
//! - Suggestion engine ("consider cloning", "consider using a reference")
//! - ASCII loan timeline visualization
//! - Error codes: ME011 TwoPhaseConflict, ME012 ReborrowConflict, ME013 PlaceConflict
//! - Feature flag support (`--polonius`, `--nll`, `--borrow-check=compare`)

use std::fmt;

use crate::lexer::token::Span;

use super::facts::{Loan, Mutability, Place, Point};
use super::solver::{BorrowError, BorrowErrorKind};

// ═══════════════════════════════════════════════════════════════════════
// Error codes
// ═══════════════════════════════════════════════════════════════════════

/// Extended memory error codes for Polonius analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoloniusErrorCode {
    /// ME011: Two-phase borrow conflict — a reserved mutable borrow
    /// conflicts with another mutable operation.
    TwoPhaseConflict,
    /// ME012: Reborrow conflict — a reborrow chain creates an invalid
    /// overlap of mutable and shared access.
    ReborrowConflict,
    /// ME013: Place conflict — borrows of overlapping places conflict
    /// (e.g., `&mut s.x` and `&mut s`).
    PlaceConflict,
}

impl fmt::Display for PoloniusErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PoloniusErrorCode::TwoPhaseConflict => write!(f, "ME011"),
            PoloniusErrorCode::ReborrowConflict => write!(f, "ME012"),
            PoloniusErrorCode::PlaceConflict => write!(f, "ME013"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PoloniusDiagnostic
// ═══════════════════════════════════════════════════════════════════════

/// A rich diagnostic produced from a Polonius borrow error.
///
/// Contains the error message, source spans, suggestions, and optional
/// loan timeline visualization.
#[derive(Debug, Clone)]
pub struct PoloniusDiagnostic {
    /// The error code (ME011, ME012, ME013, or mapped from BorrowErrorKind).
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Primary source span (where the error is reported).
    pub primary_span: Span,
    /// Additional labeled spans for context.
    pub labels: Vec<DiagnosticLabel>,
    /// Suggested fixes.
    pub suggestions: Vec<Suggestion>,
    /// Optional ASCII loan timeline.
    pub timeline: Option<String>,
}

/// A labeled source span for diagnostic context.
#[derive(Debug, Clone)]
pub struct DiagnosticLabel {
    /// The source span.
    pub span: Span,
    /// A short label for the span.
    pub message: String,
}

/// A suggested fix for a borrow error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Human-readable suggestion text.
    pub message: String,
    /// The kind of suggestion.
    pub kind: SuggestionKind,
}

/// Classification of suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestionKind {
    /// Suggest cloning the value.
    Clone,
    /// Suggest using a reference instead of moving.
    UseReference,
    /// Suggest moving the borrow earlier in the code.
    MoveBorrowEarlier,
    /// Suggest splitting the borrow into separate scopes.
    SplitBorrow,
    /// Suggest using a different field.
    UseDifferentField,
}

impl fmt::Display for Suggestion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Error formatter
// ═══════════════════════════════════════════════════════════════════════

/// Formats Polonius borrow errors into rich diagnostics.
pub struct ErrorFormatter;

impl ErrorFormatter {
    /// Converts a `BorrowError` into a `PoloniusDiagnostic`.
    pub fn format(error: &BorrowError) -> PoloniusDiagnostic {
        match error.kind {
            BorrowErrorKind::UseAfterMove => Self::format_use_after_move(error),
            BorrowErrorKind::DanglingReference => Self::format_dangling_ref(error),
            BorrowErrorKind::ConflictingBorrow => Self::format_conflicting(error),
        }
    }

    /// Formats a use-after-move error.
    fn format_use_after_move(error: &BorrowError) -> PoloniusDiagnostic {
        let place_name = error.loan.place.to_string();
        PoloniusDiagnostic {
            code: "ME001".to_string(),
            message: format!("cannot move out of `{place_name}` because it is borrowed"),
            primary_span: error.loan.span,
            labels: vec![DiagnosticLabel {
                span: error.loan.span,
                message: format!("borrow of `{place_name}` occurs here"),
            }],
            suggestions: suggest_for_move(&place_name),
            timeline: None,
        }
    }

    /// Formats a dangling reference error.
    fn format_dangling_ref(error: &BorrowError) -> PoloniusDiagnostic {
        let place_name = error.loan.place.to_string();
        PoloniusDiagnostic {
            code: "ME002".to_string(),
            message: format!(
                "`{place_name}` does not live long enough — \
                 borrowed value dropped while still in use"
            ),
            primary_span: error.loan.span,
            labels: vec![DiagnosticLabel {
                span: error.loan.span,
                message: format!("borrow of `{place_name}` occurs here"),
            }],
            suggestions: suggest_for_dangling(&place_name),
            timeline: None,
        }
    }

    /// Formats a conflicting borrow error.
    fn format_conflicting(error: &BorrowError) -> PoloniusDiagnostic {
        let place_name = error.loan.place.to_string();
        let msg = match error.loan.mutability {
            Mutability::Mutable => format!(
                "cannot borrow `{place_name}` as mutable because \
                 it is also borrowed as immutable"
            ),
            Mutability::Shared => format!(
                "cannot borrow `{place_name}` as immutable because \
                 it is also borrowed as mutable"
            ),
        };

        PoloniusDiagnostic {
            code: "ME003".to_string(),
            message: msg,
            primary_span: error.loan.span,
            labels: vec![DiagnosticLabel {
                span: error.loan.span,
                message: format!(
                    "{} borrow of `{place_name}` occurs here",
                    error.loan.mutability
                ),
            }],
            suggestions: suggest_for_conflict(&place_name),
            timeline: None,
        }
    }

    /// Formats a two-phase conflict error.
    pub fn format_two_phase_conflict(place: &Place, span: Span) -> PoloniusDiagnostic {
        let place_name = place.to_string();
        PoloniusDiagnostic {
            code: PoloniusErrorCode::TwoPhaseConflict.to_string(),
            message: format!(
                "two-phase borrow conflict on `{place_name}` — \
                 reserved mutable borrow conflicts with another mutable operation"
            ),
            primary_span: span,
            labels: vec![DiagnosticLabel {
                span,
                message: "conflict occurs here".to_string(),
            }],
            suggestions: vec![Suggestion {
                message: "consider splitting the expression into separate statements".to_string(),
                kind: SuggestionKind::SplitBorrow,
            }],
            timeline: None,
        }
    }

    /// Formats a reborrow conflict error.
    pub fn format_reborrow_conflict(place: &Place, span: Span) -> PoloniusDiagnostic {
        let place_name = place.to_string();
        PoloniusDiagnostic {
            code: PoloniusErrorCode::ReborrowConflict.to_string(),
            message: format!(
                "reborrow conflict on `{place_name}` — \
                 reborrow chain creates invalid overlap"
            ),
            primary_span: span,
            labels: vec![DiagnosticLabel {
                span,
                message: "conflicting reborrow here".to_string(),
            }],
            suggestions: vec![Suggestion {
                message: "consider cloning the value instead of reborrowing".to_string(),
                kind: SuggestionKind::Clone,
            }],
            timeline: None,
        }
    }

    /// Formats a place conflict error (overlapping field borrows).
    pub fn format_place_conflict(place: &Place, span: Span) -> PoloniusDiagnostic {
        let place_name = place.to_string();
        PoloniusDiagnostic {
            code: PoloniusErrorCode::PlaceConflict.to_string(),
            message: format!(
                "place conflict on `{place_name}` — \
                 overlapping borrows of the same location"
            ),
            primary_span: span,
            labels: vec![DiagnosticLabel {
                span,
                message: "conflicting borrow here".to_string(),
            }],
            suggestions: vec![Suggestion {
                message: "try borrowing a different field".to_string(),
                kind: SuggestionKind::UseDifferentField,
            }],
            timeline: None,
        }
    }
}

// ── Suggestion helpers ─────────────────────────────────────────────────

fn suggest_for_move(place: &str) -> Vec<Suggestion> {
    vec![
        Suggestion {
            message: format!("consider cloning `{place}` before the borrow"),
            kind: SuggestionKind::Clone,
        },
        Suggestion {
            message: "consider using a reference instead of moving".to_string(),
            kind: SuggestionKind::UseReference,
        },
    ]
}

fn suggest_for_dangling(place: &str) -> Vec<Suggestion> {
    vec![
        Suggestion {
            message: format!("consider cloning `{place}` so the borrow is not needed"),
            kind: SuggestionKind::Clone,
        },
        Suggestion {
            message: "try moving the borrow earlier in the code".to_string(),
            kind: SuggestionKind::MoveBorrowEarlier,
        },
    ]
}

fn suggest_for_conflict(place: &str) -> Vec<Suggestion> {
    vec![
        Suggestion {
            message: format!("consider cloning `{place}` to avoid the conflict"),
            kind: SuggestionKind::Clone,
        },
        Suggestion {
            message: "try splitting the borrows into separate scopes".to_string(),
            kind: SuggestionKind::SplitBorrow,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Loan timeline visualization
// ═══════════════════════════════════════════════════════════════════════

/// Generates an ASCII timeline showing borrow ranges.
///
/// Example output:
/// ```text
/// Point:  0   1   2   3   4
/// L0 (&x) ╠═══════╣
/// L1 (&mut x)     ╠═══╣
///                  ^-- conflict here
/// ```
pub struct TimelineRenderer;

impl TimelineRenderer {
    /// Renders a loan timeline given loans and their live ranges.
    pub fn render(entries: &[TimelineEntry]) -> String {
        if entries.is_empty() {
            return String::new();
        }

        let max_point = entries.iter().map(|e| e.end_point).max().unwrap_or(0);

        let mut output = String::new();

        // Header line.
        output.push_str("Point:  ");
        for p in 0..=max_point {
            output.push_str(&format!("{p:<4}"));
        }
        output.push('\n');

        // Loan lines.
        for entry in entries {
            let label = format!(
                "L{} ({}{}) ",
                entry.loan_id,
                if entry.mutability == Mutability::Mutable {
                    "&mut "
                } else {
                    "&"
                },
                entry.place_name,
            );
            output.push_str(&label);

            // Pad to align with header.
            let pad_needed = 8usize.saturating_sub(label.len());
            for _ in 0..pad_needed {
                output.push(' ');
            }

            for p in 0..=max_point {
                if p == entry.start_point && p == entry.end_point {
                    output.push_str("╠╣  ");
                } else if p == entry.start_point {
                    output.push_str("╠═══");
                } else if p == entry.end_point {
                    output.push_str("════╣");
                } else if p > entry.start_point && p < entry.end_point {
                    output.push_str("════");
                } else {
                    output.push_str("    ");
                }
            }
            output.push('\n');
        }

        output
    }
}

/// An entry for timeline rendering.
#[derive(Debug, Clone)]
pub struct TimelineEntry {
    /// Loan ID.
    pub loan_id: u32,
    /// Place name for display.
    pub place_name: String,
    /// Mutability of the borrow.
    pub mutability: Mutability,
    /// Start point index.
    pub start_point: u32,
    /// End point index.
    pub end_point: u32,
}

/// Creates timeline entries from borrow errors and live-at information.
pub fn build_timeline(
    errors: &[BorrowError],
    loan_live_points: &[(Loan, Point)],
) -> Vec<TimelineEntry> {
    use std::collections::HashMap;

    let mut ranges: HashMap<u32, (u32, u32)> = HashMap::new();

    for (loan, point) in loan_live_points {
        let entry = ranges.entry(loan.id).or_insert((u32::MAX, 0));
        entry.0 = entry.0.min(point.statement);
        entry.1 = entry.1.max(point.statement);
    }

    // Also include error loan positions.
    for error in errors {
        let entry = ranges.entry(error.loan.id).or_insert((u32::MAX, 0));
        entry.0 = entry.0.min(error.point.statement);
        entry.1 = entry.1.max(error.point.statement);
    }

    let mut entries = Vec::new();
    for (loan_id, (start, end)) in &ranges {
        // Find the loan info from errors or live points.
        let loan_info = errors
            .iter()
            .find(|e| e.loan.id == *loan_id)
            .map(|e| &e.loan)
            .or_else(|| {
                loan_live_points
                    .iter()
                    .find(|(l, _)| l.id == *loan_id)
                    .map(|(l, _)| l)
            });

        if let Some(loan) = loan_info {
            entries.push(TimelineEntry {
                loan_id: *loan_id,
                place_name: loan.place.to_string(),
                mutability: loan.mutability,
                start_point: *start,
                end_point: *end,
            });
        }
    }

    entries.sort_by_key(|e| (e.start_point, e.loan_id));
    entries
}

// ═══════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for the Polonius borrow checker.
#[derive(Debug, Clone)]
pub struct PoloniusConfig {
    /// Whether the Polonius borrow checker is enabled.
    pub enabled: bool,
    /// Whether to run both NLL and Polonius and compare results.
    pub comparison_mode: bool,
}

impl PoloniusConfig {
    /// Default configuration: Polonius disabled, NLL used.
    pub fn default_nll() -> Self {
        Self {
            enabled: false,
            comparison_mode: false,
        }
    }

    /// Configuration with Polonius enabled.
    pub fn polonius() -> Self {
        Self {
            enabled: true,
            comparison_mode: false,
        }
    }

    /// Configuration with comparison mode (runs both).
    pub fn compare() -> Self {
        Self {
            enabled: true,
            comparison_mode: true,
        }
    }

    /// Creates configuration from command-line flags.
    pub fn from_flags(polonius: bool, compare: bool) -> Self {
        Self {
            enabled: polonius || compare,
            comparison_mode: compare,
        }
    }
}

impl Default for PoloniusConfig {
    fn default() -> Self {
        Self::default_nll()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::polonius::facts::{Mutability, Place, Point, PointKind};
    use crate::analyzer::polonius::solver::{BorrowError, BorrowErrorKind};
    use crate::lexer::token::Span;

    fn span(s: usize, e: usize) -> Span {
        Span::new(s, e)
    }

    fn point(block: u32, stmt: u32) -> Point {
        Point {
            block,
            statement: stmt,
            kind: PointKind::Start,
        }
    }

    fn make_loan(id: u32, place: &str, mutability: Mutability) -> Loan {
        Loan {
            id,
            place: Place::from_var(place),
            mutability,
            span: span(0, 10),
        }
    }

    fn make_error(loan: Loan, point: Point, kind: BorrowErrorKind) -> BorrowError {
        BorrowError { loan, point, kind }
    }

    // ── S12.1: UseAfterMove error formatting ──────────────────────────

    #[test]
    fn s12_1_use_after_move_format() {
        let loan = make_loan(0, "x", Mutability::Shared);
        let error = make_error(loan, point(0, 1), BorrowErrorKind::UseAfterMove);

        let diag = ErrorFormatter::format(&error);
        assert!(diag.message.contains("cannot move out of `x`"));
        assert_eq!(diag.code, "ME001");
        assert!(!diag.suggestions.is_empty());
    }

    // ── S12.2: Dangling reference error formatting ────────────────────

    #[test]
    fn s12_2_dangling_reference_format() {
        let loan = make_loan(0, "data", Mutability::Shared);
        let error = make_error(loan, point(0, 2), BorrowErrorKind::DanglingReference);

        let diag = ErrorFormatter::format(&error);
        assert!(diag.message.contains("does not live long enough"));
        assert_eq!(diag.code, "ME002");
        assert!(
            diag.suggestions
                .iter()
                .any(|s| s.kind == SuggestionKind::Clone)
        );
    }

    // ── S12.3: Conflicting borrow error formatting ────────────────────

    #[test]
    fn s12_3_conflicting_borrow_format() {
        let loan = make_loan(0, "buf", Mutability::Mutable);
        let error = make_error(loan, point(0, 1), BorrowErrorKind::ConflictingBorrow);

        let diag = ErrorFormatter::format(&error);
        assert!(diag.message.contains("cannot borrow `buf` as mutable"));
        assert_eq!(diag.code, "ME003");
    }

    // ── S12.4: Suggestion engine produces relevant suggestions ────────

    #[test]
    fn s12_4_suggestions_for_move() {
        let suggestions = suggest_for_move("value");
        assert!(suggestions.len() >= 2);
        assert!(suggestions.iter().any(|s| s.kind == SuggestionKind::Clone));
        assert!(
            suggestions
                .iter()
                .any(|s| s.kind == SuggestionKind::UseReference)
        );
    }

    // ── S12.5: Loan timeline rendering ────────────────────────────────

    #[test]
    fn s12_5_timeline_rendering() {
        let entries = vec![
            TimelineEntry {
                loan_id: 0,
                place_name: "x".to_string(),
                mutability: Mutability::Shared,
                start_point: 0,
                end_point: 2,
            },
            TimelineEntry {
                loan_id: 1,
                place_name: "x".to_string(),
                mutability: Mutability::Mutable,
                start_point: 1,
                end_point: 3,
            },
        ];

        let timeline = TimelineRenderer::render(&entries);
        assert!(timeline.contains("Point:"));
        assert!(timeline.contains("L0"));
        assert!(timeline.contains("L1"));
        assert!(timeline.contains("&x"));
        assert!(timeline.contains("&mut x"));
    }

    // ── S12.6: Two-phase conflict error code ──────────────────────────

    #[test]
    fn s12_6_two_phase_conflict_error_code() {
        let place = Place::from_var("vec");
        let diag = ErrorFormatter::format_two_phase_conflict(&place, span(0, 20));

        assert_eq!(diag.code, "ME011");
        assert!(diag.message.contains("two-phase borrow conflict"));
    }

    // ── S12.7: Reborrow conflict error code ───────────────────────────

    #[test]
    fn s12_7_reborrow_conflict_error_code() {
        let place = Place::from_var("r1");
        let diag = ErrorFormatter::format_reborrow_conflict(&place, span(5, 15));

        assert_eq!(diag.code, "ME012");
        assert!(diag.message.contains("reborrow conflict"));
    }

    // ── S12.8: Place conflict error code ──────────────────────────────

    #[test]
    fn s12_8_place_conflict_error_code() {
        let place = Place::from_var("s").with_field("data");
        let diag = ErrorFormatter::format_place_conflict(&place, span(10, 25));

        assert_eq!(diag.code, "ME013");
        assert!(diag.message.contains("place conflict on `s.data`"));
    }

    // ── S12.9: Config from flags ──────────────────────────────────────

    #[test]
    fn s12_9_config_from_flags() {
        let default_cfg = PoloniusConfig::default_nll();
        assert!(!default_cfg.enabled);
        assert!(!default_cfg.comparison_mode);

        let polonius_cfg = PoloniusConfig::polonius();
        assert!(polonius_cfg.enabled);
        assert!(!polonius_cfg.comparison_mode);

        let compare_cfg = PoloniusConfig::compare();
        assert!(compare_cfg.enabled);
        assert!(compare_cfg.comparison_mode);

        let flag_cfg = PoloniusConfig::from_flags(false, true);
        assert!(flag_cfg.enabled);
        assert!(flag_cfg.comparison_mode);
    }

    // ── S12.10: Build timeline from errors ────────────────────────────

    #[test]
    fn s12_10_build_timeline_from_errors() {
        let loan0 = make_loan(0, "x", Mutability::Shared);
        let loan1 = make_loan(1, "x", Mutability::Mutable);

        let errors = vec![make_error(
            loan1.clone(),
            point(0, 2),
            BorrowErrorKind::ConflictingBorrow,
        )];

        let live_points = vec![
            (loan0.clone(), point(0, 0)),
            (loan0.clone(), point(0, 1)),
            (loan0.clone(), point(0, 2)),
            (loan1.clone(), point(0, 1)),
            (loan1.clone(), point(0, 2)),
            (loan1.clone(), point(0, 3)),
        ];

        let entries = build_timeline(&errors, &live_points);
        assert_eq!(entries.len(), 2);

        let timeline = TimelineRenderer::render(&entries);
        assert!(!timeline.is_empty());
        assert!(timeline.contains("L0"));
        assert!(timeline.contains("L1"));
    }
}
