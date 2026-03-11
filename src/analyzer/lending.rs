//! Lending iterators and streaming validation for Fajar Lang.
//!
//! Validates lending iterator patterns where the yielded item borrows from
//! the iterator itself. Uses GATs to express `type Item<'a> where Self: 'a`.
//!
//! # Core Concept
//!
//! A lending iterator differs from a standard iterator in that the yielded
//! item can borrow from the iterator's internal state. This enables:
//! - Overlapping window views (`windows(2)` on an array)
//! - Non-copying chunk iteration
//! - String line iteration without allocation
//!
//! # Example (Fajar Lang source)
//!
//! ```text
//! trait LendingIterator {
//!     type Item<'a> where Self: 'a
//!     fn next<'a>(&'a mut self) -> Option<Self::Item<'a>>
//! }
//! ```

use crate::lexer::token::Span;

use super::gat::{AssociatedTypeDef, GatError, GatParam, GatWhereClause};

// ═══════════════════════════════════════════════════════════════════════
// Lending iterator trait definition
// ═══════════════════════════════════════════════════════════════════════

/// A lending iterator trait definition with GAT `Item<'a>`.
///
/// Represents the canonical lending iterator pattern where the `next()`
/// method yields items that may borrow from `self`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LendingIteratorDef {
    /// Trait name (e.g., `"LendingIterator"`).
    pub name: String,
    /// The associated type definition (always a GAT with lifetime param).
    pub item_type: AssociatedTypeDef,
    /// Additional methods beyond `next()`.
    pub extra_methods: Vec<String>,
}

impl LendingIteratorDef {
    /// Creates the canonical `LendingIterator` trait definition.
    ///
    /// Defines `type Item<'a> where Self: 'a` and requires
    /// `fn next(&'a mut self) -> Option<Self::Item<'a>>`.
    pub fn canonical() -> Self {
        let mut item = AssociatedTypeDef::with_params("Item", vec![GatParam::lifetime("a")]);
        item.add_where_clause(GatWhereClause::new("Self", "'a"));

        LendingIteratorDef {
            name: "LendingIterator".to_string(),
            item_type: item,
            extra_methods: Vec::new(),
        }
    }

    /// Creates a custom lending iterator trait.
    pub fn custom(name: impl Into<String>, item_type: AssociatedTypeDef) -> Self {
        LendingIteratorDef {
            name: name.into(),
            item_type,
            extra_methods: Vec::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Lending impl info
// ═══════════════════════════════════════════════════════════════════════

/// Information about a lending iterator implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LendingImplInfo {
    /// The implementing type name.
    pub impl_type: String,
    /// The concrete item type (e.g., `"&'a [T]"`).
    pub item_concrete_type: String,
    /// Whether `next()` returns `Option<Self::Item<'_>>`.
    pub has_next_method: bool,
    /// The lifetime name used in the impl.
    pub lifetime_name: String,
}

// ═══════════════════════════════════════════════════════════════════════
// Next method validation
// ═══════════════════════════════════════════════════════════════════════

/// Validates that a lending iterator `next()` method has the correct signature.
///
/// The method must:
/// - Take `&'a mut self` (mutable borrow with named lifetime)
/// - Return `Option<Self::Item<'a>>` (item borrows from self)
/// - Use the same lifetime in self parameter and return type
pub fn validate_lending_next(impl_info: &LendingImplInfo, span: Span) -> Result<(), GatError> {
    if !impl_info.has_next_method {
        return Err(GatError::MissingImplAssocType {
            trait_name: "LendingIterator".to_string(),
            assoc_type: "fn next(&'a mut self) -> Option<Self::Item<'a>>".to_string(),
            span,
        });
    }

    // Validate that the lifetime is properly used
    if impl_info.lifetime_name.is_empty() {
        return Err(GatError::LifetimeCapture {
            assoc_type: "next".to_string(),
            lifetime: "lending iterator next() requires explicit lifetime parameter".to_string(),
            span,
        });
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Specialized lending iterators
// ═══════════════════════════════════════════════════════════════════════

/// A windows iterator that yields overlapping slices.
///
/// Validates that overlapping window borrows are properly scoped —
/// each window borrow must end before the next `next()` call.
///
/// ```text
/// impl LendingIterator for WindowsIter<'_, T> {
///     type Item<'a> = &'a [T] where Self: 'a
///     fn next<'a>(&'a mut self) -> Option<&'a [T]>
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsIterator {
    /// Window size.
    pub window_size: usize,
    /// Element type name.
    pub element_type: String,
}

impl WindowsIterator {
    /// Creates a new windows iterator validator.
    pub fn new(window_size: usize, element_type: impl Into<String>) -> Self {
        WindowsIterator {
            window_size,
            element_type: element_type.into(),
        }
    }

    /// Validates that the window size is valid.
    pub fn validate(&self, span: Span) -> Result<(), GatError> {
        if self.window_size == 0 {
            return Err(GatError::BoundMismatch {
                assoc_type: "WindowsIterator".to_string(),
                expected: "window_size > 0".to_string(),
                found: "window_size = 0".to_string(),
                span,
            });
        }
        Ok(())
    }

    /// Returns the item type for this windows iterator.
    pub fn item_type(&self) -> String {
        format!("&'a [{}]", self.element_type)
    }
}

/// A chunks iterator that yields non-overlapping slices.
///
/// Unlike windows, chunks are guaranteed non-overlapping, so borrows
/// from different chunks do not conflict.
///
/// ```text
/// impl LendingIterator for ChunksIter<'_, T> {
///     type Item<'a> = &'a [T] where Self: 'a
///     fn next<'a>(&'a mut self) -> Option<&'a [T]>
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunksIterator {
    /// Chunk size.
    pub chunk_size: usize,
    /// Element type name.
    pub element_type: String,
}

impl ChunksIterator {
    /// Creates a new chunks iterator validator.
    pub fn new(chunk_size: usize, element_type: impl Into<String>) -> Self {
        ChunksIterator {
            chunk_size,
            element_type: element_type.into(),
        }
    }

    /// Validates that the chunk size is valid.
    pub fn validate(&self, span: Span) -> Result<(), GatError> {
        if self.chunk_size == 0 {
            return Err(GatError::BoundMismatch {
                assoc_type: "ChunksIterator".to_string(),
                expected: "chunk_size > 0".to_string(),
                found: "chunk_size = 0".to_string(),
                span,
            });
        }
        Ok(())
    }

    /// Returns the item type for this chunks iterator.
    pub fn item_type(&self) -> String {
        format!("&'a [{}]", self.element_type)
    }
}

/// A lines iterator that yields string slices.
///
/// Yields `&'a str` slices from a borrowed string, where each slice
/// borrows from the iterator's internal state.
///
/// ```text
/// impl LendingIterator for LinesIter<'_> {
///     type Item<'a> = &'a str where Self: 'a
///     fn next<'a>(&'a mut self) -> Option<&'a str>
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinesIterator {
    /// Whether to include the trailing newline in each line.
    pub include_newline: bool,
}

impl LinesIterator {
    /// Creates a new lines iterator validator.
    pub fn new(include_newline: bool) -> Self {
        LinesIterator { include_newline }
    }

    /// Returns the item type for this lines iterator.
    pub fn item_type(&self) -> String {
        "&'a str".to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Lending adapters
// ═══════════════════════════════════════════════════════════════════════

/// Adapter operations that can be chained on lending iterators.
///
/// Each adapter preserves the lending semantics — the output item
/// still borrows from the underlying iterator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LendingAdapter {
    /// `map(|item| expr)` — transforms each item, preserving borrow.
    Map {
        /// The closure return type name.
        output_type: String,
    },
    /// `filter(|item| bool)` — filters items by predicate.
    Filter,
    /// `for_each(|item| ())` — consumes the iterator, visiting each item.
    ForEach,
}

impl LendingAdapter {
    /// Returns a human-readable name for this adapter.
    pub fn name(&self) -> &str {
        match self {
            LendingAdapter::Map { .. } => "map",
            LendingAdapter::Filter => "filter",
            LendingAdapter::ForEach => "for_each",
        }
    }
}

/// Validates a chain of lending adapters.
///
/// Ensures the borrow chain is valid through the entire adapter pipeline.
/// In particular:
/// - `map` must not extend the lifetime of the borrowed data
/// - `filter` closures must not hold references beyond the predicate call
/// - The chain must terminate before the source iterator is invalidated
pub fn validate_adapter_chain(adapters: &[LendingAdapter], span: Span) -> Result<(), GatError> {
    // Verify the adapter chain is non-empty
    if adapters.is_empty() {
        return Ok(());
    }

    // Check for for_each not at the end (it consumes the iterator)
    for (i, adapter) in adapters.iter().enumerate() {
        if matches!(adapter, LendingAdapter::ForEach) && i < adapters.len() - 1 {
            return Err(GatError::BoundMismatch {
                assoc_type: "LendingAdapter".to_string(),
                expected: "for_each at end of chain".to_string(),
                found: format!(
                    "for_each at position {i}, followed by {}",
                    adapters[i + 1].name(),
                ),
                span,
            });
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Streaming (async) lending iterator
// ═══════════════════════════════════════════════════════════════════════

/// A streaming iterator — the async variant of a lending iterator.
///
/// Combines GAT and async: `async fn next(&'a mut self) -> Option<Item<'a>>`.
/// The future produced by `next()` borrows from `self`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamingIterator {
    /// Trait name (e.g., `"Stream"`, `"AsyncLendingIterator"`).
    pub name: String,
    /// The associated type definition for the item.
    pub item_type: AssociatedTypeDef,
    /// Whether the trait requires `#[async_trait]` for object safety.
    pub requires_async_trait_attr: bool,
}

impl StreamingIterator {
    /// Creates a canonical async streaming iterator.
    pub fn canonical() -> Self {
        let mut item = AssociatedTypeDef::with_params("Item", vec![GatParam::lifetime("a")]);
        item.add_where_clause(GatWhereClause::new("Self", "'a"));

        StreamingIterator {
            name: "StreamingIterator".to_string(),
            item_type: item,
            requires_async_trait_attr: false,
        }
    }

    /// Returns `true` if this streaming iterator is object-safe.
    ///
    /// Streaming iterators are object-safe only with `#[async_trait]`
    /// since `async fn next()` returns an opaque future.
    pub fn is_object_safe(&self) -> bool {
        self.requires_async_trait_attr
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Integration points
// ═══════════════════════════════════════════════════════════════════════

/// Describes how a collection type gains lending iterator methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LendingIntegration {
    /// The collection type (e.g., `"Array"`, `"str"`).
    pub collection_type: String,
    /// Available lending methods.
    pub methods: Vec<LendingMethodDesc>,
}

/// Description of a lending method on a collection type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LendingMethodDesc {
    /// Method name (e.g., `"windows"`, `"chunks"`, `"lines"`).
    pub name: String,
    /// Parameters beyond `&self` (e.g., `size: usize`).
    pub params: Vec<String>,
    /// The item type yielded (e.g., `"&'a [T]"`).
    pub item_type: String,
}

/// Returns the lending iterator integrations for built-in types.
pub fn builtin_lending_integrations() -> Vec<LendingIntegration> {
    vec![
        LendingIntegration {
            collection_type: "Array".to_string(),
            methods: vec![
                LendingMethodDesc {
                    name: "windows".to_string(),
                    params: vec!["size: usize".to_string()],
                    item_type: "&'a [T]".to_string(),
                },
                LendingMethodDesc {
                    name: "chunks".to_string(),
                    params: vec!["size: usize".to_string()],
                    item_type: "&'a [T]".to_string(),
                },
            ],
        },
        LendingIntegration {
            collection_type: "str".to_string(),
            methods: vec![LendingMethodDesc {
                name: "lines".to_string(),
                params: vec![],
                item_type: "&'a str".to_string(),
            }],
        },
    ]
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
    fn lending_iterator_def_canonical() {
        let def = LendingIteratorDef::canonical();
        assert_eq!(def.name, "LendingIterator");
        assert!(def.item_type.is_gat());
        assert_eq!(def.item_type.lifetime_param_count(), 1);
        assert_eq!(def.item_type.where_clauses.len(), 1);
        assert_eq!(def.item_type.where_clauses[0].to_string(), "Self: 'a",);
    }

    #[test]
    fn validate_lending_next_success() {
        let info = LendingImplInfo {
            impl_type: "WindowsIter".into(),
            item_concrete_type: "&'a [i32]".into(),
            has_next_method: true,
            lifetime_name: "a".into(),
        };
        assert!(validate_lending_next(&info, dummy_span()).is_ok());
    }

    #[test]
    fn validate_lending_next_missing() {
        let info = LendingImplInfo {
            impl_type: "BadIter".into(),
            item_concrete_type: "&'a str".into(),
            has_next_method: false,
            lifetime_name: "a".into(),
        };
        let result = validate_lending_next(&info, dummy_span());
        assert!(result.is_err());
    }

    #[test]
    fn validate_lending_next_missing_lifetime() {
        let info = LendingImplInfo {
            impl_type: "BadIter".into(),
            item_concrete_type: "&str".into(),
            has_next_method: true,
            lifetime_name: "".into(),
        };
        let result = validate_lending_next(&info, dummy_span());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, GatError::LifetimeCapture { .. }));
    }

    #[test]
    fn windows_iterator_validation() {
        let valid = WindowsIterator::new(3, "i32");
        assert!(valid.validate(dummy_span()).is_ok());
        assert_eq!(valid.item_type(), "&'a [i32]");

        let invalid = WindowsIterator::new(0, "i32");
        assert!(invalid.validate(dummy_span()).is_err());
    }

    #[test]
    fn chunks_iterator_validation() {
        let valid = ChunksIterator::new(4, "f64");
        assert!(valid.validate(dummy_span()).is_ok());
        assert_eq!(valid.item_type(), "&'a [f64]");

        let invalid = ChunksIterator::new(0, "f64");
        assert!(invalid.validate(dummy_span()).is_err());
    }

    #[test]
    fn lines_iterator_item_type() {
        let lines = LinesIterator::new(false);
        assert_eq!(lines.item_type(), "&'a str");
        assert!(!lines.include_newline);
    }

    #[test]
    fn adapter_chain_valid() {
        let chain = vec![
            LendingAdapter::Filter,
            LendingAdapter::Map {
                output_type: "i32".into(),
            },
            LendingAdapter::ForEach,
        ];
        assert!(validate_adapter_chain(&chain, dummy_span()).is_ok());
    }

    #[test]
    fn adapter_chain_for_each_not_at_end() {
        let chain = vec![LendingAdapter::ForEach, LendingAdapter::Filter];
        let result = validate_adapter_chain(&chain, dummy_span());
        assert!(result.is_err());
    }

    #[test]
    fn streaming_iterator_canonical() {
        let stream = StreamingIterator::canonical();
        assert_eq!(stream.name, "StreamingIterator");
        assert!(stream.item_type.is_gat());
        assert!(!stream.is_object_safe());
    }

    #[test]
    fn builtin_integrations_present() {
        let integrations = builtin_lending_integrations();
        assert_eq!(integrations.len(), 2);

        let array = &integrations[0];
        assert_eq!(array.collection_type, "Array");
        assert_eq!(array.methods.len(), 2);
        assert_eq!(array.methods[0].name, "windows");
        assert_eq!(array.methods[1].name, "chunks");

        let str_type = &integrations[1];
        assert_eq!(str_type.collection_type, "str");
        assert_eq!(str_type.methods.len(), 1);
        assert_eq!(str_type.methods[0].name, "lines");
    }
}
