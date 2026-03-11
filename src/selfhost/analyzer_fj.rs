//! Analyzer in .fj — type checker, scope resolution, type unification,
//! borrow checker, context checker, error collection, trait resolution,
//! const evaluation, cross-validation.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S25.1: Type Checker Core
// ═══════════════════════════════════════════════════════════════════════

/// Type representation in the self-hosted type checker.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FjType {
    /// Primitive types.
    Int(u8), // i8..i128 (bit width)
    UInt(u8),  // u8..u128
    Float(u8), // f32, f64 (bit width)
    Bool,
    Char,
    Str,
    Void,
    Never,
    /// Named type (struct, enum, trait object).
    Named(String),
    /// Generic type variable.
    TypeVar(String),
    /// Function type.
    Function {
        params: Vec<FjType>,
        ret: Box<FjType>,
    },
    /// Array type with optional const length.
    Array(Box<FjType>, Option<usize>),
    /// Reference type.
    Ref {
        inner: Box<FjType>,
        mutable: bool,
    },
    /// Unknown (for inference).
    Unknown(u32),
}

impl fmt::Display for FjType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FjType::Int(w) => write!(f, "i{w}"),
            FjType::UInt(w) => write!(f, "u{w}"),
            FjType::Float(w) => write!(f, "f{w}"),
            FjType::Bool => write!(f, "bool"),
            FjType::Char => write!(f, "char"),
            FjType::Str => write!(f, "str"),
            FjType::Void => write!(f, "void"),
            FjType::Never => write!(f, "never"),
            FjType::Named(n) => write!(f, "{n}"),
            FjType::TypeVar(n) => write!(f, "{n}"),
            FjType::Function { params, ret } => {
                let ps: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                write!(f, "fn({}) -> {}", ps.join(", "), ret)
            }
            FjType::Array(inner, len) => {
                if let Some(n) = len {
                    write!(f, "[{inner}; {n}]")
                } else {
                    write!(f, "[{inner}]")
                }
            }
            FjType::Ref { inner, mutable } => {
                if *mutable {
                    write!(f, "&mut {inner}")
                } else {
                    write!(f, "&{inner}")
                }
            }
            FjType::Unknown(id) => write!(f, "?{id}"),
        }
    }
}

/// Result of type checking a program.
#[derive(Debug, Clone)]
pub struct TypeCheckResult {
    /// Errors found.
    pub errors: Vec<TypeCheckError>,
    /// Number of functions checked.
    pub functions_checked: usize,
    /// Whether the check passed.
    pub passed: bool,
}

impl TypeCheckResult {
    /// Creates a passing result.
    pub fn pass(functions_checked: usize) -> Self {
        Self {
            errors: Vec::new(),
            functions_checked,
            passed: true,
        }
    }

    /// Creates a failing result.
    pub fn fail(errors: Vec<TypeCheckError>, functions_checked: usize) -> Self {
        Self {
            passed: false,
            errors,
            functions_checked,
        }
    }
}

/// A type check error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeCheckError {
    /// Error code (e.g., "SE004").
    pub code: String,
    /// Error message.
    pub message: String,
    /// Source span (line, column).
    pub span: (usize, usize),
}

impl fmt::Display for TypeCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] at {}:{}: {}",
            self.code, self.span.0, self.span.1, self.message
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.2: Scope Resolution
// ═══════════════════════════════════════════════════════════════════════

/// A symbol in the symbol table.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Symbol name.
    pub name: String,
    /// Symbol type.
    pub ty: FjType,
    /// Whether this symbol is mutable.
    pub mutable: bool,
    /// Scope depth where defined.
    pub depth: usize,
}

/// A scope in the symbol table.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// Symbols in this scope.
    symbols: HashMap<String, Symbol>,
}

/// A symbol table with nested scopes.
#[derive(Debug, Clone)]
pub struct SymbolTable {
    /// Scope stack (outermost first).
    scopes: Vec<Scope>,
}

impl SymbolTable {
    /// Creates a new symbol table with a global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope {
                symbols: HashMap::new(),
            }],
        }
    }

    /// Pushes a new scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(Scope {
            symbols: HashMap::new(),
        });
    }

    /// Pops the current scope.
    pub fn pop_scope(&mut self) -> Option<Scope> {
        if self.scopes.len() > 1 {
            self.scopes.pop()
        } else {
            None
        }
    }

    /// Defines a symbol in the current scope.
    pub fn define(&mut self, name: &str, ty: FjType, mutable: bool) {
        let depth = self.scopes.len() - 1;
        if let Some(scope) = self.scopes.last_mut() {
            scope.symbols.insert(
                name.into(),
                Symbol {
                    name: name.into(),
                    ty,
                    mutable,
                    depth,
                },
            );
        }
    }

    /// Looks up a symbol, searching from innermost to outermost scope.
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.symbols.get(name) {
                return Some(sym);
            }
        }
        None
    }

    /// Returns the current scope depth.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.3: Type Unification
// ═══════════════════════════════════════════════════════════════════════

/// Type substitution for unification.
#[derive(Debug, Clone, Default)]
pub struct TypeSubstitution {
    /// Variable ID -> resolved type.
    pub bindings: HashMap<u32, FjType>,
    /// Next fresh variable ID.
    next_id: u32,
}

impl TypeSubstitution {
    /// Creates an empty substitution.
    pub fn new() -> Self {
        Self::default()
    }

    /// Generates a fresh type variable.
    pub fn fresh_var(&mut self) -> FjType {
        let id = self.next_id;
        self.next_id += 1;
        FjType::Unknown(id)
    }

    /// Binds a type variable to a type.
    pub fn bind(&mut self, id: u32, ty: FjType) {
        self.bindings.insert(id, ty);
    }

    /// Resolves a type through the substitution.
    pub fn resolve(&self, ty: &FjType) -> FjType {
        match ty {
            FjType::Unknown(id) => {
                if let Some(bound) = self.bindings.get(id) {
                    self.resolve(bound)
                } else {
                    ty.clone()
                }
            }
            _ => ty.clone(),
        }
    }
}

/// Unifies two types, updating the substitution.
pub fn unify(a: &FjType, b: &FjType, subst: &mut TypeSubstitution) -> Result<(), String> {
    let a = subst.resolve(a);
    let b = subst.resolve(b);
    match (&a, &b) {
        _ if a == b => Ok(()),
        (FjType::Unknown(id), _) => {
            subst.bind(*id, b);
            Ok(())
        }
        (_, FjType::Unknown(id)) => {
            subst.bind(*id, a);
            Ok(())
        }
        (
            FjType::Function {
                params: pa,
                ret: ra,
            },
            FjType::Function {
                params: pb,
                ret: rb,
            },
        ) => {
            if pa.len() != pb.len() {
                return Err(format!("arity mismatch: {} vs {}", pa.len(), pb.len()));
            }
            for (p1, p2) in pa.iter().zip(pb.iter()) {
                unify(p1, p2, subst)?;
            }
            unify(ra, rb, subst)
        }
        (FjType::Array(a_inner, a_len), FjType::Array(b_inner, b_len)) => {
            if a_len != b_len {
                return Err("array length mismatch".into());
            }
            unify(a_inner, b_inner, subst)
        }
        _ => Err(format!("type mismatch: {a} vs {b}")),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.4: Borrow Checker
// ═══════════════════════════════════════════════════════════════════════

/// Move state of a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveState {
    /// Variable is live and owned.
    Owned,
    /// Variable has been moved.
    Moved,
    /// Variable is borrowed immutably.
    Borrowed,
    /// Variable is borrowed mutably.
    BorrowedMut,
}

/// Borrow checker state for a function.
#[derive(Debug, Clone, Default)]
pub struct BorrowState {
    /// Variable -> move state.
    states: HashMap<String, MoveState>,
}

impl BorrowState {
    /// Creates a new borrow state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a variable as owned.
    pub fn register(&mut self, name: &str) {
        self.states.insert(name.into(), MoveState::Owned);
    }

    /// Marks a variable as moved.
    pub fn move_var(&mut self, name: &str) -> Result<(), String> {
        match self.states.get(name) {
            Some(MoveState::Owned) => {
                self.states.insert(name.into(), MoveState::Moved);
                Ok(())
            }
            Some(MoveState::Moved) => Err(format!("use of moved value: `{name}`")),
            Some(MoveState::Borrowed | MoveState::BorrowedMut) => {
                Err(format!("cannot move `{name}` while borrowed"))
            }
            None => Err(format!("unknown variable: `{name}`")),
        }
    }

    /// Borrows a variable immutably.
    pub fn borrow(&mut self, name: &str) -> Result<(), String> {
        match self.states.get(name) {
            Some(MoveState::Owned | MoveState::Borrowed) => {
                self.states.insert(name.into(), MoveState::Borrowed);
                Ok(())
            }
            Some(MoveState::Moved) => Err(format!("cannot borrow moved value: `{name}`")),
            Some(MoveState::BorrowedMut) => {
                Err(format!("cannot borrow `{name}` — already mutably borrowed"))
            }
            None => Err(format!("unknown variable: `{name}`")),
        }
    }

    /// Borrows a variable mutably.
    pub fn borrow_mut(&mut self, name: &str) -> Result<(), String> {
        match self.states.get(name) {
            Some(MoveState::Owned) => {
                self.states.insert(name.into(), MoveState::BorrowedMut);
                Ok(())
            }
            Some(MoveState::Moved) => Err(format!("cannot borrow moved value: `{name}`")),
            Some(MoveState::Borrowed) => {
                Err(format!("cannot mutably borrow `{name}` — already borrowed"))
            }
            Some(MoveState::BorrowedMut) => Err(format!(
                "cannot mutably borrow `{name}` — already mutably borrowed"
            )),
            None => Err(format!("unknown variable: `{name}`")),
        }
    }

    /// Returns the move state of a variable.
    pub fn state(&self, name: &str) -> Option<MoveState> {
        self.states.get(name).copied()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.5: Context Checker
// ═══════════════════════════════════════════════════════════════════════

/// Context validation rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextRule {
    /// Context annotation.
    pub context: String,
    /// Forbidden operation.
    pub forbidden: String,
    /// Error code.
    pub error_code: String,
    /// Error message.
    pub message: String,
}

/// Standard context rules for @kernel, @device, @safe.
pub fn standard_context_rules() -> Vec<ContextRule> {
    vec![
        ContextRule {
            context: "@kernel".into(),
            forbidden: "heap_alloc".into(),
            error_code: "KE001".into(),
            message: "heap allocation not allowed in @kernel context".into(),
        },
        ContextRule {
            context: "@kernel".into(),
            forbidden: "tensor_ops".into(),
            error_code: "KE002".into(),
            message: "tensor operations not allowed in @kernel context".into(),
        },
        ContextRule {
            context: "@device".into(),
            forbidden: "raw_pointer".into(),
            error_code: "DE001".into(),
            message: "raw pointer dereference not allowed in @device context".into(),
        },
        ContextRule {
            context: "@device".into(),
            forbidden: "irq".into(),
            error_code: "DE002".into(),
            message: "IRQ operations not allowed in @device context".into(),
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// S25.6: Error Collection
// ═══════════════════════════════════════════════════════════════════════

/// Error collector that gathers all errors without stopping.
#[derive(Debug, Clone, Default)]
pub struct ErrorCollector {
    /// Collected errors.
    errors: Vec<TypeCheckError>,
}

impl ErrorCollector {
    /// Creates a new error collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an error.
    pub fn add(&mut self, code: &str, message: &str, span: (usize, usize)) {
        self.errors.push(TypeCheckError {
            code: code.into(),
            message: message.into(),
            span,
        });
    }

    /// Returns all collected errors.
    pub fn errors(&self) -> &[TypeCheckError] {
        &self.errors
    }

    /// Returns the error count.
    pub fn count(&self) -> usize {
        self.errors.len()
    }

    /// Returns whether any errors were collected.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.7: Trait Resolution
// ═══════════════════════════════════════════════════════════════════════

/// A trait implementation record.
#[derive(Debug, Clone)]
pub struct TraitImpl {
    /// Trait name.
    pub trait_name: String,
    /// Implementing type.
    pub impl_type: String,
    /// Method names provided.
    pub methods: Vec<String>,
    /// Whether this is a blanket impl.
    pub is_blanket: bool,
}

/// Trait resolution index.
#[derive(Debug, Clone, Default)]
pub struct TraitIndex {
    /// All trait implementations.
    impls: Vec<TraitImpl>,
}

impl TraitIndex {
    /// Creates a new trait index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a trait implementation.
    pub fn register(&mut self, impl_: TraitImpl) {
        self.impls.push(impl_);
    }

    /// Finds implementations of a trait for a given type.
    pub fn find_impl(&self, trait_name: &str, for_type: &str) -> Option<&TraitImpl> {
        self.impls
            .iter()
            .find(|i| i.trait_name == trait_name && (i.impl_type == for_type || i.is_blanket))
    }

    /// Resolves a method call on a type.
    pub fn resolve_method(&self, type_name: &str, method: &str) -> Option<&TraitImpl> {
        self.impls.iter().find(|i| {
            (i.impl_type == type_name || i.is_blanket) && i.methods.contains(&method.to_string())
        })
    }

    /// Returns the number of registered implementations.
    pub fn impl_count(&self) -> usize {
        self.impls.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.8: Const Evaluation
// ═══════════════════════════════════════════════════════════════════════

/// A compile-time constant value.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    /// Integer constant.
    Int(i64),
    /// Float constant.
    Float(f64),
    /// Boolean constant.
    Bool(bool),
    /// String constant.
    Str(String),
}

impl fmt::Display for ConstValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstValue::Int(n) => write!(f, "{n}"),
            ConstValue::Float(n) => write!(f, "{n}"),
            ConstValue::Bool(b) => write!(f, "{b}"),
            ConstValue::Str(s) => write!(f, "\"{s}\""),
        }
    }
}

/// Evaluates a constant binary expression.
pub fn const_eval_binary(op: &str, lhs: &ConstValue, rhs: &ConstValue) -> Option<ConstValue> {
    match (lhs, rhs) {
        (ConstValue::Int(a), ConstValue::Int(b)) => match op {
            "+" => Some(ConstValue::Int(a + b)),
            "-" => Some(ConstValue::Int(a - b)),
            "*" => Some(ConstValue::Int(a * b)),
            "/" if *b != 0 => Some(ConstValue::Int(a / b)),
            "%" if *b != 0 => Some(ConstValue::Int(a % b)),
            "==" => Some(ConstValue::Bool(a == b)),
            "!=" => Some(ConstValue::Bool(a != b)),
            "<" => Some(ConstValue::Bool(a < b)),
            ">" => Some(ConstValue::Bool(a > b)),
            "<=" => Some(ConstValue::Bool(a <= b)),
            ">=" => Some(ConstValue::Bool(a >= b)),
            _ => None,
        },
        (ConstValue::Bool(a), ConstValue::Bool(b)) => match op {
            "&&" => Some(ConstValue::Bool(*a && *b)),
            "||" => Some(ConstValue::Bool(*a || *b)),
            "==" => Some(ConstValue::Bool(a == b)),
            "!=" => Some(ConstValue::Bool(a != b)),
            _ => None,
        },
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.9: Cross-Validation
// ═══════════════════════════════════════════════════════════════════════

/// A cross-validation result comparing two analyzer outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossValidation {
    /// Test program name.
    pub program: String,
    /// Rust analyzer errors.
    pub rust_errors: Vec<String>,
    /// .fj analyzer errors.
    pub fj_errors: Vec<String>,
    /// Whether outputs match.
    pub matches: bool,
}

impl CrossValidation {
    /// Creates a cross-validation from two error lists.
    pub fn compare(program: &str, rust_errors: Vec<String>, fj_errors: Vec<String>) -> Self {
        let matches = rust_errors == fj_errors;
        Self {
            program: program.into(),
            rust_errors,
            fj_errors,
            matches,
        }
    }
}

impl fmt::Display for CrossValidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.matches {
            write!(
                f,
                "{}: MATCH ({} errors)",
                self.program,
                self.rust_errors.len()
            )
        } else {
            write!(
                f,
                "{}: MISMATCH (Rust={}, .fj={})",
                self.program,
                self.rust_errors.len(),
                self.fj_errors.len()
            )
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S25.1 — Type Checker Core
    #[test]
    fn s25_1_fj_type_display() {
        assert_eq!(FjType::Int(32).to_string(), "i32");
        assert_eq!(FjType::Bool.to_string(), "bool");
        assert_eq!(FjType::Named("Point".into()).to_string(), "Point");
    }

    #[test]
    fn s25_1_function_type_display() {
        let ft = FjType::Function {
            params: vec![FjType::Int(32), FjType::Int(32)],
            ret: Box::new(FjType::Int(32)),
        };
        assert_eq!(ft.to_string(), "fn(i32, i32) -> i32");
    }

    #[test]
    fn s25_1_type_check_result_pass() {
        let result = TypeCheckResult::pass(10);
        assert!(result.passed);
        assert_eq!(result.functions_checked, 10);
    }

    #[test]
    fn s25_1_type_check_result_fail() {
        let err = TypeCheckError {
            code: "SE004".into(),
            message: "type mismatch".into(),
            span: (5, 10),
        };
        let result = TypeCheckResult::fail(vec![err], 3);
        assert!(!result.passed);
        assert_eq!(result.errors.len(), 1);
    }

    // S25.2 — Scope Resolution
    #[test]
    fn s25_2_symbol_table_define_lookup() {
        let mut table = SymbolTable::new();
        table.define("x", FjType::Int(32), false);
        assert!(table.lookup("x").is_some());
        assert!(table.lookup("y").is_none());
    }

    #[test]
    fn s25_2_nested_scopes() {
        let mut table = SymbolTable::new();
        table.define("x", FjType::Int(32), false);
        table.push_scope();
        table.define("y", FjType::Bool, false);
        assert!(table.lookup("x").is_some()); // visible from inner
        assert!(table.lookup("y").is_some());
        table.pop_scope();
        assert!(table.lookup("y").is_none()); // no longer visible
    }

    #[test]
    fn s25_2_shadowing() {
        let mut table = SymbolTable::new();
        table.define("x", FjType::Int(32), false);
        table.push_scope();
        table.define("x", FjType::Bool, false);
        assert_eq!(table.lookup("x").unwrap().ty, FjType::Bool);
        table.pop_scope();
        assert_eq!(table.lookup("x").unwrap().ty, FjType::Int(32));
    }

    // S25.3 — Type Unification
    #[test]
    fn s25_3_unify_same_types() {
        let mut subst = TypeSubstitution::new();
        assert!(unify(&FjType::Int(32), &FjType::Int(32), &mut subst).is_ok());
    }

    #[test]
    fn s25_3_unify_variable() {
        let mut subst = TypeSubstitution::new();
        let var = subst.fresh_var();
        assert!(unify(&var, &FjType::Int(32), &mut subst).is_ok());
        assert_eq!(subst.resolve(&var), FjType::Int(32));
    }

    #[test]
    fn s25_3_unify_mismatch() {
        let mut subst = TypeSubstitution::new();
        assert!(unify(&FjType::Int(32), &FjType::Bool, &mut subst).is_err());
    }

    #[test]
    fn s25_3_unify_functions() {
        let mut subst = TypeSubstitution::new();
        let a = FjType::Function {
            params: vec![FjType::Int(32)],
            ret: Box::new(FjType::Bool),
        };
        let b = FjType::Function {
            params: vec![FjType::Int(32)],
            ret: Box::new(FjType::Bool),
        };
        assert!(unify(&a, &b, &mut subst).is_ok());
    }

    // S25.4 — Borrow Checker
    #[test]
    fn s25_4_move_then_use() {
        let mut state = BorrowState::new();
        state.register("x");
        assert!(state.move_var("x").is_ok());
        assert!(state.move_var("x").is_err()); // use after move
    }

    #[test]
    fn s25_4_borrow_then_move() {
        let mut state = BorrowState::new();
        state.register("x");
        assert!(state.borrow("x").is_ok());
        assert!(state.move_var("x").is_err()); // can't move while borrowed
    }

    #[test]
    fn s25_4_mut_borrow_conflict() {
        let mut state = BorrowState::new();
        state.register("x");
        assert!(state.borrow_mut("x").is_ok());
        assert!(state.borrow("x").is_err()); // conflict
    }

    // S25.5 — Context Checker
    #[test]
    fn s25_5_standard_rules() {
        let rules = standard_context_rules();
        assert!(rules.len() >= 4);
        assert!(rules.iter().any(|r| r.error_code == "KE001"));
        assert!(rules.iter().any(|r| r.error_code == "DE001"));
    }

    // S25.6 — Error Collection
    #[test]
    fn s25_6_error_collector() {
        let mut collector = ErrorCollector::new();
        collector.add("SE004", "type mismatch", (5, 10));
        collector.add("SE001", "undefined variable", (8, 3));
        assert_eq!(collector.count(), 2);
        assert!(collector.has_errors());
    }

    #[test]
    fn s25_6_error_display() {
        let err = TypeCheckError {
            code: "SE004".into(),
            message: "type mismatch".into(),
            span: (5, 10),
        };
        assert!(err.to_string().contains("SE004"));
        assert!(err.to_string().contains("5:10"));
    }

    // S25.7 — Trait Resolution
    #[test]
    fn s25_7_trait_index() {
        let mut index = TraitIndex::new();
        index.register(TraitImpl {
            trait_name: "Display".into(),
            impl_type: "Point".into(),
            methods: vec!["fmt".into()],
            is_blanket: false,
        });
        assert!(index.find_impl("Display", "Point").is_some());
        assert!(index.find_impl("Display", "Circle").is_none());
    }

    #[test]
    fn s25_7_blanket_impl() {
        let mut index = TraitIndex::new();
        index.register(TraitImpl {
            trait_name: "ToString".into(),
            impl_type: "T".into(),
            methods: vec!["to_string".into()],
            is_blanket: true,
        });
        assert!(index.find_impl("ToString", "Point").is_some());
    }

    #[test]
    fn s25_7_method_resolution() {
        let mut index = TraitIndex::new();
        index.register(TraitImpl {
            trait_name: "Iterator".into(),
            impl_type: "Vec".into(),
            methods: vec!["next".into(), "map".into()],
            is_blanket: false,
        });
        assert!(index.resolve_method("Vec", "next").is_some());
        assert!(index.resolve_method("Vec", "unknown").is_none());
    }

    // S25.8 — Const Evaluation
    #[test]
    fn s25_8_const_eval_arithmetic() {
        let result = const_eval_binary("+", &ConstValue::Int(3), &ConstValue::Int(4));
        assert_eq!(result, Some(ConstValue::Int(7)));
    }

    #[test]
    fn s25_8_const_eval_comparison() {
        let result = const_eval_binary("<", &ConstValue::Int(3), &ConstValue::Int(4));
        assert_eq!(result, Some(ConstValue::Bool(true)));
    }

    #[test]
    fn s25_8_const_eval_boolean() {
        let result = const_eval_binary("&&", &ConstValue::Bool(true), &ConstValue::Bool(false));
        assert_eq!(result, Some(ConstValue::Bool(false)));
    }

    #[test]
    fn s25_8_const_value_display() {
        assert_eq!(ConstValue::Int(42).to_string(), "42");
        assert_eq!(ConstValue::Bool(true).to_string(), "true");
        assert_eq!(ConstValue::Str("hello".into()).to_string(), "\"hello\"");
    }

    // S25.9 — Cross-Validation
    #[test]
    fn s25_9_cross_validation_match() {
        let cv = CrossValidation::compare(
            "test1.fj",
            vec!["SE004: type mismatch".into()],
            vec!["SE004: type mismatch".into()],
        );
        assert!(cv.matches);
        assert!(cv.to_string().contains("MATCH"));
    }

    #[test]
    fn s25_9_cross_validation_mismatch() {
        let cv = CrossValidation::compare("test2.fj", vec!["SE004: type mismatch".into()], vec![]);
        assert!(!cv.matches);
        assert!(cv.to_string().contains("MISMATCH"));
    }

    // S25.10 — Additional
    #[test]
    fn s25_10_ref_type_display() {
        let ref_ty = FjType::Ref {
            inner: Box::new(FjType::Int(32)),
            mutable: true,
        };
        assert_eq!(ref_ty.to_string(), "&mut i32");
    }

    #[test]
    fn s25_10_array_type_display() {
        let arr = FjType::Array(Box::new(FjType::Int(32)), Some(10));
        assert_eq!(arr.to_string(), "[i32; 10]");
    }

    #[test]
    fn s25_10_scope_depth() {
        let mut table = SymbolTable::new();
        assert_eq!(table.depth(), 1);
        table.push_scope();
        assert_eq!(table.depth(), 2);
        table.pop_scope();
        assert_eq!(table.depth(), 1);
    }
}
