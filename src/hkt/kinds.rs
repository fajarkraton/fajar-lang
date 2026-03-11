//! Kind system — `* -> *` kinds, kind checking, kind inference,
//! kind unification, partial application, higher-rank kinds.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S1.1: Type Constructor Kinds
// ═══════════════════════════════════════════════════════════════════════

/// A kind in the type system.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Kind {
    /// `*` — the kind of concrete types (i32, String, etc.).
    Star,
    /// `k1 -> k2` — the kind of type constructors.
    Arrow(Box<Kind>, Box<Kind>),
    /// A kind variable for inference.
    Var(usize),
}

impl Kind {
    /// Creates `* -> *` (unary type constructor like Option, Vec).
    pub fn star_to_star() -> Self {
        Kind::Arrow(Box::new(Kind::Star), Box::new(Kind::Star))
    }

    /// Creates `* -> * -> *` (binary type constructor like Result, HashMap).
    pub fn star_star_to_star() -> Self {
        Kind::Arrow(
            Box::new(Kind::Star),
            Box::new(Kind::Arrow(Box::new(Kind::Star), Box::new(Kind::Star))),
        )
    }

    /// Creates `(* -> *) -> *` (higher-rank kind).
    pub fn higher_rank() -> Self {
        Kind::Arrow(
            Box::new(Kind::Arrow(Box::new(Kind::Star), Box::new(Kind::Star))),
            Box::new(Kind::Star),
        )
    }

    /// Returns the arity of this kind (number of arrows).
    pub fn arity(&self) -> usize {
        match self {
            Kind::Star | Kind::Var(_) => 0,
            Kind::Arrow(_, result) => 1 + result.arity(),
        }
    }

    /// Whether this is a concrete type kind (`*`).
    pub fn is_star(&self) -> bool {
        matches!(self, Kind::Star)
    }

    /// Whether this is a type constructor kind (`* -> ...`).
    pub fn is_constructor(&self) -> bool {
        matches!(self, Kind::Arrow(_, _))
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Star => write!(f, "*"),
            Kind::Arrow(from, to) => {
                if from.is_constructor() {
                    write!(f, "({from}) -> {to}")
                } else {
                    write!(f, "{from} -> {to}")
                }
            }
            Kind::Var(id) => write!(f, "?k{id}"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.2: Kind Checker
// ═══════════════════════════════════════════════════════════════════════

/// Kind checking error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KindError {
    /// Kind mismatch.
    Mismatch {
        expected: Kind,
        found: Kind,
        type_name: String,
    },
    /// Unknown type constructor.
    UnknownConstructor { name: String },
    /// Wrong number of type arguments.
    ArityMismatch {
        constructor: String,
        expected: usize,
        found: usize,
    },
    /// Infinite kind.
    InfiniteKind { var: usize },
}

impl fmt::Display for KindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KindError::Mismatch {
                expected,
                found,
                type_name,
            } => write!(
                f,
                "kind mismatch for `{type_name}`: expected `{expected}`, found `{found}`"
            ),
            KindError::UnknownConstructor { name } => {
                write!(f, "unknown type constructor `{name}`")
            }
            KindError::ArityMismatch {
                constructor,
                expected,
                found,
            } => write!(
                f,
                "`{constructor}` expects {expected} type argument(s), found {found}"
            ),
            KindError::InfiniteKind { var } => {
                write!(f, "infinite kind detected for kind variable ?k{var}")
            }
        }
    }
}

/// Kind environment mapping type names to their kinds.
#[derive(Debug, Clone, Default)]
pub struct KindEnv {
    /// Known type constructors and their kinds.
    constructors: HashMap<String, Kind>,
}

impl KindEnv {
    /// Creates a new kind environment with built-in types.
    pub fn new() -> Self {
        let mut env = Self::default();
        // Concrete types: kind *
        for ty in &[
            "i32", "i64", "f32", "f64", "bool", "char", "str", "String", "void", "never",
        ] {
            env.register(ty, Kind::Star);
        }
        // Unary constructors: kind * -> *
        for ty in &["Option", "Vec", "Box", "Future"] {
            env.register(ty, Kind::star_to_star());
        }
        // Binary constructors: kind * -> * -> *
        for ty in &["Result", "HashMap", "Either"] {
            env.register(ty, Kind::star_star_to_star());
        }
        env
    }

    /// Registers a type with its kind.
    pub fn register(&mut self, name: &str, kind: Kind) {
        self.constructors.insert(name.into(), kind);
    }

    /// Looks up the kind of a type.
    pub fn lookup(&self, name: &str) -> Option<&Kind> {
        self.constructors.get(name)
    }

    /// Returns the number of registered types.
    pub fn len(&self) -> usize {
        self.constructors.len()
    }

    /// Whether the environment is empty.
    pub fn is_empty(&self) -> bool {
        self.constructors.is_empty()
    }
}

/// Checks the kind of a type expression.
pub fn check_kind(env: &KindEnv, type_name: &str, args: &[&str]) -> Result<Kind, KindError> {
    let base_kind = env
        .lookup(type_name)
        .ok_or_else(|| KindError::UnknownConstructor {
            name: type_name.into(),
        })?
        .clone();

    let expected_arity = base_kind.arity();
    let actual_arity = args.len();

    if actual_arity > expected_arity {
        return Err(KindError::ArityMismatch {
            constructor: type_name.into(),
            expected: expected_arity,
            found: actual_arity,
        });
    }

    if actual_arity == expected_arity {
        // Fully applied — result is *
        Ok(Kind::Star)
    } else {
        // Partially applied — strip applied args from kind
        let mut result = base_kind;
        for _ in 0..actual_arity {
            if let Kind::Arrow(_, to) = result {
                result = *to;
            }
        }
        Ok(result)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.3-S1.4: Type Constructor Parameters & Kind Annotations
// ═══════════════════════════════════════════════════════════════════════

/// A type parameter with optional kind annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParam {
    /// Parameter name.
    pub name: String,
    /// Kind annotation (None = infer).
    pub kind: Option<Kind>,
}

impl fmt::Display for TypeParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            Some(kind) => write!(f, "{}: {}", self.name, kind),
            None => write!(f, "{}", self.name),
        }
    }
}

/// A trait definition with HKT parameters.
#[derive(Debug, Clone)]
pub struct HktTraitDef {
    /// Trait name.
    pub name: String,
    /// Type parameters with kind annotations.
    pub params: Vec<TypeParam>,
    /// Method signatures.
    pub methods: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// S1.5: Partial Application
// ═══════════════════════════════════════════════════════════════════════

/// A partially applied type constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialApp {
    /// Base constructor name.
    pub constructor: String,
    /// Applied arguments.
    pub applied_args: Vec<String>,
    /// Remaining kind after partial application.
    pub remaining_kind: Kind,
}

/// Creates a partial application of a type constructor.
pub fn partial_apply(
    env: &KindEnv,
    constructor: &str,
    args: &[&str],
) -> Result<PartialApp, KindError> {
    let remaining = check_kind(env, constructor, args)?;
    Ok(PartialApp {
        constructor: constructor.into(),
        applied_args: args.iter().map(|s| s.to_string()).collect(),
        remaining_kind: remaining,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S1.6: Kind Unification
// ═══════════════════════════════════════════════════════════════════════

/// Kind substitution mapping kind variables to kinds.
#[derive(Debug, Clone, Default)]
pub struct KindSubst {
    mappings: HashMap<usize, Kind>,
    next_var: usize,
}

impl KindSubst {
    /// Creates an empty substitution.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a fresh kind variable.
    pub fn fresh_var(&mut self) -> Kind {
        let id = self.next_var;
        self.next_var += 1;
        Kind::Var(id)
    }

    /// Applies the substitution to a kind.
    pub fn apply(&self, kind: &Kind) -> Kind {
        match kind {
            Kind::Star => Kind::Star,
            Kind::Var(id) => {
                if let Some(resolved) = self.mappings.get(id) {
                    self.apply(resolved)
                } else {
                    Kind::Var(*id)
                }
            }
            Kind::Arrow(from, to) => {
                Kind::Arrow(Box::new(self.apply(from)), Box::new(self.apply(to)))
            }
        }
    }

    /// Unifies two kinds, updating the substitution.
    pub fn unify(&mut self, k1: &Kind, k2: &Kind) -> Result<(), KindError> {
        let k1 = self.apply(k1);
        let k2 = self.apply(k2);

        match (&k1, &k2) {
            (Kind::Star, Kind::Star) => Ok(()),
            (Kind::Var(id), _) => {
                if k2 == Kind::Var(*id) {
                    Ok(())
                } else if self.occurs(*id, &k2) {
                    Err(KindError::InfiniteKind { var: *id })
                } else {
                    self.mappings.insert(*id, k2);
                    Ok(())
                }
            }
            (_, Kind::Var(id)) => {
                if self.occurs(*id, &k1) {
                    Err(KindError::InfiniteKind { var: *id })
                } else {
                    self.mappings.insert(*id, k1);
                    Ok(())
                }
            }
            (Kind::Arrow(f1, t1), Kind::Arrow(f2, t2)) => {
                self.unify(f1, f2)?;
                self.unify(t1, t2)
            }
            _ => Err(KindError::Mismatch {
                expected: k1.clone(),
                found: k2.clone(),
                type_name: String::new(),
            }),
        }
    }

    fn occurs(&self, var: usize, kind: &Kind) -> bool {
        match kind {
            Kind::Star => false,
            Kind::Var(id) => {
                if *id == var {
                    true
                } else if let Some(resolved) = self.mappings.get(id) {
                    self.occurs(var, resolved)
                } else {
                    false
                }
            }
            Kind::Arrow(from, to) => self.occurs(var, from) || self.occurs(var, to),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.7: Kind Error Messages
// ═══════════════════════════════════════════════════════════════════════

/// Formats a kind error with context for IDE display.
pub fn format_kind_error(err: &KindError) -> String {
    match err {
        KindError::Mismatch {
            expected,
            found,
            type_name,
        } => {
            let hint = if expected.is_star() && found.is_constructor() {
                format!(
                    "\n  hint: `{type_name}` is a type constructor (kind `{found}`), not a concrete type"
                )
            } else if expected.is_constructor() && found.is_star() {
                format!(
                    "\n  hint: expected a type constructor (kind `{expected}`), but `{type_name}` is a concrete type"
                )
            } else {
                String::new()
            };
            format!("error: {err}{hint}")
        }
        _ => format!("error: {err}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.8-S1.9: Higher-Rank Kinds & Kind Defaults
// ═══════════════════════════════════════════════════════════════════════

/// Infers the kind of a type parameter from its usage.
pub fn infer_kind_from_usage(usages: &[Kind]) -> Kind {
    if usages.is_empty() {
        // Default kind is *
        Kind::Star
    } else {
        // Use the most specific kind found
        usages
            .iter()
            .find(|k| k.is_constructor())
            .cloned()
            .unwrap_or(Kind::Star)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S1.1 — Type Constructor Kinds
    #[test]
    fn s1_1_star_kind() {
        let k = Kind::Star;
        assert!(k.is_star());
        assert!(!k.is_constructor());
        assert_eq!(k.arity(), 0);
        assert_eq!(k.to_string(), "*");
    }

    #[test]
    fn s1_1_arrow_kind() {
        let k = Kind::star_to_star();
        assert!(k.is_constructor());
        assert_eq!(k.arity(), 1);
        assert_eq!(k.to_string(), "* -> *");
    }

    #[test]
    fn s1_1_binary_kind() {
        let k = Kind::star_star_to_star();
        assert_eq!(k.arity(), 2);
        assert_eq!(k.to_string(), "* -> * -> *");
    }

    // S1.2 — Kind Checker
    #[test]
    fn s1_2_check_concrete_type() {
        let env = KindEnv::new();
        let result = check_kind(&env, "i32", &[]);
        assert_eq!(result.unwrap(), Kind::Star);
    }

    #[test]
    fn s1_2_check_applied_constructor() {
        let env = KindEnv::new();
        let result = check_kind(&env, "Option", &["i32"]);
        assert_eq!(result.unwrap(), Kind::Star);
    }

    #[test]
    fn s1_2_check_wrong_arity() {
        let env = KindEnv::new();
        let result = check_kind(&env, "Option", &["i32", "String"]);
        assert!(matches!(result, Err(KindError::ArityMismatch { .. })));
    }

    #[test]
    fn s1_2_check_unknown() {
        let env = KindEnv::new();
        let result = check_kind(&env, "Nonexistent", &[]);
        assert!(matches!(result, Err(KindError::UnknownConstructor { .. })));
    }

    // S1.3-S1.4 — Type Constructor Params & Kind Annotations
    #[test]
    fn s1_3_type_param_display() {
        let p = TypeParam {
            name: "F".into(),
            kind: Some(Kind::star_to_star()),
        };
        assert_eq!(p.to_string(), "F: * -> *");
    }

    #[test]
    fn s1_4_kind_annotation_none() {
        let p = TypeParam {
            name: "T".into(),
            kind: None,
        };
        assert_eq!(p.to_string(), "T");
    }

    // S1.5 — Partial Application
    #[test]
    fn s1_5_partial_apply_result() {
        let env = KindEnv::new();
        let pa = partial_apply(&env, "Result", &["Error"]).unwrap();
        assert_eq!(pa.remaining_kind, Kind::star_to_star());
        assert_eq!(pa.applied_args.len(), 1);
    }

    #[test]
    fn s1_5_full_application() {
        let env = KindEnv::new();
        let pa = partial_apply(&env, "Result", &["i32", "Error"]).unwrap();
        assert_eq!(pa.remaining_kind, Kind::Star);
    }

    // S1.6 — Kind Unification
    #[test]
    fn s1_6_unify_star_star() {
        let mut subst = KindSubst::new();
        assert!(subst.unify(&Kind::Star, &Kind::Star).is_ok());
    }

    #[test]
    fn s1_6_unify_var_with_star() {
        let mut subst = KindSubst::new();
        let var = subst.fresh_var();
        assert!(subst.unify(&var, &Kind::Star).is_ok());
        assert_eq!(subst.apply(&var), Kind::Star);
    }

    #[test]
    fn s1_6_unify_arrow() {
        let mut subst = KindSubst::new();
        let k1 = Kind::star_to_star();
        let k2 = Kind::star_to_star();
        assert!(subst.unify(&k1, &k2).is_ok());
    }

    #[test]
    fn s1_6_unify_mismatch() {
        let mut subst = KindSubst::new();
        let result = subst.unify(&Kind::Star, &Kind::star_to_star());
        assert!(result.is_err());
    }

    // S1.7 — Kind Error Messages
    #[test]
    fn s1_7_error_message_constructor_as_type() {
        let err = KindError::Mismatch {
            expected: Kind::Star,
            found: Kind::star_to_star(),
            type_name: "Option".into(),
        };
        let msg = format_kind_error(&err);
        assert!(msg.contains("type constructor"));
        assert!(msg.contains("hint"));
    }

    // S1.8 — Higher-Rank Kinds
    #[test]
    fn s1_8_higher_rank_kind() {
        let k = Kind::higher_rank();
        assert_eq!(k.to_string(), "(* -> *) -> *");
        assert_eq!(k.arity(), 1);
    }

    // S1.9 — Kind Defaults
    #[test]
    fn s1_9_default_kind() {
        let inferred = infer_kind_from_usage(&[]);
        assert_eq!(inferred, Kind::Star);
    }

    #[test]
    fn s1_9_infer_constructor_kind() {
        let inferred = infer_kind_from_usage(&[Kind::Star, Kind::star_to_star()]);
        assert_eq!(inferred, Kind::star_to_star());
    }

    // S1.10 — Integration
    #[test]
    fn s1_10_kind_env_builtins() {
        let env = KindEnv::new();
        assert!(env.len() >= 10);
        assert_eq!(env.lookup("Option").unwrap(), &Kind::star_to_star());
        assert_eq!(env.lookup("Result").unwrap(), &Kind::star_star_to_star());
        assert_eq!(env.lookup("i32").unwrap(), &Kind::Star);
    }

    #[test]
    fn s1_10_hkt_trait_def() {
        let def = HktTraitDef {
            name: "Functor".into(),
            params: vec![TypeParam {
                name: "F".into(),
                kind: Some(Kind::star_to_star()),
            }],
            methods: vec!["fmap".into()],
        };
        assert_eq!(def.name, "Functor");
        assert_eq!(def.params[0].kind, Some(Kind::star_to_star()));
    }
}
