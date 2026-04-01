//! Symbol table and scope management.
//!
//! Defines [`SymbolTable`], [`Symbol`], and [`ScopeKind`] for name resolution
//! during semantic analysis. Uses a stack of scopes for lexical scoping.

use crate::lexer::token::Span;

use super::type_check::Type;

/// The kind of scope, used for context validation.
///
/// Determines what statements are valid (e.g., `break` only in `Loop`,
/// `return` only in `Function`). Also used for `@kernel`/`@device`
/// enforcement in later phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Top-level module scope.
    Module,
    /// Function body scope.
    Function,
    /// Generic block scope (`{ ... }`).
    Block,
    /// Loop body scope (while, for).
    Loop,
    /// `@kernel` annotated function scope.
    Kernel,
    /// `@device` annotated function scope.
    Device,
    /// `@npu` annotated function scope.
    Npu,
    /// V16: `@gpu` annotated function scope — GPU compute (SPIR-V/PTX).
    Gpu,
    /// `@unsafe` annotated block/function scope.
    Unsafe,
    /// `@safe` annotated function scope (explicit user-space safety).
    Safe,
    /// `async fn` body scope.
    AsyncFn,
}

/// A symbol entry in the symbol table.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// The symbol's name.
    pub name: String,
    /// The symbol's resolved type.
    pub ty: Type,
    /// Whether the symbol is mutable (`let mut`).
    pub mutable: bool,
    /// Where the symbol was defined.
    pub span: Span,
    /// Whether the symbol has been read/used.
    pub used: bool,
}

impl Symbol {
    /// Creates a new symbol with `used` set to false.
    pub fn new(name: String, ty: Type, mutable: bool, span: Span) -> Self {
        Symbol {
            name,
            ty,
            mutable,
            span,
            used: false,
        }
    }
}

/// A single scope containing symbols and its kind.
#[derive(Debug)]
struct Scope {
    /// Symbols defined in this scope.
    symbols: Vec<Symbol>,
    /// The kind of this scope.
    kind: ScopeKind,
}

/// A scoped symbol table for name resolution.
///
/// Maintains a stack of scopes. Lookups walk from innermost to outermost.
/// New scopes are pushed for blocks, functions, and loops.
pub struct SymbolTable {
    /// Stack of scopes (innermost last).
    scopes: Vec<Scope>,
}

impl SymbolTable {
    /// Creates a new symbol table with a single module (global) scope.
    pub fn new() -> Self {
        SymbolTable {
            scopes: vec![Scope {
                symbols: Vec::new(),
                kind: ScopeKind::Module,
            }],
        }
    }

    /// Pushes a new scope of the given kind onto the stack.
    pub fn push_scope_kind(&mut self, kind: ScopeKind) {
        self.scopes.push(Scope {
            symbols: Vec::new(),
            kind,
        });
    }

    /// Pushes a new block scope (convenience for the common case).
    pub fn push_scope(&mut self) {
        self.push_scope_kind(ScopeKind::Block);
    }

    /// Pops the innermost scope.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Defines a symbol in the current (innermost) scope.
    pub fn define(&mut self, symbol: Symbol) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.symbols.push(symbol);
        }
    }

    /// Looks up a symbol by name, searching from innermost to outermost scope.
    ///
    /// Returns the most recent definition of the name.
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            for sym in scope.symbols.iter().rev() {
                if sym.name == name {
                    return Some(sym);
                }
            }
        }
        None
    }

    /// Finds all symbols with names starting with the given prefix.
    ///
    /// Used by glob imports to resolve `use module::*`.
    pub fn find_with_prefix(&self, prefix: &str) -> Vec<Symbol> {
        let mut results = Vec::new();
        let prefix_dot = format!("{}::", prefix);
        for scope in self.scopes.iter().rev() {
            for sym in &scope.symbols {
                if sym.name.starts_with(&prefix_dot) {
                    results.push(sym.clone());
                }
            }
        }
        results
    }

    /// Marks a symbol as used (read).
    pub fn mark_used(&mut self, name: &str) {
        for scope in self.scopes.iter_mut().rev() {
            for sym in scope.symbols.iter_mut().rev() {
                if sym.name == name {
                    sym.used = true;
                    return;
                }
            }
        }
    }

    /// Pops the innermost scope and returns unused user-defined symbols.
    ///
    /// Symbols starting with `_` are excluded (convention for intentionally unused).
    pub fn pop_scope_unused(&mut self) -> Vec<Symbol> {
        if self.scopes.len() <= 1 {
            return Vec::new();
        }
        let Some(scope) = self.scopes.pop() else {
            return Vec::new();
        };
        scope
            .symbols
            .into_iter()
            .filter(|sym| !sym.used && !sym.name.starts_with('_'))
            .collect()
    }

    /// Returns the current scope depth.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Returns `true` if currently inside a loop scope (at any nesting level).
    pub fn is_inside_loop(&self) -> bool {
        self.scopes.iter().rev().any(|s| s.kind == ScopeKind::Loop)
    }

    /// Returns `true` if currently inside a function scope (at any nesting level).
    pub fn is_inside_function(&self) -> bool {
        self.scopes.iter().rev().any(|s| {
            matches!(
                s.kind,
                ScopeKind::Function
                    | ScopeKind::Kernel
                    | ScopeKind::Device
                    | ScopeKind::Npu
                    | ScopeKind::AsyncFn
            )
        })
    }

    /// Returns `true` if currently inside a `@kernel` scope.
    pub fn is_inside_kernel(&self) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|s| s.kind == ScopeKind::Kernel)
    }

    /// Returns `true` if currently inside a `@device` scope.
    pub fn is_inside_device(&self) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|s| s.kind == ScopeKind::Device)
    }

    /// Returns `true` if currently inside a `@npu` scope.
    pub fn is_inside_npu(&self) -> bool {
        self.scopes.iter().rev().any(|s| s.kind == ScopeKind::Npu)
    }

    /// Returns `true` if currently inside a `@safe` scope.
    /// Functions annotated with @safe or implicitly safe (no annotation).
    pub fn is_inside_safe(&self) -> bool {
        self.scopes.iter().rev().any(|s| s.kind == ScopeKind::Safe)
    }

    /// Returns `true` if currently inside a `@unsafe` scope.
    pub fn is_inside_unsafe(&self) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|s| s.kind == ScopeKind::Unsafe)
    }

    /// Returns `true` if currently inside an `async fn` scope.
    pub fn is_inside_async(&self) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|s| s.kind == ScopeKind::AsyncFn)
    }

    /// Returns all defined symbol names across all scopes (for suggestion engine).
    pub fn all_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for scope in &self.scopes {
            for sym in &scope.symbols {
                if !names.contains(&sym.name) {
                    names.push(sym.name.clone());
                }
            }
        }
        names
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_span() -> Span {
        Span::new(0, 0)
    }

    #[test]
    fn define_and_lookup() {
        let mut st = SymbolTable::new();
        st.define(Symbol::new("x".into(), Type::I64, false, dummy_span()));
        let sym = st.lookup("x").unwrap();
        assert_eq!(sym.ty, Type::I64);
    }

    #[test]
    fn lookup_undefined_returns_none() {
        let st = SymbolTable::new();
        assert!(st.lookup("x").is_none());
    }

    #[test]
    fn inner_scope_shadows_outer() {
        let mut st = SymbolTable::new();
        st.define(Symbol::new("x".into(), Type::I64, false, dummy_span()));
        st.push_scope();
        st.define(Symbol::new("x".into(), Type::Str, false, dummy_span()));
        assert_eq!(st.lookup("x").unwrap().ty, Type::Str);
        st.pop_scope();
        assert_eq!(st.lookup("x").unwrap().ty, Type::I64);
    }

    #[test]
    fn inner_scope_reads_outer() {
        let mut st = SymbolTable::new();
        st.define(Symbol::new("x".into(), Type::Bool, false, dummy_span()));
        st.push_scope();
        assert_eq!(st.lookup("x").unwrap().ty, Type::Bool);
        st.pop_scope();
    }

    #[test]
    fn pop_removes_inner_symbols() {
        let mut st = SymbolTable::new();
        st.push_scope();
        st.define(Symbol::new("y".into(), Type::F64, false, dummy_span()));
        st.pop_scope();
        assert!(st.lookup("y").is_none());
    }

    #[test]
    fn depth_tracking() {
        let mut st = SymbolTable::new();
        assert_eq!(st.depth(), 1);
        st.push_scope();
        assert_eq!(st.depth(), 2);
        st.push_scope();
        assert_eq!(st.depth(), 3);
        st.pop_scope();
        assert_eq!(st.depth(), 2);
    }

    #[test]
    fn cannot_pop_global_scope() {
        let mut st = SymbolTable::new();
        st.pop_scope();
        assert_eq!(st.depth(), 1);
    }

    #[test]
    fn mutable_flag_preserved() {
        let mut st = SymbolTable::new();
        st.define(Symbol::new("x".into(), Type::I64, true, dummy_span()));
        assert!(st.lookup("x").unwrap().mutable);
    }

    #[test]
    fn mark_used_tracks_usage() {
        let mut st = SymbolTable::new();
        st.define(Symbol::new("x".into(), Type::I64, false, dummy_span()));
        assert!(!st.lookup("x").unwrap().used);
        st.mark_used("x");
        assert!(st.lookup("x").unwrap().used);
    }

    #[test]
    fn pop_scope_unused_returns_unused_symbols() {
        let mut st = SymbolTable::new();
        st.push_scope();
        st.define(Symbol::new(
            "used_var".into(),
            Type::I64,
            false,
            Span::new(0, 3),
        ));
        st.define(Symbol::new(
            "unused_var".into(),
            Type::I64,
            false,
            Span::new(4, 7),
        ));
        st.mark_used("used_var");
        let unused = st.pop_scope_unused();
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].name, "unused_var");
    }

    #[test]
    fn pop_scope_unused_ignores_underscore_prefix() {
        let mut st = SymbolTable::new();
        st.push_scope();
        st.define(Symbol::new(
            "_ignored".into(),
            Type::I64,
            false,
            dummy_span(),
        ));
        let unused = st.pop_scope_unused();
        assert!(unused.is_empty());
    }

    #[test]
    fn is_inside_loop_false_at_module_level() {
        let st = SymbolTable::new();
        assert!(!st.is_inside_loop());
    }

    #[test]
    fn is_inside_loop_true_in_loop_scope() {
        let mut st = SymbolTable::new();
        st.push_scope_kind(ScopeKind::Function);
        st.push_scope_kind(ScopeKind::Loop);
        assert!(st.is_inside_loop());
    }

    #[test]
    fn is_inside_loop_true_in_nested_block_inside_loop() {
        let mut st = SymbolTable::new();
        st.push_scope_kind(ScopeKind::Loop);
        st.push_scope(); // block inside loop
        assert!(st.is_inside_loop());
    }

    #[test]
    fn is_inside_function_false_at_module_level() {
        let st = SymbolTable::new();
        assert!(!st.is_inside_function());
    }

    #[test]
    fn is_inside_function_true_in_function_scope() {
        let mut st = SymbolTable::new();
        st.push_scope_kind(ScopeKind::Function);
        assert!(st.is_inside_function());
    }

    #[test]
    fn is_inside_function_true_in_nested_block() {
        let mut st = SymbolTable::new();
        st.push_scope_kind(ScopeKind::Function);
        st.push_scope(); // block inside function
        assert!(st.is_inside_function());
    }

    #[test]
    fn scope_kind_preserved() {
        let mut st = SymbolTable::new();
        st.push_scope_kind(ScopeKind::Kernel);
        st.push_scope_kind(ScopeKind::Loop);
        assert!(st.is_inside_loop());
        st.pop_scope();
        assert!(!st.is_inside_loop());
    }
}
