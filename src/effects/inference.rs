//! Effect inference — effect annotation syntax, EffectSet type, automatic
//! inference, propagation, mismatch errors, pure functions, built-in effects.

use std::collections::{BTreeSet, HashMap};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S17.1 / S17.2: Effect Annotation & EffectSet
// ═══════════════════════════════════════════════════════════════════════

/// A named effect label.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectLabel(pub String);

impl EffectLabel {
    /// Creates a new effect label.
    pub fn new(name: &str) -> Self {
        Self(name.into())
    }
}

impl fmt::Display for EffectLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A set of effects attached to a function type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffectSet {
    /// The effects in this set (sorted for deterministic display).
    effects: BTreeSet<EffectLabel>,
}

impl EffectSet {
    /// Creates an empty effect set (pure).
    pub fn pure_set() -> Self {
        Self::default()
    }

    /// Creates an effect set from labels.
    pub fn from_labels(labels: &[&str]) -> Self {
        Self {
            effects: labels.iter().map(|l| EffectLabel::new(l)).collect(),
        }
    }

    /// Adds an effect to the set.
    pub fn add(&mut self, label: EffectLabel) {
        self.effects.insert(label);
    }

    /// Checks whether this set contains an effect.
    pub fn contains(&self, label: &str) -> bool {
        self.effects.contains(&EffectLabel::new(label))
    }

    /// Returns whether this is a pure (empty) effect set.
    pub fn is_pure(&self) -> bool {
        self.effects.is_empty()
    }

    /// Number of effects.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Union of two effect sets.
    pub fn union(&self, other: &EffectSet) -> EffectSet {
        EffectSet {
            effects: self.effects.union(&other.effects).cloned().collect(),
        }
    }

    /// Whether this set is a subset of another.
    pub fn is_subset_of(&self, other: &EffectSet) -> bool {
        self.effects.is_subset(&other.effects)
    }

    /// Returns effects in this set but not in the other.
    pub fn difference(&self, other: &EffectSet) -> EffectSet {
        EffectSet {
            effects: self.effects.difference(&other.effects).cloned().collect(),
        }
    }

    /// Returns an iterator over the effect labels.
    pub fn iter(&self) -> impl Iterator<Item = &EffectLabel> {
        self.effects.iter()
    }
}

impl fmt::Display for EffectSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_pure() {
            write!(f, "{{}}")
        } else {
            let labels: Vec<String> = self.effects.iter().map(|e| e.0.clone()).collect();
            write!(f, "{{{}}}", labels.join(", "))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S17.3 / S17.4: Automatic Inference & Propagation
// ═══════════════════════════════════════════════════════════════════════

/// An effect annotation on a function signature.
#[derive(Debug, Clone)]
pub struct FunctionEffects {
    /// Function name.
    pub name: String,
    /// Annotated effects (if any — None means infer).
    pub annotated: Option<EffectSet>,
    /// Inferred effects from body analysis.
    pub inferred: EffectSet,
}

/// Built-in function -> effect mapping for inference.
pub fn builtin_effects() -> HashMap<String, EffectSet> {
    let mut map = HashMap::new();
    map.insert("print".into(), EffectSet::from_labels(&["IO"]));
    map.insert("println".into(), EffectSet::from_labels(&["IO"]));
    map.insert("eprintln".into(), EffectSet::from_labels(&["IO"]));
    map.insert(
        "read_file".into(),
        EffectSet::from_labels(&["IO", "FileSystem"]),
    );
    map.insert(
        "write_file".into(),
        EffectSet::from_labels(&["IO", "FileSystem"]),
    );
    map.insert("mem_alloc".into(), EffectSet::from_labels(&["Alloc"]));
    map.insert("mem_free".into(), EffectSet::from_labels(&["Alloc"]));
    map.insert("panic".into(), EffectSet::from_labels(&["Panic"]));
    map.insert("todo".into(), EffectSet::from_labels(&["Panic"]));
    map.insert("spawn".into(), EffectSet::from_labels(&["Async"]));
    map
}

/// Infers the effect set for a function given its called functions.
pub fn infer_effects(called_fns: &[&str], known_effects: &HashMap<String, EffectSet>) -> EffectSet {
    let mut result = EffectSet::pure_set();
    for name in called_fns {
        if let Some(callee_effects) = known_effects.get(*name) {
            result = result.union(callee_effects);
        }
    }
    result
}

// ═══════════════════════════════════════════════════════════════════════
// S17.5 / S17.6: Optional Annotations & Mismatch Error
// ═══════════════════════════════════════════════════════════════════════

/// Effect mismatch error.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectMismatchError {
    /// Function name.
    pub function: String,
    /// Effects that were annotated.
    pub annotated: EffectSet,
    /// Effects that were inferred.
    pub inferred: EffectSet,
    /// Missing effects (in inferred but not annotated).
    pub missing: EffectSet,
}

impl fmt::Display for EffectMismatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "function `{}` annotated with effects {} but body requires {}; missing: {}",
            self.function, self.annotated, self.inferred, self.missing
        )
    }
}

/// Validates that annotated effects cover inferred effects.
pub fn check_effect_annotation(
    function: &str,
    annotated: &EffectSet,
    inferred: &EffectSet,
) -> Result<(), EffectMismatchError> {
    if inferred.is_subset_of(annotated) {
        Ok(())
    } else {
        let missing = inferred.difference(annotated);
        Err(EffectMismatchError {
            function: function.into(),
            annotated: annotated.clone(),
            inferred: inferred.clone(),
            missing,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S17.7: Pure Functions
// ═══════════════════════════════════════════════════════════════════════

/// Checks whether a function is pure (no effects).
pub fn is_pure_function(effects: &EffectSet) -> bool {
    effects.is_pure()
}

// ═══════════════════════════════════════════════════════════════════════
// S17.8: Built-in Effects
// ═══════════════════════════════════════════════════════════════════════

/// Standard built-in effect labels.
pub const EFFECT_IO: &str = "IO";
pub const EFFECT_ALLOC: &str = "Alloc";
pub const EFFECT_PANIC: &str = "Panic";
pub const EFFECT_ASYNC: &str = "Async";
pub const EFFECT_UNSAFE: &str = "Unsafe";
pub const EFFECT_NETWORK: &str = "Network";
pub const EFFECT_FILESYSTEM: &str = "FileSystem";

/// Returns all standard effect labels.
pub fn standard_effects() -> Vec<&'static str> {
    vec![
        EFFECT_IO,
        EFFECT_ALLOC,
        EFFECT_PANIC,
        EFFECT_ASYNC,
        EFFECT_UNSAFE,
        EFFECT_NETWORK,
        EFFECT_FILESYSTEM,
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// S17.9: Effect Display
// ═══════════════════════════════════════════════════════════════════════

/// Formats effect information for `fj check` output.
pub fn format_effect_info(function: &str, effects: &EffectSet) -> String {
    if effects.is_pure() {
        format!("{function}: pure")
    } else {
        format!("{function}: with {effects}")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S17.1 — Effect Annotation Syntax
    #[test]
    fn s17_1_effect_label_creation() {
        let label = EffectLabel::new("IO");
        assert_eq!(label.to_string(), "IO");
    }

    #[test]
    fn s17_1_effect_set_from_labels() {
        let set = EffectSet::from_labels(&["IO", "Alloc"]);
        assert!(set.contains("IO"));
        assert!(set.contains("Alloc"));
        assert!(!set.contains("Panic"));
    }

    // S17.2 — EffectSet Type
    #[test]
    fn s17_2_effect_set_display() {
        let set = EffectSet::from_labels(&["Alloc", "IO"]);
        assert_eq!(set.to_string(), "{Alloc, IO}");
        assert_eq!(EffectSet::pure_set().to_string(), "{}");
    }

    #[test]
    fn s17_2_effect_set_union() {
        let a = EffectSet::from_labels(&["IO"]);
        let b = EffectSet::from_labels(&["Alloc"]);
        let c = a.union(&b);
        assert_eq!(c.len(), 2);
        assert!(c.contains("IO"));
        assert!(c.contains("Alloc"));
    }

    // S17.3 — Automatic Inference
    #[test]
    fn s17_3_infer_from_builtins() {
        let known = builtin_effects();
        let inferred = infer_effects(&["print", "read_file"], &known);
        assert!(inferred.contains("IO"));
        assert!(inferred.contains("FileSystem"));
    }

    #[test]
    fn s17_3_infer_pure() {
        let known = builtin_effects();
        let inferred = infer_effects(&["add", "multiply"], &known);
        assert!(inferred.is_pure());
    }

    // S17.4 — Effect Propagation
    #[test]
    fn s17_4_propagation_chain() {
        let mut known = builtin_effects();
        // foo calls print -> IO
        let foo_effects = infer_effects(&["print"], &known);
        known.insert("foo".into(), foo_effects);
        // bar calls foo -> also IO
        let bar_effects = infer_effects(&["foo"], &known);
        assert!(bar_effects.contains("IO"));
    }

    // S17.5 — Optional Annotation
    #[test]
    fn s17_5_annotation_matches() {
        let annotated = EffectSet::from_labels(&["IO"]);
        let inferred = EffectSet::from_labels(&["IO"]);
        assert!(check_effect_annotation("foo", &annotated, &inferred).is_ok());
    }

    // S17.6 — Mismatch Error
    #[test]
    fn s17_6_annotation_too_narrow() {
        let annotated = EffectSet::pure_set();
        let inferred = EffectSet::from_labels(&["IO"]);
        let err = check_effect_annotation("foo", &annotated, &inferred).unwrap_err();
        assert!(err.missing.contains("IO"));
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn s17_6_superset_ok() {
        let annotated = EffectSet::from_labels(&["IO", "Alloc"]);
        let inferred = EffectSet::from_labels(&["IO"]);
        assert!(check_effect_annotation("foo", &annotated, &inferred).is_ok());
    }

    // S17.7 — Pure Functions
    #[test]
    fn s17_7_pure_detection() {
        assert!(is_pure_function(&EffectSet::pure_set()));
        assert!(!is_pure_function(&EffectSet::from_labels(&["IO"])));
    }

    // S17.8 — Built-in Effects
    #[test]
    fn s17_8_standard_effects() {
        let effects = standard_effects();
        assert!(effects.contains(&"IO"));
        assert!(effects.contains(&"Alloc"));
        assert!(effects.contains(&"Panic"));
        assert!(effects.contains(&"Async"));
        assert!(effects.contains(&"Network"));
        assert_eq!(effects.len(), 7);
    }

    // S17.9 — Effect Display
    #[test]
    fn s17_9_format_pure() {
        assert_eq!(
            format_effect_info("add", &EffectSet::pure_set()),
            "add: pure"
        );
    }

    #[test]
    fn s17_9_format_effectful() {
        let info = format_effect_info("write", &EffectSet::from_labels(&["IO", "FileSystem"]));
        assert!(info.contains("IO"));
        assert!(info.contains("FileSystem"));
    }

    // S17.10 — Additional
    #[test]
    fn s17_10_subset_check() {
        let a = EffectSet::from_labels(&["IO"]);
        let b = EffectSet::from_labels(&["IO", "Alloc"]);
        assert!(a.is_subset_of(&b));
        assert!(!b.is_subset_of(&a));
    }

    #[test]
    fn s17_10_difference() {
        let a = EffectSet::from_labels(&["IO", "Alloc"]);
        let b = EffectSet::from_labels(&["IO"]);
        let diff = a.difference(&b);
        assert_eq!(diff.len(), 1);
        assert!(diff.contains("Alloc"));
    }

    #[test]
    fn s17_10_builtin_effects_completeness() {
        let builtins = builtin_effects();
        assert!(builtins.contains_key("print"));
        assert!(builtins.contains_key("panic"));
        assert!(builtins.contains_key("mem_alloc"));
    }
}
