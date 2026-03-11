//! Type-level programming — type functions, associated type families,
//! type-level booleans/lists, phantom types, type witnesses.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S4.1: Type-Level Functions
// ═══════════════════════════════════════════════════════════════════════

/// A type-level function (type alias with parameters).
#[derive(Debug, Clone)]
pub struct TypeFunction {
    /// Name of the type function.
    pub name: String,
    /// Type parameters.
    pub params: Vec<String>,
    /// Body — the computed type.
    pub body: TypeExpr,
}

/// A type-level expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpr {
    /// A named type or type parameter.
    Named(String),
    /// Type application: `F<A, B>`.
    App(String, Vec<TypeExpr>),
    /// Conditional: `if C then T else F`.
    Cond {
        condition: Box<TypeExpr>,
        then_type: Box<TypeExpr>,
        else_type: Box<TypeExpr>,
    },
    /// Tuple type.
    Tuple(Vec<TypeExpr>),
    /// Never type (bottom).
    Never,
}

impl fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeExpr::Named(n) => write!(f, "{n}"),
            TypeExpr::App(name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                write!(f, "{}<{}>", name, args_str.join(", "))
            }
            TypeExpr::Cond {
                condition,
                then_type,
                else_type,
            } => write!(f, "if {condition} then {then_type} else {else_type}"),
            TypeExpr::Tuple(elems) => {
                let inner: Vec<String> = elems.iter().map(|e| e.to_string()).collect();
                write!(f, "({})", inner.join(", "))
            }
            TypeExpr::Never => write!(f, "!"),
        }
    }
}

/// Evaluates a type function with given arguments.
pub fn eval_type_function(
    func: &TypeFunction,
    args: &[TypeExpr],
    env: &HashMap<String, TypeExpr>,
) -> Result<TypeExpr, String> {
    if args.len() != func.params.len() {
        return Err(format!(
            "type function `{}` expects {} args, got {}",
            func.name,
            func.params.len(),
            args.len()
        ));
    }

    let mut local_env = env.clone();
    for (param, arg) in func.params.iter().zip(args) {
        local_env.insert(param.clone(), arg.clone());
    }

    substitute(&func.body, &local_env)
}

fn substitute(expr: &TypeExpr, env: &HashMap<String, TypeExpr>) -> Result<TypeExpr, String> {
    match expr {
        TypeExpr::Named(name) => {
            if let Some(resolved) = env.get(name) {
                Ok(resolved.clone())
            } else {
                Ok(expr.clone())
            }
        }
        TypeExpr::App(name, args) => {
            let resolved_args: Result<Vec<TypeExpr>, String> =
                args.iter().map(|a| substitute(a, env)).collect();
            Ok(TypeExpr::App(name.clone(), resolved_args?))
        }
        TypeExpr::Cond {
            condition,
            then_type,
            else_type,
        } => {
            let cond = substitute(condition, env)?;
            match &cond {
                TypeExpr::Named(n) if n == "True" => substitute(then_type, env),
                TypeExpr::Named(n) if n == "False" => substitute(else_type, env),
                _ => Ok(TypeExpr::Cond {
                    condition: Box::new(cond),
                    then_type: Box::new(substitute(then_type, env)?),
                    else_type: Box::new(substitute(else_type, env)?),
                }),
            }
        }
        TypeExpr::Tuple(elems) => {
            let resolved: Result<Vec<TypeExpr>, String> =
                elems.iter().map(|e| substitute(e, env)).collect();
            Ok(TypeExpr::Tuple(resolved?))
        }
        TypeExpr::Never => Ok(TypeExpr::Never),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.2: Associated Type Families
// ═══════════════════════════════════════════════════════════════════════

/// An associated type in a trait.
#[derive(Debug, Clone)]
pub struct AssociatedType {
    /// Name of the associated type.
    pub name: String,
    /// Bounds on the associated type.
    pub bounds: Vec<String>,
    /// Default type (if any).
    pub default: Option<TypeExpr>,
}

/// A type family definition in a trait.
#[derive(Debug, Clone)]
pub struct TypeFamily {
    /// Trait name.
    pub trait_name: String,
    /// Associated types.
    pub assoc_types: Vec<AssociatedType>,
}

/// An instance of an associated type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssocTypeInstance {
    /// Trait name.
    pub trait_name: String,
    /// Implementing type.
    pub impl_type: String,
    /// Associated type name.
    pub assoc_name: String,
    /// Concrete type.
    pub concrete_type: TypeExpr,
}

// ═══════════════════════════════════════════════════════════════════════
// S4.3: Type-Level Booleans
// ═══════════════════════════════════════════════════════════════════════

/// Type-level boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TBool {
    /// Type-level True.
    True,
    /// Type-level False.
    False,
}

impl TBool {
    /// Type-level AND.
    pub fn and(self, other: TBool) -> TBool {
        match (self, other) {
            (TBool::True, TBool::True) => TBool::True,
            _ => TBool::False,
        }
    }

    /// Type-level OR.
    pub fn or(self, other: TBool) -> TBool {
        match (self, other) {
            (TBool::False, TBool::False) => TBool::False,
            _ => TBool::True,
        }
    }

    /// Type-level NOT.
    #[allow(clippy::should_implement_trait)]
    pub fn not(self) -> TBool {
        match self {
            TBool::True => TBool::False,
            TBool::False => TBool::True,
        }
    }
}

impl fmt::Display for TBool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TBool::True => write!(f, "True"),
            TBool::False => write!(f, "False"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.4: Type-Level Lists (HList)
// ═══════════════════════════════════════════════════════════════════════

/// A heterogeneous list at the type level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HList {
    /// Empty list.
    Nil,
    /// Cons cell: head type + tail list.
    Cons(String, Box<HList>),
}

impl HList {
    /// Creates an HList from type names.
    pub fn from_types(types: &[&str]) -> Self {
        let mut list = HList::Nil;
        for ty in types.iter().rev() {
            list = HList::Cons(ty.to_string(), Box::new(list));
        }
        list
    }

    /// Returns the length.
    pub fn len(&self) -> usize {
        match self {
            HList::Nil => 0,
            HList::Cons(_, tail) => 1 + tail.len(),
        }
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        matches!(self, HList::Nil)
    }

    /// Returns the head type.
    pub fn head(&self) -> Option<&str> {
        match self {
            HList::Nil => None,
            HList::Cons(h, _) => Some(h),
        }
    }
}

impl fmt::Display for HList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HList::Nil => write!(f, "HNil"),
            HList::Cons(head, tail) => write!(f, "HCons<{head}, {tail}>"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.5: Compile-Time Type Selection
// ═══════════════════════════════════════════════════════════════════════

/// Selects a type at compile time based on a boolean condition.
pub fn type_select(cond: TBool, then_type: &str, else_type: &str) -> String {
    match cond {
        TBool::True => then_type.into(),
        TBool::False => else_type.into(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.6: Phantom Types
// ═══════════════════════════════════════════════════════════════════════

/// A phantom type tag — zero-cost type-level marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhantomType {
    /// The phantom type name.
    pub name: String,
    /// State types that this phantom can take.
    pub states: Vec<String>,
}

/// A state machine defined with phantom types.
#[derive(Debug, Clone)]
pub struct PhantomStateMachine {
    /// State machine name.
    pub name: String,
    /// Phantom type parameter name.
    pub state_param: String,
    /// Valid transitions: (from_state, to_state, method_name).
    pub transitions: Vec<(String, String, String)>,
}

impl PhantomStateMachine {
    /// Checks if a transition is valid.
    pub fn is_valid_transition(&self, from: &str, to: &str) -> bool {
        self.transitions
            .iter()
            .any(|(f, t, _)| f == from && t == to)
    }

    /// Returns the method that performs a transition.
    pub fn transition_method(&self, from: &str, to: &str) -> Option<&str> {
        self.transitions
            .iter()
            .find(|(f, t, _)| f == from && t == to)
            .map(|(_, _, m)| m.as_str())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.7: Type Witnesses
// ═══════════════════════════════════════════════════════════════════════

/// A type equality witness: proof that A == B.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeEq {
    /// Left type.
    pub left: String,
    /// Right type.
    pub right: String,
}

impl TypeEq {
    /// Creates a reflexive witness: A == A.
    pub fn refl(ty: &str) -> Self {
        Self {
            left: ty.into(),
            right: ty.into(),
        }
    }

    /// Whether this is a valid (reflexive) witness.
    pub fn is_valid(&self) -> bool {
        self.left == self.right
    }

    /// Symmetric: if A == B then B == A.
    pub fn sym(&self) -> Self {
        Self {
            left: self.right.clone(),
            right: self.left.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.9: Type-Level Computation Limits
// ═══════════════════════════════════════════════════════════════════════

/// Default recursion limit for type-level computation.
pub const TYPE_LEVEL_RECURSION_LIMIT: usize = 256;

/// Configuration for type-level computation.
#[derive(Debug, Clone)]
pub struct TypeLevelConfig {
    /// Maximum recursion depth.
    pub recursion_limit: usize,
    /// Whether to cache intermediate results.
    pub cache_enabled: bool,
}

impl Default for TypeLevelConfig {
    fn default() -> Self {
        Self {
            recursion_limit: TYPE_LEVEL_RECURSION_LIMIT,
            cache_enabled: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S4.1 — Type-Level Functions
    #[test]
    fn s4_1_type_function_eval() {
        let func = TypeFunction {
            name: "Wrap".into(),
            params: vec!["T".into()],
            body: TypeExpr::App("Option".into(), vec![TypeExpr::Named("T".into())]),
        };
        let result = eval_type_function(&func, &[TypeExpr::Named("i32".into())], &HashMap::new());
        assert_eq!(
            result.unwrap(),
            TypeExpr::App("Option".into(), vec![TypeExpr::Named("i32".into())])
        );
    }

    #[test]
    fn s4_1_type_function_wrong_arity() {
        let func = TypeFunction {
            name: "Pair".into(),
            params: vec!["A".into(), "B".into()],
            body: TypeExpr::Tuple(vec![
                TypeExpr::Named("A".into()),
                TypeExpr::Named("B".into()),
            ]),
        };
        let result = eval_type_function(&func, &[TypeExpr::Named("i32".into())], &HashMap::new());
        assert!(result.is_err());
    }

    // S4.2 — Associated Type Families
    #[test]
    fn s4_2_type_family() {
        let family = TypeFamily {
            trait_name: "Collection".into(),
            assoc_types: vec![
                AssociatedType {
                    name: "Elem".into(),
                    bounds: vec![],
                    default: None,
                },
                AssociatedType {
                    name: "Iter".into(),
                    bounds: vec!["Iterator".into()],
                    default: None,
                },
            ],
        };
        assert_eq!(family.assoc_types.len(), 2);
    }

    // S4.3 — Type-Level Booleans
    #[test]
    fn s4_3_tbool_ops() {
        assert_eq!(TBool::True.and(TBool::True), TBool::True);
        assert_eq!(TBool::True.and(TBool::False), TBool::False);
        assert_eq!(TBool::False.or(TBool::True), TBool::True);
        assert_eq!(TBool::True.not(), TBool::False);
    }

    // S4.4 — Type-Level Lists
    #[test]
    fn s4_4_hlist_construction() {
        let list = HList::from_types(&["i32", "String", "bool"]);
        assert_eq!(list.len(), 3);
        assert_eq!(list.head(), Some("i32"));
        assert!(!list.is_empty());
    }

    #[test]
    fn s4_4_hlist_display() {
        let list = HList::from_types(&["i32", "bool"]);
        assert_eq!(list.to_string(), "HCons<i32, HCons<bool, HNil>>");
    }

    #[test]
    fn s4_4_hlist_nil() {
        let list = HList::Nil;
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.head(), None);
    }

    // S4.5 — Compile-Time Type Selection
    #[test]
    fn s4_5_type_select_true() {
        assert_eq!(type_select(TBool::True, "i32", "f64"), "i32");
    }

    #[test]
    fn s4_5_type_select_false() {
        assert_eq!(type_select(TBool::False, "i32", "f64"), "f64");
    }

    // S4.6 — Phantom Types
    #[test]
    fn s4_6_phantom_state_machine() {
        let sm = PhantomStateMachine {
            name: "Door".into(),
            state_param: "State".into(),
            transitions: vec![
                ("Locked".into(), "Unlocked".into(), "unlock".into()),
                ("Unlocked".into(), "Locked".into(), "lock".into()),
                ("Unlocked".into(), "Open".into(), "open".into()),
                ("Open".into(), "Unlocked".into(), "close".into()),
            ],
        };
        assert!(sm.is_valid_transition("Locked", "Unlocked"));
        assert!(!sm.is_valid_transition("Locked", "Open"));
        assert_eq!(sm.transition_method("Unlocked", "Open"), Some("open"));
    }

    // S4.7 — Type Witnesses
    #[test]
    fn s4_7_type_eq_refl() {
        let eq = TypeEq::refl("i32");
        assert!(eq.is_valid());
        assert_eq!(eq.left, "i32");
    }

    #[test]
    fn s4_7_type_eq_sym() {
        let eq = TypeEq {
            left: "A".into(),
            right: "B".into(),
        };
        let sym = eq.sym();
        assert_eq!(sym.left, "B");
        assert_eq!(sym.right, "A");
    }

    // S4.9 — Recursion Limits
    #[test]
    fn s4_9_default_config() {
        let config = TypeLevelConfig::default();
        assert_eq!(config.recursion_limit, 256);
        assert!(config.cache_enabled);
    }

    // S4.10 — Integration
    #[test]
    fn s4_10_type_expr_display() {
        assert_eq!(TypeExpr::Named("i32".into()).to_string(), "i32");
        assert_eq!(
            TypeExpr::App("Vec".into(), vec![TypeExpr::Named("i32".into())]).to_string(),
            "Vec<i32>"
        );
        assert_eq!(TypeExpr::Never.to_string(), "!");
    }

    #[test]
    fn s4_10_conditional_type_eval() {
        let func = TypeFunction {
            name: "IfElse".into(),
            params: vec!["C".into(), "T".into(), "F".into()],
            body: TypeExpr::Cond {
                condition: Box::new(TypeExpr::Named("C".into())),
                then_type: Box::new(TypeExpr::Named("T".into())),
                else_type: Box::new(TypeExpr::Named("F".into())),
            },
        };
        let result = eval_type_function(
            &func,
            &[
                TypeExpr::Named("True".into()),
                TypeExpr::Named("i32".into()),
                TypeExpr::Named("f64".into()),
            ],
            &HashMap::new(),
        );
        assert_eq!(result.unwrap(), TypeExpr::Named("i32".into()));
    }
}
