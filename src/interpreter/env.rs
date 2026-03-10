//! Environment — scope chain for variable bindings.
//!
//! Uses `Rc<RefCell<>>` for closures that need shared mutable access to parent scope.
//! Each environment frame holds a `HashMap<String, Value>` and an optional parent pointer.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::interpreter::value::Value;

/// A single environment frame in the scope chain.
///
/// Variables are looked up in the current frame first, then in parent frames.
/// Closures capture a reference to their defining environment, enabling
/// lexical scoping.
#[derive(Debug)]
pub struct Environment {
    /// Variable bindings in this scope.
    bindings: HashMap<String, Value>,
    /// Parent scope (None for the global scope).
    parent: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    /// Creates a new global (root) environment with no parent.
    pub fn new() -> Self {
        Environment {
            bindings: HashMap::new(),
            parent: None,
        }
    }

    /// Creates a new child environment with the given parent scope.
    ///
    /// Used when entering a new block, function call, or loop body.
    pub fn new_with_parent(parent: Rc<RefCell<Environment>>) -> Self {
        Environment {
            bindings: HashMap::new(),
            parent: Some(parent),
        }
    }

    /// Defines a new variable in the current scope.
    ///
    /// If the variable already exists in this scope, it is overwritten.
    /// Variables in parent scopes are not affected.
    pub fn define(&mut self, name: String, value: Value) {
        self.bindings.insert(name, value);
    }

    /// Looks up a variable by name, walking up the scope chain.
    ///
    /// Returns `Some(value)` if found, `None` if the variable is not
    /// defined in any enclosing scope.
    pub fn lookup(&self, name: &str) -> Option<Value> {
        if let Some(val) = self.bindings.get(name) {
            return Some(val.clone());
        }
        if let Some(parent) = &self.parent {
            return parent.borrow().lookup(name);
        }
        None
    }

    /// Assigns a new value to an existing variable in the nearest scope.
    ///
    /// Walks up the scope chain to find the variable. Returns `true` if
    /// the variable was found and updated, `false` if it does not exist.
    pub fn assign(&mut self, name: &str, value: Value) -> bool {
        if self.bindings.contains_key(name) {
            self.bindings.insert(name.to_string(), value);
            return true;
        }
        if let Some(parent) = &self.parent {
            return parent.borrow_mut().assign(name, value);
        }
        false
    }

    /// Returns `true` if the variable exists in the current scope only
    /// (does not check parent scopes).
    pub fn has_local(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    /// Returns the number of bindings in the current scope frame.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Returns `true` if the current scope frame has no bindings.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Nullifies all local bindings in the current scope (simulates drop).
    ///
    /// Called at scope exit before the environment is released.
    /// This ensures owned values are explicitly cleared, providing a hook
    /// point for future custom destructor support. Variables that have
    /// already been moved (set to `Value::Null`) are not affected.
    pub fn drop_locals(&mut self) {
        for value in self.bindings.values_mut() {
            if !matches!(value, Value::Null) {
                *value = Value::Null;
            }
        }
    }

    /// Returns the names of all local bindings that are still owned
    /// (i.e., not Null/moved).
    pub fn owned_locals(&self) -> Vec<String> {
        self.bindings
            .iter()
            .filter(|(_, v)| !matches!(v, Value::Null))
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Returns all defined names in the current scope and parent scopes.
    ///
    /// Used to inform the analyzer about names defined in prior REPL rounds.
    pub fn all_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.bindings.keys().cloned().collect();
        if let Some(parent) = &self.parent {
            names.extend(parent.borrow().all_names());
        }
        names
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_environment_is_empty() {
        let env = Environment::new();
        assert!(env.is_empty());
        assert_eq!(env.len(), 0);
    }

    #[test]
    fn define_and_lookup_variable() {
        let mut env = Environment::new();
        env.define("x".into(), Value::Int(42));
        assert_eq!(env.lookup("x"), Some(Value::Int(42)));
    }

    #[test]
    fn lookup_undefined_returns_none() {
        let env = Environment::new();
        assert_eq!(env.lookup("missing"), None);
    }

    #[test]
    fn define_overwrites_existing() {
        let mut env = Environment::new();
        env.define("x".into(), Value::Int(1));
        env.define("x".into(), Value::Int(2));
        assert_eq!(env.lookup("x"), Some(Value::Int(2)));
        assert_eq!(env.len(), 1);
    }

    #[test]
    fn child_scope_shadows_parent() {
        let parent = Rc::new(RefCell::new(Environment::new()));
        parent.borrow_mut().define("x".into(), Value::Int(1));

        let mut child = Environment::new_with_parent(Rc::clone(&parent));
        child.define("x".into(), Value::Int(2));

        assert_eq!(child.lookup("x"), Some(Value::Int(2)));
        assert_eq!(parent.borrow().lookup("x"), Some(Value::Int(1)));
    }

    #[test]
    fn child_scope_reads_parent() {
        let parent = Rc::new(RefCell::new(Environment::new()));
        parent.borrow_mut().define("x".into(), Value::Int(42));

        let child = Environment::new_with_parent(Rc::clone(&parent));
        assert_eq!(child.lookup("x"), Some(Value::Int(42)));
    }

    #[test]
    fn assign_updates_current_scope() {
        let mut env = Environment::new();
        env.define("x".into(), Value::Int(1));
        assert!(env.assign("x", Value::Int(99)));
        assert_eq!(env.lookup("x"), Some(Value::Int(99)));
    }

    #[test]
    fn assign_updates_parent_scope() {
        let parent = Rc::new(RefCell::new(Environment::new()));
        parent.borrow_mut().define("x".into(), Value::Int(1));

        let mut child = Environment::new_with_parent(Rc::clone(&parent));
        assert!(child.assign("x", Value::Int(99)));
        assert_eq!(parent.borrow().lookup("x"), Some(Value::Int(99)));
    }

    #[test]
    fn assign_returns_false_for_undefined() {
        let mut env = Environment::new();
        assert!(!env.assign("missing", Value::Int(1)));
    }

    #[test]
    fn nested_three_levels() {
        let global = Rc::new(RefCell::new(Environment::new()));
        global
            .borrow_mut()
            .define("g".into(), Value::Str("global".into()));

        let mid = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
            &global,
        ))));
        mid.borrow_mut()
            .define("m".into(), Value::Str("mid".into()));

        let inner = Environment::new_with_parent(Rc::clone(&mid));

        assert_eq!(inner.lookup("g"), Some(Value::Str("global".into())));
        assert_eq!(inner.lookup("m"), Some(Value::Str("mid".into())));
        assert_eq!(inner.lookup("missing"), None);
    }

    #[test]
    fn has_local_only_checks_current_scope() {
        let parent = Rc::new(RefCell::new(Environment::new()));
        parent.borrow_mut().define("x".into(), Value::Int(1));

        let child = Environment::new_with_parent(Rc::clone(&parent));
        assert!(!child.has_local("x"));
        assert!(parent.borrow().has_local("x"));
    }

    #[test]
    fn assign_prefers_nearest_scope() {
        let parent = Rc::new(RefCell::new(Environment::new()));
        parent.borrow_mut().define("x".into(), Value::Int(1));

        let mut child = Environment::new_with_parent(Rc::clone(&parent));
        child.define("x".into(), Value::Int(2));

        assert!(child.assign("x", Value::Int(99)));
        assert_eq!(child.lookup("x"), Some(Value::Int(99)));
        // Parent unchanged
        assert_eq!(parent.borrow().lookup("x"), Some(Value::Int(1)));
    }

    #[test]
    fn multiple_variables_in_scope() {
        let mut env = Environment::new();
        env.define("a".into(), Value::Int(1));
        env.define("b".into(), Value::Float(2.0));
        env.define("c".into(), Value::Bool(true));
        assert_eq!(env.len(), 3);
        assert_eq!(env.lookup("a"), Some(Value::Int(1)));
        assert_eq!(env.lookup("b"), Some(Value::Float(2.0)));
        assert_eq!(env.lookup("c"), Some(Value::Bool(true)));
    }

    #[test]
    fn closure_captures_environment() {
        let outer = Rc::new(RefCell::new(Environment::new()));
        outer.borrow_mut().define("captured".into(), Value::Int(42));

        // Simulate closure capturing the outer environment
        let closure_env = Rc::clone(&outer);

        // Later, in a different scope, the closure accesses the captured var
        let inner = Environment::new_with_parent(closure_env);
        assert_eq!(inner.lookup("captured"), Some(Value::Int(42)));

        // Mutation through the captured env is visible
        outer.borrow_mut().assign("captured", Value::Int(100));
        assert_eq!(inner.lookup("captured"), Some(Value::Int(100)));
    }

    #[test]
    fn default_creates_empty_environment() {
        let env = Environment::default();
        assert!(env.is_empty());
        assert_eq!(env.lookup("x"), None);
    }

    // ── S9.3 Drop insertion ──

    #[test]
    fn drop_locals_nullifies_owned() {
        let mut env = Environment::new();
        env.define("a".into(), Value::Int(1));
        env.define("b".into(), Value::Str("hello".into()));
        env.drop_locals();
        assert_eq!(env.lookup("a"), Some(Value::Null));
        assert_eq!(env.lookup("b"), Some(Value::Null));
    }

    #[test]
    fn drop_locals_skips_already_null() {
        let mut env = Environment::new();
        env.define("moved".into(), Value::Null);
        env.define("owned".into(), Value::Int(42));
        env.drop_locals();
        // Both are Null now, moved was already Null (no double-drop)
        assert_eq!(env.lookup("moved"), Some(Value::Null));
        assert_eq!(env.lookup("owned"), Some(Value::Null));
    }

    #[test]
    fn owned_locals_returns_non_null() {
        let mut env = Environment::new();
        env.define("a".into(), Value::Int(1));
        env.define("b".into(), Value::Null); // moved
        env.define("c".into(), Value::Str("hi".into()));
        let mut owned = env.owned_locals();
        owned.sort();
        assert_eq!(owned, vec!["a", "c"]);
    }

    #[test]
    fn drop_locals_does_not_affect_parent() {
        let parent = Rc::new(RefCell::new(Environment::new()));
        parent.borrow_mut().define("x".into(), Value::Int(42));

        let mut child = Environment::new_with_parent(Rc::clone(&parent));
        child.define("y".into(), Value::Int(99));
        child.drop_locals();

        // Child's y is nullified
        assert_eq!(child.lookup("y"), Some(Value::Null));
        // Parent's x is untouched
        assert_eq!(parent.borrow().lookup("x"), Some(Value::Int(42)));
    }
}
