//! GAT error messages, diagnostics, and inference for Fajar Lang.
//!
//! Provides:
//! - [`GatDiagnostic`] — rich diagnostic with error code, message, labels, and suggestions
//! - Error templates for common GAT mistakes
//! - [`infer_lifetime_params`] — auto-infer lifetime parameters where unambiguous
//! - [`GatConfig`] — feature flag configuration for GAT support
//!
//! # Error Codes
//!
//! | Code | Name | Description |
//! |------|------|-------------|
//! | GE001 | MissingGatParams | Associated type requires parameters not provided |
//! | GE002 | GatBoundMismatch | Impl bound doesn't match trait definition |
//! | GE003 | GatLifetimeCapture | Borrowed data doesn't live long enough |
//! | GE004 | AsyncTraitObjectSafety | Async trait method not object-safe |

use std::time::Instant;

use crate::lexer::token::Span;

use super::gat::GatError;

// ═══════════════════════════════════════════════════════════════════════
// GAT diagnostic
// ═══════════════════════════════════════════════════════════════════════

/// A rich diagnostic for GAT-related errors.
///
/// Contains the error code, human-readable message, source spans with
/// labels, and suggested fixes. Designed for integration with miette.
#[derive(Debug, Clone)]
pub struct GatDiagnostic {
    /// Error code (e.g., `"GE001"`).
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Primary source span.
    pub span: Span,
    /// Additional labeled spans for context.
    pub labels: Vec<GatLabel>,
    /// Suggested fixes.
    pub suggestions: Vec<GatSuggestion>,
}

/// A labeled source span within a GAT diagnostic.
#[derive(Debug, Clone)]
pub struct GatLabel {
    /// The source span to highlight.
    pub span: Span,
    /// A short message for this label.
    pub message: String,
}

impl GatLabel {
    /// Creates a new diagnostic label.
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        GatLabel {
            span,
            message: message.into(),
        }
    }
}

/// A suggested fix for a GAT error.
#[derive(Debug, Clone)]
pub struct GatSuggestion {
    /// Human-readable description of the fix.
    pub message: String,
    /// The replacement text (if applicable).
    pub replacement: Option<String>,
}

impl GatSuggestion {
    /// Creates a suggestion with only a message.
    pub fn hint(message: impl Into<String>) -> Self {
        GatSuggestion {
            message: message.into(),
            replacement: None,
        }
    }

    /// Creates a suggestion with a specific replacement.
    pub fn replace(message: impl Into<String>, replacement: impl Into<String>) -> Self {
        GatSuggestion {
            message: message.into(),
            replacement: Some(replacement.into()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Diagnostic construction from GatError
// ═══════════════════════════════════════════════════════════════════════

/// Converts a [`GatError`] into a rich [`GatDiagnostic`] with suggestions.
pub fn diagnose_gat_error(error: &GatError) -> GatDiagnostic {
    diagnose_gat_error_core(error)
}

/// Core dispatch for GAT error → diagnostic conversion (GE001-GE004).
fn diagnose_gat_error_core(error: &GatError) -> GatDiagnostic {
    match error {
        GatError::MissingParams {
            trait_name,
            assoc_type,
            expected,
            found,
            param_kind,
            span,
        } => build_missing_params_diagnostic(
            trait_name, assoc_type, *expected, *found, param_kind, *span,
        ),
        GatError::BoundMismatch {
            assoc_type,
            expected,
            found,
            span,
        } => build_bound_mismatch_diagnostic(assoc_type, expected, found, *span),
        GatError::LifetimeCapture {
            assoc_type,
            lifetime,
            span,
        } => build_lifetime_capture_diagnostic(assoc_type, lifetime, *span),
        GatError::AsyncTraitObjectSafety {
            trait_name,
            method,
            span,
        } => build_async_object_safety_diagnostic(trait_name, method, *span),
        _ => diagnose_gat_error_extended(error),
    }
}

/// Extended dispatch for GAT error → diagnostic conversion (GE005-GE008).
fn diagnose_gat_error_extended(error: &GatError) -> GatDiagnostic {
    match error {
        GatError::UndefinedAssocType {
            trait_name,
            assoc_type,
            span,
        } => build_undefined_assoc_type_diagnostic(trait_name, assoc_type, *span),
        GatError::DuplicateAssocType {
            trait_name,
            name,
            span,
        } => build_duplicate_assoc_type_diagnostic(trait_name, name, *span),
        GatError::ParamKindMismatch {
            assoc_type,
            param,
            expected,
            found,
            span,
        } => build_param_kind_mismatch_diagnostic(assoc_type, param, expected, found, *span),
        GatError::MissingImplAssocType {
            trait_name,
            assoc_type,
            span,
        } => build_missing_impl_diagnostic(trait_name, assoc_type, *span),
        // First four variants handled in diagnose_gat_error_core — fallback
        other => GatDiagnostic {
            code: "GE000".to_string(),
            message: format!("{other}"),
            span: other.span(),
            labels: vec![],
            suggestions: vec![],
        },
    }
}

/// Builds a diagnostic for missing GAT parameters (GE001).
fn build_missing_params_diagnostic(
    trait_name: &str,
    assoc_type: &str,
    expected: usize,
    found: usize,
    param_kind: &str,
    span: Span,
) -> GatDiagnostic {
    let suggestion = if expected > 0 && found == 0 {
        let params: Vec<String> = (0..expected)
            .map(|i| {
                if param_kind == "lifetime" || param_kind == "generic" {
                    format!("'{}", (b'a' + i as u8) as char)
                } else {
                    format!("T{}", i + 1)
                }
            })
            .collect();
        GatSuggestion::replace(
            format!("associated type '{assoc_type}' requires {param_kind} parameter(s)",),
            format!("{assoc_type}<{}>", params.join(", ")),
        )
    } else {
        GatSuggestion::hint(format!(
            "expected {expected} {param_kind} parameter(s), found {found}",
        ))
    };

    GatDiagnostic {
        code: "GE001".to_string(),
        message: format!(
            "associated type '{assoc_type}' on trait '{trait_name}' \
             requires {expected} {param_kind} parameter(s), found {found}"
        ),
        span,
        labels: vec![GatLabel::new(
            span,
            format!("requires {expected} {param_kind} parameter(s)"),
        )],
        suggestions: vec![suggestion],
    }
}

/// Builds a diagnostic for GAT bound mismatch (GE002).
fn build_bound_mismatch_diagnostic(
    assoc_type: &str,
    expected: &str,
    found: &str,
    span: Span,
) -> GatDiagnostic {
    GatDiagnostic {
        code: "GE002".to_string(),
        message: format!(
            "bound mismatch for associated type '{assoc_type}': \
             trait requires '{expected}', impl has '{found}'"
        ),
        span,
        labels: vec![GatLabel::new(span, format!("expected '{expected}'"))],
        suggestions: vec![GatSuggestion::hint(format!(
            "ensure the impl bound matches the trait: '{expected}'",
        ))],
    }
}

/// Builds a diagnostic for lifetime capture errors (GE003).
fn build_lifetime_capture_diagnostic(
    assoc_type: &str,
    lifetime: &str,
    span: Span,
) -> GatDiagnostic {
    GatDiagnostic {
        code: "GE003".to_string(),
        message: format!(
            "borrowed data does not live long enough for GAT projection in '{assoc_type}'"
        ),
        span,
        labels: vec![GatLabel::new(span, format!("lifetime issue: {lifetime}"))],
        suggestions: vec![
            GatSuggestion::hint("consider adding lifetime bound `where Self: 'a`"),
            GatSuggestion::hint("ensure borrowed data outlives the returned type"),
        ],
    }
}

/// Builds a diagnostic for async trait object safety (GE004).
fn build_async_object_safety_diagnostic(
    trait_name: &str,
    method: &str,
    span: Span,
) -> GatDiagnostic {
    GatDiagnostic {
        code: "GE004".to_string(),
        message: format!(
            "async trait method '{method}' on '{trait_name}' \
             is not object-safe without `#[async_trait]`"
        ),
        span,
        labels: vec![GatLabel::new(
            span,
            "async method prevents object safety".to_string(),
        )],
        suggestions: vec![
            GatSuggestion::hint(format!(
                "add `#[async_trait]` to trait '{trait_name}' for automatic boxing",
            )),
            GatSuggestion::hint(format!(
                "or change '{method}' to return `Box<dyn Future<Output = T>>`",
            )),
        ],
    }
}

/// Builds a diagnostic for undefined associated type (GE005).
fn build_undefined_assoc_type_diagnostic(
    trait_name: &str,
    assoc_type: &str,
    span: Span,
) -> GatDiagnostic {
    GatDiagnostic {
        code: "GE005".to_string(),
        message: format!("trait '{trait_name}' has no associated type '{assoc_type}'"),
        span,
        labels: vec![GatLabel::new(
            span,
            format!("'{assoc_type}' not found in '{trait_name}'"),
        )],
        suggestions: vec![GatSuggestion::hint(format!(
            "add `type {assoc_type}` to trait '{trait_name}'",
        ))],
    }
}

/// Builds a diagnostic for duplicate associated type (GE006).
fn build_duplicate_assoc_type_diagnostic(
    trait_name: &str,
    name: &str,
    span: Span,
) -> GatDiagnostic {
    GatDiagnostic {
        code: "GE006".to_string(),
        message: format!("duplicate associated type '{name}' in trait '{trait_name}'"),
        span,
        labels: vec![GatLabel::new(span, "duplicate definition here".to_string())],
        suggestions: vec![GatSuggestion::hint(
            "remove the duplicate associated type definition",
        )],
    }
}

/// Builds a diagnostic for parameter kind mismatch (GE007).
fn build_param_kind_mismatch_diagnostic(
    assoc_type: &str,
    param: &str,
    expected: &str,
    found: &str,
    span: Span,
) -> GatDiagnostic {
    GatDiagnostic {
        code: "GE007".to_string(),
        message: format!(
            "parameter kind mismatch for '{param}' in '{assoc_type}': \
             expected {expected}, found {found}"
        ),
        span,
        labels: vec![GatLabel::new(
            span,
            format!("expected {expected} parameter"),
        )],
        suggestions: vec![GatSuggestion::hint(format!(
            "change parameter '{param}' to a {expected} parameter",
        ))],
    }
}

/// Builds a diagnostic for missing associated type in impl (GE008).
fn build_missing_impl_diagnostic(trait_name: &str, assoc_type: &str, span: Span) -> GatDiagnostic {
    GatDiagnostic {
        code: "GE008".to_string(),
        message: format!("impl for '{trait_name}' is missing associated type '{assoc_type}'"),
        span,
        labels: vec![GatLabel::new(
            span,
            "missing implementation here".to_string(),
        )],
        suggestions: vec![GatSuggestion::hint(format!(
            "add `type {assoc_type} = ConcreteType` to the impl block",
        ))],
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Lifetime inference
// ═══════════════════════════════════════════════════════════════════════

/// A site where a type parameter or lifetime is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageSite {
    /// The context of the usage (e.g., `"return_type"`, `"parameter"`).
    pub context: String,
    /// The type expression containing the usage.
    pub type_expr: String,
    /// Whether this usage is in a reference position.
    pub is_reference: bool,
    /// Whether the reference is mutable (only meaningful if `is_reference`).
    pub is_mutable: bool,
}

/// An inferred lifetime parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferredLifetimeParam {
    /// The inferred lifetime name.
    pub name: String,
    /// Confidence level: `"high"` if unambiguous, `"low"` if heuristic.
    pub confidence: String,
    /// Reason for the inference.
    pub reason: String,
}

/// Attempts to infer lifetime parameters from usage sites.
///
/// When a GAT is used without explicit lifetime parameters, this function
/// examines the usage context to determine if the lifetime can be
/// unambiguously inferred.
///
/// # Rules
///
/// 1. If there is exactly one reference parameter, its lifetime is used.
/// 2. If there is a `&self` parameter, `'_` (elided) is used.
/// 3. If there are multiple reference parameters, inference fails.
pub fn infer_lifetime_params(usage_sites: &[UsageSite]) -> Vec<InferredLifetimeParam> {
    let ref_sites: Vec<&UsageSite> = usage_sites.iter().filter(|s| s.is_reference).collect();

    match ref_sites.len() {
        0 => vec![],
        1 => {
            let site = ref_sites[0];
            let name = if site.context == "self" {
                "'_".to_string()
            } else {
                "'a".to_string()
            };
            vec![InferredLifetimeParam {
                name,
                confidence: "high".to_string(),
                reason: format!("single reference in {} position", site.context,),
            }]
        }
        _ => {
            // Check if one is &self — prefer that
            let self_site = ref_sites.iter().find(|s| s.context == "self");
            if let Some(s) = self_site {
                vec![InferredLifetimeParam {
                    name: "'_".to_string(),
                    confidence: "low".to_string(),
                    reason: format!(
                        "&{} self parameter takes precedence",
                        if s.is_mutable { "mut " } else { "" },
                    ),
                }]
            } else {
                // Cannot infer — multiple references, no &self
                vec![]
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GAT configuration
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for GAT feature support.
///
/// Controls whether GAT analysis is enabled and what feature flag
/// activates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatConfig {
    /// Whether GAT support is enabled.
    pub enabled: bool,
    /// The CLI flag name that enables this feature.
    pub feature_flag_name: String,
}

impl GatConfig {
    /// Creates a new GAT config with the default flag `"--gat"`.
    pub fn new(enabled: bool) -> Self {
        GatConfig {
            enabled,
            feature_flag_name: "--gat".to_string(),
        }
    }

    /// Creates an enabled GAT config.
    pub fn enabled() -> Self {
        Self::new(true)
    }

    /// Creates a disabled GAT config.
    pub fn disabled() -> Self {
        Self::new(false)
    }
}

impl Default for GatConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Performance simulation
// ═══════════════════════════════════════════════════════════════════════

/// Simulates GAT resolution for performance benchmarking.
///
/// Processes `count` type projections and returns the elapsed time.
/// Target: < 50ms for 10,000 projections.
pub fn benchmark_gat_resolution(count: usize) -> std::time::Duration {
    let start = Instant::now();

    let mut registry = super::gat::GatRegistry::new();
    let def = super::gat::AssociatedTypeDef::with_params(
        "Item",
        vec![super::gat::GatParam::lifetime("a")],
    );
    // Ignore registration errors in benchmark
    let _ = registry.register("LendingIterator", def, Span { start: 0, end: 0 });

    for i in 0..count {
        let proj = super::gat::TypeProjection::with_args(
            format!("T{i}"),
            "LendingIterator",
            "Item",
            vec!["'a".into()],
        );
        let _ = super::gat::resolve_type_projection(&registry, &proj, Span { start: 0, end: 0 });
    }

    start.elapsed()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_span() -> Span {
        Span { start: 0, end: 10 }
    }

    #[test]
    fn diagnose_missing_params() {
        let error = GatError::MissingParams {
            trait_name: "LendingIter".into(),
            assoc_type: "Item".into(),
            expected: 1,
            found: 0,
            param_kind: "lifetime".into(),
            span: dummy_span(),
        };
        let diag = diagnose_gat_error(&error);
        assert_eq!(diag.code, "GE001");
        assert!(diag.message.contains("requires 1 lifetime parameter(s)"));
        assert!(!diag.suggestions.is_empty());
        // Should suggest Item<'a>
        let suggestion = &diag.suggestions[0];
        assert!(
            suggestion
                .replacement
                .as_ref()
                .map(|r| r.contains("Item<'a>"))
                .unwrap_or(false),
        );
    }

    #[test]
    fn diagnose_bound_mismatch() {
        let error = GatError::BoundMismatch {
            assoc_type: "Item".into(),
            expected: "Self: 'a".into(),
            found: "T: Clone".into(),
            span: dummy_span(),
        };
        let diag = diagnose_gat_error(&error);
        assert_eq!(diag.code, "GE002");
        assert!(diag.message.contains("bound mismatch"));
    }

    #[test]
    fn diagnose_lifetime_capture() {
        let error = GatError::LifetimeCapture {
            assoc_type: "Item".into(),
            lifetime: "'a too short".into(),
            span: dummy_span(),
        };
        let diag = diagnose_gat_error(&error);
        assert_eq!(diag.code, "GE003");
        assert!(
            diag.message
                .contains("borrowed data does not live long enough")
        );
        assert!(diag.suggestions.len() >= 2);
        assert!(diag.suggestions[0].message.contains("where Self: 'a"));
    }

    #[test]
    fn diagnose_async_object_safety() {
        let error = GatError::AsyncTraitObjectSafety {
            trait_name: "DataSource".into(),
            method: "fetch".into(),
            span: dummy_span(),
        };
        let diag = diagnose_gat_error(&error);
        assert_eq!(diag.code, "GE004");
        assert!(diag.message.contains("not object-safe"));
        assert!(diag.suggestions[0].message.contains("#[async_trait]"));
    }

    #[test]
    fn diagnose_undefined_assoc_type() {
        let error = GatError::UndefinedAssocType {
            trait_name: "Iterator".into(),
            assoc_type: "Value".into(),
            span: dummy_span(),
        };
        let diag = diagnose_gat_error(&error);
        assert_eq!(diag.code, "GE005");
        assert!(diag.message.contains("no associated type 'Value'"));
    }

    #[test]
    fn infer_single_reference_param() {
        let sites = vec![UsageSite {
            context: "parameter".into(),
            type_expr: "&i32".into(),
            is_reference: true,
            is_mutable: false,
        }];
        let result = infer_lifetime_params(&sites);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "'a");
        assert_eq!(result[0].confidence, "high");
    }

    #[test]
    fn infer_self_reference() {
        let sites = vec![UsageSite {
            context: "self".into(),
            type_expr: "&self".into(),
            is_reference: true,
            is_mutable: false,
        }];
        let result = infer_lifetime_params(&sites);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "'_");
    }

    #[test]
    fn infer_multiple_refs_no_self_fails() {
        let sites = vec![
            UsageSite {
                context: "param1".into(),
                type_expr: "&i32".into(),
                is_reference: true,
                is_mutable: false,
            },
            UsageSite {
                context: "param2".into(),
                type_expr: "&str".into(),
                is_reference: true,
                is_mutable: false,
            },
        ];
        let result = infer_lifetime_params(&sites);
        assert!(result.is_empty());
    }

    #[test]
    fn gat_config_defaults() {
        let config = GatConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.feature_flag_name, "--gat");

        let enabled = GatConfig::enabled();
        assert!(enabled.enabled);
    }

    #[test]
    fn benchmark_gat_resolution_performance() {
        let duration = benchmark_gat_resolution(10_000);
        // Target: < 50ms for 10K projections
        // Allow generous margin in test to avoid flaky CI
        assert!(
            duration.as_millis() < 5000,
            "GAT resolution took {}ms for 10K projections (target < 50ms)",
            duration.as_millis(),
        );
    }
}
