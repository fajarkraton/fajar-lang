//! Type inference via unification for generic functions.
//!
//! Implements Robinson's unification algorithm to infer type arguments
//! at generic function call sites.
//!
//! # Algorithm
//!
//! Given a generic function `fn max<T>(a: T, b: T) -> T` and a call `max(1, 2)`:
//! 1. Unify each formal parameter type with the corresponding actual argument type
//! 2. Build a substitution map: `{T → IntLiteral}`
//! 3. Apply substitution to the return type to get the concrete return type
//! 4. If any type parameter remains unbound, emit SE013

use std::collections::HashMap;

use crate::analyzer::type_check::Type;
use crate::lexer::token::Span;

/// A substitution map from type variable names to concrete types.
#[derive(Debug, Clone, Default)]
pub struct Substitution {
    bindings: HashMap<String, Type>,
}

impl Substitution {
    /// Creates an empty substitution.
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks up a type variable binding.
    pub fn lookup(&self, name: &str) -> Option<&Type> {
        self.bindings.get(name)
    }

    /// Binds a type variable to a concrete type.
    fn bind(&mut self, name: String, ty: Type) {
        self.bindings.insert(name, ty);
    }

    /// Applies this substitution to a type, replacing type variables with their bindings.
    pub fn apply(&self, ty: &Type) -> Type {
        match ty {
            Type::TypeVar(name) => {
                if let Some(bound) = self.bindings.get(name) {
                    // Recursively apply in case bound type contains more type vars
                    self.apply(bound)
                } else {
                    ty.clone()
                }
            }
            Type::Array(inner) => Type::Array(Box::new(self.apply(inner))),
            Type::Tuple(elems) => Type::Tuple(elems.iter().map(|t| self.apply(t)).collect()),
            Type::Function { params, ret } => Type::Function {
                params: params.iter().map(|t| self.apply(t)).collect(),
                ret: Box::new(self.apply(ret)),
            },
            Type::Ref(inner, lt) => Type::Ref(Box::new(self.apply(inner)), *lt),
            Type::RefMut(inner, lt) => Type::RefMut(Box::new(self.apply(inner)), *lt),
            // All other types are concrete — return as-is
            _ => ty.clone(),
        }
    }
}

/// An error during type unification.
#[derive(Debug, Clone)]
pub enum UnifyError {
    /// Two types cannot be unified.
    Mismatch { expected: Type, found: Type },
    /// Occurs check: type variable appears in its own binding (infinite type).
    OccursCheck { var: String, ty: Type },
}

/// Unifies two types, building up a substitution map.
///
/// Returns `Ok(())` if the types can be unified, `Err` otherwise.
pub fn unify(t1: &Type, t2: &Type, subst: &mut Substitution) -> Result<(), Box<UnifyError>> {
    let t1 = subst.apply(t1);
    let t2 = subst.apply(t2);

    match (&t1, &t2) {
        // Same type — trivially unifies
        _ if t1 == t2 => Ok(()),

        // Unknown unifies with everything (error recovery)
        (Type::Unknown, _) | (_, Type::Unknown) => Ok(()),

        // Never unifies with everything (diverging)
        (Type::Never, _) | (_, Type::Never) => Ok(()),

        // TypeVar on left — bind it
        (Type::TypeVar(name), _) => {
            if occurs_in(name, &t2) {
                return Err(Box::new(UnifyError::OccursCheck {
                    var: name.clone(),
                    ty: t2,
                }));
            }
            subst.bind(name.clone(), t2);
            Ok(())
        }

        // TypeVar on right — bind it
        (_, Type::TypeVar(name)) => {
            if occurs_in(name, &t1) {
                return Err(Box::new(UnifyError::OccursCheck {
                    var: name.clone(),
                    ty: t1,
                }));
            }
            subst.bind(name.clone(), t1);
            Ok(())
        }

        // IntLiteral unifies with any integer type
        (Type::IntLiteral, other) if other.is_integer() => Ok(()),
        (other, Type::IntLiteral) if other.is_integer() => Ok(()),
        (Type::IntLiteral, Type::IntLiteral) => Ok(()),

        // FloatLiteral unifies with any float type
        (Type::FloatLiteral, other) if other.is_float() => Ok(()),
        (other, Type::FloatLiteral) if other.is_float() => Ok(()),
        (Type::FloatLiteral, Type::FloatLiteral) => Ok(()),

        // Array — unify element types
        (Type::Array(a), Type::Array(b)) => unify(a, b, subst),

        // Tuple — unify element-wise
        (Type::Tuple(a), Type::Tuple(b)) => {
            if a.len() != b.len() {
                return Err(Box::new(UnifyError::Mismatch {
                    expected: t1,
                    found: t2,
                }));
            }
            for (x, y) in a.iter().zip(b.iter()) {
                unify(x, y, subst)?;
            }
            Ok(())
        }

        // Function — unify params and return type
        (
            Type::Function {
                params: p1,
                ret: r1,
            },
            Type::Function {
                params: p2,
                ret: r2,
            },
        ) => {
            if p1.len() != p2.len() {
                return Err(Box::new(UnifyError::Mismatch {
                    expected: t1,
                    found: t2,
                }));
            }
            for (x, y) in p1.iter().zip(p2.iter()) {
                unify(x, y, subst)?;
            }
            unify(r1, r2, subst)
        }

        // References — unify inner types
        (Type::Ref(a, _), Type::Ref(b, _)) => unify(a, b, subst),
        (Type::RefMut(a, _), Type::RefMut(b, _)) => unify(a, b, subst),

        // Everything else — mismatch
        _ => Err(Box::new(UnifyError::Mismatch {
            expected: t1,
            found: t2,
        })),
    }
}

/// Infers type arguments for a generic function call.
///
/// Given formal parameter types (with TypeVar) and actual argument types,
/// returns a substitution that maps type variables to concrete types.
pub fn infer_type_args(
    formal_params: &[Type],
    actual_args: &[Type],
    generic_names: &[String],
    span: Span,
) -> Result<Substitution, Box<InferError>> {
    let mut subst = Substitution::new();

    // Unify each formal param with actual arg
    for (formal, actual) in formal_params.iter().zip(actual_args.iter()) {
        if let Err(err) = unify(formal, actual, &mut subst) {
            return Err(Box::new(InferError::UnificationFailed {
                expected: formal.clone(),
                found: actual.clone(),
                detail: err,
                span,
            }));
        }
    }

    // Check that all generic params are bound
    for name in generic_names {
        if subst.lookup(name).is_none() {
            return Err(Box::new(InferError::Unbound {
                param: name.clone(),
                span,
            }));
        }
    }

    Ok(subst)
}

/// Errors from type argument inference.
#[derive(Debug, Clone)]
pub enum InferError {
    /// Unification failed for a parameter.
    UnificationFailed {
        expected: Type,
        found: Type,
        detail: Box<UnifyError>,
        span: Span,
    },
    /// A type parameter could not be inferred.
    Unbound { param: String, span: Span },
}

/// Occurs check: does type variable `name` appear in `ty`?
fn occurs_in(name: &str, ty: &Type) -> bool {
    match ty {
        Type::TypeVar(n) => n == name,
        Type::Array(inner) | Type::Ref(inner, _) | Type::RefMut(inner, _) => occurs_in(name, inner),
        Type::Tuple(elems) => elems.iter().any(|t| occurs_in(name, t)),
        Type::Function { params, ret } => {
            params.iter().any(|t| occurs_in(name, t)) || occurs_in(name, ret)
        }
        _ => false,
    }
}

/// Checks if a type contains any TypeVar.
pub fn has_type_vars(ty: &Type) -> bool {
    match ty {
        Type::TypeVar(_) => true,
        Type::Array(inner) | Type::Ref(inner, _) | Type::RefMut(inner, _) => has_type_vars(inner),
        Type::Tuple(elems) => elems.iter().any(has_type_vars),
        Type::Function { params, ret } => params.iter().any(has_type_vars) || has_type_vars(ret),
        _ => false,
    }
}

/// Extracts type variable names from a function type's parameters.
pub fn extract_generic_names(fn_type: &Type) -> Vec<String> {
    let mut names = Vec::new();
    if let Type::Function { params, ret } = fn_type {
        collect_type_vars(params, ret, &mut names);
    }
    names
}

fn collect_type_vars(params: &[Type], ret: &Type, names: &mut Vec<String>) {
    for p in params {
        collect_type_vars_from(p, names);
    }
    collect_type_vars_from(ret, names);
}

fn collect_type_vars_from(ty: &Type, names: &mut Vec<String>) {
    match ty {
        Type::TypeVar(n) if !names.contains(n) => {
            names.push(n.clone());
        }
        Type::Array(inner) | Type::Ref(inner, _) | Type::RefMut(inner, _) => {
            collect_type_vars_from(inner, names);
        }
        Type::Tuple(elems) => {
            for t in elems {
                collect_type_vars_from(t, names);
            }
        }
        Type::Function { params, ret } => {
            collect_type_vars(params, ret, names);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unify_same_types() {
        let mut subst = Substitution::new();
        assert!(unify(&Type::I64, &Type::I64, &mut subst).is_ok());
        assert!(unify(&Type::Bool, &Type::Bool, &mut subst).is_ok());
    }

    #[test]
    fn unify_different_types_fails() {
        let mut subst = Substitution::new();
        assert!(unify(&Type::I64, &Type::F64, &mut subst).is_err());
    }

    #[test]
    fn unify_typevar_binds() {
        let mut subst = Substitution::new();
        let t = Type::TypeVar("T".into());
        assert!(unify(&t, &Type::I64, &mut subst).is_ok());
        assert_eq!(subst.lookup("T"), Some(&Type::I64));
    }

    #[test]
    fn unify_typevar_consistent() {
        // T unifies with I64, then T must be I64
        let mut subst = Substitution::new();
        let t = Type::TypeVar("T".into());
        assert!(unify(&t, &Type::I64, &mut subst).is_ok());
        assert!(unify(&t, &Type::I64, &mut subst).is_ok());
    }

    #[test]
    fn unify_typevar_conflict() {
        // T = I64, then T = F64 → mismatch
        let mut subst = Substitution::new();
        let t = Type::TypeVar("T".into());
        assert!(unify(&t, &Type::I64, &mut subst).is_ok());
        assert!(unify(&t, &Type::F64, &mut subst).is_err());
    }

    #[test]
    fn unify_int_literal_with_integer() {
        let mut subst = Substitution::new();
        assert!(unify(&Type::IntLiteral, &Type::I32, &mut subst).is_ok());
        assert!(unify(&Type::IntLiteral, &Type::I64, &mut subst).is_ok());
    }

    #[test]
    fn unify_int_literal_with_float_fails() {
        let mut subst = Substitution::new();
        assert!(unify(&Type::IntLiteral, &Type::F64, &mut subst).is_err());
    }

    #[test]
    fn unify_typevar_with_two_int_literals() {
        // max(1, 2): T = IntLiteral, T = IntLiteral → OK
        let mut subst = Substitution::new();
        let t = Type::TypeVar("T".into());
        assert!(unify(&t, &Type::IntLiteral, &mut subst).is_ok());
        assert!(unify(&t, &Type::IntLiteral, &mut subst).is_ok());
    }

    #[test]
    fn unify_typevar_int_then_float_literal_fails() {
        // max(1, 2.0): T = IntLiteral, T = FloatLiteral → FAIL
        let mut subst = Substitution::new();
        let t = Type::TypeVar("T".into());
        assert!(unify(&t, &Type::IntLiteral, &mut subst).is_ok());
        assert!(unify(&t, &Type::FloatLiteral, &mut subst).is_err());
    }

    #[test]
    fn unify_array_types() {
        let mut subst = Substitution::new();
        let t = Type::TypeVar("T".into());
        let arr_t = Type::Array(Box::new(t));
        let arr_i64 = Type::Array(Box::new(Type::I64));
        assert!(unify(&arr_t, &arr_i64, &mut subst).is_ok());
        assert_eq!(subst.lookup("T"), Some(&Type::I64));
    }

    #[test]
    fn unify_occurs_check() {
        // T = Array<T> → infinite type → error
        let mut subst = Substitution::new();
        let t = Type::TypeVar("T".into());
        let arr_t = Type::Array(Box::new(Type::TypeVar("T".into())));
        let result = unify(&t, &arr_t, &mut subst);
        assert!(matches!(
            result.as_ref().map_err(|e| e.as_ref()),
            Err(UnifyError::OccursCheck { .. })
        ));
    }

    #[test]
    fn apply_substitution() {
        let mut subst = Substitution::new();
        subst.bind("T".into(), Type::I64);
        subst.bind("U".into(), Type::Str);

        assert_eq!(subst.apply(&Type::TypeVar("T".into())), Type::I64);
        assert_eq!(subst.apply(&Type::TypeVar("U".into())), Type::Str);
        assert_eq!(subst.apply(&Type::Bool), Type::Bool);
        assert_eq!(
            subst.apply(&Type::Array(Box::new(Type::TypeVar("T".into())))),
            Type::Array(Box::new(Type::I64))
        );
    }

    #[test]
    fn infer_type_args_basic() {
        // fn max<T>(a: T, b: T) -> T, called with (I64, I64)
        let formal = vec![Type::TypeVar("T".into()), Type::TypeVar("T".into())];
        let actual = vec![Type::I64, Type::I64];
        let generics = vec!["T".into()];
        let span = Span::new(0, 10);

        let result = infer_type_args(&formal, &actual, &generics, span);
        assert!(result.is_ok());
        let subst = result.unwrap();
        assert_eq!(subst.lookup("T"), Some(&Type::I64));
    }

    #[test]
    fn infer_type_args_conflict() {
        // fn max<T>(a: T, b: T), called with (I64, F64) → error
        let formal = vec![Type::TypeVar("T".into()), Type::TypeVar("T".into())];
        let actual = vec![Type::I64, Type::F64];
        let generics = vec!["T".into()];
        let span = Span::new(0, 10);

        let result = infer_type_args(&formal, &actual, &generics, span);
        assert!(result.is_err());
    }

    #[test]
    fn infer_type_args_unbound() {
        // fn foo<T, U>(a: T), called with (I64) → U is unbound
        let formal = vec![Type::TypeVar("T".into())];
        let actual = vec![Type::I64];
        let generics = vec!["T".into(), "U".into()];
        let span = Span::new(0, 10);

        let result = infer_type_args(&formal, &actual, &generics, span);
        assert!(matches!(
            result.as_ref().map_err(|e| e.as_ref()),
            Err(InferError::Unbound { param, .. }) if param == "U"
        ));
    }

    #[test]
    fn infer_two_type_params() {
        // fn pair<T, U>(a: T, b: U), called with (I64, Str)
        let formal = vec![Type::TypeVar("T".into()), Type::TypeVar("U".into())];
        let actual = vec![Type::I64, Type::Str];
        let generics = vec!["T".into(), "U".into()];
        let span = Span::new(0, 10);

        let result = infer_type_args(&formal, &actual, &generics, span);
        assert!(result.is_ok());
        let subst = result.unwrap();
        assert_eq!(subst.lookup("T"), Some(&Type::I64));
        assert_eq!(subst.lookup("U"), Some(&Type::Str));
    }

    #[test]
    fn has_type_vars_detection() {
        assert!(has_type_vars(&Type::TypeVar("T".into())));
        assert!(has_type_vars(&Type::Array(Box::new(Type::TypeVar(
            "T".into()
        )))));
        assert!(!has_type_vars(&Type::I64));
        assert!(!has_type_vars(&Type::Array(Box::new(Type::I64))));
    }

    #[test]
    fn extract_generic_names_from_fn() {
        let fn_type = Type::Function {
            params: vec![Type::TypeVar("T".into()), Type::TypeVar("U".into())],
            ret: Box::new(Type::TypeVar("T".into())),
        };
        let names = extract_generic_names(&fn_type);
        assert_eq!(names, vec!["T".to_string(), "U".to_string()]);
    }
}
