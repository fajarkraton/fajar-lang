//! Const trait system — traits whose methods can be evaluated at compile time.
//!
//! # Overview
//!
//! A `const trait` guarantees all its methods are const-evaluable. Types that
//! implement a const trait via `const impl` can have their trait methods called
//! in `comptime { ... }` blocks and `const fn` bodies.
//!
//! # Syntax
//!
//! ```fajar
//! const trait ConstAdd {
//!     const fn add(self, other: Self) -> Self;
//! }
//!
//! const impl ConstAdd for i32 {
//!     const fn add(self, other: Self) -> Self { self + other }
//! }
//!
//! const fn sum<T: const ConstAdd>(a: T, b: T) -> T { a.add(b) }
//! ```

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// K3.1: Const Trait Definition
// ═══════════════════════════════════════════════════════════════════════

/// A const trait definition — all methods must be const-evaluable.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstTraitDef {
    /// Trait name.
    pub name: String,
    /// Method signatures (name, param types, return type).
    pub methods: Vec<ConstMethodSig>,
    /// Generic type parameters on the trait itself.
    pub type_params: Vec<String>,
    /// K3.9: Associated types.
    pub associated_types: Vec<ConstAssociatedType>,
}

/// A method signature in a const trait.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstMethodSig {
    /// Method name.
    pub name: String,
    /// Parameter names (excluding `self`).
    pub param_names: Vec<String>,
    /// Parameter type names (excluding `self`).
    pub param_types: Vec<String>,
    /// Return type name.
    pub return_type: String,
    /// Whether the method has a default implementation.
    pub has_default: bool,
}

/// K3.9: An associated type in a const trait.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstAssociatedType {
    /// Associated type name (e.g., `Output`).
    pub name: String,
    /// Optional default type.
    pub default: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// K3.2: Const Impl
// ═══════════════════════════════════════════════════════════════════════

/// A const impl — implements a const trait for a concrete type.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstImplDef {
    /// Trait being implemented.
    pub trait_name: String,
    /// Type implementing the trait.
    pub target_type: String,
    /// Method implementations (name → body as string for display).
    pub methods: Vec<ConstMethodImpl>,
    /// K3.9: Associated type bindings.
    pub associated_type_bindings: HashMap<String, String>,
}

/// A method implementation in a const impl.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstMethodImpl {
    /// Method name.
    pub name: String,
    /// Whether validated as const-evaluable.
    pub is_validated: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// K3.3 / K3.8: Const Trait Bounds & Where Clauses
// ═══════════════════════════════════════════════════════════════════════

/// A const bound on a generic parameter: `T: const ConstAdd`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstBound {
    /// The type parameter being constrained.
    pub type_param: String,
    /// The const trait that must be implemented.
    pub trait_name: String,
}

/// K3.8: A compound where clause with multiple const bounds.
/// `where T: const Add + const Mul, U: const Display`
#[derive(Debug, Clone, PartialEq)]
pub struct ConstWhereClause {
    /// All const bounds in the where clause.
    pub bounds: Vec<ConstBound>,
}

impl ConstWhereClause {
    /// Creates a new empty where clause.
    pub fn new() -> Self {
        Self { bounds: Vec::new() }
    }

    /// Adds a const bound.
    pub fn add_bound(&mut self, type_param: &str, trait_name: &str) {
        self.bounds.push(ConstBound {
            type_param: type_param.to_string(),
            trait_name: trait_name.to_string(),
        });
    }

    /// Checks if a type parameter has a specific const bound.
    pub fn has_bound(&self, type_param: &str, trait_name: &str) -> bool {
        self.bounds
            .iter()
            .any(|b| b.type_param == type_param && b.trait_name == trait_name)
    }

    /// Gets all const trait bounds for a specific type parameter.
    pub fn bounds_for(&self, type_param: &str) -> Vec<&str> {
        self.bounds
            .iter()
            .filter(|b| b.type_param == type_param)
            .map(|b| b.trait_name.as_str())
            .collect()
    }
}

impl Default for ConstWhereClause {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K3.4 / K3.5: Const Trait Registry & Dispatch
// ═══════════════════════════════════════════════════════════════════════

/// Registry of all const traits and their implementations.
#[derive(Debug, Clone, Default)]
pub struct ConstTraitRegistry {
    /// Registered const traits: trait name → definition.
    pub traits: HashMap<String, ConstTraitDef>,
    /// Registered const impls: (trait_name, target_type) → impl.
    pub impls: HashMap<(String, String), ConstImplDef>,
    /// K3.6: Derived const trait impls (auto-generated).
    pub derived: HashMap<(String, String), Vec<String>>,
}

impl ConstTraitRegistry {
    /// Creates a new registry with built-in const traits.
    pub fn new() -> Self {
        let mut reg = Self::default();
        reg.register_builtins();
        reg
    }

    /// Registers a const trait definition.
    pub fn register_trait(&mut self, def: ConstTraitDef) {
        self.traits.insert(def.name.clone(), def);
    }

    /// Registers a const impl.
    pub fn register_impl(&mut self, imp: ConstImplDef) {
        self.impls
            .insert((imp.trait_name.clone(), imp.target_type.clone()), imp);
    }

    /// Checks if a type implements a const trait.
    pub fn type_implements(&self, type_name: &str, trait_name: &str) -> bool {
        self.impls
            .contains_key(&(trait_name.to_string(), type_name.to_string()))
    }

    /// K3.4: Dispatch — resolve a const trait method call to its implementation.
    ///
    /// Returns the method impl name if the type implements the const trait.
    pub fn resolve_method(
        &self,
        type_name: &str,
        trait_name: &str,
        method_name: &str,
    ) -> Option<String> {
        let key = (trait_name.to_string(), type_name.to_string());
        if let Some(imp) = self.impls.get(&key) {
            if imp.methods.iter().any(|m| m.name == method_name) {
                // Mangled name: TraitName_TypeName_method
                return Some(format!("{trait_name}_{type_name}_{method_name}"));
            }
        }
        None
    }

    /// K3.3: Validate that all const bounds in a where clause are satisfied.
    ///
    /// Given a mapping of type_param → concrete_type, checks that each concrete
    /// type implements the required const traits.
    pub fn check_bounds(
        &self,
        where_clause: &ConstWhereClause,
        type_bindings: &HashMap<String, String>,
    ) -> Vec<String> {
        let mut errors = Vec::new();
        for bound in &where_clause.bounds {
            if let Some(concrete) = type_bindings.get(&bound.type_param) {
                if !self.type_implements(concrete, &bound.trait_name) {
                    errors.push(format!(
                        "type '{}' does not implement const trait '{}' (required by {} bound on '{}')",
                        concrete, bound.trait_name, bound.trait_name, bound.type_param
                    ));
                }
            }
            // If type param not bound yet, skip (will be checked at instantiation)
        }
        errors
    }

    /// K3.9: Resolve an associated type for a const impl.
    pub fn resolve_associated_type(
        &self,
        type_name: &str,
        trait_name: &str,
        assoc_name: &str,
    ) -> Option<String> {
        let key = (trait_name.to_string(), type_name.to_string());
        self.impls
            .get(&key)
            .and_then(|imp| imp.associated_type_bindings.get(assoc_name).cloned())
    }

    /// K3.5: Register built-in const traits for primitive types.
    fn register_builtins(&mut self) {
        // ConstEq — compile-time equality
        self.register_trait(ConstTraitDef {
            name: "ConstEq".to_string(),
            methods: vec![ConstMethodSig {
                name: "const_eq".to_string(),
                param_names: vec!["other".into()],
                param_types: vec!["Self".into()],
                return_type: "bool".to_string(),
                has_default: false,
            }],
            type_params: vec![],
            associated_types: vec![],
        });

        // ConstOrd — compile-time ordering
        self.register_trait(ConstTraitDef {
            name: "ConstOrd".to_string(),
            methods: vec![ConstMethodSig {
                name: "const_cmp".to_string(),
                param_names: vec!["other".into()],
                param_types: vec!["Self".into()],
                return_type: "i64".to_string(),
                has_default: false,
            }],
            type_params: vec![],
            associated_types: vec![],
        });

        // ConstDefault — compile-time default value
        self.register_trait(ConstTraitDef {
            name: "ConstDefault".to_string(),
            methods: vec![ConstMethodSig {
                name: "const_default".to_string(),
                param_names: vec![],
                param_types: vec![],
                return_type: "Self".to_string(),
                has_default: false,
            }],
            type_params: vec![],
            associated_types: vec![],
        });

        // ConstAdd — compile-time addition
        self.register_trait(ConstTraitDef {
            name: "ConstAdd".to_string(),
            methods: vec![ConstMethodSig {
                name: "const_add".to_string(),
                param_names: vec!["other".into()],
                param_types: vec!["Self".into()],
                return_type: "Self".to_string(),
                has_default: false,
            }],
            type_params: vec![],
            associated_types: vec![ConstAssociatedType {
                name: "Output".to_string(),
                default: Some("Self".to_string()),
            }],
        });

        // ConstMul — compile-time multiplication
        self.register_trait(ConstTraitDef {
            name: "ConstMul".to_string(),
            methods: vec![ConstMethodSig {
                name: "const_mul".to_string(),
                param_names: vec!["other".into()],
                param_types: vec!["Self".into()],
                return_type: "Self".to_string(),
                has_default: false,
            }],
            type_params: vec![],
            associated_types: vec![ConstAssociatedType {
                name: "Output".to_string(),
                default: Some("Self".to_string()),
            }],
        });

        // Register built-in impls for numeric types
        let numeric_types = ["i8", "i16", "i32", "i64", "i128", "isize",
                             "u8", "u16", "u32", "u64", "u128", "usize",
                             "f32", "f64"];

        for ty in &numeric_types {
            // All numeric types implement ConstEq, ConstOrd, ConstDefault, ConstAdd, ConstMul
            for trait_name in &["ConstEq", "ConstOrd", "ConstDefault", "ConstAdd", "ConstMul"] {
                self.register_impl(ConstImplDef {
                    trait_name: trait_name.to_string(),
                    target_type: ty.to_string(),
                    methods: vec![ConstMethodImpl {
                        name: match *trait_name {
                            "ConstEq" => "const_eq",
                            "ConstOrd" => "const_cmp",
                            "ConstDefault" => "const_default",
                            "ConstAdd" => "const_add",
                            "ConstMul" => "const_mul",
                            _ => unreachable!(),
                        }
                        .to_string(),
                        is_validated: true,
                    }],
                    associated_type_bindings: HashMap::new(),
                });
            }
        }

        // bool implements ConstEq, ConstDefault
        for trait_name in &["ConstEq", "ConstDefault"] {
            self.register_impl(ConstImplDef {
                trait_name: trait_name.to_string(),
                target_type: "bool".to_string(),
                methods: vec![ConstMethodImpl {
                    name: match *trait_name {
                        "ConstEq" => "const_eq",
                        "ConstDefault" => "const_default",
                        _ => unreachable!(),
                    }
                    .to_string(),
                    is_validated: true,
                }],
                associated_type_bindings: HashMap::new(),
            });
        }

        // str implements ConstEq
        self.register_impl(ConstImplDef {
            trait_name: "ConstEq".to_string(),
            target_type: "str".to_string(),
            methods: vec![ConstMethodImpl {
                name: "const_eq".to_string(),
                is_validated: true,
            }],
            associated_type_bindings: HashMap::new(),
        });
    }

    // K3.6: Derive const traits

    /// Auto-derives a const trait for a struct by checking all fields implement it.
    ///
    /// Returns `Ok(())` if derivable, or error message if not.
    pub fn derive_for_struct(
        &mut self,
        struct_name: &str,
        trait_name: &str,
        field_types: &[&str],
    ) -> Result<(), String> {
        // Check all fields implement the trait
        for ft in field_types {
            if !self.type_implements(ft, trait_name) {
                return Err(format!(
                    "cannot derive '{trait_name}' for '{struct_name}': field type '{ft}' \
                     does not implement '{trait_name}'"
                ));
            }
        }

        // Register the derived impl
        let trait_def = self
            .traits
            .get(trait_name)
            .ok_or_else(|| format!("unknown const trait '{trait_name}'"))?;

        let methods: Vec<ConstMethodImpl> = trait_def
            .methods
            .iter()
            .map(|m| ConstMethodImpl {
                name: m.name.clone(),
                is_validated: true,
            })
            .collect();

        let method_names: Vec<String> = methods.iter().map(|m| m.name.clone()).collect();

        self.register_impl(ConstImplDef {
            trait_name: trait_name.to_string(),
            target_type: struct_name.to_string(),
            methods,
            associated_type_bindings: HashMap::new(),
        });

        self.derived
            .entry((trait_name.to_string(), struct_name.to_string()))
            .or_default()
            .extend(method_names);

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// K3.7: Const Trait Objects
// ═══════════════════════════════════════════════════════════════════════

/// A const trait object — `&dyn const ConstAdd` for compile-time polymorphism.
///
/// Since const evaluation is always monomorphized, const trait objects are
/// represented as a tagged union of (type_name, value) at compile time.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstTraitObject {
    /// The const trait this object satisfies.
    pub trait_name: String,
    /// The concrete type name.
    pub concrete_type: String,
}

impl ConstTraitObject {
    /// Creates a new const trait object after verifying the type implements the trait.
    pub fn new(
        registry: &ConstTraitRegistry,
        trait_name: &str,
        concrete_type: &str,
    ) -> Result<Self, String> {
        if registry.type_implements(concrete_type, trait_name) {
            Ok(Self {
                trait_name: trait_name.to_string(),
                concrete_type: concrete_type.to_string(),
            })
        } else {
            Err(format!(
                "type '{concrete_type}' does not implement const trait '{trait_name}'"
            ))
        }
    }

    /// Dispatches a method call on this const trait object.
    pub fn dispatch_method(
        &self,
        registry: &ConstTraitRegistry,
        method_name: &str,
    ) -> Option<String> {
        registry.resolve_method(&self.concrete_type, &self.trait_name, method_name)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — K3.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── K3.1: Const trait definition ──

    #[test]
    fn k3_1_define_const_trait() {
        let trait_def = ConstTraitDef {
            name: "ConstHash".to_string(),
            methods: vec![ConstMethodSig {
                name: "const_hash".to_string(),
                param_names: vec![],
                param_types: vec![],
                return_type: "u64".to_string(),
                has_default: false,
            }],
            type_params: vec![],
            associated_types: vec![],
        };
        assert_eq!(trait_def.name, "ConstHash");
        assert_eq!(trait_def.methods.len(), 1);
        assert_eq!(trait_def.methods[0].name, "const_hash");
    }

    // ── K3.2: Const impl ──

    #[test]
    fn k3_2_register_const_impl() {
        let mut reg = ConstTraitRegistry::new();
        reg.register_trait(ConstTraitDef {
            name: "ConstHash".to_string(),
            methods: vec![ConstMethodSig {
                name: "const_hash".to_string(),
                param_names: vec![],
                param_types: vec![],
                return_type: "u64".to_string(),
                has_default: false,
            }],
            type_params: vec![],
            associated_types: vec![],
        });
        reg.register_impl(ConstImplDef {
            trait_name: "ConstHash".to_string(),
            target_type: "Point".to_string(),
            methods: vec![ConstMethodImpl {
                name: "const_hash".to_string(),
                is_validated: true,
            }],
            associated_type_bindings: HashMap::new(),
        });
        assert!(reg.type_implements("Point", "ConstHash"));
        assert!(!reg.type_implements("Point", "ConstEq"));
    }

    // ── K3.3: Const trait bounds ──

    #[test]
    fn k3_3_check_bounds_satisfied() {
        let reg = ConstTraitRegistry::new();
        let mut wc = ConstWhereClause::new();
        wc.add_bound("T", "ConstAdd");

        let mut bindings = HashMap::new();
        bindings.insert("T".to_string(), "i64".to_string());

        let errors = reg.check_bounds(&wc, &bindings);
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
    }

    #[test]
    fn k3_3_check_bounds_violated() {
        let reg = ConstTraitRegistry::new();
        let mut wc = ConstWhereClause::new();
        wc.add_bound("T", "ConstAdd");

        let mut bindings = HashMap::new();
        bindings.insert("T".to_string(), "SomeUnknownType".to_string());

        let errors = reg.check_bounds(&wc, &bindings);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("does not implement"));
    }

    // ── K3.4: Const trait method dispatch ──

    #[test]
    fn k3_4_dispatch_method() {
        let reg = ConstTraitRegistry::new();
        let resolved = reg.resolve_method("i64", "ConstAdd", "const_add");
        assert_eq!(resolved, Some("ConstAdd_i64_const_add".to_string()));
    }

    #[test]
    fn k3_4_dispatch_unknown_method() {
        let reg = ConstTraitRegistry::new();
        let resolved = reg.resolve_method("i64", "ConstAdd", "nonexistent");
        assert_eq!(resolved, None);
    }

    // ── K3.5: Built-in const traits ──

    #[test]
    fn k3_5_builtins_registered() {
        let reg = ConstTraitRegistry::new();

        // Traits exist
        assert!(reg.traits.contains_key("ConstEq"));
        assert!(reg.traits.contains_key("ConstOrd"));
        assert!(reg.traits.contains_key("ConstDefault"));
        assert!(reg.traits.contains_key("ConstAdd"));
        assert!(reg.traits.contains_key("ConstMul"));

        // Numeric types implement them
        assert!(reg.type_implements("i64", "ConstEq"));
        assert!(reg.type_implements("i64", "ConstOrd"));
        assert!(reg.type_implements("i64", "ConstDefault"));
        assert!(reg.type_implements("i64", "ConstAdd"));
        assert!(reg.type_implements("f64", "ConstMul"));
        assert!(reg.type_implements("u32", "ConstAdd"));

        // bool implements ConstEq and ConstDefault
        assert!(reg.type_implements("bool", "ConstEq"));
        assert!(reg.type_implements("bool", "ConstDefault"));
        assert!(!reg.type_implements("bool", "ConstAdd"));

        // str implements ConstEq
        assert!(reg.type_implements("str", "ConstEq"));
    }

    // ── K3.6: Derive const traits ──

    #[test]
    fn k3_6_derive_const_eq_for_struct() {
        let mut reg = ConstTraitRegistry::new();
        // Point { x: i64, y: i64 } — both fields implement ConstEq
        let result = reg.derive_for_struct("Point", "ConstEq", &["i64", "i64"]);
        assert!(result.is_ok());
        assert!(reg.type_implements("Point", "ConstEq"));
    }

    #[test]
    fn k3_6_derive_fails_if_field_missing_impl() {
        let mut reg = ConstTraitRegistry::new();
        // MyStruct { data: SomeType } — SomeType doesn't implement ConstEq
        let result = reg.derive_for_struct("MyStruct", "ConstEq", &["SomeUnknownType"]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not implement"));
    }

    // ── K3.7: Const trait objects ──

    #[test]
    fn k3_7_const_trait_object_creation() {
        let reg = ConstTraitRegistry::new();
        let obj = ConstTraitObject::new(&reg, "ConstAdd", "i64");
        assert!(obj.is_ok());
        let obj = obj.unwrap();
        assert_eq!(obj.trait_name, "ConstAdd");
        assert_eq!(obj.concrete_type, "i64");
    }

    #[test]
    fn k3_7_const_trait_object_dispatch() {
        let reg = ConstTraitRegistry::new();
        let obj = ConstTraitObject::new(&reg, "ConstAdd", "i64").unwrap();
        let resolved = obj.dispatch_method(&reg, "const_add");
        assert_eq!(resolved, Some("ConstAdd_i64_const_add".to_string()));
    }

    #[test]
    fn k3_7_const_trait_object_invalid_type() {
        let reg = ConstTraitRegistry::new();
        let obj = ConstTraitObject::new(&reg, "ConstAdd", "bool");
        assert!(obj.is_err());
    }

    // ── K3.8: Const where clauses ──

    #[test]
    fn k3_8_compound_where_clause() {
        let reg = ConstTraitRegistry::new();
        let mut wc = ConstWhereClause::new();
        wc.add_bound("T", "ConstAdd");
        wc.add_bound("T", "ConstMul");
        wc.add_bound("U", "ConstEq");

        // Check T bounds
        assert!(wc.has_bound("T", "ConstAdd"));
        assert!(wc.has_bound("T", "ConstMul"));
        assert!(!wc.has_bound("T", "ConstEq"));

        // Check U bounds
        let u_bounds = wc.bounds_for("U");
        assert_eq!(u_bounds, vec!["ConstEq"]);

        // Validate with i64 (implements all), bool for U
        let mut bindings = HashMap::new();
        bindings.insert("T".to_string(), "i64".to_string());
        bindings.insert("U".to_string(), "bool".to_string());
        let errors = reg.check_bounds(&wc, &bindings);
        assert!(errors.is_empty());
    }

    // ── K3.9: Const associated types ──

    #[test]
    fn k3_9_associated_type_in_trait() {
        let trait_def = ConstTraitDef {
            name: "ConstAdd".to_string(),
            methods: vec![ConstMethodSig {
                name: "const_add".to_string(),
                param_names: vec!["other".into()],
                param_types: vec!["Self".into()],
                return_type: "Output".to_string(),
                has_default: false,
            }],
            type_params: vec![],
            associated_types: vec![ConstAssociatedType {
                name: "Output".to_string(),
                default: Some("Self".to_string()),
            }],
        };
        assert_eq!(trait_def.associated_types.len(), 1);
        assert_eq!(trait_def.associated_types[0].name, "Output");
    }

    #[test]
    fn k3_9_resolve_associated_type() {
        let mut reg = ConstTraitRegistry::new();
        // Register impl with associated type binding
        let mut bindings = HashMap::new();
        bindings.insert("Output".to_string(), "i128".to_string());
        reg.register_impl(ConstImplDef {
            trait_name: "ConstAdd".to_string(),
            target_type: "BigNum".to_string(),
            methods: vec![ConstMethodImpl {
                name: "const_add".to_string(),
                is_validated: true,
            }],
            associated_type_bindings: bindings,
        });

        let resolved = reg.resolve_associated_type("BigNum", "ConstAdd", "Output");
        assert_eq!(resolved, Some("i128".to_string()));
    }

    // ── K3.10: Integration tests ──

    #[test]
    fn k3_10_full_pipeline_const_trait_system() {
        let mut reg = ConstTraitRegistry::new();

        // Define a custom const trait
        reg.register_trait(ConstTraitDef {
            name: "ConstDisplay".to_string(),
            methods: vec![ConstMethodSig {
                name: "const_to_string".to_string(),
                param_names: vec![],
                param_types: vec![],
                return_type: "str".to_string(),
                has_default: false,
            }],
            type_params: vec![],
            associated_types: vec![],
        });

        // Implement it for i64
        reg.register_impl(ConstImplDef {
            trait_name: "ConstDisplay".to_string(),
            target_type: "i64".to_string(),
            methods: vec![ConstMethodImpl {
                name: "const_to_string".to_string(),
                is_validated: true,
            }],
            associated_type_bindings: HashMap::new(),
        });

        // Check where clause: T: const ConstAdd + const ConstDisplay
        let mut wc = ConstWhereClause::new();
        wc.add_bound("T", "ConstAdd");
        wc.add_bound("T", "ConstDisplay");

        let mut bindings = HashMap::new();
        bindings.insert("T".to_string(), "i64".to_string());
        let errors = reg.check_bounds(&wc, &bindings);
        assert!(errors.is_empty());

        // Dispatch method
        let resolved = reg.resolve_method("i64", "ConstDisplay", "const_to_string");
        assert!(resolved.is_some());

        // Create trait object
        let obj = ConstTraitObject::new(&reg, "ConstDisplay", "i64").unwrap();
        assert_eq!(obj.concrete_type, "i64");
    }

    #[test]
    fn k3_10_derive_chain() {
        let mut reg = ConstTraitRegistry::new();

        // Derive ConstEq for Point (fields: i64, i64)
        reg.derive_for_struct("Point", "ConstEq", &["i64", "i64"])
            .unwrap();

        // Now derive ConstEq for Line (fields: Point, Point)
        reg.derive_for_struct("Line", "ConstEq", &["Point", "Point"])
            .unwrap();

        assert!(reg.type_implements("Line", "ConstEq"));
    }
}
