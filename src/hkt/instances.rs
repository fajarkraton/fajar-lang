//! Functor / Applicative / Monad instances — type class hierarchy
//! for Option, Result, Vec, plus Foldable and Traversable.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S2.1: Functor Trait
// ═══════════════════════════════════════════════════════════════════════

/// A Functor type class — types that can be mapped over.
///
/// Laws:
/// - Identity: `fmap(id, fa) == fa`
/// - Composition: `fmap(f . g, fa) == fmap(f, fmap(g, fa))`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctorDef {
    /// Type constructor name (e.g., "Option", "Result", "Vec").
    pub type_constructor: String,
    /// The `fmap` method signature.
    pub fmap_signature: String,
}

/// Represents a Functor instance declaration.
#[derive(Debug, Clone)]
pub struct FunctorInstance {
    /// The type constructor.
    pub for_type: String,
    /// Implementation status.
    pub implemented: bool,
}

/// Simulates fmap over an Option-like value.
pub fn fmap_option(value: Option<i64>, f: fn(i64) -> i64) -> Option<i64> {
    value.map(f)
}

/// Simulates fmap over a Vec-like value.
pub fn fmap_vec(values: Vec<i64>, f: fn(i64) -> i64) -> Vec<i64> {
    values.into_iter().map(f).collect()
}

/// Simulates fmap over a Result-like value.
pub fn fmap_result(value: Result<i64, String>, f: fn(i64) -> i64) -> Result<i64, String> {
    value.map(f)
}

// ═══════════════════════════════════════════════════════════════════════
// S2.2: Applicative Trait
// ═══════════════════════════════════════════════════════════════════════

/// An Applicative type class — Functor with `pure` and `ap`.
///
/// Laws:
/// - Identity: `ap(pure(id), v) == v`
/// - Homomorphism: `ap(pure(f), pure(x)) == pure(f(x))`
#[derive(Debug, Clone)]
pub struct ApplicativeDef {
    /// Type constructor name.
    pub type_constructor: String,
    /// Extends Functor.
    pub extends_functor: bool,
}

/// Wraps a value in Option (pure).
pub fn pure_option(value: i64) -> Option<i64> {
    Some(value)
}

/// Wraps a value in Vec (pure).
pub fn pure_vec(value: i64) -> Vec<i64> {
    vec![value]
}

/// Wraps a value in Ok (pure for Result).
pub fn pure_result(value: i64) -> Result<i64, String> {
    Ok(value)
}

/// Applies a wrapped function to a wrapped value (ap for Option).
pub fn ap_option(ff: Option<fn(i64) -> i64>, fa: Option<i64>) -> Option<i64> {
    match (ff, fa) {
        (Some(f), Some(a)) => Some(f(a)),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S2.3: Monad Trait
// ═══════════════════════════════════════════════════════════════════════

/// A Monad type class — Applicative with `bind` (>>=).
///
/// Laws:
/// - Left identity: `bind(pure(a), f) == f(a)`
/// - Right identity: `bind(m, pure) == m`
/// - Associativity: `bind(bind(m, f), g) == bind(m, |x| bind(f(x), g))`
#[derive(Debug, Clone)]
pub struct MonadDef {
    /// Type constructor name.
    pub type_constructor: String,
    /// Extends Applicative.
    pub extends_applicative: bool,
}

/// Monadic bind for Option.
pub fn bind_option(value: Option<i64>, f: fn(i64) -> Option<i64>) -> Option<i64> {
    value.and_then(f)
}

/// Monadic bind for Result.
pub fn bind_result(
    value: Result<i64, String>,
    f: fn(i64) -> Result<i64, String>,
) -> Result<i64, String> {
    value.and_then(f)
}

/// Monadic bind for Vec (flatMap).
pub fn bind_vec(values: Vec<i64>, f: fn(i64) -> Vec<i64>) -> Vec<i64> {
    values.into_iter().flat_map(f).collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S2.4-S2.6: Instance Verification
// ═══════════════════════════════════════════════════════════════════════

/// Verifies monad laws for a given instance.
#[derive(Debug, Clone)]
pub struct MonadLawCheck {
    /// Instance type.
    pub instance: String,
    /// Left identity passes.
    pub left_identity: bool,
    /// Right identity passes.
    pub right_identity: bool,
    /// Associativity passes.
    pub associativity: bool,
}

impl MonadLawCheck {
    /// Whether all laws pass.
    pub fn all_pass(&self) -> bool {
        self.left_identity && self.right_identity && self.associativity
    }
}

impl fmt::Display for MonadLawCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Monad laws for {}: left_id={}, right_id={}, assoc={}",
            self.instance,
            if self.left_identity { "PASS" } else { "FAIL" },
            if self.right_identity { "PASS" } else { "FAIL" },
            if self.associativity { "PASS" } else { "FAIL" },
        )
    }
}

fn opt_double(x: i64) -> Option<i64> {
    Some(x * 2)
}
fn opt_inc(x: i64) -> Option<i64> {
    Some(x + 1)
}
fn opt_double_then_inc(x: i64) -> Option<i64> {
    bind_option(opt_double(x), opt_inc)
}

/// Checks monad laws for Option.
pub fn check_option_monad_laws() -> MonadLawCheck {
    let a = 42_i64;

    // Left identity: bind(pure(a), f) == f(a)
    let left_id = bind_option(pure_option(a), opt_double) == opt_double(a);

    // Right identity: bind(m, pure) == m
    let m = Some(a);
    let right_id = bind_option(m, pure_option) == m;

    // Associativity: bind(bind(m, f), g) == bind(m, |x| bind(f(x), g))
    let assoc_left = bind_option(bind_option(m, opt_double), opt_inc);
    let assoc_right = bind_option(m, opt_double_then_inc);
    let assoc = assoc_left == assoc_right;

    MonadLawCheck {
        instance: "Option".into(),
        left_identity: left_id,
        right_identity: right_id,
        associativity: assoc,
    }
}

fn res_double(x: i64) -> Result<i64, String> {
    Ok(x * 2)
}
fn res_inc(x: i64) -> Result<i64, String> {
    Ok(x + 1)
}
fn res_double_then_inc(x: i64) -> Result<i64, String> {
    bind_result(res_double(x), res_inc)
}

/// Checks monad laws for Result.
pub fn check_result_monad_laws() -> MonadLawCheck {
    let a = 42_i64;

    let left_id = bind_result(pure_result(a), res_double) == res_double(a);

    let m: Result<i64, String> = Ok(a);
    let right_id = bind_result(m.clone(), pure_result) == m;

    let assoc_left = bind_result(bind_result(m.clone(), res_double), res_inc);
    let assoc_right = bind_result(m, res_double_then_inc);
    let assoc = assoc_left == assoc_right;

    MonadLawCheck {
        instance: "Result".into(),
        left_identity: left_id,
        right_identity: right_id,
        associativity: assoc,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S2.8: Foldable Trait
// ═══════════════════════════════════════════════════════════════════════

/// Foldable type class — types that can be folded to a summary value.
#[derive(Debug, Clone)]
pub struct FoldableDef {
    /// Type constructor name.
    pub type_constructor: String,
}

/// Folds a Vec with an accumulator.
pub fn fold_vec(values: &[i64], init: i64, f: fn(i64, i64) -> i64) -> i64 {
    values.iter().fold(init, |acc, &x| f(acc, x))
}

/// Folds an Option with an accumulator.
pub fn fold_option(value: Option<i64>, init: i64, f: fn(i64, i64) -> i64) -> i64 {
    match value {
        Some(x) => f(init, x),
        None => init,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S2.9: Traversable Trait
// ═══════════════════════════════════════════════════════════════════════

/// Traversable type class — Foldable + Functor with traverse.
#[derive(Debug, Clone)]
pub struct TraversableDef {
    /// Type constructor name.
    pub type_constructor: String,
    /// Extends both Foldable and Functor.
    pub extends: Vec<String>,
}

/// Traverse a Vec with a function that may fail (Option).
pub fn traverse_vec_option(values: &[i64], f: fn(i64) -> Option<i64>) -> Option<Vec<i64>> {
    let mut result = Vec::with_capacity(values.len());
    for &v in values {
        match f(v) {
            Some(x) => result.push(x),
            None => return None,
        }
    }
    Some(result)
}

/// Traverse a Vec with a function that may fail (Result).
pub fn traverse_vec_result(
    values: &[i64],
    f: fn(i64) -> Result<i64, String>,
) -> Result<Vec<i64>, String> {
    let mut result = Vec::with_capacity(values.len());
    for &v in values {
        match f(v) {
            Ok(x) => result.push(x),
            Err(e) => return Err(e),
        }
    }
    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════════
// Type class registry
// ═══════════════════════════════════════════════════════════════════════

/// Registry of type class instances.
#[derive(Debug, Clone, Default)]
pub struct TypeClassRegistry {
    /// Functor instances.
    pub functors: Vec<FunctorInstance>,
    /// Monad law checks.
    pub monad_checks: Vec<MonadLawCheck>,
}

impl TypeClassRegistry {
    /// Creates a registry with built-in instances.
    pub fn with_builtins() -> Self {
        let mut registry = Self::default();
        for ty in &["Option", "Result", "Vec"] {
            registry.functors.push(FunctorInstance {
                for_type: ty.to_string(),
                implemented: true,
            });
        }
        registry.monad_checks.push(check_option_monad_laws());
        registry.monad_checks.push(check_result_monad_laws());
        registry
    }

    /// Returns all instances that pass all monad laws.
    pub fn valid_monads(&self) -> Vec<&MonadLawCheck> {
        self.monad_checks.iter().filter(|c| c.all_pass()).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S2.1 — Functor
    #[test]
    fn s2_1_fmap_option_some() {
        assert_eq!(fmap_option(Some(5), |x| x * 2), Some(10));
    }

    #[test]
    fn s2_1_fmap_option_none() {
        assert_eq!(fmap_option(None, |x| x * 2), None);
    }

    #[test]
    fn s2_1_fmap_vec() {
        assert_eq!(fmap_vec(vec![1, 2, 3], |x| x + 10), vec![11, 12, 13]);
    }

    #[test]
    fn s2_1_fmap_result() {
        assert_eq!(fmap_result(Ok(5), |x| x * 3), Ok(15));
        assert_eq!(fmap_result(Err("err".into()), |x| x * 3), Err("err".into()));
    }

    // S2.2 — Applicative
    #[test]
    fn s2_2_pure_option() {
        assert_eq!(pure_option(42), Some(42));
    }

    #[test]
    fn s2_2_ap_option() {
        fn double(x: i64) -> i64 {
            x * 2
        }
        assert_eq!(ap_option(Some(double as fn(i64) -> i64), Some(5)), Some(10));
        assert_eq!(ap_option(None, Some(5)), None);
    }

    // S2.3 — Monad
    #[test]
    fn s2_3_bind_option() {
        let result = bind_option(Some(10), |x| if x > 5 { Some(x * 2) } else { None });
        assert_eq!(result, Some(20));
    }

    #[test]
    fn s2_3_bind_option_none() {
        let result = bind_option(None, |x| Some(x * 2));
        assert_eq!(result, None);
    }

    #[test]
    fn s2_3_bind_vec() {
        let result = bind_vec(vec![1, 2, 3], |x| vec![x, x * 10]);
        assert_eq!(result, vec![1, 10, 2, 20, 3, 30]);
    }

    #[test]
    fn s2_3_bind_result() {
        let result = bind_result(Ok(5), |x| Ok(x + 1));
        assert_eq!(result, Ok(6));
    }

    // S2.7 — Monad Laws
    #[test]
    fn s2_7_option_monad_laws() {
        let check = check_option_monad_laws();
        assert!(check.all_pass(), "{}", check);
    }

    #[test]
    fn s2_7_result_monad_laws() {
        let check = check_result_monad_laws();
        assert!(check.all_pass(), "{}", check);
    }

    // S2.8 — Foldable
    #[test]
    fn s2_8_fold_vec() {
        assert_eq!(fold_vec(&[1, 2, 3, 4], 0, |a, b| a + b), 10);
    }

    #[test]
    fn s2_8_fold_option() {
        assert_eq!(fold_option(Some(5), 10, |a, b| a + b), 15);
        assert_eq!(fold_option(None, 10, |a, b| a + b), 10);
    }

    // S2.9 — Traversable
    #[test]
    fn s2_9_traverse_vec_option_all_some() {
        let result = traverse_vec_option(&[1, 2, 3], |x| Some(x * 2));
        assert_eq!(result, Some(vec![2, 4, 6]));
    }

    #[test]
    fn s2_9_traverse_vec_option_one_none() {
        let result = traverse_vec_option(&[1, 2, 3], |x| if x == 2 { None } else { Some(x) });
        assert_eq!(result, None);
    }

    #[test]
    fn s2_9_traverse_vec_result() {
        let result = traverse_vec_result(&[1, 2, 3], |x| Ok(x + 10));
        assert_eq!(result, Ok(vec![11, 12, 13]));
    }

    // S2.10 — Registry
    #[test]
    fn s2_10_registry_builtins() {
        let reg = TypeClassRegistry::with_builtins();
        assert_eq!(reg.functors.len(), 3);
        assert_eq!(reg.valid_monads().len(), 2);
    }
}
