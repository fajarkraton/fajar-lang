//! Effect polymorphism — effect variables, bounds, unification,
//! higher-order effects, subtyping, row polymorphism, instantiation.

use std::collections::HashMap;
use std::fmt;

use super::inference::EffectSet;

// ═══════════════════════════════════════════════════════════════════════
// S18.1: Effect Variables
// ═══════════════════════════════════════════════════════════════════════

/// An effect variable used in polymorphic function signatures.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EffectVar {
    /// Variable name (e.g., "E").
    pub name: String,
    /// Unique ID for unification.
    pub id: u32,
}

impl EffectVar {
    /// Creates a new effect variable.
    pub fn new(name: &str, id: u32) -> Self {
        Self {
            name: name.into(),
            id,
        }
    }
}

impl fmt::Display for EffectVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "eff {}", self.name)
    }
}

/// An effect type — either a concrete set or a variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectType {
    /// Concrete effect set.
    Concrete(EffectSet),
    /// Effect variable (to be resolved).
    Variable(EffectVar),
    /// Row: concrete effects + open variable tail `{IO | E}`.
    Row {
        concrete: EffectSet,
        tail: EffectVar,
    },
}

impl fmt::Display for EffectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EffectType::Concrete(set) => write!(f, "{set}"),
            EffectType::Variable(var) => write!(f, "{var}"),
            EffectType::Row { concrete, tail } => {
                write!(f, "{{{concrete} | {}}}", tail.name)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S18.2: Effect Bounds
// ═══════════════════════════════════════════════════════════════════════

/// A bound on an effect variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectBound {
    /// The variable being bounded.
    pub variable: EffectVar,
    /// Required effects (lower bound).
    pub required: EffectSet,
}

impl fmt::Display for EffectBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.variable.name, self.required)
    }
}

/// Checks whether an effect set satisfies a bound.
pub fn satisfies_bound(effects: &EffectSet, bound: &EffectBound) -> bool {
    bound.required.is_subset_of(effects)
}

// ═══════════════════════════════════════════════════════════════════════
// S18.3: Effect Unification
// ═══════════════════════════════════════════════════════════════════════

/// Substitution mapping effect variables to concrete sets.
#[derive(Debug, Clone, Default)]
pub struct EffectSubstitution {
    /// Variable ID -> concrete effect set.
    pub mappings: HashMap<u32, EffectSet>,
}

impl EffectSubstitution {
    /// Creates an empty substitution.
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a variable to a concrete set.
    pub fn bind(&mut self, var: &EffectVar, effects: EffectSet) {
        self.mappings.insert(var.id, effects);
    }

    /// Looks up a variable's binding.
    pub fn lookup(&self, var: &EffectVar) -> Option<&EffectSet> {
        self.mappings.get(&var.id)
    }

    /// Applies substitution to an effect type.
    pub fn apply(&self, effect: &EffectType) -> EffectType {
        match effect {
            EffectType::Concrete(set) => EffectType::Concrete(set.clone()),
            EffectType::Variable(var) => {
                if let Some(bound) = self.lookup(var) {
                    EffectType::Concrete(bound.clone())
                } else {
                    EffectType::Variable(var.clone())
                }
            }
            EffectType::Row { concrete, tail } => {
                if let Some(tail_effects) = self.lookup(tail) {
                    EffectType::Concrete(concrete.union(tail_effects))
                } else {
                    EffectType::Row {
                        concrete: concrete.clone(),
                        tail: tail.clone(),
                    }
                }
            }
        }
    }
}

/// Unifies two effect types, producing a substitution.
pub fn unify_effects(
    lhs: &EffectType,
    rhs: &EffectType,
    subst: &mut EffectSubstitution,
) -> Result<(), String> {
    match (lhs, rhs) {
        (EffectType::Concrete(a), EffectType::Concrete(b)) => {
            if a == b {
                Ok(())
            } else {
                Err(format!("effect mismatch: {a} vs {b}"))
            }
        }
        (EffectType::Variable(var), EffectType::Concrete(set))
        | (EffectType::Concrete(set), EffectType::Variable(var)) => {
            subst.bind(var, set.clone());
            Ok(())
        }
        (EffectType::Variable(a), EffectType::Variable(b)) => {
            if a == b {
                Ok(())
            } else {
                // Bind a to whatever b is (or create equality constraint)
                subst.bind(a, EffectSet::pure_set());
                subst.bind(b, EffectSet::pure_set());
                Ok(())
            }
        }
        (EffectType::Row { concrete, tail }, EffectType::Concrete(full)) => {
            let remainder = full.difference(concrete);
            subst.bind(tail, remainder);
            Ok(())
        }
        (EffectType::Concrete(full), EffectType::Row { concrete, tail }) => {
            let remainder = full.difference(concrete);
            subst.bind(tail, remainder);
            Ok(())
        }
        _ => Err("cannot unify effect types".into()),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S18.4: Higher-Order Effects
// ═══════════════════════════════════════════════════════════════════════

/// Computes effects of a higher-order function call.
pub fn higher_order_effects(fn_effects: &EffectSet, callback_effects: &EffectSet) -> EffectSet {
    fn_effects.union(callback_effects)
}

// ═══════════════════════════════════════════════════════════════════════
// S18.5: Effect Subtyping
// ═══════════════════════════════════════════════════════════════════════

/// Checks effect subtyping: fewer effects <: more effects.
pub fn is_effect_subtype(sub: &EffectSet, super_: &EffectSet) -> bool {
    sub.is_subset_of(super_)
}

// ═══════════════════════════════════════════════════════════════════════
// S18.8: Effect Constraints in Traits
// ═══════════════════════════════════════════════════════════════════════

/// An effect constraint on a trait method.
#[derive(Debug, Clone)]
pub struct TraitEffectConstraint {
    /// Trait name.
    pub trait_name: String,
    /// Method name.
    pub method_name: String,
    /// Required effect set (empty = pure).
    pub required_effects: EffectSet,
}

/// Checks that an impl method's effects satisfy the trait constraint.
pub fn check_trait_effect_constraint(
    constraint: &TraitEffectConstraint,
    impl_effects: &EffectSet,
) -> Result<(), String> {
    if impl_effects.is_subset_of(&constraint.required_effects) {
        Ok(())
    } else {
        let extra = impl_effects.difference(&constraint.required_effects);
        Err(format!(
            "impl of `{}::{}` has extra effects {} not allowed by trait",
            constraint.trait_name, constraint.method_name, extra
        ))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S18.9: Effect Variance
// ═══════════════════════════════════════════════════════════════════════

/// Variance position for effect sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectVariance {
    /// Covariant — fewer effects is subtype (return position).
    Covariant,
    /// Contravariant — more effects is subtype (argument position).
    Contravariant,
}

/// Checks variance-aware effect compatibility.
pub fn check_variance(actual: &EffectSet, expected: &EffectSet, variance: EffectVariance) -> bool {
    match variance {
        EffectVariance::Covariant => actual.is_subset_of(expected),
        EffectVariance::Contravariant => expected.is_subset_of(actual),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S18.1 — Effect Variables
    #[test]
    fn s18_1_effect_var_display() {
        let var = EffectVar::new("E", 0);
        assert_eq!(var.to_string(), "eff E");
    }

    #[test]
    fn s18_1_effect_type_display() {
        let concrete = EffectType::Concrete(EffectSet::from_labels(&["IO"]));
        assert_eq!(concrete.to_string(), "{IO}");
        let var = EffectType::Variable(EffectVar::new("E", 0));
        assert_eq!(var.to_string(), "eff E");
    }

    // S18.2 — Effect Bounds
    #[test]
    fn s18_2_bound_satisfaction() {
        let bound = EffectBound {
            variable: EffectVar::new("E", 0),
            required: EffectSet::from_labels(&["IO"]),
        };
        assert!(satisfies_bound(
            &EffectSet::from_labels(&["IO", "Alloc"]),
            &bound
        ));
        assert!(!satisfies_bound(
            &EffectSet::from_labels(&["Alloc"]),
            &bound
        ));
    }

    // S18.3 — Effect Unification
    #[test]
    fn s18_3_unify_concrete() {
        let a = EffectType::Concrete(EffectSet::from_labels(&["IO"]));
        let b = EffectType::Concrete(EffectSet::from_labels(&["IO"]));
        let mut subst = EffectSubstitution::new();
        assert!(unify_effects(&a, &b, &mut subst).is_ok());
    }

    #[test]
    fn s18_3_unify_var_to_concrete() {
        let var = EffectVar::new("E", 0);
        let a = EffectType::Variable(var.clone());
        let b = EffectType::Concrete(EffectSet::from_labels(&["IO"]));
        let mut subst = EffectSubstitution::new();
        assert!(unify_effects(&a, &b, &mut subst).is_ok());
        assert!(subst.lookup(&var).unwrap().contains("IO"));
    }

    #[test]
    fn s18_3_unify_mismatch() {
        let a = EffectType::Concrete(EffectSet::from_labels(&["IO"]));
        let b = EffectType::Concrete(EffectSet::from_labels(&["Alloc"]));
        let mut subst = EffectSubstitution::new();
        assert!(unify_effects(&a, &b, &mut subst).is_err());
    }

    // S18.4 — Higher-Order Effects
    #[test]
    fn s18_4_higher_order() {
        let fn_eff = EffectSet::from_labels(&["Alloc"]);
        let cb_eff = EffectSet::from_labels(&["IO"]);
        let combined = higher_order_effects(&fn_eff, &cb_eff);
        assert!(combined.contains("Alloc"));
        assert!(combined.contains("IO"));
    }

    // S18.5 — Effect Subtyping
    #[test]
    fn s18_5_subtyping() {
        let fewer = EffectSet::from_labels(&["IO"]);
        let more = EffectSet::from_labels(&["IO", "Alloc"]);
        assert!(is_effect_subtype(&fewer, &more));
        assert!(!is_effect_subtype(&more, &fewer));
    }

    // S18.6 — Row Polymorphism
    #[test]
    fn s18_6_row_display() {
        let row = EffectType::Row {
            concrete: EffectSet::from_labels(&["IO"]),
            tail: EffectVar::new("E", 0),
        };
        assert!(row.to_string().contains("IO"));
        assert!(row.to_string().contains("E"));
    }

    #[test]
    fn s18_6_row_unification() {
        let tail = EffectVar::new("E", 0);
        let row = EffectType::Row {
            concrete: EffectSet::from_labels(&["IO"]),
            tail: tail.clone(),
        };
        let full = EffectType::Concrete(EffectSet::from_labels(&["IO", "Alloc"]));
        let mut subst = EffectSubstitution::new();
        assert!(unify_effects(&row, &full, &mut subst).is_ok());
        assert!(subst.lookup(&tail).unwrap().contains("Alloc"));
    }

    // S18.7 — Effect Instantiation
    #[test]
    fn s18_7_substitution_apply() {
        let var = EffectVar::new("E", 0);
        let mut subst = EffectSubstitution::new();
        subst.bind(&var, EffectSet::from_labels(&["IO"]));
        let result = subst.apply(&EffectType::Variable(var));
        match result {
            EffectType::Concrete(set) => assert!(set.contains("IO")),
            _ => panic!("expected concrete"),
        }
    }

    // S18.8 — Trait Effect Constraints
    #[test]
    fn s18_8_pure_trait_satisfied() {
        let constraint = TraitEffectConstraint {
            trait_name: "Pure".into(),
            method_name: "compute".into(),
            required_effects: EffectSet::pure_set(),
        };
        assert!(check_trait_effect_constraint(&constraint, &EffectSet::pure_set()).is_ok());
    }

    #[test]
    fn s18_8_pure_trait_violated() {
        let constraint = TraitEffectConstraint {
            trait_name: "Pure".into(),
            method_name: "compute".into(),
            required_effects: EffectSet::pure_set(),
        };
        let err = check_trait_effect_constraint(&constraint, &EffectSet::from_labels(&["IO"]))
            .unwrap_err();
        assert!(err.contains("extra effects"));
    }

    // S18.9 — Effect Variance
    #[test]
    fn s18_9_covariant() {
        let fewer = EffectSet::from_labels(&["IO"]);
        let more = EffectSet::from_labels(&["IO", "Alloc"]);
        assert!(check_variance(&fewer, &more, EffectVariance::Covariant));
        assert!(!check_variance(&more, &fewer, EffectVariance::Covariant));
    }

    #[test]
    fn s18_9_contravariant() {
        let fewer = EffectSet::from_labels(&["IO"]);
        let more = EffectSet::from_labels(&["IO", "Alloc"]);
        assert!(check_variance(&more, &fewer, EffectVariance::Contravariant));
        assert!(!check_variance(
            &fewer,
            &more,
            EffectVariance::Contravariant
        ));
    }

    // S18.10 — Additional
    #[test]
    fn s18_10_bound_display() {
        let bound = EffectBound {
            variable: EffectVar::new("E", 0),
            required: EffectSet::from_labels(&["IO", "Alloc"]),
        };
        assert!(bound.to_string().contains("E"));
        assert!(bound.to_string().contains("IO"));
    }
}
