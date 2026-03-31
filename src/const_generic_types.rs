//! Const generics in the type system — fixed-size arrays `[T; N]`, const arithmetic
//! in type positions, dependent type verification, and const generic specialization
//! for structs, enums, functions, methods, and trait impls.
//!
//! Builds on `const_generics.rs` (K1) and `dependent/nat.rs` (type-level Nat).

use std::collections::HashMap;

use crate::dependent::nat::{NatConstraint, NatValue};

// ═══════════════════════════════════════════════════════════════════════
// K8.1: [T; N] Fixed Array Type
// ═══════════════════════════════════════════════════════════════════════

/// A type with const generic parameters resolved.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConstType {
    /// A primitive or named type: `i32`, `f64`, `Point`.
    Named(String),
    /// A fixed-size array: `[T; N]`.
    Array {
        elem: Box<ConstType>,
        size: ConstSize,
    },
    /// A const-generic struct: `Matrix<f64, 3, 4>`.
    Generic {
        base: String,
        type_args: Vec<ConstType>,
        const_args: Vec<ConstSize>,
    },
    /// A tuple type.
    Tuple(Vec<ConstType>),
    /// A reference type.
    Ref(Box<ConstType>),
}

/// A const size — either concrete or symbolic.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConstSize {
    /// A concrete size: `3`, `256`.
    Literal(u64),
    /// A const generic parameter: `N`.
    Param(String),
    /// An arithmetic expression: `N + 1`, `R * C`.
    Expr(ConstSizeExpr),
}

/// Arithmetic on const sizes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConstSizeExpr {
    Add(Box<ConstSize>, Box<ConstSize>),
    Mul(Box<ConstSize>, Box<ConstSize>),
    Sub(Box<ConstSize>, Box<ConstSize>),
}

impl ConstSize {
    /// Try to evaluate to a concrete value.
    pub fn evaluate(&self, env: &HashMap<String, u64>) -> Option<u64> {
        match self {
            ConstSize::Literal(n) => Some(*n),
            ConstSize::Param(name) => env.get(name).copied(),
            ConstSize::Expr(expr) => expr.evaluate(env),
        }
    }

    /// Substitute known param values.
    pub fn substitute(&self, env: &HashMap<String, u64>) -> ConstSize {
        match self {
            ConstSize::Literal(_) => self.clone(),
            ConstSize::Param(name) => {
                if let Some(&val) = env.get(name) {
                    ConstSize::Literal(val)
                } else {
                    self.clone()
                }
            }
            ConstSize::Expr(expr) => {
                let result = expr.substitute(env);
                // Try to simplify
                if let Some(val) = result.evaluate(env) {
                    ConstSize::Literal(val)
                } else {
                    ConstSize::Expr(result)
                }
            }
        }
    }

    /// Convert to NatValue for constraint checking.
    pub fn to_nat(&self) -> NatValue {
        match self {
            ConstSize::Literal(n) => NatValue::Literal(*n),
            ConstSize::Param(name) => NatValue::Param(name.clone()),
            ConstSize::Expr(expr) => expr.to_nat(),
        }
    }
}

impl ConstSizeExpr {
    fn evaluate(&self, env: &HashMap<String, u64>) -> Option<u64> {
        match self {
            ConstSizeExpr::Add(a, b) => Some(a.evaluate(env)?.saturating_add(b.evaluate(env)?)),
            ConstSizeExpr::Mul(a, b) => Some(a.evaluate(env)?.saturating_mul(b.evaluate(env)?)),
            ConstSizeExpr::Sub(a, b) => Some(a.evaluate(env)?.saturating_sub(b.evaluate(env)?)),
        }
    }

    fn substitute(&self, env: &HashMap<String, u64>) -> ConstSizeExpr {
        match self {
            ConstSizeExpr::Add(a, b) => {
                ConstSizeExpr::Add(Box::new(a.substitute(env)), Box::new(b.substitute(env)))
            }
            ConstSizeExpr::Mul(a, b) => {
                ConstSizeExpr::Mul(Box::new(a.substitute(env)), Box::new(b.substitute(env)))
            }
            ConstSizeExpr::Sub(a, b) => {
                ConstSizeExpr::Sub(Box::new(a.substitute(env)), Box::new(b.substitute(env)))
            }
        }
    }

    fn to_nat(&self) -> NatValue {
        match self {
            ConstSizeExpr::Add(a, b) => NatValue::Add(Box::new(a.to_nat()), Box::new(b.to_nat())),
            ConstSizeExpr::Mul(a, b) => NatValue::Mul(Box::new(a.to_nat()), Box::new(b.to_nat())),
            ConstSizeExpr::Sub(a, b) => NatValue::Sub(Box::new(a.to_nat()), Box::new(b.to_nat())),
        }
    }
}

impl std::fmt::Display for ConstType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstType::Named(n) => write!(f, "{n}"),
            ConstType::Array { elem, size } => write!(f, "[{elem}; {size}]"),
            ConstType::Generic {
                base,
                type_args,
                const_args,
            } => {
                write!(f, "{base}<")?;
                let mut first = true;
                for ta in type_args {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{ta}")?;
                    first = false;
                }
                for ca in const_args {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{ca}")?;
                    first = false;
                }
                write!(f, ">")
            }
            ConstType::Tuple(elems) => {
                write!(f, "(")?;
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{e}")?;
                }
                write!(f, ")")
            }
            ConstType::Ref(inner) => write!(f, "&{inner}"),
        }
    }
}

impl std::fmt::Display for ConstSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstSize::Literal(n) => write!(f, "{n}"),
            ConstSize::Param(name) => write!(f, "{name}"),
            ConstSize::Expr(expr) => write!(f, "{expr}"),
        }
    }
}

impl std::fmt::Display for ConstSizeExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstSizeExpr::Add(a, b) => write!(f, "{a} + {b}"),
            ConstSizeExpr::Mul(a, b) => write!(f, "{a} * {b}"),
            ConstSizeExpr::Sub(a, b) => write!(f, "{a} - {b}"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K8.2 / K8.3: Const Generic Functions and Matrix Type
// ═══════════════════════════════════════════════════════════════════════

/// A const-generic function signature.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstGenericFn {
    /// Function name.
    pub name: String,
    /// Type parameters (e.g., `T`).
    pub type_params: Vec<String>,
    /// Const parameters (e.g., `N: usize`).
    pub const_params: Vec<String>,
    /// Parameter types.
    pub param_types: Vec<ConstType>,
    /// Return type.
    pub return_type: ConstType,
    /// Constraints on const params.
    pub constraints: Vec<NatConstraint>,
}

impl ConstGenericFn {
    /// Generate mangled name for a concrete specialization.
    pub fn mangled_name(&self, type_args: &[&str], const_args: &[u64]) -> String {
        let mut parts = vec![self.name.clone()];
        for ta in type_args {
            parts.push(ta.to_string());
        }
        for ca in const_args {
            parts.push(format!("N{ca}"));
        }
        parts.join("_")
    }

    /// Check constraints for a concrete specialization.
    pub fn check_constraints(&self, const_args: &[u64]) -> Vec<String> {
        let mut env = HashMap::new();
        for (name, val) in self.const_params.iter().zip(const_args.iter()) {
            env.insert(name.clone(), *val);
        }
        let mut errors = Vec::new();
        for c in &self.constraints {
            if let Err(e) = c.check(&env) {
                errors.push(e.to_string());
            }
        }
        errors
    }

    /// Specialize return type with concrete args.
    pub fn specialize_return(
        &self,
        type_args: &HashMap<String, String>,
        const_args: &HashMap<String, u64>,
    ) -> ConstType {
        specialize_type(&self.return_type, type_args, const_args)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K8.4 / K8.5: Const Generic Methods and Trait Impls
// ═══════════════════════════════════════════════════════════════════════

/// A const-generic trait implementation.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstGenericTraitImpl {
    /// Trait name.
    pub trait_name: String,
    /// Target type pattern (may contain const params).
    pub target: ConstType,
    /// Const params for this impl.
    pub const_params: Vec<String>,
    /// Methods in this impl.
    pub methods: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// K8.6: Const Arithmetic in Types
// ═══════════════════════════════════════════════════════════════════════

/// Check that a const arithmetic expression doesn't overflow usize.
pub fn check_size_overflow(size: &ConstSize, env: &HashMap<String, u64>) -> Result<u64, String> {
    match size.evaluate(env) {
        Some(val) => {
            if val > u32::MAX as u64 {
                Err(format!("const size {val} exceeds maximum ({})", u32::MAX))
            } else {
                Ok(val)
            }
        }
        None => Err("const size cannot be evaluated (unresolved parameters)".to_string()),
    }
}

/// Specialize a ConstType by substituting type and const params.
pub fn specialize_type(
    ty: &ConstType,
    type_args: &HashMap<String, String>,
    const_args: &HashMap<String, u64>,
) -> ConstType {
    match ty {
        ConstType::Named(name) => {
            if let Some(concrete) = type_args.get(name) {
                ConstType::Named(concrete.clone())
            } else {
                ty.clone()
            }
        }
        ConstType::Array { elem, size } => ConstType::Array {
            elem: Box::new(specialize_type(elem, type_args, const_args)),
            size: size.substitute(const_args),
        },
        ConstType::Generic {
            base,
            type_args: ta,
            const_args: ca,
        } => ConstType::Generic {
            base: base.clone(),
            type_args: ta
                .iter()
                .map(|t| specialize_type(t, type_args, const_args))
                .collect(),
            const_args: ca.iter().map(|c| c.substitute(const_args)).collect(),
        },
        ConstType::Tuple(elems) => ConstType::Tuple(
            elems
                .iter()
                .map(|e| specialize_type(e, type_args, const_args))
                .collect(),
        ),
        ConstType::Ref(inner) => {
            ConstType::Ref(Box::new(specialize_type(inner, type_args, const_args)))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K8.8: Const Generic Enum
// ═══════════════════════════════════════════════════════════════════════

/// A const-generic enum definition (e.g., SmallVec<T, const N: usize>).
#[derive(Debug, Clone, PartialEq)]
pub struct ConstGenericEnum {
    /// Enum name.
    pub name: String,
    /// Type parameters.
    pub type_params: Vec<String>,
    /// Const parameters.
    pub const_params: Vec<String>,
    /// Variants.
    pub variants: Vec<ConstEnumVariant>,
}

/// A variant in a const-generic enum.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstEnumVariant {
    /// Variant name.
    pub name: String,
    /// Payload type (may reference const params).
    pub payload: Option<ConstType>,
}

// ═══════════════════════════════════════════════════════════════════════
// K8.9: Default Const Values
// ═══════════════════════════════════════════════════════════════════════

/// A const generic parameter with optional default value.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstParamWithDefault {
    /// Parameter name.
    pub name: String,
    /// Default value (if any).
    pub default: Option<u64>,
}

/// Resolve const args, filling in defaults where not explicitly provided.
pub fn resolve_with_defaults(
    params: &[ConstParamWithDefault],
    explicit_args: &[Option<u64>],
) -> Result<Vec<u64>, String> {
    let mut result = Vec::new();
    for (i, param) in params.iter().enumerate() {
        let val = explicit_args
            .get(i)
            .copied()
            .flatten()
            .or(param.default)
            .ok_or_else(|| {
                format!(
                    "const parameter '{}' has no value and no default",
                    param.name
                )
            })?;
        result.push(val);
    }
    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════════
// K8.7: Dependent Type Verification
// ═══════════════════════════════════════════════════════════════════════

/// Verify that two const sizes are equal (for type checking assignments).
pub fn check_size_eq(
    expected: &ConstSize,
    found: &ConstSize,
    env: &HashMap<String, u64>,
    context: &str,
) -> Result<(), String> {
    let ev = expected.evaluate(env);
    let fv = found.evaluate(env);
    match (ev, fv) {
        (Some(a), Some(b)) if a == b => Ok(()),
        (Some(a), Some(b)) => Err(format!(
            "type-level size mismatch in {context}: expected {expected} (= {a}), found {found} (= {b})"
        )),
        _ => Ok(()), // Defer if not fully resolved
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — K8.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── K8.1: [T; N] fixed array type ──

    #[test]
    fn k8_1_array_type_display() {
        let t = ConstType::Array {
            elem: Box::new(ConstType::Named("i32".into())),
            size: ConstSize::Literal(3),
        };
        assert_eq!(t.to_string(), "[i32; 3]");
    }

    #[test]
    fn k8_1_array_type_with_param() {
        let t = ConstType::Array {
            elem: Box::new(ConstType::Named("T".into())),
            size: ConstSize::Param("N".into()),
        };
        assert_eq!(t.to_string(), "[T; N]");
    }

    #[test]
    fn k8_1_array_distinct_sizes() {
        let a3 = ConstType::Array {
            elem: Box::new(ConstType::Named("i32".into())),
            size: ConstSize::Literal(3),
        };
        let a4 = ConstType::Array {
            elem: Box::new(ConstType::Named("i32".into())),
            size: ConstSize::Literal(4),
        };
        assert_ne!(a3, a4); // [i32; 3] != [i32; 4]
    }

    // ── K8.2: Matrix type ──

    #[test]
    fn k8_2_matrix_type() {
        let t = ConstType::Generic {
            base: "Matrix".into(),
            type_args: vec![ConstType::Named("f64".into())],
            const_args: vec![ConstSize::Literal(3), ConstSize::Literal(3)],
        };
        assert_eq!(t.to_string(), "Matrix<f64, 3, 3>");
    }

    // ── K8.3: Const generic functions ──

    #[test]
    fn k8_3_const_generic_fn_dot_product() {
        let f = ConstGenericFn {
            name: "dot".into(),
            type_params: vec![],
            const_params: vec!["N".into()],
            param_types: vec![
                ConstType::Array {
                    elem: Box::new(ConstType::Named("f64".into())),
                    size: ConstSize::Param("N".into()),
                },
                ConstType::Array {
                    elem: Box::new(ConstType::Named("f64".into())),
                    size: ConstSize::Param("N".into()),
                },
            ],
            return_type: ConstType::Named("f64".into()),
            constraints: vec![NatConstraint::GreaterThan(NatValue::Param("N".into()), 0)],
        };

        assert_eq!(f.mangled_name(&[], &[3]), "dot_N3");
        assert!(f.check_constraints(&[3]).is_empty());
        assert!(!f.check_constraints(&[0]).is_empty()); // N > 0 violated
    }

    // ── K8.4: Const generic methods ──

    #[test]
    fn k8_4_const_generic_method_len() {
        // impl<const N: usize> [T; N] { fn len() -> usize { N } }
        let f = ConstGenericFn {
            name: "len".into(),
            type_params: vec!["T".into()],
            const_params: vec!["N".into()],
            param_types: vec![],
            return_type: ConstType::Named("usize".into()),
            constraints: vec![],
        };
        assert_eq!(f.mangled_name(&["i32"], &[5]), "len_i32_N5");
    }

    // ── K8.5: Const generic trait impl ──

    #[test]
    fn k8_5_trait_impl_for_array() {
        let impl_def = ConstGenericTraitImpl {
            trait_name: "Display".into(),
            target: ConstType::Array {
                elem: Box::new(ConstType::Named("T".into())),
                size: ConstSize::Param("N".into()),
            },
            const_params: vec!["N".into()],
            methods: vec!["fmt".into()],
        };
        assert_eq!(impl_def.trait_name, "Display");
        assert_eq!(impl_def.const_params, vec!["N"]);
    }

    // ── K8.6: Const arithmetic in types ──

    #[test]
    fn k8_6_concat_return_type() {
        // fn concat<const A: usize, const B: usize>(x: [T; A], y: [T; B]) -> [T; A + B]
        let ret = ConstType::Array {
            elem: Box::new(ConstType::Named("T".into())),
            size: ConstSize::Expr(ConstSizeExpr::Add(
                Box::new(ConstSize::Param("A".into())),
                Box::new(ConstSize::Param("B".into())),
            )),
        };
        assert_eq!(ret.to_string(), "[T; A + B]");

        let mut env = HashMap::new();
        env.insert("A".to_string(), 3);
        env.insert("B".to_string(), 4);

        let specialized = ret.to_string();
        assert!(specialized.contains("A + B"));

        // Evaluate the size
        if let ConstType::Array { size, .. } = &ret {
            assert_eq!(size.evaluate(&env), Some(7));
        }
    }

    // ── K8.7: Dependent type verification ──

    #[test]
    fn k8_7_size_eq_pass() {
        let a = ConstSize::Literal(3);
        let b = ConstSize::Literal(3);
        assert!(check_size_eq(&a, &b, &HashMap::new(), "test").is_ok());
    }

    #[test]
    fn k8_7_size_eq_fail() {
        let a = ConstSize::Literal(3);
        let b = ConstSize::Literal(4);
        let err = check_size_eq(&a, &b, &HashMap::new(), "matmul").unwrap_err();
        assert!(err.contains("mismatch"));
        assert!(err.contains("matmul"));
    }

    #[test]
    fn k8_7_overflow_check() {
        let big = ConstSize::Literal(u64::MAX);
        let env = HashMap::new();
        assert!(check_size_overflow(&big, &env).is_err());

        let ok = ConstSize::Literal(1024);
        assert_eq!(check_size_overflow(&ok, &env), Ok(1024));
    }

    // ── K8.8: Const generic enum ──

    #[test]
    fn k8_8_smallvec_enum() {
        let e = ConstGenericEnum {
            name: "SmallVec".into(),
            type_params: vec!["T".into()],
            const_params: vec!["N".into()],
            variants: vec![
                ConstEnumVariant {
                    name: "Inline".into(),
                    payload: Some(ConstType::Array {
                        elem: Box::new(ConstType::Named("T".into())),
                        size: ConstSize::Param("N".into()),
                    }),
                },
                ConstEnumVariant {
                    name: "Heap".into(),
                    payload: Some(ConstType::Named("Vec".into())),
                },
            ],
        };
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].name, "Inline");
    }

    // ── K8.9: Default const values ──

    #[test]
    fn k8_9_default_const_values() {
        let params = vec![
            ConstParamWithDefault {
                name: "N".into(),
                default: Some(1024),
            },
            ConstParamWithDefault {
                name: "M".into(),
                default: None,
            },
        ];

        // All explicit
        let result = resolve_with_defaults(&params, &[Some(256), Some(512)]);
        assert_eq!(result, Ok(vec![256, 512]));

        // N uses default, M explicit
        let result = resolve_with_defaults(&params, &[None, Some(64)]);
        assert_eq!(result, Ok(vec![1024, 64]));

        // M missing without default → error
        let result = resolve_with_defaults(&params, &[Some(256)]);
        assert!(result.is_err());
    }

    // ── K8.10: Integration — type specialization ──

    #[test]
    fn k8_10_specialize_array_type() {
        let ty = ConstType::Array {
            elem: Box::new(ConstType::Named("T".into())),
            size: ConstSize::Param("N".into()),
        };

        let mut ta = HashMap::new();
        ta.insert("T".to_string(), "f64".to_string());
        let mut ca = HashMap::new();
        ca.insert("N".to_string(), 5);

        let specialized = specialize_type(&ty, &ta, &ca);
        assert_eq!(specialized.to_string(), "[f64; 5]");
    }

    #[test]
    fn k8_10_specialize_matrix_type() {
        let ty = ConstType::Generic {
            base: "Matrix".into(),
            type_args: vec![ConstType::Named("T".into())],
            const_args: vec![ConstSize::Param("R".into()), ConstSize::Param("C".into())],
        };

        let mut ta = HashMap::new();
        ta.insert("T".to_string(), "f64".to_string());
        let mut ca = HashMap::new();
        ca.insert("R".to_string(), 3);
        ca.insert("C".to_string(), 4);

        let specialized = specialize_type(&ty, &ta, &ca);
        assert_eq!(specialized.to_string(), "Matrix<f64, 3, 4>");
    }

    #[test]
    fn k8_10_specialize_concat_return() {
        let f = ConstGenericFn {
            name: "concat".into(),
            type_params: vec!["T".into()],
            const_params: vec!["A".into(), "B".into()],
            param_types: vec![],
            return_type: ConstType::Array {
                elem: Box::new(ConstType::Named("T".into())),
                size: ConstSize::Expr(ConstSizeExpr::Add(
                    Box::new(ConstSize::Param("A".into())),
                    Box::new(ConstSize::Param("B".into())),
                )),
            },
            constraints: vec![],
        };

        let mut ta = HashMap::new();
        ta.insert("T".to_string(), "i32".to_string());
        let mut ca = HashMap::new();
        ca.insert("A".to_string(), 3);
        ca.insert("B".to_string(), 4);

        let ret = f.specialize_return(&ta, &ca);
        assert_eq!(ret.to_string(), "[i32; 7]"); // 3 + 4 = 7
    }
}
