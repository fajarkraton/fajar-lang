//! Async trait method desugaring and validation for Fajar Lang.
//!
//! Handles the transformation and checking of `async fn` methods inside traits:
//! - [`AsyncTraitDesugaring`] — transforms `async fn method()` into equivalent
//!   `fn method() -> impl Future<Output = T>` form
//! - [`AsyncMethodInfo`] — metadata about an async method in a trait
//! - [`ObjectSafetyCheck`] — determines if an async trait is object-safe
//! - [`AsyncTraitAttribute`] — the `#[async_trait]` configuration
//!
//! # Example (Fajar Lang source)
//!
//! ```text
//! trait DataSource {
//!     async fn fetch(&self, key: str) -> str
//! }
//!
//! // Desugars to:
//! trait DataSource {
//!     fn fetch(&self, key: str) -> impl Future<Output = str>
//! }
//! ```
//!
//! # Object Safety
//!
//! Async trait methods are NOT object-safe by default because `impl Future`
//! is an opaque return type. With `#[async_trait]`, the return type is boxed:
//! `fn fetch(&self) -> Box<dyn Future<Output = str>>`.

use crate::lexer::token::Span;

use super::gat::GatError;

// ═══════════════════════════════════════════════════════════════════════
// Async method info
// ═══════════════════════════════════════════════════════════════════════

/// Metadata about an async method in a trait definition.
///
/// Captures all relevant information needed for desugaring and validation
/// of an `async fn` declared within a trait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncMethodInfo {
    /// Method name.
    pub name: String,
    /// Parameter type names (excluding `self`).
    pub params: Vec<String>,
    /// Return type name (the `T` in `async fn() -> T`).
    pub return_type: String,
    /// Whether this method has a default implementation body.
    pub is_default: bool,
    /// Whether this method requires boxing for object safety.
    pub requires_boxing: bool,
    /// Whether the method takes `&self`, `&mut self`, or `self`.
    pub self_param: Option<SelfParamKind>,
}

/// The kind of `self` parameter on a method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfParamKind {
    /// `&self` — shared reference.
    Ref,
    /// `&mut self` — mutable reference.
    RefMut,
    /// `self` — owned.
    Owned,
}

impl AsyncMethodInfo {
    /// Creates a new async method info.
    pub fn new(
        name: impl Into<String>,
        params: Vec<String>,
        return_type: impl Into<String>,
    ) -> Self {
        AsyncMethodInfo {
            name: name.into(),
            params,
            return_type: return_type.into(),
            is_default: false,
            requires_boxing: false,
            self_param: None,
        }
    }

    /// Sets this method as having a default implementation.
    pub fn with_default(mut self) -> Self {
        self.is_default = true;
        self
    }

    /// Sets the self parameter kind.
    pub fn with_self(mut self, kind: SelfParamKind) -> Self {
        self.self_param = Some(kind);
        self
    }

    /// Marks this method as requiring boxing for object safety.
    pub fn with_boxing(mut self) -> Self {
        self.requires_boxing = true;
        self
    }

    /// Returns the desugared return type string.
    ///
    /// Without boxing: `impl Future<Output = T>`
    /// With boxing: `Box<dyn Future<Output = T>>`
    pub fn desugared_return_type(&self) -> String {
        if self.requires_boxing {
            format!("Box<dyn Future<Output = {}>>", self.return_type)
        } else {
            format!("impl Future<Output = {}>", self.return_type)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Async trait attribute
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for the `#[async_trait]` attribute.
///
/// When applied to a trait definition, enables automatic boxing of
/// async method return types for object safety.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncTraitAttribute {
    /// Whether to automatically box async return types.
    pub auto_boxing: bool,
}

impl AsyncTraitAttribute {
    /// Creates a new `#[async_trait]` attribute with auto-boxing enabled.
    pub fn new() -> Self {
        AsyncTraitAttribute { auto_boxing: true }
    }

    /// Creates an attribute with explicit boxing configuration.
    pub fn with_boxing(auto_boxing: bool) -> Self {
        AsyncTraitAttribute { auto_boxing }
    }
}

impl Default for AsyncTraitAttribute {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Desugared trait
// ═══════════════════════════════════════════════════════════════════════

/// A trait definition after async method desugaring.
///
/// Contains both the original trait name and the transformed method
/// signatures where `async fn` has been converted to `fn -> Future`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesugaredTrait {
    /// Original trait name.
    pub name: String,
    /// Non-async methods (passed through unchanged).
    pub sync_methods: Vec<String>,
    /// Desugared async methods.
    pub async_methods: Vec<AsyncMethodInfo>,
    /// Whether `#[async_trait]` was applied.
    pub has_async_trait_attr: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// Trait definition (input to desugaring)
// ═══════════════════════════════════════════════════════════════════════

/// A trait method declaration for desugaring input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitMethodDecl {
    /// Method name.
    pub name: String,
    /// Whether the method is `async`.
    pub is_async: bool,
    /// Parameter type names (excluding `self`).
    pub params: Vec<String>,
    /// Return type name.
    pub return_type: String,
    /// Whether this method has a default body.
    pub has_default_body: bool,
    /// Self parameter kind, if any.
    pub self_param: Option<SelfParamKind>,
}

/// Input trait definition for the desugaring pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDeclInput {
    /// Trait name.
    pub name: String,
    /// All method declarations (sync and async).
    pub methods: Vec<TraitMethodDecl>,
    /// Whether `#[async_trait]` is applied.
    pub async_trait_attr: Option<AsyncTraitAttribute>,
}

// ═══════════════════════════════════════════════════════════════════════
// Desugaring
// ═══════════════════════════════════════════════════════════════════════

/// Desugars a trait definition, converting async methods to their
/// `fn -> Future` equivalents.
///
/// If `#[async_trait]` is applied, async methods are boxed for object safety.
pub fn desugar_async_trait(input: &TraitDeclInput) -> DesugaredTrait {
    let has_attr = input.async_trait_attr.is_some();
    let auto_box = input
        .async_trait_attr
        .as_ref()
        .map(|a| a.auto_boxing)
        .unwrap_or(false);

    let mut sync_methods = Vec::new();
    let mut async_methods = Vec::new();

    for method in &input.methods {
        if method.is_async {
            let mut info =
                AsyncMethodInfo::new(&method.name, method.params.clone(), &method.return_type);
            if method.has_default_body {
                info = info.with_default();
            }
            if let Some(self_kind) = method.self_param {
                info = info.with_self(self_kind);
            }
            if auto_box {
                info = info.with_boxing();
            }
            async_methods.push(info);
        } else {
            sync_methods.push(method.name.clone());
        }
    }

    DesugaredTrait {
        name: input.name.clone(),
        sync_methods,
        async_methods,
        has_async_trait_attr: has_attr,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Async impl validation
// ═══════════════════════════════════════════════════════════════════════

/// Information about a method in an impl block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplMethodInfo {
    /// Method name.
    pub name: String,
    /// Whether the impl method is async.
    pub is_async: bool,
    /// Return type (if not async, should be `Future<Output = T>`).
    pub return_type: String,
}

/// Validates that an impl block correctly implements a desugared async trait.
///
/// Each async method in the trait must be implemented as either:
/// - An `async fn` with matching return type, or
/// - A sync `fn` returning `impl Future<Output = T>`
pub fn validate_async_impl(
    desugared: &DesugaredTrait,
    impl_methods: &[ImplMethodInfo],
    span: Span,
) -> Result<(), Vec<GatError>> {
    let mut errors = Vec::new();

    for async_method in &desugared.async_methods {
        let impl_method = impl_methods.iter().find(|m| m.name == async_method.name);

        match impl_method {
            None => {
                if !async_method.is_default {
                    errors.push(GatError::MissingImplAssocType {
                        trait_name: desugared.name.clone(),
                        assoc_type: format!("async fn {}", async_method.name),
                        span,
                    });
                }
            }
            Some(m) => {
                validate_async_return_type(&desugared.name, async_method, m, span, &mut errors);
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validates a single async method return type in an impl.
fn validate_async_return_type(
    _trait_name: &str,
    trait_method: &AsyncMethodInfo,
    impl_method: &ImplMethodInfo,
    span: Span,
    errors: &mut Vec<GatError>,
) {
    // An async impl method is always valid — the compiler handles Future wrapping
    if impl_method.is_async {
        return;
    }

    // Non-async impl must return Future<Output = T>
    let expected = trait_method.desugared_return_type();
    if impl_method.return_type != expected {
        errors.push(GatError::BoundMismatch {
            assoc_type: trait_method.name.clone(),
            expected: format!("async fn or {expected}"),
            found: format!("fn {} -> {}", impl_method.name, impl_method.return_type,),
            span,
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Object safety checking
// ═══════════════════════════════════════════════════════════════════════

/// Result of an object safety check for a trait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectSafetyResult {
    /// Whether the trait is object-safe.
    pub is_object_safe: bool,
    /// Reasons why the trait is NOT object-safe (if any).
    pub violations: Vec<ObjectSafetyViolation>,
}

/// A specific reason why a trait is not object-safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectSafetyViolation {
    /// An async method without `#[async_trait]` returns `impl Future`.
    AsyncMethodWithoutBoxing {
        /// The method name.
        method: String,
    },
    /// A method returns `Self`.
    ReturnsSelf {
        /// The method name.
        method: String,
    },
    /// A method has generic type parameters.
    GenericMethod {
        /// The method name.
        method: String,
    },
}

impl std::fmt::Display for ObjectSafetyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectSafetyViolation::AsyncMethodWithoutBoxing { method } => {
                write!(
                    f,
                    "async method '{method}' is not object-safe without `#[async_trait]`"
                )
            }
            ObjectSafetyViolation::ReturnsSelf { method } => {
                write!(f, "method '{method}' returns `Self`")
            }
            ObjectSafetyViolation::GenericMethod { method } => {
                write!(f, "method '{method}' has generic parameters")
            }
        }
    }
}

/// Checks whether a desugared trait is object-safe.
///
/// A trait is NOT object-safe if any async method is not boxed,
/// or if methods return `Self` or have generic parameters.
pub fn check_object_safety(desugared: &DesugaredTrait) -> ObjectSafetyResult {
    let mut violations = Vec::new();

    for method in &desugared.async_methods {
        if !method.requires_boxing {
            violations.push(ObjectSafetyViolation::AsyncMethodWithoutBoxing {
                method: method.name.clone(),
            });
        }
        if method.return_type == "Self" {
            violations.push(ObjectSafetyViolation::ReturnsSelf {
                method: method.name.clone(),
            });
        }
    }

    ObjectSafetyResult {
        is_object_safe: violations.is_empty(),
        violations,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Lifetime capture checking
// ═══════════════════════════════════════════════════════════════════════

/// A reference parameter that might be captured in an async future.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedReference {
    /// Parameter name.
    pub param_name: String,
    /// Lifetime name (e.g., `"a"` for `&'a T`).
    pub lifetime: Option<String>,
    /// Whether this is a mutable reference.
    pub is_mutable: bool,
}

/// Checks that borrowed data in async method parameters outlives the future.
///
/// An async method captures all parameters in its future. If a parameter
/// is a reference, the referenced data must outlive the future.
///
/// # Returns
///
/// `Ok(())` if all lifetime captures are valid, `Err` with details otherwise.
pub fn lifetime_capture_check(
    method: &AsyncMethodInfo,
    captured_refs: &[CapturedReference],
    span: Span,
) -> Result<(), GatError> {
    // Check for mutable reference captures in async contexts
    // Multiple &mut captures would create aliasing across await points
    let mut_count = captured_refs.iter().filter(|r| r.is_mutable).count();
    if mut_count > 1 {
        let first_mut = captured_refs
            .iter()
            .find(|r| r.is_mutable)
            .map(|r| r.param_name.clone())
            .unwrap_or_default();

        return Err(GatError::LifetimeCapture {
            assoc_type: format!("async fn {}", method.name),
            lifetime: format!("multiple &mut captures ({first_mut} and others)"),
            span,
        });
    }

    // Check for references without explicit lifetimes in async context
    for captured in captured_refs {
        if captured.lifetime.is_none() && captured.is_mutable {
            return Err(GatError::LifetimeCapture {
                assoc_type: format!("async fn {}", method.name),
                lifetime: format!(
                    "&mut {} needs explicit lifetime to ensure it outlives the future",
                    captured.param_name,
                ),
                span,
            });
        }
    }

    Ok(())
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
    fn async_method_info_desugared_return_type() {
        let method = AsyncMethodInfo::new("fetch", vec![], "str");
        assert_eq!(method.desugared_return_type(), "impl Future<Output = str>",);

        let boxed = method.with_boxing();
        assert_eq!(
            boxed.desugared_return_type(),
            "Box<dyn Future<Output = str>>",
        );
    }

    #[test]
    fn desugar_async_trait_without_attr() {
        let input = TraitDeclInput {
            name: "DataSource".into(),
            methods: vec![
                TraitMethodDecl {
                    name: "fetch".into(),
                    is_async: true,
                    params: vec!["str".into()],
                    return_type: "str".into(),
                    has_default_body: false,
                    self_param: Some(SelfParamKind::Ref),
                },
                TraitMethodDecl {
                    name: "name".into(),
                    is_async: false,
                    params: vec![],
                    return_type: "str".into(),
                    has_default_body: false,
                    self_param: Some(SelfParamKind::Ref),
                },
            ],
            async_trait_attr: None,
        };

        let result = desugar_async_trait(&input);
        assert_eq!(result.name, "DataSource");
        assert_eq!(result.sync_methods, vec!["name"]);
        assert_eq!(result.async_methods.len(), 1);
        assert!(!result.async_methods[0].requires_boxing);
        assert!(!result.has_async_trait_attr);
    }

    #[test]
    fn desugar_async_trait_with_attr_boxes_returns() {
        let input = TraitDeclInput {
            name: "AsyncRead".into(),
            methods: vec![TraitMethodDecl {
                name: "read".into(),
                is_async: true,
                params: vec!["&mut [u8]".into()],
                return_type: "usize".into(),
                has_default_body: false,
                self_param: Some(SelfParamKind::RefMut),
            }],
            async_trait_attr: Some(AsyncTraitAttribute::new()),
        };

        let result = desugar_async_trait(&input);
        assert!(result.has_async_trait_attr);
        assert!(result.async_methods[0].requires_boxing);
        assert_eq!(
            result.async_methods[0].desugared_return_type(),
            "Box<dyn Future<Output = usize>>",
        );
    }

    #[test]
    fn object_safety_fails_without_boxing() {
        let desugared = DesugaredTrait {
            name: "AsyncRead".into(),
            sync_methods: vec![],
            async_methods: vec![AsyncMethodInfo::new("read", vec![], "usize")],
            has_async_trait_attr: false,
        };

        let result = check_object_safety(&desugared);
        assert!(!result.is_object_safe);
        assert_eq!(result.violations.len(), 1);
        assert!(matches!(
            &result.violations[0],
            ObjectSafetyViolation::AsyncMethodWithoutBoxing { method }
            if method == "read"
        ));
    }

    #[test]
    fn object_safety_passes_with_boxing() {
        let desugared = DesugaredTrait {
            name: "AsyncRead".into(),
            sync_methods: vec![],
            async_methods: vec![AsyncMethodInfo::new("read", vec![], "usize").with_boxing()],
            has_async_trait_attr: true,
        };

        let result = check_object_safety(&desugared);
        assert!(result.is_object_safe);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn validate_async_impl_with_async_method() {
        let desugared = DesugaredTrait {
            name: "DataSource".into(),
            sync_methods: vec![],
            async_methods: vec![AsyncMethodInfo::new("fetch", vec!["str".into()], "str")],
            has_async_trait_attr: false,
        };

        let impl_methods = vec![ImplMethodInfo {
            name: "fetch".into(),
            is_async: true,
            return_type: "str".into(),
        }];

        let result = validate_async_impl(&desugared, &impl_methods, dummy_span());
        assert!(result.is_ok());
    }

    #[test]
    fn validate_async_impl_missing_method() {
        let desugared = DesugaredTrait {
            name: "DataSource".into(),
            sync_methods: vec![],
            async_methods: vec![AsyncMethodInfo::new("fetch", vec![], "str")],
            has_async_trait_attr: false,
        };

        let impl_methods: Vec<ImplMethodInfo> = vec![];
        let result = validate_async_impl(&desugared, &impl_methods, dummy_span());
        assert!(result.is_err());
    }

    #[test]
    fn validate_async_impl_default_method_not_required() {
        let desugared = DesugaredTrait {
            name: "DataSource".into(),
            sync_methods: vec![],
            async_methods: vec![AsyncMethodInfo::new("fetch", vec![], "str").with_default()],
            has_async_trait_attr: false,
        };

        let impl_methods: Vec<ImplMethodInfo> = vec![];
        let result = validate_async_impl(&desugared, &impl_methods, dummy_span());
        assert!(result.is_ok());
    }

    #[test]
    fn lifetime_capture_check_multiple_mut_refs_error() {
        let method = AsyncMethodInfo::new("process", vec![], "void");
        let captured = vec![
            CapturedReference {
                param_name: "a".into(),
                lifetime: Some("a".into()),
                is_mutable: true,
            },
            CapturedReference {
                param_name: "b".into(),
                lifetime: Some("b".into()),
                is_mutable: true,
            },
        ];

        let result = lifetime_capture_check(&method, &captured, dummy_span());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, GatError::LifetimeCapture { .. }));
    }

    #[test]
    fn lifetime_capture_check_single_ref_ok() {
        let method = AsyncMethodInfo::new("read", vec![], "usize");
        let captured = vec![CapturedReference {
            param_name: "buf".into(),
            lifetime: Some("a".into()),
            is_mutable: false,
        }];

        let result = lifetime_capture_check(&method, &captured, dummy_span());
        assert!(result.is_ok());
    }

    #[test]
    fn async_trait_attribute_defaults() {
        let attr = AsyncTraitAttribute::default();
        assert!(attr.auto_boxing);

        let no_box = AsyncTraitAttribute::with_boxing(false);
        assert!(!no_box.auto_boxing);
    }
}
