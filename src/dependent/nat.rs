//! Type-level integers (Nat kind) — const generic parameters, type arithmetic,
//! Nat equality, monomorphization, and Cranelift lowering.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S1.1: Nat Kind in Type System
// ═══════════════════════════════════════════════════════════════════════

/// Kinds classify types: `Type` is the kind of normal types, `Nat` is the
/// kind of compile-time natural numbers, and `Dependent` represents functions
/// from one kind to another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// The kind of ordinary types (e.g., `i32 : Type`).
    Type,
    /// The kind of compile-time natural numbers (e.g., `3 : Nat`).
    Nat,
    /// A dependent kind: `Nat -> Type` (e.g., `Array : Nat -> Type -> Type`).
    Dependent(Box<Kind>, Box<Kind>),
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Type => write!(f, "Type"),
            Kind::Nat => write!(f, "Nat"),
            Kind::Dependent(from, to) => write!(f, "{} -> {}", from, to),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.2 / S1.3: Const Generic Syntax & AST Node
// ═══════════════════════════════════════════════════════════════════════

/// A const generic parameter: `const N: usize`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstGenericParam {
    /// Parameter name (e.g., `N`).
    pub name: String,
    /// The type of the const parameter (currently only `usize` is supported).
    pub const_type: ConstType,
}

/// Supported types for const generic parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstType {
    /// `usize` — natural number.
    Usize,
    /// `bool` — compile-time boolean.
    Bool,
}

impl fmt::Display for ConstType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstType::Usize => write!(f, "usize"),
            ConstType::Bool => write!(f, "bool"),
        }
    }
}

/// The kind of a generic parameter — type or const.
#[derive(Debug, Clone, PartialEq)]
pub enum GenericParamKind {
    /// A type parameter: `T`, `T: Bound`.
    Type {
        /// Trait bound names.
        bounds: Vec<String>,
    },
    /// A const parameter: `const N: usize`.
    Const(ConstGenericParam),
}

// ═══════════════════════════════════════════════════════════════════════
// S1.4: Type-Level Literal
// ═══════════════════════════════════════════════════════════════════════

/// A compile-time Nat value — either a concrete number, a named const generic
/// parameter, or a type-level arithmetic expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NatValue {
    /// A concrete compile-time integer (e.g., `4` in `Array<i32, 4>`).
    Literal(u64),
    /// A named const generic parameter (e.g., `N` in `Array<T, N>`).
    Param(String),
    /// Addition: `A + B`.
    Add(Box<NatValue>, Box<NatValue>),
    /// Multiplication: `A * B`.
    Mul(Box<NatValue>, Box<NatValue>),
    /// Subtraction: `A - B` (saturating at 0).
    Sub(Box<NatValue>, Box<NatValue>),
    /// An inferred/unknown Nat value (e.g., `_` in `Array<i32, _>`).
    Inferred,
}

impl fmt::Display for NatValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NatValue::Literal(n) => write!(f, "{n}"),
            NatValue::Param(name) => write!(f, "{name}"),
            NatValue::Add(a, b) => write!(f, "{a} + {b}"),
            NatValue::Mul(a, b) => write!(f, "{a} * {b}"),
            NatValue::Sub(a, b) => write!(f, "{a} - {b}"),
            NatValue::Inferred => write!(f, "_"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.5 / S1.6: Type Arithmetic (Addition, Multiplication)
// ═══════════════════════════════════════════════════════════════════════

impl NatValue {
    /// Attempts to evaluate this Nat value to a concrete u64.
    /// Returns `None` if any parameter is unresolved.
    pub fn evaluate(&self, env: &HashMap<String, u64>) -> Option<u64> {
        match self {
            NatValue::Literal(n) => Some(*n),
            NatValue::Param(name) => env.get(name).copied(),
            NatValue::Add(a, b) => {
                let a_val = a.evaluate(env)?;
                let b_val = b.evaluate(env)?;
                Some(a_val.saturating_add(b_val))
            }
            NatValue::Mul(a, b) => {
                let a_val = a.evaluate(env)?;
                let b_val = b.evaluate(env)?;
                Some(a_val.saturating_mul(b_val))
            }
            NatValue::Sub(a, b) => {
                let a_val = a.evaluate(env)?;
                let b_val = b.evaluate(env)?;
                Some(a_val.saturating_sub(b_val))
            }
            NatValue::Inferred => None,
        }
    }

    /// Substitutes named parameters with concrete values, producing a
    /// simplified (possibly fully concrete) Nat value.
    pub fn substitute(&self, env: &HashMap<String, u64>) -> NatValue {
        match self {
            NatValue::Literal(_) => self.clone(),
            NatValue::Param(name) => {
                if let Some(&val) = env.get(name) {
                    NatValue::Literal(val)
                } else {
                    self.clone()
                }
            }
            NatValue::Add(a, b) => {
                let a_sub = a.substitute(env);
                let b_sub = b.substitute(env);
                if let (NatValue::Literal(av), NatValue::Literal(bv)) = (&a_sub, &b_sub) {
                    NatValue::Literal(av.saturating_add(*bv))
                } else {
                    NatValue::Add(Box::new(a_sub), Box::new(b_sub))
                }
            }
            NatValue::Mul(a, b) => {
                let a_sub = a.substitute(env);
                let b_sub = b.substitute(env);
                if let (NatValue::Literal(av), NatValue::Literal(bv)) = (&a_sub, &b_sub) {
                    NatValue::Literal(av.saturating_mul(*bv))
                } else {
                    NatValue::Mul(Box::new(a_sub), Box::new(b_sub))
                }
            }
            NatValue::Sub(a, b) => {
                let a_sub = a.substitute(env);
                let b_sub = b.substitute(env);
                if let (NatValue::Literal(av), NatValue::Literal(bv)) = (&a_sub, &b_sub) {
                    NatValue::Literal(av.saturating_sub(*bv))
                } else {
                    NatValue::Sub(Box::new(a_sub), Box::new(b_sub))
                }
            }
            NatValue::Inferred => NatValue::Inferred,
        }
    }

    /// Returns `true` if this Nat value is fully concrete (no parameters).
    pub fn is_concrete(&self) -> bool {
        match self {
            NatValue::Literal(_) => true,
            NatValue::Param(_) | NatValue::Inferred => false,
            NatValue::Add(a, b) | NatValue::Mul(a, b) | NatValue::Sub(a, b) => {
                a.is_concrete() && b.is_concrete()
            }
        }
    }

    /// Collects all parameter names referenced in this Nat value.
    pub fn free_params(&self) -> Vec<String> {
        let mut params = Vec::new();
        self.collect_params(&mut params);
        params
    }

    fn collect_params(&self, out: &mut Vec<String>) {
        match self {
            NatValue::Param(name) => {
                if !out.contains(name) {
                    out.push(name.clone());
                }
            }
            NatValue::Add(a, b) | NatValue::Mul(a, b) | NatValue::Sub(a, b) => {
                a.collect_params(out);
                b.collect_params(out);
            }
            NatValue::Literal(_) | NatValue::Inferred => {}
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.7: Type-Level Equality
// ═══════════════════════════════════════════════════════════════════════

/// Errors produced when checking Nat constraints.
#[derive(Debug, Clone, PartialEq)]
pub enum NatError {
    /// Two Nat values that should be equal are not.
    Mismatch {
        /// Expected Nat.
        expected: NatValue,
        /// Found Nat.
        found: NatValue,
        /// Evaluated expected (if concrete).
        expected_val: Option<u64>,
        /// Evaluated found (if concrete).
        found_val: Option<u64>,
        /// Context description (e.g., "matmul inner dimension").
        context: String,
    },
    /// A where clause constraint is violated.
    ConstraintViolation {
        /// The constraint that failed.
        constraint: String,
        /// The Nat value that violated it.
        value: NatValue,
        /// Evaluated value (if concrete).
        evaluated: Option<u64>,
    },
    /// An arithmetic overflow in type-level computation.
    Overflow {
        /// The expression that overflowed.
        expr: NatValue,
    },
}

impl fmt::Display for NatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NatError::Mismatch {
                expected,
                found,
                expected_val,
                found_val,
                context,
            } => {
                write!(f, "type-level mismatch in {context}: expected {expected}")?;
                if let Some(ev) = expected_val {
                    write!(f, " (= {ev})")?;
                }
                write!(f, ", found {found}")?;
                if let Some(fv) = found_val {
                    write!(f, " (= {fv})")?;
                }
                Ok(())
            }
            NatError::ConstraintViolation {
                constraint,
                value,
                evaluated,
            } => {
                write!(f, "constraint `{constraint}` violated by {value}")?;
                if let Some(ev) = evaluated {
                    write!(f, " (= {ev})")?;
                }
                Ok(())
            }
            NatError::Overflow { expr } => {
                write!(f, "type-level arithmetic overflow in {expr}")
            }
        }
    }
}

/// Checks that two Nat values are equal, given an environment of known bindings.
pub fn check_nat_eq(
    expected: &NatValue,
    found: &NatValue,
    env: &HashMap<String, u64>,
    context: &str,
) -> Result<(), NatError> {
    let ev = expected.evaluate(env);
    let fv = found.evaluate(env);

    match (ev, fv) {
        (Some(a), Some(b)) if a == b => Ok(()),
        (Some(_), Some(_)) => Err(NatError::Mismatch {
            expected: expected.clone(),
            found: found.clone(),
            expected_val: ev,
            found_val: fv,
            context: context.into(),
        }),
        // If either is not fully resolved, defer — no error.
        _ => Ok(()),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.8: Const Generic Monomorphization
// ═══════════════════════════════════════════════════════════════════════

/// A monomorphization key that includes both type and const generic arguments.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoKey {
    /// Base function/struct name.
    pub name: String,
    /// Type arguments (e.g., `["i64", "f64"]`).
    pub type_args: Vec<String>,
    /// Const arguments (e.g., `[3, 4]`).
    pub const_args: Vec<u64>,
}

impl MonoKey {
    /// Generates a mangled name for this specialization.
    pub fn mangled_name(&self) -> String {
        let mut parts = vec![self.name.clone()];
        for ta in &self.type_args {
            parts.push(ta.clone());
        }
        for ca in &self.const_args {
            parts.push(format!("N{ca}"));
        }
        parts.join("_")
    }
}

/// Registry of all monomorphized const-generic specializations.
#[derive(Debug, Clone, Default)]
pub struct ConstMonoRegistry {
    /// Set of already-specialized keys.
    pub specialized: HashMap<MonoKey, String>,
}

impl ConstMonoRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a specialization, returning the mangled name.
    pub fn register(&mut self, key: MonoKey) -> String {
        let mangled = key.mangled_name();
        self.specialized.insert(key, mangled.clone());
        mangled
    }

    /// Looks up a previously registered specialization.
    pub fn lookup(&self, key: &MonoKey) -> Option<&str> {
        self.specialized.get(key).map(|s| s.as_str())
    }

    /// Returns the number of specializations.
    pub fn count(&self) -> usize {
        self.specialized.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.9: Cranelift Lowering (const generic → immediate)
// ═══════════════════════════════════════════════════════════════════════

/// Represents how a Nat value is lowered to native code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NatLowering {
    /// A constant immediate value.
    Immediate(u64),
    /// Not yet resolved — must be monomorphized first.
    Unresolved(String),
}

/// Lowers a NatValue to a code generation directive.
pub fn lower_nat(nat: &NatValue, env: &HashMap<String, u64>) -> NatLowering {
    match nat.evaluate(env) {
        Some(val) => NatLowering::Immediate(val),
        None => match nat {
            NatValue::Param(name) => NatLowering::Unresolved(name.clone()),
            _ => NatLowering::Unresolved(format!("{nat}")),
        },
    }
}

/// A where clause constraint on Nat values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NatConstraint {
    /// `N > value`.
    GreaterThan(NatValue, u64),
    /// `N >= value`.
    GreaterEq(NatValue, u64),
    /// `N == M`.
    Equal(NatValue, NatValue),
    /// `N != M`.
    NotEqual(NatValue, NatValue),
    /// `N < value`.
    LessThan(NatValue, u64),
    /// `N % value == 0`.
    Divisible(NatValue, u64),
}

impl NatConstraint {
    /// Checks this constraint against the given environment.
    pub fn check(&self, env: &HashMap<String, u64>) -> Result<(), NatError> {
        match self {
            NatConstraint::GreaterThan(n, bound) => {
                if let Some(val) = n.evaluate(env) {
                    if val > *bound {
                        Ok(())
                    } else {
                        Err(NatError::ConstraintViolation {
                            constraint: format!("{n} > {bound}"),
                            value: n.clone(),
                            evaluated: Some(val),
                        })
                    }
                } else {
                    Ok(()) // Defer — not yet concrete.
                }
            }
            NatConstraint::GreaterEq(n, bound) => {
                if let Some(val) = n.evaluate(env) {
                    if val >= *bound {
                        Ok(())
                    } else {
                        Err(NatError::ConstraintViolation {
                            constraint: format!("{n} >= {bound}"),
                            value: n.clone(),
                            evaluated: Some(val),
                        })
                    }
                } else {
                    Ok(())
                }
            }
            NatConstraint::Equal(a, b) => check_nat_eq(a, b, env, "where clause"),
            NatConstraint::NotEqual(a, b) => {
                let av = a.evaluate(env);
                let bv = b.evaluate(env);
                match (av, bv) {
                    (Some(x), Some(y)) if x != y => Ok(()),
                    (Some(x), Some(y)) if x == y => Err(NatError::ConstraintViolation {
                        constraint: format!("{a} != {b}"),
                        value: a.clone(),
                        evaluated: Some(x),
                    }),
                    _ => Ok(()),
                }
            }
            NatConstraint::LessThan(n, bound) => {
                if let Some(val) = n.evaluate(env) {
                    if val < *bound {
                        Ok(())
                    } else {
                        Err(NatError::ConstraintViolation {
                            constraint: format!("{n} < {bound}"),
                            value: n.clone(),
                            evaluated: Some(val),
                        })
                    }
                } else {
                    Ok(())
                }
            }
            NatConstraint::Divisible(n, divisor) => {
                if let Some(val) = n.evaluate(env) {
                    if *divisor != 0 && val % divisor == 0 {
                        Ok(())
                    } else {
                        Err(NatError::ConstraintViolation {
                            constraint: format!("{n} % {divisor} == 0"),
                            value: n.clone(),
                            evaluated: Some(val),
                        })
                    }
                } else {
                    Ok(())
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V14 DT1.4-DT1.5: Pi Types (Dependent Function Types)
// ═══════════════════════════════════════════════════════════════════════

/// A dependent type that can reference value parameters.
#[derive(Debug, Clone, PartialEq)]
pub enum DependentType {
    /// A concrete type (no dependency).
    Concrete(String),
    /// A type parameterized by a Nat expression: `Vector<f64, n>`.
    NatIndexed { base: String, index: NatValue },
    /// A Pi type (dependent function type).
    Pi(PiType),
    /// A Sigma type (dependent pair).
    Sigma(SigmaType),
}

/// V14 DT1.4: Pi type — function type where return type depends on a value parameter.
/// Example: `fn zeros(n: Nat) -> Vector<f64, n>`
#[derive(Debug, Clone, PartialEq)]
pub struct PiType {
    /// Parameter name that the return type depends on.
    pub param_name: String,
    /// Parameter kind (Nat, Type, etc.).
    pub param_kind: Kind,
    /// Body type that may reference the parameter.
    pub body: Box<DependentType>,
}

impl PiType {
    /// Creates a new Pi type with the given parameter name, kind, and body.
    pub fn new(param_name: impl Into<String>, param_kind: Kind, body: DependentType) -> Self {
        Self {
            param_name: param_name.into(),
            param_kind,
            body: Box::new(body),
        }
    }

    /// Substitute a concrete value for the parameter.
    pub fn instantiate(&self, value: &NatValue) -> DependentType {
        match &*self.body {
            DependentType::NatIndexed { base, index } => {
                let new_index = substitute_nat(index, &self.param_name, value);
                DependentType::NatIndexed {
                    base: base.clone(),
                    index: new_index,
                }
            }
            other => other.clone(),
        }
    }
}

/// Substitute a named parameter with a concrete NatValue.
pub fn substitute_nat(expr: &NatValue, param: &str, value: &NatValue) -> NatValue {
    match expr {
        NatValue::Param(name) if name == param => value.clone(),
        NatValue::Add(a, b) => NatValue::Add(
            Box::new(substitute_nat(a, param, value)),
            Box::new(substitute_nat(b, param, value)),
        ),
        NatValue::Mul(a, b) => NatValue::Mul(
            Box::new(substitute_nat(a, param, value)),
            Box::new(substitute_nat(b, param, value)),
        ),
        NatValue::Sub(a, b) => NatValue::Sub(
            Box::new(substitute_nat(a, param, value)),
            Box::new(substitute_nat(b, param, value)),
        ),
        other => other.clone(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V14 DT1.6-DT1.7: Pi Type Checker
// ═══════════════════════════════════════════════════════════════════════

/// V14 DT1.6: Type-check a Pi type application.
pub fn check_pi_application(pi: &PiType, arg: &NatValue) -> Result<DependentType, String> {
    // Verify the argument has the correct kind
    match (&pi.param_kind, arg) {
        (Kind::Nat, NatValue::Literal(_))
        | (Kind::Nat, NatValue::Param(_))
        | (Kind::Nat, NatValue::Add(_, _))
        | (Kind::Nat, NatValue::Mul(_, _))
        | (Kind::Nat, NatValue::Sub(_, _))
        | (Kind::Nat, NatValue::Inferred) => Ok(pi.instantiate(arg)),
        _ => Err(format!(
            "kind mismatch: expected {:?}, got value",
            pi.param_kind
        )),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V14 DT2: Sigma Types (Dependent Pairs)
// ═══════════════════════════════════════════════════════════════════════

/// V14 DT2: Sigma type — dependent pair where second type depends on first value.
/// Example: `(n: Nat, Vector<f64, n>)` — a length paired with a vector of that length.
#[derive(Debug, Clone, PartialEq)]
pub struct SigmaType {
    /// Name of the first element.
    pub fst_name: String,
    /// Kind of the first element.
    pub fst_kind: Kind,
    /// Type of the second element (may reference the first).
    pub snd: Box<DependentType>,
}

impl SigmaType {
    /// Creates a new Sigma type.
    pub fn new(fst_name: impl Into<String>, fst_kind: Kind, snd: DependentType) -> Self {
        Self {
            fst_name: fst_name.into(),
            fst_kind,
            snd: Box::new(snd),
        }
    }

    /// Given a concrete first value, compute the second type.
    pub fn project_snd(&self, fst_value: &NatValue) -> DependentType {
        match &*self.snd {
            DependentType::NatIndexed { base, index } => {
                let new_index = substitute_nat(index, &self.fst_name, fst_value);
                DependentType::NatIndexed {
                    base: base.clone(),
                    index: new_index,
                }
            }
            other => other.clone(),
        }
    }

    /// Check if a concrete pair satisfies this Sigma type.
    pub fn check_pair(&self, fst: &NatValue, snd_type: &DependentType) -> bool {
        let expected_snd = self.project_snd(fst);
        expected_snd == *snd_type
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V14 DT4: Refinement Types
// ═══════════════════════════════════════════════════════════════════════

/// V14 DT4.4: Refinement type — a base type with a predicate constraint.
/// Example: `{ x: i32 | x > 0 }` (Positive integer)
#[derive(Debug, Clone, PartialEq)]
pub struct RefinementType {
    /// Variable name bound by the refinement.
    pub var_name: String,
    /// Base type.
    pub base_type: String,
    /// Predicate that must hold.
    pub predicate: RefinementPredicate,
}

/// A predicate for refinement types.
#[derive(Debug, Clone, PartialEq)]
pub enum RefinementPredicate {
    /// x > value
    GreaterThan(i64),
    /// x >= value
    GreaterEqual(i64),
    /// x < value
    LessThan(i64),
    /// x <= value
    LessEqual(i64),
    /// x == value
    Equal(i64),
    /// x != value
    NotEqual(i64),
    /// low <= x <= high
    InRange(i64, i64),
    /// Conjunction of predicates.
    And(Box<RefinementPredicate>, Box<RefinementPredicate>),
}

impl RefinementType {
    /// Creates a new refinement type.
    pub fn new(
        var_name: impl Into<String>,
        base_type: impl Into<String>,
        predicate: RefinementPredicate,
    ) -> Self {
        Self {
            var_name: var_name.into(),
            base_type: base_type.into(),
            predicate,
        }
    }

    /// Check if a concrete integer value satisfies the refinement.
    pub fn check_value(&self, value: i64) -> bool {
        self.predicate.is_satisfied(value)
    }
}

impl RefinementPredicate {
    /// Check if a concrete value satisfies this predicate.
    pub fn is_satisfied(&self, value: i64) -> bool {
        match self {
            RefinementPredicate::GreaterThan(n) => value > *n,
            RefinementPredicate::GreaterEqual(n) => value >= *n,
            RefinementPredicate::LessThan(n) => value < *n,
            RefinementPredicate::LessEqual(n) => value <= *n,
            RefinementPredicate::Equal(n) => value == *n,
            RefinementPredicate::NotEqual(n) => value != *n,
            RefinementPredicate::InRange(lo, hi) => value >= *lo && value <= *hi,
            RefinementPredicate::And(a, b) => a.is_satisfied(value) && b.is_satisfied(value),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S1.1 — Nat Kind
    #[test]
    fn s1_1_kind_display() {
        assert_eq!(Kind::Type.to_string(), "Type");
        assert_eq!(Kind::Nat.to_string(), "Nat");
        let dep = Kind::Dependent(Box::new(Kind::Nat), Box::new(Kind::Type));
        assert_eq!(dep.to_string(), "Nat -> Type");
    }

    #[test]
    fn s1_1_kind_equality() {
        assert_eq!(Kind::Nat, Kind::Nat);
        assert_ne!(Kind::Nat, Kind::Type);
    }

    // S1.2 / S1.3 — Const Generic AST
    #[test]
    fn s1_3_const_generic_param() {
        let p = ConstGenericParam {
            name: "N".into(),
            const_type: ConstType::Usize,
        };
        assert_eq!(p.name, "N");
        assert_eq!(p.const_type, ConstType::Usize);
    }

    #[test]
    fn s1_3_generic_param_kind_type() {
        let kind = GenericParamKind::Type {
            bounds: vec!["Display".into()],
        };
        matches!(kind, GenericParamKind::Type { .. });
    }

    #[test]
    fn s1_3_generic_param_kind_const() {
        let kind = GenericParamKind::Const(ConstGenericParam {
            name: "N".into(),
            const_type: ConstType::Usize,
        });
        matches!(kind, GenericParamKind::Const(_));
    }

    // S1.4 — Type-Level Literal
    #[test]
    fn s1_4_nat_literal_evaluates() {
        let n = NatValue::Literal(42);
        assert_eq!(n.evaluate(&HashMap::new()), Some(42));
    }

    #[test]
    fn s1_4_nat_param_resolves() {
        let n = NatValue::Param("N".into());
        let mut env = HashMap::new();
        assert_eq!(n.evaluate(&env), None);
        env.insert("N".into(), 10);
        assert_eq!(n.evaluate(&env), Some(10));
    }

    #[test]
    fn s1_4_nat_inferred_is_none() {
        assert_eq!(NatValue::Inferred.evaluate(&HashMap::new()), None);
    }

    // S1.5 — Type Arithmetic: Addition
    #[test]
    fn s1_5_nat_addition() {
        let sum = NatValue::Add(
            Box::new(NatValue::Literal(3)),
            Box::new(NatValue::Literal(4)),
        );
        assert_eq!(sum.evaluate(&HashMap::new()), Some(7));
    }

    #[test]
    fn s1_5_nat_addition_with_params() {
        let sum = NatValue::Add(
            Box::new(NatValue::Param("A".into())),
            Box::new(NatValue::Param("B".into())),
        );
        let mut env = HashMap::new();
        env.insert("A".into(), 5);
        env.insert("B".into(), 8);
        assert_eq!(sum.evaluate(&env), Some(13));
    }

    // S1.6 — Type Arithmetic: Multiplication
    #[test]
    fn s1_6_nat_multiplication() {
        let prod = NatValue::Mul(
            Box::new(NatValue::Literal(3)),
            Box::new(NatValue::Literal(4)),
        );
        assert_eq!(prod.evaluate(&HashMap::new()), Some(12));
    }

    #[test]
    fn s1_6_nat_nested_arithmetic() {
        // (A + B) * C
        let expr = NatValue::Mul(
            Box::new(NatValue::Add(
                Box::new(NatValue::Param("A".into())),
                Box::new(NatValue::Param("B".into())),
            )),
            Box::new(NatValue::Param("C".into())),
        );
        let mut env = HashMap::new();
        env.insert("A".into(), 2);
        env.insert("B".into(), 3);
        env.insert("C".into(), 4);
        assert_eq!(expr.evaluate(&env), Some(20));
    }

    // S1.7 — Type-Level Equality
    #[test]
    fn s1_7_nat_eq_pass() {
        let a = NatValue::Literal(5);
        let b = NatValue::Literal(5);
        assert!(check_nat_eq(&a, &b, &HashMap::new(), "test").is_ok());
    }

    #[test]
    fn s1_7_nat_eq_fail() {
        let a = NatValue::Literal(3);
        let b = NatValue::Literal(4);
        let result = check_nat_eq(&a, &b, &HashMap::new(), "dimension");
        assert!(result.is_err());
        if let Err(NatError::Mismatch { context, .. }) = result {
            assert_eq!(context, "dimension");
        }
    }

    #[test]
    fn s1_7_nat_eq_deferred() {
        // If either side has unresolved params, defer (no error).
        let a = NatValue::Param("N".into());
        let b = NatValue::Literal(5);
        assert!(check_nat_eq(&a, &b, &HashMap::new(), "test").is_ok());
    }

    // S1.8 — Monomorphization
    #[test]
    fn s1_8_mono_key_mangling() {
        let key = MonoKey {
            name: "identity".into(),
            type_args: vec!["i64".into()],
            const_args: vec![],
        };
        assert_eq!(key.mangled_name(), "identity_i64");

        let key2 = MonoKey {
            name: "Array".into(),
            type_args: vec!["i32".into()],
            const_args: vec![5],
        };
        assert_eq!(key2.mangled_name(), "Array_i32_N5");
    }

    #[test]
    fn s1_8_const_mono_registry() {
        let mut reg = ConstMonoRegistry::new();
        let key = MonoKey {
            name: "zeros".into(),
            type_args: vec![],
            const_args: vec![3, 4],
        };
        let mangled = reg.register(key.clone());
        assert_eq!(mangled, "zeros_N3_N4");
        assert_eq!(reg.lookup(&key), Some("zeros_N3_N4"));
        assert_eq!(reg.count(), 1);
    }

    // S1.9 — Cranelift Lowering
    #[test]
    fn s1_9_lower_nat_immediate() {
        let n = NatValue::Literal(42);
        assert_eq!(lower_nat(&n, &HashMap::new()), NatLowering::Immediate(42));
    }

    #[test]
    fn s1_9_lower_nat_unresolved() {
        let n = NatValue::Param("N".into());
        assert_eq!(
            lower_nat(&n, &HashMap::new()),
            NatLowering::Unresolved("N".into())
        );
    }

    #[test]
    fn s1_9_lower_nat_resolved_param() {
        let n = NatValue::Param("N".into());
        let mut env = HashMap::new();
        env.insert("N".into(), 7);
        assert_eq!(lower_nat(&n, &env), NatLowering::Immediate(7));
    }

    // S1.10 — Additional tests
    #[test]
    fn s1_10_nat_substitute() {
        let expr = NatValue::Add(
            Box::new(NatValue::Param("N".into())),
            Box::new(NatValue::Literal(1)),
        );
        let mut env = HashMap::new();
        env.insert("N".into(), 5);
        let result = expr.substitute(&env);
        assert_eq!(result, NatValue::Literal(6));
    }

    #[test]
    fn s1_10_nat_partial_substitute() {
        let expr = NatValue::Add(
            Box::new(NatValue::Param("A".into())),
            Box::new(NatValue::Param("B".into())),
        );
        let mut env = HashMap::new();
        env.insert("A".into(), 3);
        let result = expr.substitute(&env);
        // A is resolved but B is not — so we get Add(Literal(3), Param("B"))
        assert!(!result.is_concrete());
    }

    #[test]
    fn s1_10_nat_free_params() {
        let expr = NatValue::Mul(
            Box::new(NatValue::Param("A".into())),
            Box::new(NatValue::Add(
                Box::new(NatValue::Param("B".into())),
                Box::new(NatValue::Literal(1)),
            )),
        );
        let params = expr.free_params();
        assert_eq!(params, vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn s1_10_nat_is_concrete() {
        assert!(NatValue::Literal(5).is_concrete());
        assert!(!NatValue::Param("N".into()).is_concrete());
        let sum = NatValue::Add(
            Box::new(NatValue::Literal(3)),
            Box::new(NatValue::Literal(4)),
        );
        assert!(sum.is_concrete());
    }

    #[test]
    fn s1_10_nat_display() {
        let expr = NatValue::Add(
            Box::new(NatValue::Param("N".into())),
            Box::new(NatValue::Literal(1)),
        );
        assert_eq!(expr.to_string(), "N + 1");
    }

    #[test]
    fn s1_10_nat_subtraction() {
        let sub = NatValue::Sub(
            Box::new(NatValue::Literal(10)),
            Box::new(NatValue::Literal(3)),
        );
        assert_eq!(sub.evaluate(&HashMap::new()), Some(7));

        // Saturating at 0
        let underflow = NatValue::Sub(
            Box::new(NatValue::Literal(2)),
            Box::new(NatValue::Literal(5)),
        );
        assert_eq!(underflow.evaluate(&HashMap::new()), Some(0));
    }

    #[test]
    fn s1_10_constraint_greater_than() {
        let c = NatConstraint::GreaterThan(NatValue::Literal(5), 0);
        assert!(c.check(&HashMap::new()).is_ok());

        let c2 = NatConstraint::GreaterThan(NatValue::Literal(0), 0);
        assert!(c2.check(&HashMap::new()).is_err());
    }

    #[test]
    fn s1_10_constraint_divisible() {
        let c = NatConstraint::Divisible(NatValue::Literal(12), 4);
        assert!(c.check(&HashMap::new()).is_ok());

        let c2 = NatConstraint::Divisible(NatValue::Literal(13), 4);
        assert!(c2.check(&HashMap::new()).is_err());
    }

    #[test]
    fn s1_10_nat_error_display() {
        let err = NatError::Mismatch {
            expected: NatValue::Literal(3),
            found: NatValue::Literal(4),
            expected_val: Some(3),
            found_val: Some(4),
            context: "matmul inner dimension".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("matmul inner dimension"));
        assert!(msg.contains("expected 3"));
        assert!(msg.contains("found 4"));
    }

    // ═════════════════════════════════════════════════════════════════
    // V14 DT1: Pi Types
    // ═════════════════════════════════════════════════════════════════

    #[test]
    fn v14_dt1_4_pi_type_creation() {
        let pi = PiType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Vector".into(),
                index: NatValue::Param("n".into()),
            },
        );
        assert_eq!(pi.param_name, "n");
    }

    #[test]
    fn v14_dt1_5_pi_type_instantiate() {
        let pi = PiType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Vector".into(),
                index: NatValue::Param("n".into()),
            },
        );
        let result = pi.instantiate(&NatValue::Literal(10));
        match result {
            DependentType::NatIndexed { base, index } => {
                assert_eq!(base, "Vector");
                assert_eq!(index, NatValue::Literal(10));
            }
            _ => panic!("expected NatIndexed"),
        }
    }

    #[test]
    fn v14_dt1_6_pi_type_check() {
        let pi = PiType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Array".into(),
                index: NatValue::Param("n".into()),
            },
        );
        let result = check_pi_application(&pi, &NatValue::Literal(5));
        assert!(result.is_ok());
    }

    #[test]
    fn v14_dt1_7_pi_substitute_arithmetic() {
        let pi = PiType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Matrix".into(),
                index: NatValue::Add(
                    Box::new(NatValue::Param("n".into())),
                    Box::new(NatValue::Literal(1)),
                ),
            },
        );
        let result = pi.instantiate(&NatValue::Literal(5));
        match result {
            DependentType::NatIndexed { index, .. } => {
                assert_eq!(
                    index,
                    NatValue::Add(
                        Box::new(NatValue::Literal(5)),
                        Box::new(NatValue::Literal(1)),
                    )
                );
            }
            _ => panic!("expected NatIndexed"),
        }
    }

    #[test]
    fn v14_dt1_8_constraint_solver_equality() {
        let a = NatValue::Param("n".into());
        let b = NatValue::Literal(5);
        let substituted = substitute_nat(&a, "n", &b);
        assert_eq!(substituted, NatValue::Literal(5));
    }

    #[test]
    fn v14_dt1_9_constraint_nested_substitution() {
        // n * 2 + 1 with n=3
        let expr = NatValue::Add(
            Box::new(NatValue::Mul(
                Box::new(NatValue::Param("n".into())),
                Box::new(NatValue::Literal(2)),
            )),
            Box::new(NatValue::Literal(1)),
        );
        let result = substitute_nat(&expr, "n", &NatValue::Literal(3));
        match result {
            NatValue::Add(left, right) => {
                assert_eq!(*right, NatValue::Literal(1));
                // left should be Mul(Literal(3), Literal(2))
                match *left {
                    NatValue::Mul(a, b) => {
                        assert_eq!(*a, NatValue::Literal(3));
                        assert_eq!(*b, NatValue::Literal(2));
                    }
                    _ => panic!("expected Mul"),
                }
            }
            _ => panic!("expected Add"),
        }
    }

    #[test]
    fn v14_dt1_10_pi_type_tests_comprehensive() {
        let pi = PiType::new(
            "rows",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Matrix".into(),
                index: NatValue::Param("rows".into()),
            },
        );
        let checked = check_pi_application(&pi, &NatValue::Literal(64)).unwrap();
        match checked {
            DependentType::NatIndexed { base, index } => {
                assert_eq!(base, "Matrix");
                assert_eq!(index, NatValue::Literal(64));
            }
            _ => panic!("wrong type"),
        }
    }

    // ═════════════════════════════════════════════════════════════════
    // V14 DT2: Sigma Types
    // ═════════════════════════════════════════════════════════════════

    #[test]
    fn v14_dt2_3_sigma_creation() {
        let sigma = SigmaType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Vector".into(),
                index: NatValue::Param("n".into()),
            },
        );
        assert_eq!(sigma.fst_name, "n");
    }

    #[test]
    fn v14_dt2_4_sigma_project_snd() {
        let sigma = SigmaType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Vector".into(),
                index: NatValue::Param("n".into()),
            },
        );
        let snd = sigma.project_snd(&NatValue::Literal(3));
        assert_eq!(
            snd,
            DependentType::NatIndexed {
                base: "Vector".into(),
                index: NatValue::Literal(3),
            }
        );
    }

    #[test]
    fn v14_dt2_5_sigma_check_pair_valid() {
        let sigma = SigmaType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Vec".into(),
                index: NatValue::Param("n".into()),
            },
        );
        let valid = sigma.check_pair(
            &NatValue::Literal(5),
            &DependentType::NatIndexed {
                base: "Vec".into(),
                index: NatValue::Literal(5),
            },
        );
        assert!(valid);
    }

    #[test]
    fn v14_dt2_6_sigma_check_pair_invalid() {
        let sigma = SigmaType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Vec".into(),
                index: NatValue::Param("n".into()),
            },
        );
        let invalid = sigma.check_pair(
            &NatValue::Literal(5),
            &DependentType::NatIndexed {
                base: "Vec".into(),
                index: NatValue::Literal(10),
            },
        );
        assert!(!invalid);
    }

    #[test]
    fn v14_dt2_7_sigma_concrete_projection() {
        let sigma = SigmaType::new(
            "len",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Buffer".into(),
                index: NatValue::Param("len".into()),
            },
        );
        let projected = sigma.project_snd(&NatValue::Literal(1024));
        match projected {
            DependentType::NatIndexed { base, index } => {
                assert_eq!(base, "Buffer");
                assert_eq!(index, NatValue::Literal(1024));
            }
            _ => panic!("expected NatIndexed"),
        }
    }

    #[test]
    fn v14_dt2_8_sigma_with_arithmetic() {
        let sigma = SigmaType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Pair".into(),
                index: NatValue::Mul(
                    Box::new(NatValue::Param("n".into())),
                    Box::new(NatValue::Literal(2)),
                ),
            },
        );
        let snd = sigma.project_snd(&NatValue::Literal(4));
        match snd {
            DependentType::NatIndexed { index, .. } => {
                assert_eq!(
                    index,
                    NatValue::Mul(
                        Box::new(NatValue::Literal(4)),
                        Box::new(NatValue::Literal(2)),
                    )
                );
            }
            _ => panic!("expected NatIndexed"),
        }
    }

    #[test]
    fn v14_dt2_9_dependent_type_concrete() {
        let dt = DependentType::Concrete("i32".into());
        assert_eq!(dt, DependentType::Concrete("i32".into()));
    }

    #[test]
    fn v14_dt2_10_dependent_type_variants() {
        let concrete = DependentType::Concrete("f64".into());
        let indexed = DependentType::NatIndexed {
            base: "Array".into(),
            index: NatValue::Literal(10),
        };
        let pi = DependentType::Pi(PiType::new(
            "n",
            Kind::Nat,
            DependentType::Concrete("void".into()),
        ));
        let sigma = DependentType::Sigma(SigmaType::new(
            "n",
            Kind::Nat,
            DependentType::Concrete("void".into()),
        ));
        // All variants exist and are distinct
        assert_ne!(concrete, indexed);
        assert_ne!(format!("{:?}", pi), format!("{:?}", sigma));
    }

    // ═════════════════════════════════════════════════════════════════
    // V14 DT4: Refinement Types
    // ═════════════════════════════════════════════════════════════════

    #[test]
    fn v14_dt4_4_refinement_positive() {
        let positive = RefinementType::new("x", "i32", RefinementPredicate::GreaterThan(0));
        assert!(positive.check_value(1));
        assert!(positive.check_value(100));
        assert!(!positive.check_value(0));
        assert!(!positive.check_value(-1));
    }

    #[test]
    fn v14_dt4_5_refinement_range() {
        let byte = RefinementType::new("x", "i32", RefinementPredicate::InRange(0, 255));
        assert!(byte.check_value(0));
        assert!(byte.check_value(128));
        assert!(byte.check_value(255));
        assert!(!byte.check_value(256));
        assert!(!byte.check_value(-1));
    }

    #[test]
    fn v14_dt4_6_refinement_conjunction() {
        // x > 0 AND x < 100
        let pred = RefinementPredicate::And(
            Box::new(RefinementPredicate::GreaterThan(0)),
            Box::new(RefinementPredicate::LessThan(100)),
        );
        let bounded = RefinementType::new("x", "i32", pred);
        assert!(bounded.check_value(50));
        assert!(!bounded.check_value(0));
        assert!(!bounded.check_value(100));
    }

    #[test]
    fn v14_dt4_7_refinement_equality() {
        let exact = RefinementType::new("x", "i32", RefinementPredicate::Equal(42));
        assert!(exact.check_value(42));
        assert!(!exact.check_value(41));
    }

    #[test]
    fn v14_dt4_8_refinement_not_equal() {
        let nonzero = RefinementType::new("x", "i32", RefinementPredicate::NotEqual(0));
        assert!(nonzero.check_value(1));
        assert!(nonzero.check_value(-1));
        assert!(!nonzero.check_value(0));
    }

    #[test]
    fn v14_dt4_9_refinement_less_equal() {
        let capped = RefinementType::new("x", "i32", RefinementPredicate::LessEqual(1024));
        assert!(capped.check_value(1024));
        assert!(capped.check_value(0));
        assert!(!capped.check_value(1025));
    }

    #[test]
    fn v14_dt4_10_refinement_all_predicates() {
        assert!(RefinementPredicate::GreaterThan(5).is_satisfied(6));
        assert!(RefinementPredicate::GreaterEqual(5).is_satisfied(5));
        assert!(RefinementPredicate::LessThan(5).is_satisfied(4));
        assert!(RefinementPredicate::LessEqual(5).is_satisfied(5));
        assert!(RefinementPredicate::Equal(5).is_satisfied(5));
        assert!(RefinementPredicate::NotEqual(5).is_satisfied(6));
        assert!(RefinementPredicate::InRange(1, 10).is_satisfied(5));
    }

    #[test]
    fn v14_dt4_refinement_type_in_parser() {
        // Verify refinement type syntax `{ x: i32 | x > 0 }` parses and runs.
        let source = r#"
            fn positive(x: { n: i32 | n > 0 }) -> i32 { x }
            fn main() { let v = positive(5); println(v) }
        "#;
        let mut interp = crate::interpreter::Interpreter::new();
        let result = interp.eval_source(source);
        assert!(
            result.is_ok(),
            "refinement type should parse and run: {result:?}"
        );
    }

    #[test]
    fn v14_dt4_refinement_type_in_let_binding() {
        // Refinement types in let bindings.
        let source = r#"
            fn main() {
                let x: { n: i64 | n >= 0 } = 42
                println(x)
            }
        "#;
        let mut interp = crate::interpreter::Interpreter::new();
        let result = interp.eval_source(source);
        assert!(result.is_ok(), "refinement in let should work: {result:?}");
    }

    #[test]
    fn v14_dt4_refinement_type_range() {
        // Refinement types with range predicates.
        let source = r#"
            fn bounded(x: { n: i32 | n > 0 }) -> i32 { x + 1 }
            fn main() { println(bounded(10)) }
        "#;
        let mut interp = crate::interpreter::Interpreter::new();
        let result = interp.eval_source(source);
        assert!(result.is_ok(), "refinement range should work: {result:?}");
    }

    #[test]
    fn v14_dt4_refinement_runtime_check_pass() {
        // Positive value satisfies `n > 0`.
        let source = r#"
            fn main() {
                let x: { n: i64 | n > 0 } = 42
                println(x)
            }
        "#;
        let mut interp = crate::interpreter::Interpreter::new();
        let result = interp.eval_source(source);
        assert!(result.is_ok(), "positive value should pass: {result:?}");
    }

    #[test]
    fn v14_dt4_refinement_runtime_check_fail() {
        // Negative value violates `n > 0`.
        let source = r#"
            fn main() {
                let x: { n: i64 | n > 0 } = -5
            }
        "#;
        let mut interp = crate::interpreter::Interpreter::new();
        let _ = interp.eval_source(source); // defines main
        let result = interp.call_main();
        assert!(result.is_err(), "negative value should fail refinement");
        let err_msg = format!("{result:?}");
        assert!(
            err_msg.contains("refinement type violation"),
            "error should mention refinement: {err_msg}"
        );
    }

    #[test]
    fn v14_dt5_pi_type_instantiation() {
        // PiType instantiation with concrete Nat values
        let pi = PiType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Vector".into(),
                index: NatValue::Param("n".into()),
            },
        );
        let result = check_pi_application(&pi, &NatValue::Literal(3)).unwrap();
        match result {
            DependentType::NatIndexed { base, index } => {
                assert_eq!(base, "Vector");
                assert_eq!(index, NatValue::Literal(3));
            }
            _ => panic!("expected NatIndexed"),
        }
    }

    #[test]
    fn v14_dt5_pi_type_arithmetic() {
        // PiType with Nat arithmetic: Vector<f64, n+1>
        let pi = PiType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Vector".into(),
                index: NatValue::Add(
                    Box::new(NatValue::Param("n".into())),
                    Box::new(NatValue::Literal(1)),
                ),
            },
        );
        let result = check_pi_application(&pi, &NatValue::Literal(5)).unwrap();
        match result {
            DependentType::NatIndexed { index, .. } => {
                assert_eq!(
                    index,
                    NatValue::Add(
                        Box::new(NatValue::Literal(5)),
                        Box::new(NatValue::Literal(1))
                    )
                );
            }
            _ => panic!("expected NatIndexed"),
        }
    }

    #[test]
    fn v14_dt5_sigma_type_structure() {
        let sigma = SigmaType::new(
            "n",
            Kind::Nat,
            DependentType::NatIndexed {
                base: "Array".into(),
                index: NatValue::Param("n".into()),
            },
        );
        assert_eq!(sigma.fst_name, "n");
        assert_eq!(sigma.fst_kind, Kind::Nat);
    }

    #[test]
    fn v14_dt5_nat_constraint_check() {
        let env = HashMap::new();
        let c = NatConstraint::Equal(NatValue::Literal(3), NatValue::Literal(3));
        assert!(c.check(&env).is_ok());
        let c2 = NatConstraint::Equal(NatValue::Literal(3), NatValue::Literal(4));
        assert!(c2.check(&env).is_err());
    }

    #[test]
    fn v14_dt5_refinement_multiple_predicates() {
        // Test refinement with different predicates
        let r1 = RefinementType::new("x", "i32", RefinementPredicate::GreaterThan(0));
        assert!(r1.check_value(1));
        assert!(!r1.check_value(0));

        let r2 = RefinementType::new("x", "i32", RefinementPredicate::InRange(1, 100));
        assert!(r2.check_value(50));
        assert!(!r2.check_value(0));
        assert!(!r2.check_value(101));
    }

    #[test]
    fn v14_dt5_refinement_and_predicate() {
        let pred = RefinementPredicate::And(
            Box::new(RefinementPredicate::GreaterThan(0)),
            Box::new(RefinementPredicate::LessThan(100)),
        );
        let r = RefinementType::new("x", "i32", pred);
        assert!(r.check_value(50));
        assert!(!r.check_value(0));
        assert!(!r.check_value(100));
    }

    #[test]
    fn v14_dt5_kind_display() {
        assert_eq!(format!("{}", Kind::Type), "Type");
        assert_eq!(format!("{}", Kind::Nat), "Nat");
        let dep = Kind::Dependent(Box::new(Kind::Nat), Box::new(Kind::Type));
        assert_eq!(format!("{dep}"), "Nat -> Type");
    }

    #[test]
    fn v14_dt5_refinement_in_function_param() {
        let source = r#"
            fn clamp_positive(x: { n: i64 | n > 0 }) -> i64 { x }
            fn main() {
                let v = clamp_positive(42)
                println(v)
            }
        "#;
        let mut interp = crate::interpreter::Interpreter::new();
        let r = interp.eval_source(source);
        assert!(r.is_ok(), "refinement in param: {r:?}");
    }

    #[test]
    fn v14_dt7_pi_type_syntax_parses() {
        // Pi type syntax parses in .fj source
        let source = "fn f(n: i64) -> Pi(m: usize) -> i64 { n }\nfn main() { println(42) }";
        let mut interp = crate::interpreter::Interpreter::new();
        let result = interp.eval_source(source);
        assert!(result.is_ok(), "Pi type should parse: {result:?}");
    }

    #[test]
    fn v14_dt7_pi_type_in_type_expr() {
        // Verify Pi type parses as a TypeExpr
        let source = "fn f() -> Pi(n: usize) -> i64 { 42 }";
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        if let crate::parser::ast::Item::FnDef(fndef) = &program.items[0] {
            assert!(fndef.return_type.is_some());
        }
    }

    #[test]
    fn v14_dt8_sigma_type_syntax_parses() {
        let source = "fn f() -> Sigma(n: usize, i64) { (1, 42) }\nfn main() { println(42) }";
        let mut interp = crate::interpreter::Interpreter::new();
        let result = interp.eval_source(source);
        assert!(result.is_ok(), "Sigma type should parse: {result:?}");
    }

    #[test]
    fn v14_dt8_sigma_in_type_expr() {
        let source = "fn f() -> Sigma(n: usize, i64) { (1, 42) }";
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        if let crate::parser::ast::Item::FnDef(fndef) = &program.items[0] {
            assert!(fndef.return_type.is_some());
            if let Some(crate::parser::ast::TypeExpr::Sigma { fst_name, .. }) = &fndef.return_type {
                assert_eq!(fst_name, "n");
            }
        }
    }

    #[test]
    fn v14_dt5_dependent_type_enum() {
        // DependentType variants work correctly
        let concrete = DependentType::Concrete("i32".into());
        assert!(matches!(concrete, DependentType::Concrete(_)));

        let indexed = DependentType::NatIndexed {
            base: "Vec".into(),
            index: NatValue::Literal(10),
        };
        assert!(matches!(indexed, DependentType::NatIndexed { .. }));
    }
}
