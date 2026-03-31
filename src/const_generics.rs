//! Const generics integration — wires parser `const N: usize` syntax to
//! the dependent type system (`NatValue`, `MonoKey`) and the interpreter/codegen.
//!
//! # Overview
//!
//! This module bridges:
//! - **Parser** (`GenericParam { is_comptime: true, const_type }`) — syntax
//! - **Dependent types** (`NatValue`, `NatConstraint`, `MonoKey`) — type-level math
//! - **Analyzer** — validates const params as Nat-kinded, checks bounds
//! - **Interpreter/Codegen** — monomorphizes const-generic functions at call sites
//!
//! # Syntax
//!
//! ```fajar
//! fn zeros<const N: usize>() -> [f64; N] { ... }
//! struct Matrix<T, const R: usize, const C: usize> { data: [T; R * C] }
//! impl<const N: usize> Matrix<f64, N, N> { fn identity() -> Self { ... } }
//! ```

use std::collections::HashMap;

use crate::dependent::nat::{ConstType, MonoKey, NatConstraint, NatValue};
use crate::parser::ast::GenericParam;

// ═══════════════════════════════════════════════════════════════════════
// K1.1 / K1.2: Const Parameter Classification
// ═══════════════════════════════════════════════════════════════════════

/// Classifies a generic parameter as either a type param or a const param.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamKind {
    /// A type parameter: `T`, `T: Display`.
    Type,
    /// A const parameter: `const N: usize`.
    Const { const_type: ConstType },
}

/// Analyzes a `GenericParam` from the parser and determines its kind.
pub fn classify_param(gp: &GenericParam) -> ParamKind {
    if gp.is_comptime {
        let ct = match gp.const_type.as_deref() {
            Some("usize") | Some("u64") | Some("u32") | Some("u16") | Some("u8") => {
                ConstType::Usize
            }
            Some("bool") => ConstType::Bool,
            Some("i64") | Some("i32") | Some("i16") | Some("i8") | Some("isize") => {
                ConstType::Usize
            }
            // Default to usize for untyped comptime params
            None | Some(_) => ConstType::Usize,
        };
        ParamKind::Const { const_type: ct }
    } else {
        ParamKind::Type
    }
}

/// Extracts const param names from a generic param list.
pub fn const_param_names(params: &[GenericParam]) -> Vec<String> {
    params
        .iter()
        .filter(|gp| gp.is_comptime)
        .map(|gp| gp.name.clone())
        .collect()
}

/// Extracts type param names from a generic param list.
pub fn type_param_names(params: &[GenericParam]) -> Vec<String> {
    params
        .iter()
        .filter(|gp| !gp.is_comptime && !gp.is_effect)
        .map(|gp| gp.name.clone())
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// K1.3: Const Monomorphization
// ═══════════════════════════════════════════════════════════════════════

/// Builds a `MonoKey` from a function call with type and const arguments.
///
/// # Example
///
/// `zeros<3>()` with base name "zeros" and const_args = [3]
/// produces MonoKey { name: "zeros", type_args: [], const_args: [3] }
/// which mangles to "zeros_N3".
pub fn build_mono_key(
    base_name: &str,
    type_args: &[String],
    const_args: &[u64],
) -> MonoKey {
    MonoKey {
        name: base_name.to_string(),
        type_args: type_args.to_vec(),
        const_args: const_args.to_vec(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K1.4: Type-Level Arithmetic in Types
// ═══════════════════════════════════════════════════════════════════════

/// Parses a simple const expression string into a `NatValue`.
///
/// Supports: literal integers, param names, and binary `+`, `*`, `-`.
///
/// # Examples
///
/// - `"3"` → `NatValue::Literal(3)`
/// - `"N"` → `NatValue::Param("N")`
/// - `"N + 1"` → `NatValue::Add(Param("N"), Literal(1))`
pub fn parse_nat_expr(input: &str) -> NatValue {
    let input = input.trim();

    // Try binary operators (lowest precedence first)
    if let Some(idx) = input.rfind('+') {
        if idx > 0 {
            let left = parse_nat_expr(&input[..idx]);
            let right = parse_nat_expr(&input[idx + 1..]);
            return NatValue::Add(Box::new(left), Box::new(right));
        }
    }

    if let Some(idx) = input.rfind('-') {
        if idx > 0 {
            let left = parse_nat_expr(&input[..idx]);
            let right = parse_nat_expr(&input[idx + 1..]);
            return NatValue::Sub(Box::new(left), Box::new(right));
        }
    }

    if let Some(idx) = input.rfind('*') {
        if idx > 0 {
            let left = parse_nat_expr(&input[..idx]);
            let right = parse_nat_expr(&input[idx + 1..]);
            return NatValue::Mul(Box::new(left), Box::new(right));
        }
    }

    // Try literal integer
    if let Ok(n) = input.parse::<u64>() {
        return NatValue::Literal(n);
    }

    // Otherwise treat as a named parameter
    NatValue::Param(input.to_string())
}

/// Evaluates a `NatValue` to a concrete u64 given an environment of const bindings.
pub fn eval_nat(nat: &NatValue, env: &HashMap<String, u64>) -> Option<u64> {
    nat.evaluate(env)
}

// ═══════════════════════════════════════════════════════════════════════
// K1.5: Const Parameter Bounds / Constraints
// ═══════════════════════════════════════════════════════════════════════

/// Checks a set of const generic constraints against concrete values.
///
/// Returns a list of violated constraints as error strings.
pub fn check_constraints(
    constraints: &[NatConstraint],
    env: &HashMap<String, u64>,
) -> Vec<String> {
    let mut errors = Vec::new();
    for c in constraints {
        if let Err(e) = c.check(env) {
            errors.push(e.to_string());
        }
    }
    errors
}

/// Creates a "greater than" constraint: `param > bound`.
pub fn constraint_gt(param: &str, bound: u64) -> NatConstraint {
    NatConstraint::GreaterThan(NatValue::Param(param.to_string()), bound)
}

/// Creates a "greater or equal" constraint: `param >= bound`.
pub fn constraint_ge(param: &str, bound: u64) -> NatConstraint {
    NatConstraint::GreaterEq(NatValue::Param(param.to_string()), bound)
}

/// Creates an equality constraint between two nat expressions.
pub fn constraint_eq(a: NatValue, b: NatValue) -> NatConstraint {
    NatConstraint::Equal(a, b)
}

// ═══════════════════════════════════════════════════════════════════════
// K1.6: Const Parameter Inference
// ═══════════════════════════════════════════════════════════════════════

/// Attempts to infer const generic arguments from a target array size.
///
/// Example: given `let arr: [i32; 3] = zeros()` where `zeros` has param `N`,
/// infer `N = 3` from the target type.
pub fn infer_const_from_array_size(
    target_size: u64,
    const_param_name: &str,
) -> HashMap<String, u64> {
    let mut env = HashMap::new();
    env.insert(const_param_name.to_string(), target_size);
    env
}

/// Attempts to infer const generics from concrete arguments at a call site.
///
/// Returns a map of param_name -> value for all successfully inferred params.
pub fn infer_const_args(
    const_params: &[String],
    explicit_args: &[Option<u64>],
) -> HashMap<String, u64> {
    let mut env = HashMap::new();
    for (i, name) in const_params.iter().enumerate() {
        if let Some(Some(val)) = explicit_args.get(i) {
            env.insert(name.clone(), *val);
        }
    }
    env
}

// ═══════════════════════════════════════════════════════════════════════
// K1.7: Const Params in Structs
// ═══════════════════════════════════════════════════════════════════════

/// Represents a const-generic struct specialization.
///
/// Example: `Matrix<f64, 3, 4>` is `ConstStructSpec { name: "Matrix", type_args: ["f64"], const_args: [3, 4] }`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstStructSpec {
    /// Base struct name.
    pub name: String,
    /// Concrete type arguments.
    pub type_args: Vec<String>,
    /// Concrete const arguments.
    pub const_args: Vec<u64>,
}

impl ConstStructSpec {
    /// Generates a mangled name: `Matrix_f64_N3_N4`.
    pub fn mangled_name(&self) -> String {
        let key = build_mono_key(&self.name, &self.type_args, &self.const_args);
        key.mangled_name()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K1.8: Const Params in Enums
// ═══════════════════════════════════════════════════════════════════════

/// Represents a const-generic enum specialization.
///
/// Example: `SmallVec<i32, 8>` → `ConstEnumSpec { name: "SmallVec", type_args: ["i32"], const_args: [8] }`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstEnumSpec {
    /// Base enum name.
    pub name: String,
    /// Concrete type arguments.
    pub type_args: Vec<String>,
    /// Concrete const arguments.
    pub const_args: Vec<u64>,
}

impl ConstEnumSpec {
    /// Generates a mangled name: `SmallVec_i32_N8`.
    pub fn mangled_name(&self) -> String {
        let key = build_mono_key(&self.name, &self.type_args, &self.const_args);
        key.mangled_name()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K1.9: Const Params in Impl Blocks
// ═══════════════════════════════════════════════════════════════════════

/// Represents a const-generic impl block target.
///
/// Example: `impl<const N: usize> Matrix<f64, N, N>` targets square matrices.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstImplTarget {
    /// Target struct/enum name.
    pub target_name: String,
    /// Const param names from the impl block.
    pub const_params: Vec<String>,
    /// Type param names from the impl block.
    pub type_params: Vec<String>,
    /// Constraints on const params (e.g., `N > 0`).
    pub constraints: Vec<NatConstraint>,
}

impl ConstImplTarget {
    /// Checks if a concrete specialization matches this impl target.
    ///
    /// Example: `Matrix<f64, 3, 3>` matches `impl<const N: usize> Matrix<f64, N, N>`
    /// because both const args are equal (both = N).
    pub fn matches_spec(&self, spec: &ConstStructSpec) -> bool {
        if self.target_name != spec.name {
            return false;
        }

        // Build an env from the first matching and check constraints
        let env = self.try_bind_const_args(&spec.const_args);
        match env {
            Some(bindings) => {
                // Check all constraints
                for c in &self.constraints {
                    if c.check(&bindings).is_err() {
                        return false;
                    }
                }
                true
            }
            None => false,
        }
    }

    /// Try to bind const args to param names, returning None if inconsistent.
    fn try_bind_const_args(&self, const_args: &[u64]) -> Option<HashMap<String, u64>> {
        let mut env = HashMap::new();

        // Simple binding: first N const params get the first N const args
        for (i, name) in self.const_params.iter().enumerate() {
            if let Some(&val) = const_args.get(i) {
                if let Some(&existing) = env.get(name) {
                    // Same param used twice — must have same value
                    if existing != val {
                        return None;
                    }
                } else {
                    env.insert(name.clone(), val);
                }
            }
        }

        Some(env)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — K1.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::Span;
    use crate::parser::ast::GenericParam;

    fn dummy_span() -> Span {
        Span::new(0, 0)
    }

    // ── K1.1: Const parameter syntax parsing ──

    #[test]
    fn k1_1_parse_const_generic_param() {
        // Parser accepts `const N: usize` and sets is_comptime + const_type
        let source = "fn zeros<const N: usize>() -> i64 { 0 }";
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();

        let fndef = match &program.items[0] {
            crate::parser::ast::Item::FnDef(f) => f,
            _ => panic!("expected FnDef"),
        };

        assert_eq!(fndef.generic_params.len(), 1);
        let gp = &fndef.generic_params[0];
        assert_eq!(gp.name, "N");
        assert!(gp.is_comptime);
        assert_eq!(gp.const_type, Some("usize".to_string()));
    }

    #[test]
    fn k1_1_parse_comptime_generic_param() {
        // `comptime` syntax also works
        let source = "fn fill<comptime N: usize>(val: i64) -> i64 { val }";
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();

        let fndef = match &program.items[0] {
            crate::parser::ast::Item::FnDef(f) => f,
            _ => panic!("expected FnDef"),
        };

        assert_eq!(fndef.generic_params.len(), 1);
        assert!(fndef.generic_params[0].is_comptime);
    }

    // ── K1.2: Const param in type system ──

    #[test]
    fn k1_2_classify_const_param() {
        let gp = GenericParam {
            name: "N".to_string(),
            bounds: vec![],
            is_comptime: true,
            is_effect: false,
            const_type: Some("usize".to_string()),
            span: dummy_span(),
        };
        assert_eq!(
            classify_param(&gp),
            ParamKind::Const {
                const_type: ConstType::Usize
            }
        );
    }

    #[test]
    fn k1_2_classify_type_param() {
        let gp = GenericParam {
            name: "T".to_string(),
            bounds: vec![],
            is_comptime: false,
            is_effect: false,
            const_type: None,
            span: dummy_span(),
        };
        assert_eq!(classify_param(&gp), ParamKind::Type);
    }

    #[test]
    fn k1_2_extract_const_and_type_param_names() {
        let params = vec![
            GenericParam {
                name: "T".to_string(),
                bounds: vec![],
                is_comptime: false,
                is_effect: false,
                const_type: None,
                span: dummy_span(),
            },
            GenericParam {
                name: "N".to_string(),
                bounds: vec![],
                is_comptime: true,
                is_effect: false,
                const_type: Some("usize".to_string()),
                span: dummy_span(),
            },
        ];
        assert_eq!(type_param_names(&params), vec!["T"]);
        assert_eq!(const_param_names(&params), vec!["N"]);
    }

    // ── K1.3: Const monomorphization ──

    #[test]
    fn k1_3_mono_key_simple() {
        let key = build_mono_key("zeros", &[], &[3]);
        assert_eq!(key.mangled_name(), "zeros_N3");
    }

    #[test]
    fn k1_3_mono_key_mixed() {
        let key = build_mono_key("matrix_new", &["f64".into()], &[3, 4]);
        assert_eq!(key.mangled_name(), "matrix_new_f64_N3_N4");
    }

    // ── K1.4: Type-level arithmetic ──

    #[test]
    fn k1_4_nat_expr_literal() {
        let nat = parse_nat_expr("42");
        assert_eq!(nat, NatValue::Literal(42));
    }

    #[test]
    fn k1_4_nat_expr_param() {
        let nat = parse_nat_expr("N");
        assert_eq!(nat, NatValue::Param("N".to_string()));
    }

    #[test]
    fn k1_4_nat_expr_add() {
        let nat = parse_nat_expr("N + 1");
        let mut env = HashMap::new();
        env.insert("N".to_string(), 5);
        assert_eq!(eval_nat(&nat, &env), Some(6));
    }

    #[test]
    fn k1_4_nat_expr_mul() {
        let nat = parse_nat_expr("R * C");
        let mut env = HashMap::new();
        env.insert("R".to_string(), 3);
        env.insert("C".to_string(), 4);
        assert_eq!(eval_nat(&nat, &env), Some(12));
    }

    // ── K1.5: Const parameter bounds ──

    #[test]
    fn k1_5_constraint_gt_passes() {
        let constraints = vec![constraint_gt("N", 0)];
        let mut env = HashMap::new();
        env.insert("N".to_string(), 3);
        let errors = check_constraints(&constraints, &env);
        assert!(errors.is_empty());
    }

    #[test]
    fn k1_5_constraint_gt_fails() {
        let constraints = vec![constraint_gt("N", 0)];
        let mut env = HashMap::new();
        env.insert("N".to_string(), 0);
        let errors = check_constraints(&constraints, &env);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("violated"));
    }

    #[test]
    fn k1_5_constraint_equality() {
        let c = constraint_eq(NatValue::Param("A".into()), NatValue::Param("B".into()));
        let mut env = HashMap::new();
        env.insert("A".to_string(), 4);
        env.insert("B".to_string(), 4);
        assert!(c.check(&env).is_ok());

        env.insert("B".to_string(), 5);
        assert!(c.check(&env).is_err());
    }

    // ── K1.6: Const parameter inference ──

    #[test]
    fn k1_6_infer_from_array_size() {
        let env = infer_const_from_array_size(5, "N");
        assert_eq!(env.get("N"), Some(&5));
    }

    #[test]
    fn k1_6_infer_const_args_explicit() {
        let params = vec!["R".to_string(), "C".to_string()];
        let args = vec![Some(3), Some(4)];
        let env = infer_const_args(&params, &args);
        assert_eq!(env.get("R"), Some(&3));
        assert_eq!(env.get("C"), Some(&4));
    }

    // ── K1.7: Const params in structs ──

    #[test]
    fn k1_7_struct_spec_mangled_name() {
        let spec = ConstStructSpec {
            name: "Matrix".to_string(),
            type_args: vec!["f64".to_string()],
            const_args: vec![3, 4],
        };
        assert_eq!(spec.mangled_name(), "Matrix_f64_N3_N4");
    }

    #[test]
    fn k1_7_parse_struct_with_const_generic() {
        let source = "struct Matrix<T, const R: usize, const C: usize> { rows: i64 }";
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();

        let sdef = match &program.items[0] {
            crate::parser::ast::Item::StructDef(s) => s,
            _ => panic!("expected StructDef"),
        };

        assert_eq!(sdef.generic_params.len(), 3);
        assert!(!sdef.generic_params[0].is_comptime); // T
        assert!(sdef.generic_params[1].is_comptime); // const R
        assert!(sdef.generic_params[2].is_comptime); // const C
        assert_eq!(sdef.generic_params[1].name, "R");
        assert_eq!(sdef.generic_params[2].name, "C");
    }

    // ── K1.8: Const params in enums ──

    #[test]
    fn k1_8_enum_spec_mangled_name() {
        let spec = ConstEnumSpec {
            name: "SmallVec".to_string(),
            type_args: vec!["i32".to_string()],
            const_args: vec![8],
        };
        assert_eq!(spec.mangled_name(), "SmallVec_i32_N8");
    }

    #[test]
    fn k1_8_parse_enum_with_const_generic() {
        let source = "enum SmallVec<T, const N: usize> { Inline(T), Heap(T) }";
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();

        let edef = match &program.items[0] {
            crate::parser::ast::Item::EnumDef(e) => e,
            _ => panic!("expected EnumDef"),
        };

        assert_eq!(edef.generic_params.len(), 2);
        assert!(!edef.generic_params[0].is_comptime); // T
        assert!(edef.generic_params[1].is_comptime); // const N
        assert_eq!(edef.generic_params[1].name, "N");
        assert_eq!(
            edef.generic_params[1].const_type,
            Some("usize".to_string())
        );
    }

    // ── K1.9: Const params in impl blocks ──

    #[test]
    fn k1_9_impl_target_matches_square_matrix() {
        let target = ConstImplTarget {
            target_name: "Matrix".to_string(),
            const_params: vec!["N".to_string(), "N".to_string()],
            type_params: vec!["T".to_string()],
            constraints: vec![],
        };

        // Square matrix: Matrix<f64, 3, 3> should match
        let square = ConstStructSpec {
            name: "Matrix".to_string(),
            type_args: vec!["f64".to_string()],
            const_args: vec![3, 3],
        };
        assert!(target.matches_spec(&square));

        // Non-square: Matrix<f64, 3, 4> should NOT match (N=3, then N=4 conflict)
        let rect = ConstStructSpec {
            name: "Matrix".to_string(),
            type_args: vec!["f64".to_string()],
            const_args: vec![3, 4],
        };
        assert!(!target.matches_spec(&rect));
    }

    #[test]
    fn k1_9_impl_target_with_constraint() {
        let target = ConstImplTarget {
            target_name: "Buffer".to_string(),
            const_params: vec!["N".to_string()],
            type_params: vec![],
            constraints: vec![constraint_gt("N", 0)],
        };

        // Buffer<3> — N > 0, passes
        let valid = ConstStructSpec {
            name: "Buffer".to_string(),
            type_args: vec![],
            const_args: vec![3],
        };
        assert!(target.matches_spec(&valid));

        // Buffer<0> — N > 0, fails
        let invalid = ConstStructSpec {
            name: "Buffer".to_string(),
            type_args: vec![],
            const_args: vec![0],
        };
        assert!(!target.matches_spec(&invalid));
    }

    #[test]
    fn k1_9_parse_impl_with_const_generic() {
        // Current impl syntax: `impl<const N: usize> Matrix { ... }`
        // (target type doesn't take generic args in current parser)
        let source = r#"
impl<const N: usize> Matrix {
    fn size() -> i64 { 0 }
}
"#;
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();

        let impl_block = match &program.items[0] {
            crate::parser::ast::Item::ImplBlock(i) => i,
            _ => panic!("expected ImplBlock"),
        };

        assert_eq!(impl_block.generic_params.len(), 1);
        assert!(impl_block.generic_params[0].is_comptime);
        assert_eq!(impl_block.generic_params[0].name, "N");
        assert_eq!(
            impl_block.generic_params[0].const_type,
            Some("usize".to_string())
        );
    }

    // ── K1.10: Integration test — full pipeline parse + classify ──

    #[test]
    fn k1_10_full_pipeline_const_generic_function() {
        let source = "fn repeat<T, const N: usize>(val: T) -> T { val }";
        let tokens = crate::lexer::tokenize(source).unwrap();
        let program = crate::parser::parse(tokens).unwrap();

        let fndef = match &program.items[0] {
            crate::parser::ast::Item::FnDef(f) => f,
            _ => panic!("expected FnDef"),
        };

        // Check generic params
        assert_eq!(fndef.generic_params.len(), 2);

        // T is a type param
        let t_param = &fndef.generic_params[0];
        assert_eq!(t_param.name, "T");
        assert!(!t_param.is_comptime);
        assert_eq!(classify_param(t_param), ParamKind::Type);

        // N is a const param
        let n_param = &fndef.generic_params[1];
        assert_eq!(n_param.name, "N");
        assert!(n_param.is_comptime);
        assert_eq!(
            classify_param(n_param),
            ParamKind::Const {
                const_type: ConstType::Usize
            }
        );

        // Build mono key for repeat<i64, 5>
        let key = build_mono_key("repeat", &["i64".into()], &[5]);
        assert_eq!(key.mangled_name(), "repeat_i64_N5");

        // Test const param extraction
        assert_eq!(type_param_names(&fndef.generic_params), vec!["T"]);
        assert_eq!(const_param_names(&fndef.generic_params), vec!["N"]);
    }

    #[test]
    fn k1_10_full_pipeline_nat_arithmetic_evaluation() {
        // Simulate: struct Matrix<T, const R: usize, const C: usize> with field [T; R * C]
        let size_expr = NatValue::Mul(
            Box::new(NatValue::Param("R".to_string())),
            Box::new(NatValue::Param("C".to_string())),
        );

        // Specialize for Matrix<f64, 3, 4>
        let mut env = HashMap::new();
        env.insert("R".to_string(), 3);
        env.insert("C".to_string(), 4);

        let concrete_size = eval_nat(&size_expr, &env);
        assert_eq!(concrete_size, Some(12)); // 3 * 4 = 12

        // Verify substitution
        let substituted = size_expr.substitute(&env);
        assert_eq!(substituted, NatValue::Literal(12));
    }

    #[test]
    fn k1_10_full_pipeline_constraints_checked() {
        // fn safe_div<const N: usize>(arr: [f64; N]) where N > 0
        let constraints = vec![constraint_gt("N", 0), constraint_ge("N", 1)];

        // Valid: N = 5
        let mut env = HashMap::new();
        env.insert("N".to_string(), 5);
        assert!(check_constraints(&constraints, &env).is_empty());

        // Invalid: N = 0
        env.insert("N".to_string(), 0);
        let errors = check_constraints(&constraints, &env);
        assert_eq!(errors.len(), 2); // both GT and GE fail
    }
}
