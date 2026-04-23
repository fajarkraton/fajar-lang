//! Sprint S3: Semantic Analyzer — Type checker, scope management, type inference,
//! generic instantiation, trait resolution, import resolution, mutability checking,
//! and error collection for the self-hosted compiler AST.

use std::collections::HashMap;
use std::fmt;

use super::ast_tree::{AstProgram, AstSpan, BinOp, Expr, FnDefNode, Item, Stmt, TypeExpr, UnaryOp};

// ═══════════════════════════════════════════════════════════════════════
// S3.1: Type Representation
// ═══════════════════════════════════════════════════════════════════════

/// Internal type representation for the self-hosted analyzer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    /// Primitive integer types (bit width).
    Int(u8),
    UInt(u8),
    /// Floating-point types (bit width).
    Float(u8),
    Bool,
    Char,
    Str,
    Void,
    Never,
    /// Named type (struct, enum, alias).
    Named(String),
    /// Generic type parameter.
    Param(String),
    /// Instantiated generic: `Vec<i32>`.
    Generic(String, Vec<Type>),
    /// Function type.
    Fn {
        params: Vec<Type>,
        ret: Box<Type>,
    },
    /// Array type.
    Array(Box<Type>, Option<usize>),
    /// Reference type.
    Ref {
        inner: Box<Type>,
        mutable: bool,
    },
    /// Tuple type.
    Tuple(Vec<Type>),
    /// Unknown (inference variable).
    Infer(u32),
    /// Error sentinel.
    Error,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int(w) => write!(f, "i{w}"),
            Type::UInt(w) => write!(f, "u{w}"),
            Type::Float(w) => write!(f, "f{w}"),
            Type::Bool => write!(f, "bool"),
            Type::Char => write!(f, "char"),
            Type::Str => write!(f, "str"),
            Type::Void => write!(f, "void"),
            Type::Never => write!(f, "never"),
            Type::Named(n) => write!(f, "{n}"),
            Type::Param(n) => write!(f, "{n}"),
            Type::Generic(n, args) => {
                let a: Vec<String> = args.iter().map(|t| t.to_string()).collect();
                write!(f, "{n}<{}>", a.join(", "))
            }
            Type::Fn { params, ret } => {
                let p: Vec<String> = params.iter().map(|t| t.to_string()).collect();
                write!(f, "fn({}) -> {ret}", p.join(", "))
            }
            Type::Array(inner, len) => {
                if let Some(n) = len {
                    write!(f, "[{inner}; {n}]")
                } else {
                    write!(f, "[{inner}]")
                }
            }
            Type::Ref { inner, mutable } => {
                if *mutable {
                    write!(f, "&mut {inner}")
                } else {
                    write!(f, "&{inner}")
                }
            }
            Type::Tuple(elems) => {
                let e: Vec<String> = elems.iter().map(|t| t.to_string()).collect();
                write!(f, "({})", e.join(", "))
            }
            Type::Infer(id) => write!(f, "?{id}"),
            Type::Error => write!(f, "<error>"),
        }
    }
}

/// Resolves a TypeExpr syntax node into an internal Type.
pub fn resolve_type_expr(te: &TypeExpr) -> Type {
    match te {
        TypeExpr::Name(name, _) => match name.as_str() {
            "i8" => Type::Int(8),
            "i16" => Type::Int(16),
            "i32" => Type::Int(32),
            "i64" => Type::Int(64),
            "i128" => Type::Int(128),
            "u8" => Type::UInt(8),
            "u16" => Type::UInt(16),
            "u32" => Type::UInt(32),
            "u64" => Type::UInt(64),
            "u128" => Type::UInt(128),
            "f32" => Type::Float(32),
            "f64" => Type::Float(64),
            "bool" => Type::Bool,
            "char" => Type::Char,
            "str" => Type::Str,
            "void" => Type::Void,
            _ => Type::Named(name.clone()),
        },
        TypeExpr::Generic(name, params, _) => {
            Type::Generic(name.clone(), params.iter().map(resolve_type_expr).collect())
        }
        TypeExpr::Array(inner, len, _) => Type::Array(Box::new(resolve_type_expr(inner)), *len),
        TypeExpr::Ref(inner, mutable, _) => Type::Ref {
            inner: Box::new(resolve_type_expr(inner)),
            mutable: *mutable,
        },
        TypeExpr::Fn(params, ret, _) => Type::Fn {
            params: params.iter().map(resolve_type_expr).collect(),
            ret: Box::new(resolve_type_expr(ret)),
        },
        TypeExpr::Tuple(elems, _) => Type::Tuple(elems.iter().map(resolve_type_expr).collect()),
        TypeExpr::Ptr(inner, _, _) => Type::Ref {
            inner: Box::new(resolve_type_expr(inner)),
            mutable: true,
        },
        TypeExpr::Never(_) => Type::Never,
        TypeExpr::Inferred(_) => Type::Infer(0),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.2: Semantic Error
// ═══════════════════════════════════════════════════════════════════════

/// A semantic analysis error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticError {
    /// Error code (SE001-SE012).
    pub code: String,
    /// Error message.
    pub message: String,
    /// Source span.
    pub span: AstSpan,
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] at {}: {}", self.code, self.span, self.message)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.3: Scope Management
// ═══════════════════════════════════════════════════════════════════════

/// Kind of scope for contextual checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Global,
    Function,
    Block,
    Loop,
    Impl,
    Module,
}

/// A variable binding in scope.
#[derive(Debug, Clone)]
pub struct Binding {
    /// Variable name.
    pub name: String,
    /// Variable type.
    pub ty: Type,
    /// Whether mutable.
    pub mutable: bool,
    /// Whether this binding has been used.
    pub used: bool,
    /// Definition span.
    pub span: AstSpan,
}

/// A single scope layer.
#[derive(Debug, Clone)]
pub struct ScopeLayer {
    /// Scope kind.
    pub kind: ScopeKind,
    /// Variable bindings.
    bindings: HashMap<String, Binding>,
}

/// Scope stack for nested scope management.
#[derive(Debug, Clone)]
pub struct ScopeStack {
    layers: Vec<ScopeLayer>,
}

impl ScopeStack {
    /// Creates a new scope stack with a global scope.
    pub fn new() -> Self {
        Self {
            layers: vec![ScopeLayer {
                kind: ScopeKind::Global,
                bindings: HashMap::new(),
            }],
        }
    }

    /// Pushes a new scope.
    pub fn enter(&mut self, kind: ScopeKind) {
        self.layers.push(ScopeLayer {
            kind,
            bindings: HashMap::new(),
        });
    }

    /// Pops the current scope, returning unused bindings for warnings.
    pub fn leave(&mut self) -> Vec<Binding> {
        if let Some(layer) = self.layers.pop() {
            layer
                .bindings
                .into_values()
                .filter(|b| !b.used && !b.name.starts_with('_'))
                .collect()
        } else {
            vec![]
        }
    }

    /// Defines a new binding in the current scope.
    pub fn define(&mut self, name: &str, ty: Type, mutable: bool, span: AstSpan) {
        if let Some(layer) = self.layers.last_mut() {
            layer.bindings.insert(
                name.into(),
                Binding {
                    name: name.into(),
                    ty,
                    mutable,
                    used: false,
                    span,
                },
            );
        }
    }

    /// Looks up a binding by name (innermost first).
    pub fn lookup(&self, name: &str) -> Option<&Binding> {
        for layer in self.layers.iter().rev() {
            if let Some(binding) = layer.bindings.get(name) {
                return Some(binding);
            }
        }
        None
    }

    /// Marks a binding as used (innermost first).
    pub fn mark_used(&mut self, name: &str) {
        for layer in self.layers.iter_mut().rev() {
            if let Some(binding) = layer.bindings.get_mut(name) {
                binding.used = true;
                return;
            }
        }
    }

    /// Checks if currently inside a loop scope.
    pub fn in_loop(&self) -> bool {
        self.layers.iter().rev().any(|l| l.kind == ScopeKind::Loop)
    }

    /// Checks if currently inside a function scope.
    pub fn in_function(&self) -> bool {
        self.layers
            .iter()
            .rev()
            .any(|l| l.kind == ScopeKind::Function)
    }

    /// Returns the current scope depth.
    pub fn depth(&self) -> usize {
        self.layers.len()
    }
}

impl Default for ScopeStack {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.4: Type Inference Engine (Hindley-Milner style)
// ═══════════════════════════════════════════════════════════════════════

/// Type inference context with substitution.
#[derive(Debug, Clone)]
pub struct InferenceCtx {
    /// Next inference variable ID.
    next_id: u32,
    /// Substitution: infer var -> resolved type.
    subst: HashMap<u32, Type>,
}

impl InferenceCtx {
    /// Creates a new inference context.
    pub fn new() -> Self {
        Self {
            next_id: 0,
            subst: HashMap::new(),
        }
    }

    /// Generates a fresh inference variable.
    pub fn fresh(&mut self) -> Type {
        let id = self.next_id;
        self.next_id += 1;
        Type::Infer(id)
    }

    /// Binds an inference variable to a type.
    pub fn bind(&mut self, id: u32, ty: Type) {
        self.subst.insert(id, ty);
    }

    /// Resolves a type through the substitution chain.
    pub fn resolve(&self, ty: &Type) -> Type {
        match ty {
            Type::Infer(id) => {
                if let Some(bound) = self.subst.get(id) {
                    self.resolve(bound)
                } else {
                    ty.clone()
                }
            }
            Type::Fn { params, ret } => Type::Fn {
                params: params.iter().map(|p| self.resolve(p)).collect(),
                ret: Box::new(self.resolve(ret)),
            },
            Type::Array(inner, len) => Type::Array(Box::new(self.resolve(inner)), *len),
            Type::Ref { inner, mutable } => Type::Ref {
                inner: Box::new(self.resolve(inner)),
                mutable: *mutable,
            },
            Type::Tuple(elems) => Type::Tuple(elems.iter().map(|e| self.resolve(e)).collect()),
            Type::Generic(name, args) => {
                Type::Generic(name.clone(), args.iter().map(|a| self.resolve(a)).collect())
            }
            _ => ty.clone(),
        }
    }

    /// Unifies two types. Returns an error message on failure.
    pub fn unify(&mut self, a: &Type, b: &Type) -> Result<(), String> {
        let a = self.resolve(a);
        let b = self.resolve(b);

        if a == b {
            return Ok(());
        }

        match (&a, &b) {
            (Type::Infer(id), _) => {
                if self.occurs_check(*id, &b) {
                    return Err(format!("infinite type: ?{id} ~ {b}"));
                }
                self.bind(*id, b);
                Ok(())
            }
            (_, Type::Infer(id)) => {
                if self.occurs_check(*id, &a) {
                    return Err(format!("infinite type: ?{id} ~ {a}"));
                }
                self.bind(*id, a);
                Ok(())
            }
            (Type::Never, _) | (_, Type::Never) => Ok(()), // never unifies with anything
            (Type::Error, _) | (_, Type::Error) => Ok(()), // error propagates silently
            (
                Type::Fn {
                    params: pa,
                    ret: ra,
                },
                Type::Fn {
                    params: pb,
                    ret: rb,
                },
            ) => {
                if pa.len() != pb.len() {
                    return Err(format!(
                        "function arity mismatch: {} vs {}",
                        pa.len(),
                        pb.len()
                    ));
                }
                for (p1, p2) in pa.iter().zip(pb.iter()) {
                    self.unify(p1, p2)?;
                }
                self.unify(ra, rb)
            }
            (Type::Array(a_inner, a_len), Type::Array(b_inner, b_len)) => {
                if a_len != b_len {
                    return Err("array length mismatch".into());
                }
                self.unify(a_inner, b_inner)
            }
            (Type::Tuple(a_elems), Type::Tuple(b_elems)) => {
                if a_elems.len() != b_elems.len() {
                    return Err(format!(
                        "tuple length mismatch: {} vs {}",
                        a_elems.len(),
                        b_elems.len()
                    ));
                }
                for (e1, e2) in a_elems.iter().zip(b_elems.iter()) {
                    self.unify(e1, e2)?;
                }
                Ok(())
            }
            (
                Type::Ref {
                    inner: ai,
                    mutable: am,
                },
                Type::Ref {
                    inner: bi,
                    mutable: bm,
                },
            ) => {
                if am != bm {
                    return Err("mutability mismatch in reference types".into());
                }
                self.unify(ai, bi)
            }
            _ => Err(format!("type mismatch: expected {a}, found {b}")),
        }
    }

    /// Occurs check: prevents infinite types.
    fn occurs_check(&self, id: u32, ty: &Type) -> bool {
        match ty {
            Type::Infer(other_id) => {
                if *other_id == id {
                    return true;
                }
                if let Some(bound) = self.subst.get(other_id) {
                    return self.occurs_check(id, bound);
                }
                false
            }
            Type::Fn { params, ret } => {
                params.iter().any(|p| self.occurs_check(id, p)) || self.occurs_check(id, ret)
            }
            Type::Array(inner, _) | Type::Ref { inner, .. } => self.occurs_check(id, inner),
            Type::Tuple(elems) | Type::Generic(_, elems) => {
                elems.iter().any(|e| self.occurs_check(id, e))
            }
            _ => false,
        }
    }
}

impl Default for InferenceCtx {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.5: Generic Instantiation
// ═══════════════════════════════════════════════════════════════════════

/// Substitution map for generic instantiation.
#[derive(Debug, Clone, Default)]
pub struct GenericSubst {
    /// Type parameter name -> concrete type.
    pub mappings: HashMap<String, Type>,
}

impl GenericSubst {
    /// Creates a new substitution.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a mapping.
    pub fn bind(&mut self, param: &str, ty: Type) {
        self.mappings.insert(param.into(), ty);
    }

    /// Applies the substitution to a type.
    pub fn apply(&self, ty: &Type) -> Type {
        match ty {
            Type::Param(name) => self
                .mappings
                .get(name)
                .cloned()
                .unwrap_or_else(|| ty.clone()),
            Type::Named(name) => self
                .mappings
                .get(name)
                .cloned()
                .unwrap_or_else(|| ty.clone()),
            Type::Fn { params, ret } => Type::Fn {
                params: params.iter().map(|p| self.apply(p)).collect(),
                ret: Box::new(self.apply(ret)),
            },
            Type::Array(inner, len) => Type::Array(Box::new(self.apply(inner)), *len),
            Type::Ref { inner, mutable } => Type::Ref {
                inner: Box::new(self.apply(inner)),
                mutable: *mutable,
            },
            Type::Tuple(elems) => Type::Tuple(elems.iter().map(|e| self.apply(e)).collect()),
            Type::Generic(name, args) => {
                Type::Generic(name.clone(), args.iter().map(|a| self.apply(a)).collect())
            }
            _ => ty.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.6: Trait Resolution
// ═══════════════════════════════════════════════════════════════════════

/// A registered trait definition.
#[derive(Debug, Clone)]
pub struct TraitDef {
    /// Trait name.
    pub name: String,
    /// Required method signatures: (method_name, param_types, return_type).
    pub methods: Vec<(String, Vec<Type>, Type)>,
}

/// A trait implementation.
#[derive(Debug, Clone)]
pub struct TraitImplV2 {
    /// Trait being implemented.
    pub trait_name: String,
    /// Type implementing the trait.
    pub for_type: Type,
    /// Methods provided.
    pub methods: Vec<String>,
}

/// Trait registry for resolution.
#[derive(Debug, Clone, Default)]
pub struct TraitRegistry {
    /// Registered trait definitions.
    traits: HashMap<String, TraitDef>,
    /// Registered implementations.
    impls: Vec<TraitImplV2>,
}

impl TraitRegistry {
    /// Creates a new trait registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a trait definition.
    pub fn define_trait(&mut self, def: TraitDef) {
        self.traits.insert(def.name.clone(), def);
    }

    /// Registers a trait implementation.
    pub fn add_impl(&mut self, impl_: TraitImplV2) {
        self.impls.push(impl_);
    }

    /// Looks up a trait definition.
    pub fn get_trait(&self, name: &str) -> Option<&TraitDef> {
        self.traits.get(name)
    }

    /// Finds an implementation of a trait for a given type.
    pub fn find_impl(&self, trait_name: &str, for_type: &Type) -> Option<&TraitImplV2> {
        self.impls
            .iter()
            .find(|i| i.trait_name == trait_name && i.for_type == *for_type)
    }

    /// Checks if a type implements a trait.
    pub fn implements(&self, trait_name: &str, for_type: &Type) -> bool {
        self.find_impl(trait_name, for_type).is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.7: Import Resolution
// ═══════════════════════════════════════════════════════════════════════

/// A resolved import target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    /// Full qualified path.
    pub path: Vec<String>,
    /// Alias (if any).
    pub alias: Option<String>,
    /// The name this import is accessible as.
    pub local_name: String,
    /// The type of the imported symbol.
    pub kind: ImportKind,
}

/// What kind of symbol is being imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportKind {
    Function,
    Type,
    Module,
    Constant,
}

/// Import resolver.
#[derive(Debug, Clone, Default)]
pub struct ImportResolver {
    /// Known modules and their exports.
    known_modules: HashMap<String, Vec<String>>,
    /// Resolved imports.
    resolved: Vec<ResolvedImport>,
}

impl ImportResolver {
    /// Creates a new import resolver.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a module and its exported symbols.
    pub fn register_module(&mut self, module: &str, exports: Vec<String>) {
        self.known_modules.insert(module.into(), exports);
    }

    /// Resolves an import path.
    pub fn resolve(
        &mut self,
        path: &[String],
        alias: Option<&str>,
    ) -> Result<ResolvedImport, String> {
        if path.is_empty() {
            return Err("empty import path".into());
        }

        let local_name = alias
            .map(String::from)
            .unwrap_or_else(|| path.last().cloned().unwrap_or_default());

        let resolved = ResolvedImport {
            path: path.to_vec(),
            alias: alias.map(String::from),
            local_name,
            kind: ImportKind::Function,
        };

        self.resolved.push(resolved.clone());
        Ok(resolved)
    }

    /// Returns all resolved imports.
    pub fn imports(&self) -> &[ResolvedImport] {
        &self.resolved
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.8: The Analyzer
// ═══════════════════════════════════════════════════════════════════════

/// Semantic analyzer for the self-hosted AST.
pub struct AnalyzerV2 {
    /// Scope stack.
    scopes: ScopeStack,
    /// Type inference context.
    infer: InferenceCtx,
    /// Trait registry (wired in later phases).
    #[allow(dead_code)]
    traits: TraitRegistry,
    /// Import resolver (wired in later phases).
    #[allow(dead_code)]
    imports: ImportResolver,
    /// Collected errors.
    errors: Vec<SemanticError>,
    /// Collected warnings.
    warnings: Vec<SemanticError>,
    /// Known function signatures.
    functions: HashMap<String, (Vec<Type>, Type)>,
    /// Known struct definitions.
    structs: HashMap<String, Vec<(String, Type)>>,
}

impl AnalyzerV2 {
    /// Creates a new analyzer.
    pub fn new() -> Self {
        Self {
            scopes: ScopeStack::new(),
            infer: InferenceCtx::new(),
            traits: TraitRegistry::new(),
            imports: ImportResolver::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
        }
    }

    /// Returns collected errors.
    pub fn errors(&self) -> &[SemanticError] {
        &self.errors
    }

    /// Returns collected warnings.
    pub fn warnings(&self) -> &[SemanticError] {
        &self.warnings
    }

    /// Returns whether analysis passed.
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }

    /// Adds an error.
    fn error(&mut self, code: &str, message: &str, span: AstSpan) {
        self.errors.push(SemanticError {
            code: code.into(),
            message: message.into(),
            span,
        });
    }

    /// Adds a warning.
    fn warn(&mut self, code: &str, message: &str, span: AstSpan) {
        self.warnings.push(SemanticError {
            code: code.into(),
            message: message.into(),
            span,
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // S3.9: Analyze Program
    // ═══════════════════════════════════════════════════════════════════

    /// Analyzes a complete program.
    pub fn analyze(&mut self, program: &AstProgram) {
        // First pass: collect top-level definitions.
        for item in &program.items {
            self.collect_item(item);
        }

        // Second pass: check bodies.
        for item in &program.items {
            self.check_item(item);
        }
    }

    /// First pass: collect top-level names.
    fn collect_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(f) => {
                let params: Vec<Type> = f.params.iter().map(|p| resolve_type_expr(&p.ty)).collect();
                let ret = f
                    .ret_type
                    .as_ref()
                    .map(resolve_type_expr)
                    .unwrap_or(Type::Void);
                self.functions
                    .insert(f.name.clone(), (params.clone(), ret.clone()));
                self.scopes.define(
                    &f.name,
                    Type::Fn {
                        params,
                        ret: Box::new(ret),
                    },
                    false,
                    f.span,
                );
            }
            Item::StructDef(s) => {
                let fields: Vec<(String, Type)> = s
                    .fields
                    .iter()
                    .map(|(name, ty, _)| (name.clone(), resolve_type_expr(ty)))
                    .collect();
                self.structs.insert(s.name.clone(), fields);
                self.scopes
                    .define(&s.name, Type::Named(s.name.clone()), false, s.span);
            }
            _ => {}
        }
    }

    /// Second pass: type-check item bodies.
    fn check_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(f) => self.check_fn(f),
            Item::Stmt(stmt) => {
                self.check_stmt(stmt);
            }
            _ => {}
        }
    }

    /// Type-checks a function body.
    fn check_fn(&mut self, f: &FnDefNode) {
        self.scopes.enter(ScopeKind::Function);

        for param in &f.params {
            let ty = resolve_type_expr(&param.ty);
            self.scopes.define(&param.name, ty, param.mutable, f.span);
        }

        let body_ty = self.check_expr(&f.body);
        let expected_ret = f
            .ret_type
            .as_ref()
            .map(resolve_type_expr)
            .unwrap_or(Type::Void);

        if let Err(msg) = self.infer.unify(&body_ty, &expected_ret) {
            self.error(
                "SE004",
                &format!("function `{}` return type mismatch: {msg}", f.name),
                f.span,
            );
        }

        let unused = self.scopes.leave();
        for binding in unused {
            self.warn(
                "SE009",
                &format!("unused variable: `{}`", binding.name),
                binding.span,
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // S3.10: Statement & Expression Checking
    // ═══════════════════════════════════════════════════════════════════

    /// Type-checks a statement.
    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name,
                mutable,
                ty,
                init,
                span,
            } => {
                let declared_ty = ty.as_ref().map(resolve_type_expr);
                let init_ty = init.as_ref().map(|e| self.check_expr(e));

                let final_ty = match (declared_ty, init_ty) {
                    (Some(decl), Some(init)) => {
                        if let Err(msg) = self.infer.unify(&decl, &init) {
                            self.error("SE004", &format!("let `{name}`: {msg}"), *span);
                        }
                        decl
                    }
                    (Some(decl), None) => decl,
                    (None, Some(init)) => init,
                    (None, None) => {
                        self.error(
                            "SE003",
                            &format!(
                                "cannot infer type for `{name}` without initializer or annotation"
                            ),
                            *span,
                        );
                        Type::Error
                    }
                };

                self.scopes.define(name, final_ty, *mutable, *span);
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                let cond_ty = self.check_expr(condition);
                if let Err(msg) = self.infer.unify(&cond_ty, &Type::Bool) {
                    self.error("SE004", &format!("while condition: {msg}"), *span);
                }
                self.scopes.enter(ScopeKind::Loop);
                self.check_expr(body);
                let unused = self.scopes.leave();
                for b in unused {
                    self.warn("SE009", &format!("unused variable: `{}`", b.name), b.span);
                }
            }
            Stmt::For {
                name,
                iter,
                body,
                span,
            } => {
                let iter_ty = self.check_expr(iter);
                self.scopes.enter(ScopeKind::Loop);
                // Infer element type from array
                let elem_ty = match &iter_ty {
                    Type::Array(inner, _) => (**inner).clone(),
                    _ => self.infer.fresh(),
                };
                self.scopes.define(name, elem_ty, false, *span);
                self.check_expr(body);
                let unused = self.scopes.leave();
                for b in unused {
                    self.warn("SE009", &format!("unused variable: `{}`", b.name), b.span);
                }
            }
            Stmt::Return { value, span } => {
                if !self.scopes.in_function() {
                    self.error("SE006", "return outside of function", *span);
                }
                if let Some(v) = value {
                    self.check_expr(v);
                }
            }
            Stmt::Break { span } if !self.scopes.in_loop() => {
                self.error("SE007", "break outside of loop", *span);
            }
            Stmt::Continue { span } if !self.scopes.in_loop() => {
                self.error("SE007", "continue outside of loop", *span);
            }
            Stmt::ExprStmt { expr, .. } => {
                self.check_expr(expr);
            }
            Stmt::FnDef(f) => self.check_fn(f),
            _ => {}
        }
    }

    /// Type-checks an expression and returns its type.
    fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::IntLit { .. } => Type::Int(64),
            Expr::FloatLit { .. } => Type::Float(64),
            Expr::BoolLit { .. } => Type::Bool,
            Expr::StringLit { .. } => Type::Str,
            Expr::CharLit { .. } => Type::Char,
            Expr::NullLit { .. } => Type::Void,

            Expr::Ident { name, span } => {
                if let Some(binding) = self.scopes.lookup(name) {
                    let ty = binding.ty.clone();
                    self.scopes.mark_used(name);
                    ty
                } else {
                    self.error("SE001", &format!("undefined variable: `{name}`"), *span);
                    Type::Error
                }
            }

            Expr::BinOp {
                op,
                left,
                right,
                span,
            } => {
                let lt = self.check_expr(left);
                let rt = self.check_expr(right);
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow => {
                        if let Err(msg) = self.infer.unify(&lt, &rt) {
                            self.error("SE004", &format!("binary `{op}`: {msg}"), *span);
                        }
                        self.infer.resolve(&lt)
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        if let Err(msg) = self.infer.unify(&lt, &rt) {
                            self.error("SE004", &format!("comparison `{op}`: {msg}"), *span);
                        }
                        Type::Bool
                    }
                    BinOp::And | BinOp::Or => {
                        if let Err(msg) = self.infer.unify(&lt, &Type::Bool) {
                            self.error("SE004", &format!("logical `{op}` left: {msg}"), *span);
                        }
                        if let Err(msg) = self.infer.unify(&rt, &Type::Bool) {
                            self.error("SE004", &format!("logical `{op}` right: {msg}"), *span);
                        }
                        Type::Bool
                    }
                    _ => {
                        if let Err(msg) = self.infer.unify(&lt, &rt) {
                            self.error("SE004", &format!("binary `{op}`: {msg}"), *span);
                        }
                        self.infer.resolve(&lt)
                    }
                }
            }

            Expr::UnaryOp { op, operand, span } => {
                let inner = self.check_expr(operand);
                match op {
                    UnaryOp::Neg => inner,
                    UnaryOp::Not => {
                        if let Err(msg) = self.infer.unify(&inner, &Type::Bool) {
                            self.error("SE004", &format!("unary `!`: {msg}"), *span);
                        }
                        Type::Bool
                    }
                    UnaryOp::Ref => Type::Ref {
                        inner: Box::new(inner),
                        mutable: false,
                    },
                    UnaryOp::RefMut => Type::Ref {
                        inner: Box::new(inner),
                        mutable: true,
                    },
                    _ => inner,
                }
            }

            Expr::Call { callee, args, span } => {
                let callee_ty = self.check_expr(callee);
                let arg_types: Vec<Type> = args.iter().map(|a| self.check_expr(a)).collect();

                match &callee_ty {
                    Type::Fn { params, ret } => {
                        if params.len() != arg_types.len() {
                            self.error(
                                "SE005",
                                &format!(
                                    "expected {} arguments, found {}",
                                    params.len(),
                                    arg_types.len()
                                ),
                                *span,
                            );
                        } else {
                            for (p, a) in params.iter().zip(arg_types.iter()) {
                                if let Err(msg) = self.infer.unify(p, a) {
                                    self.error("SE004", &format!("argument: {msg}"), *span);
                                }
                            }
                        }
                        *ret.clone()
                    }
                    Type::Error => Type::Error,
                    _ => {
                        self.error(
                            "SE005",
                            &format!("cannot call non-function type: {callee_ty}"),
                            *span,
                        );
                        Type::Error
                    }
                }
            }

            Expr::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let cond_ty = self.check_expr(condition);
                if let Err(msg) = self.infer.unify(&cond_ty, &Type::Bool) {
                    self.error("SE004", &format!("if condition: {msg}"), *span);
                }
                let then_ty = self.check_expr(then_branch);
                if let Some(else_expr) = else_branch {
                    let else_ty = self.check_expr(else_expr);
                    if let Err(msg) = self.infer.unify(&then_ty, &else_ty) {
                        self.error(
                            "SE004",
                            &format!("if/else branch type mismatch: {msg}"),
                            *span,
                        );
                    }
                    self.infer.resolve(&then_ty)
                } else {
                    Type::Void
                }
            }

            Expr::Block { stmts, expr, .. } => {
                self.scopes.enter(ScopeKind::Block);
                for stmt in stmts {
                    self.check_stmt(stmt);
                }
                let ty = if let Some(e) = expr {
                    self.check_expr(e)
                } else {
                    Type::Void
                };
                let unused = self.scopes.leave();
                for b in unused {
                    self.warn("SE009", &format!("unused variable: `{}`", b.name), b.span);
                }
                ty
            }

            Expr::ArrayLit { elements, span } => {
                if elements.is_empty() {
                    let elem_ty = self.infer.fresh();
                    Type::Array(Box::new(elem_ty), Some(0))
                } else {
                    let first_ty = self.check_expr(&elements[0]);
                    for elem in &elements[1..] {
                        let elem_ty = self.check_expr(elem);
                        if let Err(msg) = self.infer.unify(&first_ty, &elem_ty) {
                            self.error("SE004", &format!("array element: {msg}"), *span);
                        }
                    }
                    Type::Array(
                        Box::new(self.infer.resolve(&first_ty)),
                        Some(elements.len()),
                    )
                }
            }

            Expr::Assign {
                target,
                value,
                span,
            } => {
                // Check mutability
                if let Expr::Ident { name, .. } = target.as_ref() {
                    if let Some(binding) = self.scopes.lookup(name) {
                        if !binding.mutable {
                            self.error(
                                "SE008",
                                &format!("cannot assign to immutable variable `{name}`"),
                                *span,
                            );
                        }
                    }
                }
                let target_ty = self.check_expr(target);
                let value_ty = self.check_expr(value);
                if let Err(msg) = self.infer.unify(&target_ty, &value_ty) {
                    self.error("SE004", &format!("assignment: {msg}"), *span);
                }
                Type::Void
            }

            Expr::FieldAccess {
                object,
                field,
                span,
            } => {
                let obj_ty = self.check_expr(object);
                match &obj_ty {
                    Type::Named(name) => {
                        if let Some(fields) = self.structs.get(name) {
                            if let Some((_, ty)) = fields.iter().find(|(n, _)| n == field) {
                                ty.clone()
                            } else {
                                self.error(
                                    "SE002",
                                    &format!("no field `{field}` on type `{name}`"),
                                    *span,
                                );
                                Type::Error
                            }
                        } else {
                            self.infer.fresh()
                        }
                    }
                    _ => self.infer.fresh(),
                }
            }

            Expr::Index {
                object,
                index,
                span,
            } => {
                let obj_ty = self.check_expr(object);
                let idx_ty = self.check_expr(index);
                // Index should be an integer
                if !matches!(
                    self.infer.resolve(&idx_ty),
                    Type::Int(_) | Type::UInt(_) | Type::Infer(_)
                ) {
                    self.error("SE004", "array index must be integer", *span);
                }
                match &obj_ty {
                    Type::Array(inner, _) => *inner.clone(),
                    _ => self.infer.fresh(),
                }
            }

            Expr::TupleLit { elements, .. } => {
                let types: Vec<Type> = elements.iter().map(|e| self.check_expr(e)).collect();
                Type::Tuple(types)
            }

            Expr::Cast { expr, ty, .. } => {
                self.check_expr(expr);
                resolve_type_expr(ty)
            }

            Expr::Try { expr, .. } => self.check_expr(expr),

            Expr::Path { segments, .. } => {
                // For now, treat as unresolved
                let name = segments.join("::");
                Type::Named(name)
            }

            Expr::Lambda { params, body, .. } => {
                self.scopes.enter(ScopeKind::Function);
                let param_types: Vec<Type> = params
                    .iter()
                    .map(|p| {
                        let ty =
                            p.ty.as_ref()
                                .map(resolve_type_expr)
                                .unwrap_or_else(|| self.infer.fresh());
                        self.scopes
                            .define(&p.name, ty.clone(), false, AstSpan::dummy());
                        ty
                    })
                    .collect();
                let ret_ty = self.check_expr(body);
                let _unused = self.scopes.leave();
                Type::Fn {
                    params: param_types,
                    ret: Box::new(ret_ty),
                }
            }

            _ => self.infer.fresh(),
        }
    }
}

impl Default for AnalyzerV2 {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selfhost::ast_tree::*;

    fn span() -> AstSpan {
        AstSpan::dummy()
    }

    fn int_expr(v: i64) -> Expr {
        Expr::IntLit {
            value: v,
            span: span(),
        }
    }

    fn ident_expr(name: &str) -> Expr {
        Expr::Ident {
            name: name.into(),
            span: span(),
        }
    }

    // S3.1 — Type representation
    #[test]
    fn s3_1_type_display() {
        assert_eq!(Type::Int(32).to_string(), "i32");
        assert_eq!(Type::Bool.to_string(), "bool");
        assert_eq!(Type::Str.to_string(), "str");
        assert_eq!(Type::Named("Point".into()).to_string(), "Point");
    }

    #[test]
    fn s3_1_resolve_type_expr() {
        let te = TypeExpr::Name("i32".into(), span());
        assert_eq!(resolve_type_expr(&te), Type::Int(32));

        let te2 = TypeExpr::Name("bool".into(), span());
        assert_eq!(resolve_type_expr(&te2), Type::Bool);

        let te3 = TypeExpr::Name("MyStruct".into(), span());
        assert_eq!(resolve_type_expr(&te3), Type::Named("MyStruct".into()));
    }

    // S3.2 — Semantic error
    #[test]
    fn s3_2_error_display() {
        let err = SemanticError {
            code: "SE004".into(),
            message: "type mismatch".into(),
            span: AstSpan::new(0, 5, 1, 1),
        };
        assert!(err.to_string().contains("SE004"));
        assert!(err.to_string().contains("type mismatch"));
    }

    // S3.3 — Scope management
    #[test]
    fn s3_3_scope_define_lookup() {
        let mut scopes = ScopeStack::new();
        scopes.define("x", Type::Int(32), false, span());
        assert!(scopes.lookup("x").is_some());
        assert!(scopes.lookup("y").is_none());
    }

    #[test]
    fn s3_3_scope_nesting() {
        let mut scopes = ScopeStack::new();
        scopes.define("x", Type::Int(32), false, span());
        scopes.enter(ScopeKind::Block);
        scopes.define("y", Type::Bool, false, span());
        assert!(scopes.lookup("x").is_some());
        assert!(scopes.lookup("y").is_some());
        scopes.leave();
        assert!(scopes.lookup("y").is_none());
    }

    #[test]
    fn s3_3_scope_shadowing() {
        let mut scopes = ScopeStack::new();
        scopes.define("x", Type::Int(32), false, span());
        scopes.enter(ScopeKind::Block);
        scopes.define("x", Type::Bool, false, span());
        assert_eq!(scopes.lookup("x").unwrap().ty, Type::Bool);
        scopes.leave();
        assert_eq!(scopes.lookup("x").unwrap().ty, Type::Int(32));
    }

    #[test]
    fn s3_3_in_loop_check() {
        let mut scopes = ScopeStack::new();
        assert!(!scopes.in_loop());
        scopes.enter(ScopeKind::Loop);
        assert!(scopes.in_loop());
        scopes.leave();
        assert!(!scopes.in_loop());
    }

    // S3.4 — Type inference
    #[test]
    fn s3_4_unify_same() {
        let mut ctx = InferenceCtx::new();
        assert!(ctx.unify(&Type::Int(32), &Type::Int(32)).is_ok());
    }

    #[test]
    fn s3_4_unify_infer_var() {
        let mut ctx = InferenceCtx::new();
        let var = ctx.fresh();
        assert!(ctx.unify(&var, &Type::Int(32)).is_ok());
        assert_eq!(ctx.resolve(&var), Type::Int(32));
    }

    #[test]
    fn s3_4_unify_mismatch() {
        let mut ctx = InferenceCtx::new();
        assert!(ctx.unify(&Type::Int(32), &Type::Bool).is_err());
    }

    #[test]
    fn s3_4_unify_functions() {
        let mut ctx = InferenceCtx::new();
        let a = Type::Fn {
            params: vec![Type::Int(32)],
            ret: Box::new(Type::Bool),
        };
        let b = Type::Fn {
            params: vec![Type::Int(32)],
            ret: Box::new(Type::Bool),
        };
        assert!(ctx.unify(&a, &b).is_ok());
    }

    #[test]
    fn s3_4_occurs_check() {
        let mut ctx = InferenceCtx::new();
        let var = ctx.fresh(); // ?0
        let infinite = Type::Array(Box::new(var.clone()), None);
        assert!(ctx.unify(&var, &infinite).is_err());
    }

    // S3.5 — Generic substitution
    #[test]
    fn s3_5_generic_subst() {
        let mut subst = GenericSubst::new();
        subst.bind("T", Type::Int(32));
        assert_eq!(subst.apply(&Type::Param("T".into())), Type::Int(32));
        assert_eq!(subst.apply(&Type::Bool), Type::Bool); // unchanged
    }

    // S3.6 — Trait resolution
    #[test]
    fn s3_6_trait_registry() {
        let mut registry = TraitRegistry::new();
        registry.define_trait(TraitDef {
            name: "Display".into(),
            methods: vec![("fmt".into(), vec![], Type::Str)],
        });
        registry.add_impl(TraitImplV2 {
            trait_name: "Display".into(),
            for_type: Type::Named("Point".into()),
            methods: vec!["fmt".into()],
        });
        assert!(registry.implements("Display", &Type::Named("Point".into())));
        assert!(!registry.implements("Display", &Type::Named("Circle".into())));
    }

    // S3.7 — Import resolution
    #[test]
    fn s3_7_import_resolve() {
        let mut resolver = ImportResolver::new();
        let result = resolver.resolve(&["std".into(), "io".into(), "println".into()], None);
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.local_name, "println");
        assert_eq!(resolver.imports().len(), 1);
    }

    // S3.8 — Analyzer creation
    #[test]
    fn s3_8_analyzer_new() {
        let analyzer = AnalyzerV2::new();
        assert!(analyzer.passed());
        assert!(analyzer.errors().is_empty());
    }

    // S3.9 — Analyze program
    #[test]
    fn s3_9_analyze_empty_program() {
        let mut analyzer = AnalyzerV2::new();
        let prog = AstProgram::new("test.fj", vec![]);
        analyzer.analyze(&prog);
        assert!(analyzer.passed());
    }

    #[test]
    fn s3_9_analyze_undefined_variable() {
        let mut analyzer = AnalyzerV2::new();
        let prog = AstProgram::new(
            "test.fj",
            vec![Item::Stmt(Stmt::ExprStmt {
                expr: Box::new(ident_expr("x")),
                span: span(),
            })],
        );
        analyzer.analyze(&prog);
        assert!(!analyzer.passed());
        assert!(analyzer.errors()[0].code == "SE001");
    }

    #[test]
    fn s3_9_analyze_fn_type_mismatch() {
        let mut analyzer = AnalyzerV2::new();
        let prog = AstProgram::new(
            "test.fj",
            vec![Item::FnDef(FnDefNode {
                name: "f".into(),
                type_params: vec![],
                params: vec![],
                ret_type: Some(TypeExpr::Name("i32".into(), span())),
                body: Box::new(Expr::BoolLit {
                    value: true,
                    span: span(),
                }),
                is_pub: false,
                context: None,
                is_async: false,
                is_gen: false,
                span: span(),
            })],
        );
        analyzer.analyze(&prog);
        assert!(!analyzer.passed());
        assert!(analyzer.errors()[0].code == "SE004");
    }

    // S3.10 — Statement & expression checking
    #[test]
    fn s3_10_analyze_let_binding() {
        let mut analyzer = AnalyzerV2::new();
        let prog = AstProgram::new(
            "test.fj",
            vec![
                Item::Stmt(Stmt::Let {
                    name: "x".into(),
                    mutable: false,
                    ty: Some(TypeExpr::Name("i64".into(), span())),
                    init: Some(Box::new(int_expr(42))),
                    span: span(),
                }),
                Item::Stmt(Stmt::ExprStmt {
                    expr: Box::new(ident_expr("x")),
                    span: span(),
                }),
            ],
        );
        analyzer.analyze(&prog);
        assert!(analyzer.passed());
    }

    #[test]
    fn s3_10_analyze_immutable_assign() {
        let mut analyzer = AnalyzerV2::new();
        let prog = AstProgram::new(
            "test.fj",
            vec![
                Item::Stmt(Stmt::Let {
                    name: "x".into(),
                    mutable: false,
                    ty: Some(TypeExpr::Name("i64".into(), span())),
                    init: Some(Box::new(int_expr(42))),
                    span: span(),
                }),
                Item::Stmt(Stmt::ExprStmt {
                    expr: Box::new(Expr::Assign {
                        target: Box::new(ident_expr("x")),
                        value: Box::new(int_expr(10)),
                        span: span(),
                    }),
                    span: span(),
                }),
            ],
        );
        analyzer.analyze(&prog);
        assert!(!analyzer.passed());
        assert!(analyzer.errors()[0].code == "SE008");
    }

    #[test]
    fn s3_10_break_outside_loop() {
        let mut analyzer = AnalyzerV2::new();
        let prog = AstProgram::new("test.fj", vec![Item::Stmt(Stmt::Break { span: span() })]);
        analyzer.analyze(&prog);
        assert!(!analyzer.passed());
        assert!(analyzer.errors()[0].code == "SE007");
    }

    #[test]
    fn s3_10_if_condition_non_bool() {
        let mut analyzer = AnalyzerV2::new();
        let prog = AstProgram::new(
            "test.fj",
            vec![Item::Stmt(Stmt::ExprStmt {
                expr: Box::new(Expr::If {
                    condition: Box::new(int_expr(42)),
                    then_branch: Box::new(int_expr(1)),
                    else_branch: Some(Box::new(int_expr(2))),
                    span: span(),
                }),
                span: span(),
            })],
        );
        analyzer.analyze(&prog);
        assert!(!analyzer.passed());
        assert!(analyzer.errors()[0].code == "SE004");
    }
}
