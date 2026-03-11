//! Trait resolution — impl index, go-to-implementation, blanket impls,
//! trait bounds, associated types, trait objects, suggestions, orphan rules.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S29.1: Full Trait Impl Index
// ═══════════════════════════════════════════════════════════════════════

/// A trait implementation entry in the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplEntry {
    /// Trait name.
    pub trait_name: String,
    /// Implementing type.
    pub impl_type: String,
    /// Source file path.
    pub file: String,
    /// Line number.
    pub line: usize,
    /// Method names.
    pub methods: Vec<String>,
    /// Whether this is a blanket impl.
    pub is_blanket: bool,
    /// Generic parameters (if any).
    pub generics: Vec<String>,
}

/// Incremental trait impl index for the workspace.
#[derive(Debug, Clone, Default)]
pub struct TraitImplIndex {
    /// All implementations.
    impls: Vec<ImplEntry>,
    /// Trait -> impl indices.
    by_trait: HashMap<String, Vec<usize>>,
    /// Type -> impl indices.
    by_type: HashMap<String, Vec<usize>>,
}

impl TraitImplIndex {
    /// Creates an empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an impl to the index.
    pub fn add(&mut self, entry: ImplEntry) {
        let idx = self.impls.len();
        self.by_trait
            .entry(entry.trait_name.clone())
            .or_default()
            .push(idx);
        self.by_type
            .entry(entry.impl_type.clone())
            .or_default()
            .push(idx);
        self.impls.push(entry);
    }

    /// Finds all implementations of a trait.
    pub fn impls_for_trait(&self, trait_name: &str) -> Vec<&ImplEntry> {
        self.by_trait
            .get(trait_name)
            .map(|indices| indices.iter().filter_map(|&i| self.impls.get(i)).collect())
            .unwrap_or_default()
    }

    /// Finds all trait implementations for a type.
    pub fn impls_for_type(&self, type_name: &str) -> Vec<&ImplEntry> {
        let mut results: Vec<&ImplEntry> = self
            .by_type
            .get(type_name)
            .map(|indices| indices.iter().filter_map(|&i| self.impls.get(i)).collect())
            .unwrap_or_default();

        // Include blanket impls
        for entry in &self.impls {
            if entry.is_blanket && !results.contains(&entry) {
                results.push(entry);
            }
        }
        results
    }

    /// Returns total impl count.
    pub fn count(&self) -> usize {
        self.impls.len()
    }

    /// Removes all impls from a specific file (for incremental update).
    pub fn remove_file(&mut self, file: &str) {
        self.impls.retain(|e| e.file != file);
        self.rebuild_indices();
    }

    fn rebuild_indices(&mut self) {
        self.by_trait.clear();
        self.by_type.clear();
        for (idx, entry) in self.impls.iter().enumerate() {
            self.by_trait
                .entry(entry.trait_name.clone())
                .or_default()
                .push(idx);
            self.by_type
                .entry(entry.impl_type.clone())
                .or_default()
                .push(idx);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S29.2: Go-to-Implementation
// ═══════════════════════════════════════════════════════════════════════

/// A go-to-implementation target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplLocation {
    /// File path.
    pub file: String,
    /// Line number.
    pub line: usize,
    /// Implementing type.
    pub impl_type: String,
    /// Trait name.
    pub trait_name: String,
}

/// Resolves go-to-implementation targets for a trait method call.
pub fn goto_implementation(
    index: &TraitImplIndex,
    trait_name: &str,
    method: &str,
) -> Vec<ImplLocation> {
    index
        .impls_for_trait(trait_name)
        .iter()
        .filter(|e| e.methods.contains(&method.to_string()))
        .map(|e| ImplLocation {
            file: e.file.clone(),
            line: e.line,
            impl_type: e.impl_type.clone(),
            trait_name: e.trait_name.clone(),
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S29.3: Blanket Impl Display
// ═══════════════════════════════════════════════════════════════════════

/// Formats blanket impl information for hover display.
pub fn format_blanket_impl(entry: &ImplEntry) -> String {
    if entry.is_blanket {
        let generics = if entry.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", entry.generics.join(", "))
        };
        format!(
            "impl{} {} for {}",
            generics, entry.trait_name, entry.impl_type
        )
    } else {
        format!("impl {} for {}", entry.trait_name, entry.impl_type)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S29.4: Trait Bound Checking
// ═══════════════════════════════════════════════════════════════════════

/// An unsatisfied trait bound diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsatisfiedBound {
    /// Type that doesn't satisfy the bound.
    pub ty: String,
    /// Required trait.
    pub required_trait: String,
    /// Location in source.
    pub span: (usize, usize),
}

impl fmt::Display for UnsatisfiedBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the trait `{}` is not implemented for `{}`",
            self.required_trait, self.ty
        )
    }
}

/// Checks trait bounds against the impl index.
pub fn check_trait_bound(
    index: &TraitImplIndex,
    ty: &str,
    required_trait: &str,
    span: (usize, usize),
) -> Option<UnsatisfiedBound> {
    let impls = index.impls_for_type(ty);
    let satisfied = impls.iter().any(|e| e.trait_name == required_trait);
    if satisfied {
        None
    } else {
        Some(UnsatisfiedBound {
            ty: ty.into(),
            required_trait: required_trait.into(),
            span,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S29.5: Associated Type Resolution
// ═══════════════════════════════════════════════════════════════════════

/// An associated type binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssocTypeBinding {
    /// Trait name.
    pub trait_name: String,
    /// Associated type name.
    pub assoc_name: String,
    /// Concrete type it resolves to.
    pub resolved_type: String,
    /// Implementing type.
    pub impl_type: String,
}

impl fmt::Display for AssocTypeBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "<{} as {}>::{} = {}",
            self.impl_type, self.trait_name, self.assoc_name, self.resolved_type
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S29.6: Trait Object Info
// ═══════════════════════════════════════════════════════════════════════

/// Vtable entry for a trait object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VtableEntry {
    /// Method name.
    pub method: String,
    /// Method signature.
    pub signature: String,
    /// Offset in vtable.
    pub offset: usize,
}

/// Trait object information for hover.
#[derive(Debug, Clone)]
pub struct TraitObjectInfo {
    /// Trait name.
    pub trait_name: String,
    /// Vtable entries.
    pub vtable: Vec<VtableEntry>,
    /// Whether the trait is object-safe.
    pub object_safe: bool,
}

impl fmt::Display for TraitObjectInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "dyn {} ({} methods, {})",
            self.trait_name,
            self.vtable.len(),
            if self.object_safe {
                "object-safe"
            } else {
                "NOT object-safe"
            }
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S29.7: Impl Suggestions
// ═══════════════════════════════════════════════════════════════════════

/// A suggested trait implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplSuggestion {
    /// Trait to implement.
    pub trait_name: String,
    /// Type to implement for.
    pub for_type: String,
    /// Skeleton code.
    pub skeleton: String,
}

/// Generates an impl skeleton for a trait.
pub fn generate_impl_skeleton(
    trait_name: &str,
    for_type: &str,
    methods: &[&str],
) -> ImplSuggestion {
    let mut body = format!("impl {trait_name} for {for_type} {{\n");
    for method in methods {
        body.push_str(&format!("    fn {method}(&self) {{ todo!() }}\n"));
    }
    body.push('}');
    ImplSuggestion {
        trait_name: trait_name.into(),
        for_type: for_type.into(),
        skeleton: body,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S29.8: Orphan Rule Checking
// ═══════════════════════════════════════════════════════════════════════

/// Orphan rule violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanViolation {
    /// Trait name.
    pub trait_name: String,
    /// Type name.
    pub impl_type: String,
    /// Reason for violation.
    pub reason: String,
}

/// Checks if an impl violates the orphan rule.
pub fn check_orphan_rule(
    trait_name: &str,
    impl_type: &str,
    local_traits: &[&str],
    local_types: &[&str],
) -> Option<OrphanViolation> {
    let trait_is_local = local_traits.contains(&trait_name);
    let type_is_local = local_types.contains(&impl_type);

    if !trait_is_local && !type_is_local {
        Some(OrphanViolation {
            trait_name: trait_name.into(),
            impl_type: impl_type.into(),
            reason: format!(
                "neither trait `{trait_name}` nor type `{impl_type}` is defined in the current crate"
            ),
        })
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S29.9: Trait Hierarchy View
// ═══════════════════════════════════════════════════════════════════════

/// A node in the trait hierarchy tree.
#[derive(Debug, Clone)]
pub struct TraitNode {
    /// Trait name.
    pub name: String,
    /// Super traits.
    pub super_traits: Vec<String>,
    /// Sub traits (derived from super_traits of others).
    pub sub_traits: Vec<String>,
}

impl fmt::Display for TraitNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.super_traits.is_empty() {
            write!(f, "{}", self.name)
        } else {
            write!(f, "{}: {}", self.name, self.super_traits.join(" + "))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(trait_name: &str, impl_type: &str, methods: &[&str]) -> ImplEntry {
        ImplEntry {
            trait_name: trait_name.into(),
            impl_type: impl_type.into(),
            file: "src/lib.fj".into(),
            line: 10,
            methods: methods.iter().map(|m| m.to_string()).collect(),
            is_blanket: false,
            generics: vec![],
        }
    }

    // S29.1 — Full Trait Impl Index
    #[test]
    fn s29_1_index_add_lookup() {
        let mut index = TraitImplIndex::new();
        index.add(sample_entry("Display", "Point", &["fmt"]));
        assert_eq!(index.count(), 1);
        assert_eq!(index.impls_for_trait("Display").len(), 1);
    }

    #[test]
    fn s29_1_index_by_type() {
        let mut index = TraitImplIndex::new();
        index.add(sample_entry("Display", "Point", &["fmt"]));
        index.add(sample_entry("Debug", "Point", &["fmt"]));
        assert_eq!(index.impls_for_type("Point").len(), 2);
    }

    #[test]
    fn s29_1_incremental_update() {
        let mut index = TraitImplIndex::new();
        index.add(sample_entry("Display", "Point", &["fmt"]));
        index.add(ImplEntry {
            file: "src/other.fj".into(),
            ..sample_entry("Debug", "Circle", &["fmt"])
        });
        index.remove_file("src/other.fj");
        assert_eq!(index.count(), 1);
    }

    // S29.2 — Go-to-Implementation
    #[test]
    fn s29_2_goto_impl() {
        let mut index = TraitImplIndex::new();
        index.add(sample_entry("Display", "Point", &["fmt"]));
        index.add(sample_entry("Display", "Circle", &["fmt"]));
        let locations = goto_implementation(&index, "Display", "fmt");
        assert_eq!(locations.len(), 2);
    }

    #[test]
    fn s29_2_goto_impl_no_match() {
        let index = TraitImplIndex::new();
        let locations = goto_implementation(&index, "Display", "fmt");
        assert!(locations.is_empty());
    }

    // S29.3 — Blanket Impl Display
    #[test]
    fn s29_3_blanket_impl_format() {
        let entry = ImplEntry {
            is_blanket: true,
            generics: vec!["T: Display".into()],
            ..sample_entry("ToString", "T", &["to_string"])
        };
        let display = format_blanket_impl(&entry);
        assert!(display.contains("impl<T: Display>"));
        assert!(display.contains("ToString"));
    }

    #[test]
    fn s29_3_blanket_impls_included() {
        let mut index = TraitImplIndex::new();
        index.add(ImplEntry {
            is_blanket: true,
            ..sample_entry("ToString", "T", &["to_string"])
        });
        let impls = index.impls_for_type("Point");
        assert_eq!(impls.len(), 1); // blanket impl applies
    }

    // S29.4 — Trait Bound Checking
    #[test]
    fn s29_4_bound_satisfied() {
        let mut index = TraitImplIndex::new();
        index.add(sample_entry("Display", "Point", &["fmt"]));
        assert!(check_trait_bound(&index, "Point", "Display", (1, 1)).is_none());
    }

    #[test]
    fn s29_4_bound_unsatisfied() {
        let index = TraitImplIndex::new();
        let diag = check_trait_bound(&index, "Point", "Display", (1, 1)).unwrap();
        assert!(diag.to_string().contains("not implemented"));
    }

    // S29.5 — Associated Type Resolution
    #[test]
    fn s29_5_assoc_type_display() {
        let binding = AssocTypeBinding {
            trait_name: "Iterator".into(),
            assoc_name: "Item".into(),
            resolved_type: "i32".into(),
            impl_type: "Range".into(),
        };
        assert!(binding.to_string().contains("Item = i32"));
    }

    // S29.6 — Trait Object Info
    #[test]
    fn s29_6_trait_object_info() {
        let info = TraitObjectInfo {
            trait_name: "Draw".into(),
            vtable: vec![
                VtableEntry {
                    method: "draw".into(),
                    signature: "fn(&self)".into(),
                    offset: 0,
                },
                VtableEntry {
                    method: "bounds".into(),
                    signature: "fn(&self) -> Rect".into(),
                    offset: 8,
                },
            ],
            object_safe: true,
        };
        assert!(info.to_string().contains("2 methods"));
        assert!(info.to_string().contains("object-safe"));
    }

    // S29.7 — Impl Suggestions
    #[test]
    fn s29_7_generate_skeleton() {
        let suggestion = generate_impl_skeleton("Display", "Point", &["fmt"]);
        assert!(suggestion.skeleton.contains("impl Display for Point"));
        assert!(suggestion.skeleton.contains("fn fmt"));
        assert!(suggestion.skeleton.contains("todo!()"));
    }

    // S29.8 — Orphan Rule
    #[test]
    fn s29_8_orphan_rule_violated() {
        let result = check_orphan_rule("Display", "Vec", &["MyTrait"], &["MyType"]);
        assert!(result.is_some());
    }

    #[test]
    fn s29_8_orphan_rule_ok_local_trait() {
        let result = check_orphan_rule("MyTrait", "Vec", &["MyTrait"], &["MyType"]);
        assert!(result.is_none());
    }

    #[test]
    fn s29_8_orphan_rule_ok_local_type() {
        let result = check_orphan_rule("Display", "MyType", &["MyTrait"], &["MyType"]);
        assert!(result.is_none());
    }

    // S29.9 — Trait Hierarchy
    #[test]
    fn s29_9_trait_hierarchy_display() {
        let node = TraitNode {
            name: "Error".into(),
            super_traits: vec!["Display".into(), "Debug".into()],
            sub_traits: vec![],
        };
        assert!(node.to_string().contains("Error: Display + Debug"));
    }

    #[test]
    fn s29_9_root_trait() {
        let node = TraitNode {
            name: "Any".into(),
            super_traits: vec![],
            sub_traits: vec!["Display".into()],
        };
        assert_eq!(node.to_string(), "Any");
    }

    // S29.10 — Additional
    #[test]
    fn s29_10_multiple_traits_same_type() {
        let mut index = TraitImplIndex::new();
        index.add(sample_entry("Display", "Point", &["fmt"]));
        index.add(sample_entry("Debug", "Point", &["fmt"]));
        index.add(sample_entry("Clone", "Point", &["clone"]));
        assert_eq!(index.impls_for_type("Point").len(), 3);
        assert_eq!(index.impls_for_trait("Clone").len(), 1);
    }
}
