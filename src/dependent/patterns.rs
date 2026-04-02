//! Dependent pattern matching — type-level value patterns, exhaustiveness
//! with Nats, proof witnesses, type-safe indexing, dependent if-else,
//! where clauses, Nat ranges, inductive proofs, dependent return types.

use std::collections::HashMap;
use std::fmt;

use super::nat::{NatConstraint, NatError, NatValue};

// ═══════════════════════════════════════════════════════════════════════
// S4.1: Type-Level Value Patterns
// ═══════════════════════════════════════════════════════════════════════

/// A pattern that matches on Nat values in type position.
#[derive(Debug, Clone, PartialEq)]
pub enum NatPattern {
    /// Match a specific literal value.
    Literal(u64),
    /// Match any value, binding it to a name.
    Wildcard,
    /// Match a range of values: `1..=10`.
    Range { start: u64, end_inclusive: u64 },
    /// Bind the matched value to a name: `n`.
    Binding(String),
}

impl fmt::Display for NatPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NatPattern::Literal(n) => write!(f, "{n}"),
            NatPattern::Wildcard => write!(f, "_"),
            NatPattern::Range {
                start,
                end_inclusive,
            } => write!(f, "{start}..={end_inclusive}"),
            NatPattern::Binding(name) => write!(f, "{name}"),
        }
    }
}

/// A match arm for type-level Nat matching.
#[derive(Debug, Clone)]
pub struct NatMatchArm {
    /// The pattern to match.
    pub pattern: NatPattern,
    /// The result type when this arm matches.
    pub result_type: String,
}

/// Checks whether a Nat value matches a pattern.
pub fn nat_pattern_matches(pattern: &NatPattern, value: u64) -> bool {
    match pattern {
        NatPattern::Literal(n) => *n == value,
        NatPattern::Wildcard | NatPattern::Binding(_) => true,
        NatPattern::Range {
            start,
            end_inclusive,
        } => value >= *start && value <= *end_inclusive,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.2: Exhaustiveness with Nats
// ═══════════════════════════════════════════════════════════════════════

/// Exhaustiveness check result for Nat match expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum ExhaustivenessResult {
    /// All values are covered.
    Exhaustive,
    /// Some values are not covered.
    NonExhaustive {
        /// Example uncovered value.
        example: u64,
    },
}

/// Checks whether a set of Nat patterns is exhaustive for a given range.
///
/// If no range is provided, checks for unbounded Nat (must have wildcard).
pub fn check_nat_exhaustiveness(
    patterns: &[NatPattern],
    bound: Option<u64>,
) -> ExhaustivenessResult {
    // If any pattern is a wildcard or binding, it's exhaustive.
    if patterns
        .iter()
        .any(|p| matches!(p, NatPattern::Wildcard | NatPattern::Binding(_)))
    {
        return ExhaustivenessResult::Exhaustive;
    }

    // For bounded Nats, check all values are covered.
    if let Some(max) = bound {
        for val in 0..=max {
            if !patterns.iter().any(|p| nat_pattern_matches(p, val)) {
                return ExhaustivenessResult::NonExhaustive { example: val };
            }
        }
        return ExhaustivenessResult::Exhaustive;
    }

    // Unbounded without wildcard — always non-exhaustive.
    // Find an uncovered value by checking 0..100.
    for val in 0..=100 {
        if !patterns.iter().any(|p| nat_pattern_matches(p, val)) {
            return ExhaustivenessResult::NonExhaustive { example: val };
        }
    }
    ExhaustivenessResult::NonExhaustive { example: 101 }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.3: Proof Witnesses
// ═══════════════════════════════════════════════════════════════════════

/// A compile-time proof witness that a constraint holds.
#[derive(Debug, Clone, PartialEq)]
pub struct ProofWitness {
    /// The constraint that was proven.
    pub constraint: NatConstraint,
    /// The Nat values that satisfy the constraint.
    pub values: HashMap<String, u64>,
}

/// Attempts to generate a proof witness for a constraint.
pub fn prove_constraint(
    constraint: &NatConstraint,
    env: &HashMap<String, u64>,
) -> Result<ProofWitness, NatError> {
    constraint.check(env)?;
    Ok(ProofWitness {
        constraint: constraint.clone(),
        values: env.clone(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S4.4: Type-Safe Indexing
// ═══════════════════════════════════════════════════════════════════════

/// Result of a type-safe index check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafeIndexResult {
    /// Index is provably safe — returns `T`, not `Option<T>`.
    Safe,
    /// Index may be out of bounds — returns `Option<T>`.
    MaybeOutOfBounds,
    /// Index is provably out of bounds — compile error.
    DefinitelyOutOfBounds,
}

/// Checks whether `array.get::<I>()` can be proven safe at compile time.
///
/// When `I < N` is proven, the return type is `T` (not `Option<T>`).
pub fn check_safe_index(
    array_len: &NatValue,
    index: &NatValue,
    env: &HashMap<String, u64>,
) -> SafeIndexResult {
    let n = array_len.evaluate(env);
    let i = index.evaluate(env);

    match (n, i) {
        (Some(len), Some(idx)) if idx < len => SafeIndexResult::Safe,
        (Some(len), Some(idx)) if idx >= len => SafeIndexResult::DefinitelyOutOfBounds,
        _ => SafeIndexResult::MaybeOutOfBounds,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.5: Dependent If-Else
// ═══════════════════════════════════════════════════════════════════════

/// A dependent if-else branch with Nat-conditional types.
#[derive(Debug, Clone)]
pub struct DepBranch {
    /// Condition on a Nat value.
    pub condition: NatCondition,
    /// Result type when condition is true.
    pub then_type: String,
    /// Result type when condition is false.
    pub else_type: String,
}

/// A condition on a Nat value.
#[derive(Debug, Clone, PartialEq)]
pub enum NatCondition {
    /// `N == value`.
    IsZero(NatValue),
    /// `N > 0`.
    IsPositive(NatValue),
    /// `N == M`.
    Equal(NatValue, NatValue),
    /// `N < M`.
    LessThan(NatValue, NatValue),
}

/// Evaluates a Nat condition.
pub fn eval_nat_condition(cond: &NatCondition, env: &HashMap<String, u64>) -> Option<bool> {
    match cond {
        NatCondition::IsZero(n) => n.evaluate(env).map(|v| v == 0),
        NatCondition::IsPositive(n) => n.evaluate(env).map(|v| v > 0),
        NatCondition::Equal(a, b) => {
            let av = a.evaluate(env)?;
            let bv = b.evaluate(env)?;
            Some(av == bv)
        }
        NatCondition::LessThan(a, b) => {
            let av = a.evaluate(env)?;
            let bv = b.evaluate(env)?;
            Some(av < bv)
        }
    }
}

/// Determines the result type of a dependent if-else.
pub fn resolve_dep_branch(branch: &DepBranch, env: &HashMap<String, u64>) -> Option<String> {
    eval_nat_condition(&branch.condition, env).map(|b| {
        if b {
            branch.then_type.clone()
        } else {
            branch.else_type.clone()
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S4.6: Where Clauses on Nats
// ═══════════════════════════════════════════════════════════════════════

/// A where clause on a function with const generics.
#[derive(Debug, Clone)]
pub struct WhereClause {
    /// The constraints that must be satisfied.
    pub constraints: Vec<NatConstraint>,
}

impl WhereClause {
    /// Creates an empty where clause.
    pub fn empty() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }

    /// Adds a constraint.
    pub fn with(mut self, constraint: NatConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Checks all constraints.
    pub fn check_all(&self, env: &HashMap<String, u64>) -> Result<(), Vec<NatError>> {
        let errors: Vec<NatError> = self
            .constraints
            .iter()
            .filter_map(|c| c.check(env).err())
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.7: Nat Range Type
// ═══════════════════════════════════════════════════════════════════════

/// A bounded natural number type: `Nat<1..=10>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NatRange {
    /// Lower bound (inclusive).
    pub min: u64,
    /// Upper bound (inclusive).
    pub max: u64,
}

impl fmt::Display for NatRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Nat<{}..={}>", self.min, self.max)
    }
}

impl NatRange {
    /// Creates a new bounded Nat range.
    pub fn new(min: u64, max: u64) -> Result<Self, String> {
        if min > max {
            return Err(format!("invalid Nat range: {min} > {max}"));
        }
        Ok(Self { min, max })
    }

    /// Checks whether a value is within this range.
    pub fn contains(&self, value: u64) -> bool {
        value >= self.min && value <= self.max
    }

    /// Number of values in the range.
    pub fn count(&self) -> u64 {
        self.max - self.min + 1
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.8: Inductive Proofs
// ═══════════════════════════════════════════════════════════════════════

/// An inductive proof step for recursive functions on Nats.
#[derive(Debug, Clone)]
pub struct InductiveProof {
    /// Base case: the Nat value where the property holds trivially.
    pub base_case: u64,
    /// Inductive step: if the property holds for N, it holds for N+step.
    pub step: u64,
    /// The property being proven (description).
    pub property: String,
}

impl InductiveProof {
    /// Checks whether this inductive proof covers a given value.
    pub fn covers(&self, value: u64) -> bool {
        if value == self.base_case {
            return true;
        }
        if self.step == 0 {
            return false;
        }
        value >= self.base_case && (value - self.base_case).is_multiple_of(self.step)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V14 DT3: Propositional Equality — Proof Terms
// ═══════════════════════════════════════════════════════════════════════

/// V14 DT3: Proof term for propositional equality.
#[derive(Debug, Clone, PartialEq)]
pub enum ProofTerm {
    /// Reflexivity: a == a
    Refl(String),
    /// Symmetry: if a == b then b == a
    Sym(Box<ProofTerm>),
    /// Transitivity: if a == b and b == c then a == c
    Trans(Box<ProofTerm>, Box<ProofTerm>),
    /// Congruence: if a == b then f(a) == f(b)
    Cong(String, Box<ProofTerm>),
}

impl ProofTerm {
    /// Verify that a proof term is well-formed.
    pub fn is_valid(&self) -> bool {
        match self {
            ProofTerm::Refl(_) => true,
            ProofTerm::Sym(inner) => inner.is_valid(),
            ProofTerm::Trans(p1, p2) => p1.is_valid() && p2.is_valid(),
            ProofTerm::Cong(_, inner) => inner.is_valid(),
        }
    }

    /// Depth of the proof tree.
    pub fn depth(&self) -> usize {
        match self {
            ProofTerm::Refl(_) => 1,
            ProofTerm::Sym(inner) => 1 + inner.depth(),
            ProofTerm::Trans(p1, p2) => 1 + p1.depth().max(p2.depth()),
            ProofTerm::Cong(_, inner) => 1 + inner.depth(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.9: Dependent Return Types
// ═══════════════════════════════════════════════════════════════════════

/// A function signature with dependent return type.
#[derive(Debug, Clone)]
pub struct DepFnSignature {
    /// Function name.
    pub name: String,
    /// Const generic parameters.
    pub const_params: Vec<(String, String)>, // (name, type_name)
    /// Where clauses.
    pub where_clause: WhereClause,
    /// Return type (may reference const params).
    pub return_type: String,
}

/// Resolves the return type of a dependent function given const args.
pub fn resolve_dep_return_type(
    sig: &DepFnSignature,
    const_args: &[u64],
) -> Result<String, Vec<NatError>> {
    let mut env = HashMap::new();
    for (i, (name, _)) in sig.const_params.iter().enumerate() {
        if i < const_args.len() {
            env.insert(name.clone(), const_args[i]);
        }
    }

    // Check where clauses.
    sig.where_clause.check_all(&env)?;

    // Return the type string with substituted values.
    let mut result = sig.return_type.clone();
    for (name, val) in &env {
        result = result.replace(name, &val.to_string());
    }
    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S4.1 — Type-Level Value Patterns
    #[test]
    fn s4_1_nat_pattern_literal() {
        assert!(nat_pattern_matches(&NatPattern::Literal(5), 5));
        assert!(!nat_pattern_matches(&NatPattern::Literal(5), 6));
    }

    #[test]
    fn s4_1_nat_pattern_wildcard() {
        assert!(nat_pattern_matches(&NatPattern::Wildcard, 42));
    }

    #[test]
    fn s4_1_nat_pattern_range() {
        let p = NatPattern::Range {
            start: 1,
            end_inclusive: 10,
        };
        assert!(nat_pattern_matches(&p, 1));
        assert!(nat_pattern_matches(&p, 10));
        assert!(!nat_pattern_matches(&p, 0));
        assert!(!nat_pattern_matches(&p, 11));
    }

    #[test]
    fn s4_1_nat_pattern_display() {
        assert_eq!(NatPattern::Literal(5).to_string(), "5");
        assert_eq!(NatPattern::Wildcard.to_string(), "_");
        let r = NatPattern::Range {
            start: 1,
            end_inclusive: 10,
        };
        assert_eq!(r.to_string(), "1..=10");
    }

    // S4.2 — Exhaustiveness
    #[test]
    fn s4_2_exhaustive_with_wildcard() {
        let patterns = vec![NatPattern::Literal(0), NatPattern::Wildcard];
        assert_eq!(
            check_nat_exhaustiveness(&patterns, Some(5)),
            ExhaustivenessResult::Exhaustive
        );
    }

    #[test]
    fn s4_2_exhaustive_all_values() {
        let patterns = vec![
            NatPattern::Literal(0),
            NatPattern::Literal(1),
            NatPattern::Literal(2),
        ];
        assert_eq!(
            check_nat_exhaustiveness(&patterns, Some(2)),
            ExhaustivenessResult::Exhaustive
        );
    }

    #[test]
    fn s4_2_non_exhaustive() {
        let patterns = vec![NatPattern::Literal(0), NatPattern::Literal(2)];
        let result = check_nat_exhaustiveness(&patterns, Some(3));
        assert!(matches!(
            result,
            ExhaustivenessResult::NonExhaustive { example: 1 }
        ));
    }

    // S4.3 — Proof Witnesses
    #[test]
    fn s4_3_prove_greater_than() {
        let constraint = NatConstraint::GreaterThan(NatValue::Param("N".into()), 0);
        let mut env = HashMap::new();
        env.insert("N".into(), 5);
        let witness = prove_constraint(&constraint, &env).unwrap();
        assert_eq!(witness.values["N"], 5);
    }

    #[test]
    fn s4_3_prove_fails() {
        let constraint = NatConstraint::GreaterThan(NatValue::Param("N".into()), 10);
        let mut env = HashMap::new();
        env.insert("N".into(), 5);
        assert!(prove_constraint(&constraint, &env).is_err());
    }

    // S4.4 — Type-Safe Indexing
    #[test]
    fn s4_4_safe_index() {
        let len = NatValue::Literal(10);
        let idx = NatValue::Literal(3);
        assert_eq!(
            check_safe_index(&len, &idx, &HashMap::new()),
            SafeIndexResult::Safe
        );
    }

    #[test]
    fn s4_4_oob_index() {
        let len = NatValue::Literal(5);
        let idx = NatValue::Literal(5);
        assert_eq!(
            check_safe_index(&len, &idx, &HashMap::new()),
            SafeIndexResult::DefinitelyOutOfBounds
        );
    }

    #[test]
    fn s4_4_maybe_oob() {
        let len = NatValue::Param("N".into());
        let idx = NatValue::Literal(0);
        assert_eq!(
            check_safe_index(&len, &idx, &HashMap::new()),
            SafeIndexResult::MaybeOutOfBounds
        );
    }

    // S4.5 — Dependent If-Else
    #[test]
    fn s4_5_dep_branch_resolves() {
        let branch = DepBranch {
            condition: NatCondition::IsZero(NatValue::Param("N".into())),
            then_type: "()".into(),
            else_type: "Array<T, N>".into(),
        };
        let mut env = HashMap::new();
        env.insert("N".into(), 0);
        assert_eq!(resolve_dep_branch(&branch, &env), Some("()".into()));

        env.insert("N".into(), 5);
        assert_eq!(
            resolve_dep_branch(&branch, &env),
            Some("Array<T, N>".into())
        );
    }

    #[test]
    fn s4_5_dep_condition_less_than() {
        let cond = NatCondition::LessThan(NatValue::Literal(3), NatValue::Literal(5));
        assert_eq!(eval_nat_condition(&cond, &HashMap::new()), Some(true));
    }

    // S4.6 — Where Clauses
    #[test]
    fn s4_6_where_clause_pass() {
        let wc =
            WhereClause::empty().with(NatConstraint::GreaterThan(NatValue::Param("N".into()), 0));
        let mut env = HashMap::new();
        env.insert("N".into(), 5);
        assert!(wc.check_all(&env).is_ok());
    }

    #[test]
    fn s4_6_where_clause_fail() {
        let wc =
            WhereClause::empty().with(NatConstraint::GreaterThan(NatValue::Param("N".into()), 0));
        let mut env = HashMap::new();
        env.insert("N".into(), 0);
        let result = wc.check_all(&env);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().len(), 1);
    }

    // S4.7 — Nat Range
    #[test]
    fn s4_7_nat_range() {
        let r = NatRange::new(1, 10).unwrap();
        assert!(r.contains(1));
        assert!(r.contains(10));
        assert!(!r.contains(0));
        assert!(!r.contains(11));
        assert_eq!(r.count(), 10);
    }

    #[test]
    fn s4_7_nat_range_display() {
        let r = NatRange::new(1, 10).unwrap();
        assert_eq!(r.to_string(), "Nat<1..=10>");
    }

    #[test]
    fn s4_7_nat_range_invalid() {
        assert!(NatRange::new(10, 5).is_err());
    }

    // S4.8 — Inductive Proofs
    #[test]
    fn s4_8_inductive_proof_base() {
        let proof = InductiveProof {
            base_case: 0,
            step: 1,
            property: "fibonacci is defined".into(),
        };
        assert!(proof.covers(0));
        assert!(proof.covers(5));
        assert!(proof.covers(100));
    }

    #[test]
    fn s4_8_inductive_proof_step() {
        let proof = InductiveProof {
            base_case: 0,
            step: 2,
            property: "even numbers".into(),
        };
        assert!(proof.covers(0));
        assert!(proof.covers(2));
        assert!(proof.covers(4));
        assert!(!proof.covers(1));
        assert!(!proof.covers(3));
    }

    // S4.9 — Dependent Return Types
    #[test]
    fn s4_9_dep_return_type() {
        let sig = DepFnSignature {
            name: "make_array".into(),
            const_params: vec![("N".into(), "usize".into())],
            where_clause: WhereClause::empty()
                .with(NatConstraint::GreaterThan(NatValue::Param("N".into()), 0)),
            return_type: "Array<i32, N>".into(),
        };
        let result = resolve_dep_return_type(&sig, &[5]).unwrap();
        assert_eq!(result, "Array<i32, 5>");
    }

    #[test]
    fn s4_9_dep_return_type_where_fails() {
        let sig = DepFnSignature {
            name: "make_array".into(),
            const_params: vec![("N".into(), "usize".into())],
            where_clause: WhereClause::empty()
                .with(NatConstraint::GreaterThan(NatValue::Param("N".into()), 0)),
            return_type: "Array<i32, N>".into(),
        };
        assert!(resolve_dep_return_type(&sig, &[0]).is_err());
    }

    // S4.10 — Additional tests
    #[test]
    fn s4_10_exhaustive_with_range_and_wildcard() {
        let patterns = vec![
            NatPattern::Range {
                start: 0,
                end_inclusive: 5,
            },
            NatPattern::Wildcard,
        ];
        assert_eq!(
            check_nat_exhaustiveness(&patterns, None),
            ExhaustivenessResult::Exhaustive
        );
    }

    #[test]
    fn s4_10_safe_index_with_env() {
        let len = NatValue::Param("N".into());
        let idx = NatValue::Literal(2);
        let mut env = HashMap::new();
        env.insert("N".into(), 10);
        assert_eq!(check_safe_index(&len, &idx, &env), SafeIndexResult::Safe);
    }

    #[test]
    fn s4_10_dep_branch_unresolved() {
        let branch = DepBranch {
            condition: NatCondition::IsZero(NatValue::Param("N".into())),
            then_type: "()".into(),
            else_type: "T".into(),
        };
        // No env — condition cannot be evaluated.
        assert_eq!(resolve_dep_branch(&branch, &HashMap::new()), None);
    }

    // ═════════════════════════════════════════════════════════════════
    // V14 DT3: Propositional Equality — Proof Terms
    // ═════════════════════════════════════════════════════════════════

    #[test]
    fn v14_dt3_3_proof_refl() {
        let proof = ProofTerm::Refl("x".into());
        assert!(proof.is_valid());
        assert_eq!(proof.depth(), 1);
    }

    #[test]
    fn v14_dt3_4_proof_sym() {
        let proof = ProofTerm::Sym(Box::new(ProofTerm::Refl("x".into())));
        assert!(proof.is_valid());
        assert_eq!(proof.depth(), 2);
    }

    #[test]
    fn v14_dt3_5_proof_trans() {
        let proof = ProofTerm::Trans(
            Box::new(ProofTerm::Refl("a".into())),
            Box::new(ProofTerm::Refl("b".into())),
        );
        assert!(proof.is_valid());
        assert_eq!(proof.depth(), 2);
    }

    #[test]
    fn v14_dt3_6_proof_cong() {
        let proof = ProofTerm::Cong("f".into(), Box::new(ProofTerm::Refl("x".into())));
        assert!(proof.is_valid());
        assert_eq!(proof.depth(), 2);
    }

    #[test]
    fn v14_dt3_7_proof_complex() {
        // Trans(Sym(Refl(a)), Cong(f, Refl(b))) — depth 3
        let proof = ProofTerm::Trans(
            Box::new(ProofTerm::Sym(Box::new(ProofTerm::Refl("a".into())))),
            Box::new(ProofTerm::Cong(
                "f".into(),
                Box::new(ProofTerm::Refl("b".into())),
            )),
        );
        assert!(proof.is_valid());
        assert_eq!(proof.depth(), 3);
    }

    #[test]
    fn v14_dt3_8_proof_deep_nesting() {
        let mut proof = ProofTerm::Refl("x".into());
        for _ in 0..5 {
            proof = ProofTerm::Sym(Box::new(proof));
        }
        assert!(proof.is_valid());
        assert_eq!(proof.depth(), 6);
    }

    #[test]
    fn v14_dt3_9_proof_equality() {
        let p1 = ProofTerm::Refl("x".into());
        let p2 = ProofTerm::Refl("x".into());
        assert_eq!(p1, p2);
        let p3 = ProofTerm::Refl("y".into());
        assert_ne!(p1, p3);
    }

    #[test]
    fn v14_dt3_10_proof_term_all_variants() {
        let refl = ProofTerm::Refl("a".into());
        let sym = ProofTerm::Sym(Box::new(refl.clone()));
        let trans = ProofTerm::Trans(Box::new(refl.clone()), Box::new(refl.clone()));
        let cong = ProofTerm::Cong("f".into(), Box::new(refl));
        assert!(sym.is_valid());
        assert!(trans.is_valid());
        assert!(cong.is_valid());
    }

    #[test]
    fn v14_dt6_proof_term_depth() {
        let refl = ProofTerm::Refl("x".into());
        assert_eq!(refl.depth(), 1);
        let sym = ProofTerm::Sym(Box::new(refl.clone()));
        assert_eq!(sym.depth(), 2);
        let trans = ProofTerm::Trans(Box::new(sym.clone()), Box::new(refl.clone()));
        assert_eq!(trans.depth(), 3);
    }

    #[test]
    fn v14_dt6_dep_fn_signature_resolve() {
        let sig = DepFnSignature {
            name: "zeros".into(),
            const_params: vec![("N".into(), "usize".into())],
            where_clause: WhereClause::empty(),
            return_type: "Vector<f64, N>".into(),
        };
        let result = resolve_dep_return_type(&sig, &[10]).unwrap();
        assert_eq!(result, "Vector<f64, 10>");
    }

    #[test]
    fn v14_dt6_dep_fn_multiple_params() {
        let sig = DepFnSignature {
            name: "matrix".into(),
            const_params: vec![
                ("ROWS".into(), "usize".into()),
                ("COLS".into(), "usize".into()),
            ],
            where_clause: WhereClause::empty(),
            return_type: "Mat<f64, ROWS, COLS>".into(),
        };
        let result = resolve_dep_return_type(&sig, &[3, 4]).unwrap();
        assert_eq!(result, "Mat<f64, 3, 4>");
    }

    #[test]
    fn v14_dt6_nat_pattern_matching() {
        assert!(nat_pattern_matches(&NatPattern::Literal(5), 5));
        assert!(!nat_pattern_matches(&NatPattern::Literal(5), 6));
        assert!(nat_pattern_matches(&NatPattern::Wildcard, 999));
        assert!(nat_pattern_matches(&NatPattern::Binding("n".into()), 42));
        assert!(nat_pattern_matches(
            &NatPattern::Range { start: 1, end_inclusive: 10 },
            5,
        ));
        assert!(!nat_pattern_matches(
            &NatPattern::Range { start: 1, end_inclusive: 10 },
            11,
        ));
    }

    #[test]
    fn v14_dt6_nat_match_arm() {
        let arms = vec![
            NatMatchArm { pattern: NatPattern::Literal(0), result_type: "Empty".into() },
            NatMatchArm { pattern: NatPattern::Literal(1), result_type: "Single".into() },
            NatMatchArm { pattern: NatPattern::Wildcard, result_type: "Array".into() },
        ];
        let val = 1u64;
        let matched = arms.iter().find(|a| nat_pattern_matches(&a.pattern, val));
        assert_eq!(matched.unwrap().result_type, "Single");
    }

    #[test]
    fn v14_dt6_where_clause_check() {
        use super::super::nat::{NatConstraint, NatValue};
        let wc = WhereClause {
            constraints: vec![NatConstraint::GreaterThan(
                NatValue::Param("N".into()),
                0,
            )],
        };
        let mut env = HashMap::new();
        env.insert("N".to_string(), 5);
        assert!(wc.check_all(&env).is_ok());
        env.insert("N".to_string(), 0);
        assert!(wc.check_all(&env).is_err());
    }
}
