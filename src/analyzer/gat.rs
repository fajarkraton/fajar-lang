//! Generic Associated Types (GAT) — type system extension for Fajar Lang.
//!
//! Provides the core data structures and validation logic for GAT support:
//! - [`GatParam`] — a generic parameter on an associated type (lifetime or type)
//! - [`AssociatedTypeDef`] — an associated type definition with GAT parameters
//! - [`TypeProjection`] — a qualified type projection like `<T as Trait>::Item<'a>`
//! - [`GatRegistry`] — stores trait → associated type definitions
//! - [`GatError`] — errors specific to GAT validation
//!
//! # Example (Fajar Lang source)
//!
//! ```text
//! trait LendingIterator {
//!     type Item<'a> where Self: 'a
//!     fn next(&'a mut self) -> Option<Self::Item<'a>>
//! }
//! ```

use std::collections::HashMap;

use crate::lexer::token::Span;

// ═══════════════════════════════════════════════════════════════════════
// GAT parameter kinds
// ═══════════════════════════════════════════════════════════════════════

/// The kind of a generic parameter on an associated type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatParamKind {
    /// A lifetime parameter (e.g., `'a`).
    Lifetime,
    /// A type parameter (e.g., `T`).
    Type,
}

impl std::fmt::Display for GatParamKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GatParamKind::Lifetime => write!(f, "lifetime"),
            GatParamKind::Type => write!(f, "type"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GAT parameter
// ═══════════════════════════════════════════════════════════════════════

/// A generic parameter on an associated type definition.
///
/// Represents either a lifetime parameter (`'a`) or a type parameter (`T`)
/// with optional trait bounds.
///
/// # Examples
///
/// ```text
/// type Item<'a>           → GatParam { name: "a", kind: Lifetime, bounds: [] }
/// type Output<T: Clone>   → GatParam { name: "T", kind: Type, bounds: ["Clone"] }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatParam {
    /// Parameter name (without leading `'` for lifetimes).
    pub name: String,
    /// Whether this is a lifetime or type parameter.
    pub kind: GatParamKind,
    /// Trait bounds on the parameter (e.g., `["Clone", "Send"]`).
    pub bounds: Vec<String>,
}

impl GatParam {
    /// Creates a new lifetime GAT parameter.
    pub fn lifetime(name: impl Into<String>) -> Self {
        GatParam {
            name: name.into(),
            kind: GatParamKind::Lifetime,
            bounds: Vec::new(),
        }
    }

    /// Creates a new type GAT parameter with optional bounds.
    pub fn type_param(name: impl Into<String>, bounds: Vec<String>) -> Self {
        GatParam {
            name: name.into(),
            kind: GatParamKind::Type,
            bounds,
        }
    }

    /// Returns the display name (with `'` prefix for lifetimes).
    pub fn display_name(&self) -> String {
        match self.kind {
            GatParamKind::Lifetime => format!("'{}", self.name),
            GatParamKind::Type => self.name.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GAT where clause
// ═══════════════════════════════════════════════════════════════════════

/// A where clause on an associated type definition.
///
/// Constrains the relationship between the associated type's parameters
/// and the implementing type.
///
/// # Example
///
/// ```text
/// type Item<'a> where Self: 'a
/// → GatWhereClause { subject: "Self", bound: "'a" }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatWhereClause {
    /// The subject of the bound (e.g., `"Self"`, `"T"`).
    pub subject: String,
    /// The bound constraint (e.g., `"'a"`, `"Clone"`).
    pub bound: String,
}

impl GatWhereClause {
    /// Creates a new GAT where clause.
    pub fn new(subject: impl Into<String>, bound: impl Into<String>) -> Self {
        GatWhereClause {
            subject: subject.into(),
            bound: bound.into(),
        }
    }
}

impl std::fmt::Display for GatWhereClause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.subject, self.bound)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Associated type definition
// ═══════════════════════════════════════════════════════════════════════

/// An associated type definition within a trait, optionally with GAT parameters.
///
/// In a trait definition:
/// ```text
/// trait Collection {
///     type Item                                    // simple associated type
///     type Iter<'a> where Self: 'a                 // GAT with lifetime
///     type Output<T: Clone> = Vec<T>               // GAT with default
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociatedTypeDef {
    /// The name of the associated type (e.g., `"Item"`).
    pub name: String,
    /// Generic parameters on this associated type (makes it a GAT).
    pub generic_params: Vec<GatParam>,
    /// Where clauses constraining the associated type's parameters.
    pub where_clauses: Vec<GatWhereClause>,
    /// Optional default type (e.g., `"Vec<T>"` in `type Output<T> = Vec<T>`).
    pub default_type: Option<String>,
}

impl AssociatedTypeDef {
    /// Creates a simple associated type with no GAT parameters.
    pub fn simple(name: impl Into<String>) -> Self {
        AssociatedTypeDef {
            name: name.into(),
            generic_params: Vec::new(),
            where_clauses: Vec::new(),
            default_type: None,
        }
    }

    /// Creates a GAT associated type with the given parameters.
    pub fn with_params(name: impl Into<String>, params: Vec<GatParam>) -> Self {
        AssociatedTypeDef {
            name: name.into(),
            generic_params: params,
            where_clauses: Vec::new(),
            default_type: None,
        }
    }

    /// Adds a where clause to this associated type definition.
    pub fn add_where_clause(&mut self, clause: GatWhereClause) {
        self.where_clauses.push(clause);
    }

    /// Sets the default type for this associated type.
    pub fn set_default(&mut self, default: impl Into<String>) {
        self.default_type = Some(default.into());
    }

    /// Returns `true` if this is a GAT (has generic parameters).
    pub fn is_gat(&self) -> bool {
        !self.generic_params.is_empty()
    }

    /// Returns the number of lifetime parameters.
    pub fn lifetime_param_count(&self) -> usize {
        self.generic_params
            .iter()
            .filter(|p| p.kind == GatParamKind::Lifetime)
            .count()
    }

    /// Returns the number of type parameters.
    pub fn type_param_count(&self) -> usize {
        self.generic_params
            .iter()
            .filter(|p| p.kind == GatParamKind::Type)
            .count()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Type projection
// ═══════════════════════════════════════════════════════════════════════

/// A qualified type projection: `<T as Trait>::AssocType<Args>`.
///
/// Represents a reference to an associated type through a trait bound,
/// optionally with GAT arguments.
///
/// # Examples
///
/// ```text
/// <T as Iterator>::Item          → no GAT args
/// <T as LendingIter>::Item<'a>   → with lifetime arg
/// <T as Collection>::Output<u32> → with type arg
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeProjection {
    /// The base type (e.g., `"T"`, `"Vec<i32>"`).
    pub base_type: String,
    /// The trait name (e.g., `"Iterator"`, `"LendingIterator"`).
    pub trait_name: String,
    /// The associated type name (e.g., `"Item"`, `"Output"`).
    pub assoc_type: String,
    /// Arguments to the GAT (e.g., `["'a"]`, `["u32"]`).
    pub args: Vec<String>,
}

impl TypeProjection {
    /// Creates a simple type projection with no GAT arguments.
    pub fn simple(
        base_type: impl Into<String>,
        trait_name: impl Into<String>,
        assoc_type: impl Into<String>,
    ) -> Self {
        TypeProjection {
            base_type: base_type.into(),
            trait_name: trait_name.into(),
            assoc_type: assoc_type.into(),
            args: Vec::new(),
        }
    }

    /// Creates a type projection with GAT arguments.
    pub fn with_args(
        base_type: impl Into<String>,
        trait_name: impl Into<String>,
        assoc_type: impl Into<String>,
        args: Vec<String>,
    ) -> Self {
        TypeProjection {
            base_type: base_type.into(),
            trait_name: trait_name.into(),
            assoc_type: assoc_type.into(),
            args,
        }
    }
}

impl std::fmt::Display for TypeProjection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<{} as {}>::{}",
            self.base_type, self.trait_name, self.assoc_type
        )?;
        if !self.args.is_empty() {
            write!(f, "<{}>", self.args.join(", "))?;
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Resolved type from projection
// ═══════════════════════════════════════════════════════════════════════

/// The result of resolving a type projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedType {
    /// The resolved concrete type name.
    pub name: String,
    /// Whether the resolution was through a default type.
    pub from_default: bool,
    /// The trait that provided the resolution.
    pub source_trait: String,
}

// ═══════════════════════════════════════════════════════════════════════
// GAT errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors specific to Generic Associated Type validation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum GatError {
    /// GE001: Required GAT parameters are missing in usage.
    #[error("GE001: associated type '{assoc_type}' on trait '{trait_name}' requires {expected} {param_kind} parameter(s), found {found}")]
    MissingParams {
        /// The trait name.
        trait_name: String,
        /// The associated type name.
        assoc_type: String,
        /// Expected parameter count.
        expected: usize,
        /// Actual parameter count.
        found: usize,
        /// Kind of missing parameter.
        param_kind: String,
        /// Source location.
        span: Span,
    },

    /// GE002: GAT bound does not match between trait definition and impl.
    #[error("GE002: bound mismatch for associated type '{assoc_type}': trait requires '{expected}', impl has '{found}'")]
    BoundMismatch {
        /// The associated type name.
        assoc_type: String,
        /// Expected bound.
        expected: String,
        /// Actual bound in impl.
        found: String,
        /// Source location.
        span: Span,
    },

    /// GE003: Borrowed data does not live long enough for GAT projection.
    #[error("GE003: lifetime capture error in '{assoc_type}': borrowed data does not live long enough for GAT projection")]
    LifetimeCapture {
        /// The associated type name.
        assoc_type: String,
        /// The lifetime that's too short.
        lifetime: String,
        /// Source location.
        span: Span,
    },

    /// GE004: Async trait method requires boxing for object safety.
    #[error("GE004: async trait method '{method}' on '{trait_name}' is not object-safe without `#[async_trait]`")]
    AsyncTraitObjectSafety {
        /// The trait name.
        trait_name: String,
        /// The method name.
        method: String,
        /// Source location.
        span: Span,
    },

    /// GE005: Associated type not found in trait definition.
    #[error("GE005: trait '{trait_name}' has no associated type '{assoc_type}'")]
    UndefinedAssocType {
        /// The trait name.
        trait_name: String,
        /// The associated type name.
        assoc_type: String,
        /// Source location.
        span: Span,
    },

    /// GE006: Duplicate associated type definition.
    #[error("GE006: duplicate associated type '{name}' in trait '{trait_name}'")]
    DuplicateAssocType {
        /// The trait name.
        trait_name: String,
        /// The associated type name.
        name: String,
        /// Source location.
        span: Span,
    },

    /// GE007: GAT parameter kind mismatch (lifetime vs type).
    #[error("GE007: parameter kind mismatch for '{param}' in '{assoc_type}': expected {expected}, found {found}")]
    ParamKindMismatch {
        /// The associated type name.
        assoc_type: String,
        /// The parameter name.
        param: String,
        /// Expected kind.
        expected: String,
        /// Actual kind.
        found: String,
        /// Source location.
        span: Span,
    },

    /// GE008: Missing associated type implementation.
    #[error("GE008: impl for '{trait_name}' is missing associated type '{assoc_type}'")]
    MissingImplAssocType {
        /// The trait name.
        trait_name: String,
        /// The associated type name.
        assoc_type: String,
        /// Source location.
        span: Span,
    },
}

impl GatError {
    /// Returns the source span of this error.
    pub fn span(&self) -> Span {
        match self {
            GatError::MissingParams { span, .. }
            | GatError::BoundMismatch { span, .. }
            | GatError::LifetimeCapture { span, .. }
            | GatError::AsyncTraitObjectSafety { span, .. }
            | GatError::UndefinedAssocType { span, .. }
            | GatError::DuplicateAssocType { span, .. }
            | GatError::ParamKindMismatch { span, .. }
            | GatError::MissingImplAssocType { span, .. } => *span,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GAT registry
// ═══════════════════════════════════════════════════════════════════════

/// Registry of trait associated type definitions with their GAT parameters.
///
/// Stores the mapping from `(trait_name, assoc_type_name)` to the full
/// [`AssociatedTypeDef`], enabling validation and resolution.
#[derive(Debug, Clone, Default)]
pub struct GatRegistry {
    /// Map from trait name to its associated type definitions.
    trait_assoc_types: HashMap<String, Vec<AssociatedTypeDef>>,
}

impl GatRegistry {
    /// Creates an empty GAT registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an associated type definition for a trait.
    ///
    /// Returns `Err` if the associated type name is already registered
    /// for this trait.
    pub fn register(
        &mut self,
        trait_name: &str,
        assoc_type: AssociatedTypeDef,
        span: Span,
    ) -> Result<(), GatError> {
        let entries = self
            .trait_assoc_types
            .entry(trait_name.to_string())
            .or_default();

        if entries.iter().any(|a| a.name == assoc_type.name) {
            return Err(GatError::DuplicateAssocType {
                trait_name: trait_name.to_string(),
                name: assoc_type.name.clone(),
                span,
            });
        }

        entries.push(assoc_type);
        Ok(())
    }

    /// Looks up an associated type definition by trait and type name.
    pub fn lookup(&self, trait_name: &str, assoc_type: &str) -> Option<&AssociatedTypeDef> {
        self.trait_assoc_types
            .get(trait_name)
            .and_then(|defs| defs.iter().find(|d| d.name == assoc_type))
    }

    /// Returns all associated types for a trait.
    pub fn trait_assoc_types(&self, trait_name: &str) -> &[AssociatedTypeDef] {
        self.trait_assoc_types
            .get(trait_name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns `true` if the trait has any GAT-parameterized associated types.
    pub fn has_gats(&self, trait_name: &str) -> bool {
        self.trait_assoc_types
            .get(trait_name)
            .map(|defs| defs.iter().any(|d| d.is_gat()))
            .unwrap_or(false)
    }

    /// Returns the number of registered traits.
    pub fn trait_count(&self) -> usize {
        self.trait_assoc_types.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Type projection resolution
// ═══════════════════════════════════════════════════════════════════════

/// Resolves a type projection against the GAT registry.
///
/// Given `<T as Trait>::Item<'a>`, looks up the associated type definition
/// in the registry and validates that the provided arguments match the
/// expected parameters.
pub fn resolve_type_projection(
    registry: &GatRegistry,
    projection: &TypeProjection,
    span: Span,
) -> Result<ResolvedType, GatError> {
    let assoc_def = registry
        .lookup(&projection.trait_name, &projection.assoc_type)
        .ok_or_else(|| GatError::UndefinedAssocType {
            trait_name: projection.trait_name.clone(),
            assoc_type: projection.assoc_type.clone(),
            span,
        })?;

    validate_projection_args(assoc_def, projection, span)?;

    let name = if let Some(ref default) = assoc_def.default_type {
        default.clone()
    } else {
        format!("{}", projection)
    };

    Ok(ResolvedType {
        name,
        from_default: assoc_def.default_type.is_some(),
        source_trait: projection.trait_name.clone(),
    })
}

/// Validates that a type projection's arguments match the associated type's parameters.
fn validate_projection_args(
    assoc_def: &AssociatedTypeDef,
    projection: &TypeProjection,
    span: Span,
) -> Result<(), GatError> {
    let expected_count = assoc_def.generic_params.len();
    let found_count = projection.args.len();

    if expected_count != found_count {
        let param_kind = if assoc_def.lifetime_param_count() > 0 {
            "generic"
        } else {
            "type"
        };
        return Err(GatError::MissingParams {
            trait_name: projection.trait_name.clone(),
            assoc_type: projection.assoc_type.clone(),
            expected: expected_count,
            found: found_count,
            param_kind: param_kind.to_string(),
            span,
        });
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// GAT impl validation
// ═══════════════════════════════════════════════════════════════════════

/// Information about an associated type in an impl block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplAssocType {
    /// The associated type name.
    pub name: String,
    /// The number of generic parameters provided.
    pub param_count: usize,
    /// The parameter kinds (Lifetime or Type).
    pub param_kinds: Vec<GatParamKind>,
    /// The concrete type assigned.
    pub concrete_type: String,
}

/// Validates that an impl block's associated types match the trait definition.
///
/// Checks:
/// - All required associated types are present
/// - GAT parameter counts match
/// - GAT parameter kinds match (lifetime vs type)
pub fn validate_gat_impl(
    registry: &GatRegistry,
    trait_name: &str,
    impl_assoc_types: &[ImplAssocType],
    span: Span,
) -> Result<(), Vec<GatError>> {
    let trait_defs = registry.trait_assoc_types(trait_name);
    let mut errors = Vec::new();

    for trait_def in trait_defs {
        let impl_type = impl_assoc_types.iter().find(|i| i.name == trait_def.name);
        validate_single_assoc_type(trait_name, trait_def, impl_type, span, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validates a single associated type from a trait against its impl.
fn validate_single_assoc_type(
    trait_name: &str,
    trait_def: &AssociatedTypeDef,
    impl_type: Option<&ImplAssocType>,
    span: Span,
    errors: &mut Vec<GatError>,
) {
    match impl_type {
        None => {
            if trait_def.default_type.is_none() {
                errors.push(GatError::MissingImplAssocType {
                    trait_name: trait_name.to_string(),
                    assoc_type: trait_def.name.clone(),
                    span,
                });
            }
        }
        Some(impl_at) => {
            let expected = trait_def.generic_params.len();
            if expected != impl_at.param_count {
                errors.push(GatError::MissingParams {
                    trait_name: trait_name.to_string(),
                    assoc_type: trait_def.name.clone(),
                    expected,
                    found: impl_at.param_count,
                    param_kind: "generic".to_string(),
                    span,
                });
                return;
            }
            validate_param_kinds(
                trait_name,
                &trait_def.name,
                &trait_def.generic_params,
                &impl_at.param_kinds,
                span,
                errors,
            );
        }
    }
}

/// Validates that parameter kinds in an impl match the trait definition.
fn validate_param_kinds(
    _trait_name: &str,
    assoc_type_name: &str,
    trait_params: &[GatParam],
    impl_kinds: &[GatParamKind],
    span: Span,
    errors: &mut Vec<GatError>,
) {
    for (i, (trait_param, impl_kind)) in trait_params.iter().zip(impl_kinds.iter()).enumerate() {
        if trait_param.kind != *impl_kind {
            errors.push(GatError::ParamKindMismatch {
                assoc_type: assoc_type_name.to_string(),
                param: trait_params
                    .get(i)
                    .map(|p| p.display_name())
                    .unwrap_or_else(|| format!("#{i}")),
                expected: trait_param.kind.to_string(),
                found: impl_kind.to_string(),
                span,
            });
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_span() -> Span {
        Span { start: 0, end: 0 }
    }

    #[test]
    fn gat_param_lifetime_creation() {
        let p = GatParam::lifetime("a");
        assert_eq!(p.name, "a");
        assert_eq!(p.kind, GatParamKind::Lifetime);
        assert!(p.bounds.is_empty());
        assert_eq!(p.display_name(), "'a");
    }

    #[test]
    fn gat_param_type_with_bounds() {
        let p = GatParam::type_param("T", vec!["Clone".into(), "Send".into()]);
        assert_eq!(p.name, "T");
        assert_eq!(p.kind, GatParamKind::Type);
        assert_eq!(p.bounds, vec!["Clone", "Send"]);
        assert_eq!(p.display_name(), "T");
    }

    #[test]
    fn associated_type_simple_vs_gat() {
        let simple = AssociatedTypeDef::simple("Item");
        assert!(!simple.is_gat());
        assert_eq!(simple.lifetime_param_count(), 0);
        assert_eq!(simple.type_param_count(), 0);

        let gat = AssociatedTypeDef::with_params("Item", vec![GatParam::lifetime("a")]);
        assert!(gat.is_gat());
        assert_eq!(gat.lifetime_param_count(), 1);
        assert_eq!(gat.type_param_count(), 0);
    }

    #[test]
    fn associated_type_with_default_and_where_clause() {
        let mut def = AssociatedTypeDef::with_params(
            "Output",
            vec![GatParam::type_param("T", vec!["Clone".into()])],
        );
        def.add_where_clause(GatWhereClause::new("Self", "'a"));
        def.set_default("Vec<T>");

        assert_eq!(def.default_type, Some("Vec<T>".to_string()));
        assert_eq!(def.where_clauses.len(), 1);
        assert_eq!(def.where_clauses[0].to_string(), "Self: 'a");
    }

    #[test]
    fn type_projection_display() {
        let proj = TypeProjection::simple("T", "Iterator", "Item");
        assert_eq!(format!("{proj}"), "<T as Iterator>::Item");

        let proj_gat = TypeProjection::with_args("T", "LendingIter", "Item", vec!["'a".into()]);
        assert_eq!(format!("{proj_gat}"), "<T as LendingIter>::Item<'a>");
    }

    #[test]
    fn registry_register_and_lookup() {
        let mut registry = GatRegistry::new();
        let def = AssociatedTypeDef::with_params("Item", vec![GatParam::lifetime("a")]);
        registry
            .register("LendingIterator", def, dummy_span())
            .unwrap();

        let result = registry.lookup("LendingIterator", "Item");
        assert!(result.is_some());
        assert!(result.unwrap().is_gat());
        assert!(registry.has_gats("LendingIterator"));
        assert_eq!(registry.trait_count(), 1);
    }

    #[test]
    fn registry_duplicate_assoc_type_error() {
        let mut registry = GatRegistry::new();
        let def1 = AssociatedTypeDef::simple("Item");
        registry.register("Iterator", def1, dummy_span()).unwrap();

        let def2 = AssociatedTypeDef::simple("Item");
        let result = registry.register("Iterator", def2, dummy_span());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, GatError::DuplicateAssocType { .. }));
    }

    #[test]
    fn resolve_projection_success() {
        let mut registry = GatRegistry::new();
        let def = AssociatedTypeDef::with_params("Item", vec![GatParam::lifetime("a")]);
        registry.register("LendingIter", def, dummy_span()).unwrap();

        let proj = TypeProjection::with_args("T", "LendingIter", "Item", vec!["'a".into()]);
        let resolved = resolve_type_projection(&registry, &proj, dummy_span());
        assert!(resolved.is_ok());
        let r = resolved.unwrap();
        assert_eq!(r.source_trait, "LendingIter");
        assert!(!r.from_default);
    }

    #[test]
    fn resolve_projection_missing_args() {
        let mut registry = GatRegistry::new();
        let def = AssociatedTypeDef::with_params("Item", vec![GatParam::lifetime("a")]);
        registry.register("LendingIter", def, dummy_span()).unwrap();

        let proj = TypeProjection::simple("T", "LendingIter", "Item");
        let result = resolve_type_projection(&registry, &proj, dummy_span());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            GatError::MissingParams {
                expected: 1,
                found: 0,
                ..
            }
        ));
    }

    #[test]
    fn validate_gat_impl_all_present() {
        let mut registry = GatRegistry::new();
        let def = AssociatedTypeDef::with_params("Item", vec![GatParam::lifetime("a")]);
        registry.register("LendingIter", def, dummy_span()).unwrap();

        let impl_types = vec![ImplAssocType {
            name: "Item".to_string(),
            param_count: 1,
            param_kinds: vec![GatParamKind::Lifetime],
            concrete_type: "&'a str".to_string(),
        }];

        let result = validate_gat_impl(&registry, "LendingIter", &impl_types, dummy_span());
        assert!(result.is_ok());
    }

    #[test]
    fn validate_gat_impl_missing_assoc_type() {
        let mut registry = GatRegistry::new();
        let def = AssociatedTypeDef::simple("Item");
        registry.register("Iterator", def, dummy_span()).unwrap();

        let impl_types: Vec<ImplAssocType> = vec![];
        let result = validate_gat_impl(&registry, "Iterator", &impl_types, dummy_span());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], GatError::MissingImplAssocType { .. }));
    }
}
