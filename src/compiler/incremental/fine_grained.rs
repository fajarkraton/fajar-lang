//! Fine-grained dependency tracking — function, type, trait, and const level.
//!
//! Extends the file-level `DependencyGraph` with intra-file symbol tracking.
//! When a single function body changes (but its signature doesn't), only that
//! function and its direct callers need recompilation — not the whole file.
//!
//! # Granularity Levels
//!
//! - **Function**: Track which functions call which other functions
//! - **Type**: Track struct/enum definitions and their users
//! - **Trait impl**: Track which trait impls changed → invalidate monomorphizations
//! - **Constant**: Track const value changes → propagate to users
//! - **Macro**: Track macro expansion input → output mapping
//! - **Import**: Track cross-module type inference dependencies

use std::collections::{HashMap, HashSet};

use super::compute_content_hash;

// ═══════════════════════════════════════════════════════════════════════
// I2.1: Symbol-Level Dependency Node
// ═══════════════════════════════════════════════════════════════════════

/// A symbol in the dependency graph (more granular than file).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolId {
    /// File containing this symbol.
    pub file: String,
    /// Symbol name.
    pub name: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
}

impl SymbolId {
    pub fn function(file: &str, name: &str) -> Self {
        Self {
            file: file.into(),
            name: name.into(),
            kind: SymbolKind::Function,
        }
    }
    pub fn r#struct(file: &str, name: &str) -> Self {
        Self {
            file: file.into(),
            name: name.into(),
            kind: SymbolKind::Struct,
        }
    }
    pub fn r#enum(file: &str, name: &str) -> Self {
        Self {
            file: file.into(),
            name: name.into(),
            kind: SymbolKind::Enum,
        }
    }
    pub fn r#trait(file: &str, name: &str) -> Self {
        Self {
            file: file.into(),
            name: name.into(),
            kind: SymbolKind::Trait,
        }
    }
    pub fn r#const(file: &str, name: &str) -> Self {
        Self {
            file: file.into(),
            name: name.into(),
            kind: SymbolKind::Const,
        }
    }
    pub fn r#macro(file: &str, name: &str) -> Self {
        Self {
            file: file.into(),
            name: name.into(),
            kind: SymbolKind::Macro,
        }
    }

    /// Qualified name: `file::name`.
    pub fn qualified(&self) -> String {
        format!("{}::{}", self.file, self.name)
    }
}

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}::{}({})", self.file, self.name, self.kind)
    }
}

/// Kind of symbol for dependency tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Const,
    Macro,
    TraitImpl,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Function => write!(f, "fn"),
            SymbolKind::Struct => write!(f, "struct"),
            SymbolKind::Enum => write!(f, "enum"),
            SymbolKind::Trait => write!(f, "trait"),
            SymbolKind::Const => write!(f, "const"),
            SymbolKind::Macro => write!(f, "macro"),
            SymbolKind::TraitImpl => write!(f, "impl"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I2.4: Symbol Info — signature vs body
// ═══════════════════════════════════════════════════════════════════════

/// Information about a symbol for change detection.
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    /// The symbol identifier.
    pub id: SymbolId,
    /// Hash of the symbol's signature (name + params + return type).
    pub signature_hash: String,
    /// Hash of the symbol's body (implementation).
    pub body_hash: String,
    /// Symbols this symbol depends on (calls, references).
    pub deps: HashSet<SymbolId>,
}

impl SymbolInfo {
    /// Check if only the body changed (signature unchanged).
    pub fn body_only_change(&self, old: &SymbolInfo) -> bool {
        self.signature_hash == old.signature_hash && self.body_hash != old.body_hash
    }

    /// Check if the signature changed (requires recompiling callers).
    pub fn signature_changed(&self, old: &SymbolInfo) -> bool {
        self.signature_hash != old.signature_hash
    }

    /// Check if anything changed.
    pub fn has_changed(&self, old: &SymbolInfo) -> bool {
        self.signature_hash != old.signature_hash || self.body_hash != old.body_hash
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Fine-Grained Dependency Graph
// ═══════════════════════════════════════════════════════════════════════

/// Fine-grained dependency graph at symbol level.
#[derive(Debug, Clone, Default)]
pub struct FineGrainedGraph {
    /// All known symbols: qualified name → info.
    pub symbols: HashMap<String, SymbolInfo>,
    /// Forward edges: symbol → symbols it depends on.
    pub edges: HashMap<String, HashSet<String>>,
    /// Reverse edges: symbol → symbols that depend on it.
    pub reverse_edges: HashMap<String, HashSet<String>>,
    /// I2.3: Import graph: file → files it imports.
    pub import_graph: HashMap<String, HashSet<String>>,
    /// I2.5: Trait impls: (trait, type) → impl symbol id.
    pub trait_impls: HashMap<(String, String), String>,
    /// I2.6: Const values: const name → value hash.
    pub const_values: HashMap<String, String>,
    /// I2.7: Macro expansions: macro call site → expansion hash.
    pub macro_expansions: HashMap<String, String>,
    /// I2.8: Cross-module inferred types: symbol → inferred type (from another module).
    pub cross_module_types: HashMap<String, String>,
}

impl FineGrainedGraph {
    /// Creates a new empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// I2.1: Register a function symbol with its dependencies.
    pub fn add_function(
        &mut self,
        file: &str,
        name: &str,
        signature: &str,
        body: &str,
        calls: &[&str],
    ) {
        let id = SymbolId::function(file, name);
        let qualified = id.qualified();
        let sig_hash = compute_content_hash(signature);
        let body_hash = compute_content_hash(body);

        let mut deps = HashSet::new();
        let mut dep_ids = HashSet::new();
        for &callee in calls {
            let dep_qualified = if callee.contains("::") {
                callee.to_string()
            } else {
                format!("{file}::{callee}")
            };
            deps.insert(dep_qualified.clone());
            dep_ids.insert(SymbolId::function(file, callee));
        }

        // Register forward edges
        self.edges.insert(qualified.clone(), deps.clone());

        // Register reverse edges
        for dep in &deps {
            self.reverse_edges
                .entry(dep.clone())
                .or_default()
                .insert(qualified.clone());
        }

        self.symbols.insert(
            qualified,
            SymbolInfo {
                id,
                signature_hash: sig_hash,
                body_hash,
                deps: dep_ids,
            },
        );
    }

    /// I2.2: Register a type (struct/enum) symbol.
    pub fn add_type(&mut self, file: &str, name: &str, kind: SymbolKind, definition: &str) {
        let id = SymbolId {
            file: file.into(),
            name: name.into(),
            kind,
        };
        let qualified = id.qualified();
        let def_hash = compute_content_hash(definition);

        self.symbols.insert(
            qualified,
            SymbolInfo {
                id,
                signature_hash: def_hash.clone(),
                body_hash: def_hash,
                deps: HashSet::new(),
            },
        );
    }

    /// I2.3: Register an import edge between files.
    pub fn add_import(&mut self, from_file: &str, to_file: &str) {
        self.import_graph
            .entry(from_file.to_string())
            .or_default()
            .insert(to_file.to_string());
    }

    /// I2.5: Register a trait implementation.
    pub fn add_trait_impl(
        &mut self,
        file: &str,
        trait_name: &str,
        type_name: &str,
        impl_body: &str,
    ) {
        let key = (trait_name.to_string(), type_name.to_string());
        let impl_id = format!("{file}::impl_{trait_name}_for_{type_name}");
        let body_hash = compute_content_hash(impl_body);

        self.trait_impls.insert(key, impl_id.clone());

        let id = SymbolId {
            file: file.into(),
            name: format!("impl_{trait_name}_for_{type_name}"),
            kind: SymbolKind::TraitImpl,
        };
        self.symbols.insert(
            impl_id,
            SymbolInfo {
                id,
                signature_hash: body_hash.clone(),
                body_hash,
                deps: HashSet::new(),
            },
        );
    }

    /// I2.6: Register a const value for change tracking.
    pub fn add_const(&mut self, file: &str, name: &str, value_repr: &str) {
        let id = SymbolId::r#const(file, name);
        let qualified = id.qualified();
        let val_hash = compute_content_hash(value_repr);

        self.const_values
            .insert(qualified.clone(), val_hash.clone());

        self.symbols.insert(
            qualified,
            SymbolInfo {
                id,
                signature_hash: val_hash.clone(),
                body_hash: val_hash,
                deps: HashSet::new(),
            },
        );
    }

    /// I2.7: Register a macro expansion.
    pub fn add_macro_expansion(&mut self, call_site: &str, expansion: &str) {
        let exp_hash = compute_content_hash(expansion);
        self.macro_expansions
            .insert(call_site.to_string(), exp_hash);
    }

    /// I2.8: Register a cross-module inferred type.
    pub fn add_cross_module_type(&mut self, symbol: &str, inferred_type: &str) {
        self.cross_module_types
            .insert(symbol.to_string(), inferred_type.to_string());
    }

    // ═══════════════════════════════════════════════════════════════════
    // Change Detection
    // ═══════════════════════════════════════════════════════════════════

    /// I2.1/I2.4: Detect changed symbols between old and new graph.
    ///
    /// Returns `(signature_changed, body_only_changed)` symbol sets.
    pub fn detect_changes(&self, old: &FineGrainedGraph) -> ChangeSet {
        let mut sig_changed = HashSet::new();
        let mut body_changed = HashSet::new();
        let mut added = HashSet::new();
        let mut removed = HashSet::new();

        // Check existing symbols for changes
        for (name, new_info) in &self.symbols {
            match old.symbols.get(name) {
                Some(old_info) => {
                    if new_info.signature_changed(old_info) {
                        sig_changed.insert(name.clone());
                    } else if new_info.body_only_change(old_info) {
                        body_changed.insert(name.clone());
                    }
                }
                None => {
                    added.insert(name.clone());
                }
            }
        }

        // Check for removed symbols
        for name in old.symbols.keys() {
            if !self.symbols.contains_key(name) {
                removed.insert(name.clone());
            }
        }

        ChangeSet {
            signature_changed: sig_changed,
            body_only_changed: body_changed,
            added,
            removed,
        }
    }

    /// Compute minimal recompilation set from a change set.
    ///
    /// - Signature changes → recompile symbol + all transitive dependents
    /// - Body-only changes → recompile only the changed symbol
    pub fn recompilation_set(&self, changes: &ChangeSet) -> HashSet<String> {
        let mut to_recompile = HashSet::new();

        // Body-only changes: just the symbol itself
        for sym in &changes.body_only_changed {
            to_recompile.insert(sym.clone());
        }

        // Signature changes: symbol + transitive dependents
        for sym in &changes.signature_changed {
            to_recompile.insert(sym.clone());
            self.collect_transitive_dependents(sym, &mut to_recompile);
        }

        // Added symbols: just themselves
        for sym in &changes.added {
            to_recompile.insert(sym.clone());
        }

        // Removed symbols: recompile their dependents
        for sym in &changes.removed {
            if let Some(deps) = self.reverse_edges.get(sym) {
                for dep in deps {
                    to_recompile.insert(dep.clone());
                }
            }
        }

        to_recompile
    }

    /// Collect all transitive dependents of a symbol.
    fn collect_transitive_dependents(&self, sym: &str, out: &mut HashSet<String>) {
        if let Some(deps) = self.reverse_edges.get(sym) {
            for dep in deps {
                if out.insert(dep.clone()) {
                    self.collect_transitive_dependents(dep, out);
                }
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // I2.9: DOT Visualization
    // ═══════════════════════════════════════════════════════════════════

    /// Generate DOT format for visualization.
    ///
    /// Usage: `fj build --dep-graph | dot -Tsvg > deps.svg`
    pub fn to_dot(&self) -> String {
        let mut dot = String::from("digraph dependencies {\n");
        dot.push_str("  rankdir=LR;\n");
        dot.push_str("  node [shape=box, fontname=\"monospace\", fontsize=10];\n\n");

        // Nodes with color by kind
        for (name, info) in &self.symbols {
            let color = match info.id.kind {
                SymbolKind::Function => "#4A90D9",
                SymbolKind::Struct => "#50C878",
                SymbolKind::Enum => "#FFB347",
                SymbolKind::Trait => "#DA70D6",
                SymbolKind::Const => "#87CEEB",
                SymbolKind::Macro => "#FF6B6B",
                SymbolKind::TraitImpl => "#DDA0DD",
            };
            let label = format!("{}\\n({})", info.id.name, info.id.kind);
            dot.push_str(&format!(
                "  \"{}\" [label=\"{}\", style=filled, fillcolor=\"{}\"];\n",
                name, label, color
            ));
        }

        dot.push('\n');

        // Edges
        for (from, tos) in &self.edges {
            for to in tos {
                dot.push_str(&format!("  \"{}\" -> \"{}\";\n", from, to));
            }
        }

        dot.push_str("}\n");
        dot
    }

    /// Total symbol count.
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Total edge count.
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|e| e.len()).sum()
    }
}

/// Set of detected changes between two graph snapshots.
#[derive(Debug, Clone, Default)]
pub struct ChangeSet {
    /// Symbols whose signature changed (callers must recompile).
    pub signature_changed: HashSet<String>,
    /// Symbols where only the body changed (only themselves recompile).
    pub body_only_changed: HashSet<String>,
    /// Newly added symbols.
    pub added: HashSet<String>,
    /// Removed symbols.
    pub removed: HashSet<String>,
}

impl ChangeSet {
    /// Total number of changes.
    pub fn total_changes(&self) -> usize {
        self.signature_changed.len()
            + self.body_only_changed.len()
            + self.added.len()
            + self.removed.len()
    }

    /// Whether there are no changes.
    pub fn is_empty(&self) -> bool {
        self.total_changes() == 0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — I2.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── I2.1: Function-level tracking ──

    #[test]
    fn i2_1_function_dependency_tracking() {
        let mut g = FineGrainedGraph::new();
        g.add_function(
            "main.fj",
            "main",
            "fn main()",
            "print(hello())",
            &["hello", "print"],
        );
        g.add_function("main.fj", "hello", "fn hello() -> str", "\"hi\"", &[]);

        assert_eq!(g.symbol_count(), 2);
        // main depends on hello
        let main_q = "main.fj::main";
        let deps = g.edges.get(main_q).unwrap();
        assert!(deps.contains("main.fj::hello"));
    }

    #[test]
    fn i2_1_body_change_only_recompiles_self() {
        let mut old = FineGrainedGraph::new();
        old.add_function("a.fj", "foo", "fn foo() -> i64", "42", &[]);
        old.add_function("a.fj", "bar", "fn bar() -> i64", "foo()", &["foo"]);

        let mut new = FineGrainedGraph::new();
        new.add_function("a.fj", "foo", "fn foo() -> i64", "99", &[]); // body changed
        new.add_function("a.fj", "bar", "fn bar() -> i64", "foo()", &["foo"]);

        let changes = new.detect_changes(&old);
        assert!(changes.body_only_changed.contains("a.fj::foo"));
        assert!(changes.signature_changed.is_empty());

        let recomp = new.recompilation_set(&changes);
        assert!(recomp.contains("a.fj::foo"));
        assert!(!recomp.contains("a.fj::bar")); // bar not affected by body-only change
    }

    // ── I2.2: Type-level tracking ──

    #[test]
    fn i2_2_type_change_tracking() {
        let mut old = FineGrainedGraph::new();
        old.add_type(
            "a.fj",
            "Point",
            SymbolKind::Struct,
            "struct Point { x: f64, y: f64 }",
        );

        let mut new = FineGrainedGraph::new();
        new.add_type(
            "a.fj",
            "Point",
            SymbolKind::Struct,
            "struct Point { x: f64, y: f64, z: f64 }",
        );

        let changes = new.detect_changes(&old);
        assert!(changes.signature_changed.contains("a.fj::Point"));
    }

    // ── I2.3: Import graph ──

    #[test]
    fn i2_3_import_graph() {
        let mut g = FineGrainedGraph::new();
        g.add_import("main.fj", "lib.fj");
        g.add_import("main.fj", "utils.fj");
        g.add_import("lib.fj", "core.fj");

        assert_eq!(g.import_graph.get("main.fj").unwrap().len(), 2);
        assert!(g.import_graph.get("main.fj").unwrap().contains("lib.fj"));
        assert!(g.import_graph.get("lib.fj").unwrap().contains("core.fj"));
    }

    // ── I2.4: Signature vs body change ──

    #[test]
    fn i2_4_signature_change_recompiles_callers() {
        let mut old = FineGrainedGraph::new();
        old.add_function("a.fj", "compute", "fn compute(x: i64) -> i64", "x * 2", &[]);
        old.add_function("a.fj", "main", "fn main()", "compute(5)", &["compute"]);

        let mut new = FineGrainedGraph::new();
        // Signature changed: added parameter
        new.add_function(
            "a.fj",
            "compute",
            "fn compute(x: i64, y: i64) -> i64",
            "x * y",
            &[],
        );
        new.add_function("a.fj", "main", "fn main()", "compute(5)", &["compute"]);

        let changes = new.detect_changes(&old);
        assert!(changes.signature_changed.contains("a.fj::compute"));

        let recomp = new.recompilation_set(&changes);
        assert!(recomp.contains("a.fj::compute"));
        assert!(recomp.contains("a.fj::main")); // caller must recompile
    }

    // ── I2.5: Trait impl tracking ──

    #[test]
    fn i2_5_trait_impl_tracking() {
        let mut g = FineGrainedGraph::new();
        g.add_trait_impl("a.fj", "Display", "Point", "fn fmt() { ... }");

        let key = ("Display".to_string(), "Point".to_string());
        assert!(g.trait_impls.contains_key(&key));
        assert_eq!(g.symbol_count(), 1);
    }

    #[test]
    fn i2_5_trait_impl_change_detected() {
        let mut old = FineGrainedGraph::new();
        old.add_trait_impl("a.fj", "Display", "Point", "fn fmt() { v1 }");

        let mut new = FineGrainedGraph::new();
        new.add_trait_impl("a.fj", "Display", "Point", "fn fmt() { v2 }");

        let changes = new.detect_changes(&old);
        assert!(!changes.is_empty());
    }

    // ── I2.6: Constant tracking ──

    #[test]
    fn i2_6_const_change_detected() {
        let mut old = FineGrainedGraph::new();
        old.add_const("a.fj", "MAX", "1024");

        let mut new = FineGrainedGraph::new();
        new.add_const("a.fj", "MAX", "2048");

        let changes = new.detect_changes(&old);
        assert!(changes.signature_changed.contains("a.fj::MAX"));
    }

    #[test]
    fn i2_6_const_unchanged() {
        let mut old = FineGrainedGraph::new();
        old.add_const("a.fj", "PI", "3.14159");

        let mut new = FineGrainedGraph::new();
        new.add_const("a.fj", "PI", "3.14159");

        let changes = new.detect_changes(&old);
        assert!(changes.is_empty());
    }

    // ── I2.7: Macro expansion tracking ──

    #[test]
    fn i2_7_macro_expansion_tracking() {
        let mut g = FineGrainedGraph::new();
        g.add_macro_expansion("main.fj:10", "println(\"hello\")");
        g.add_macro_expansion("main.fj:20", "vec![1, 2, 3]");

        assert_eq!(g.macro_expansions.len(), 2);
    }

    // ── I2.8: Cross-module type inference ──

    #[test]
    fn i2_8_cross_module_type_tracking() {
        let mut g = FineGrainedGraph::new();
        g.add_cross_module_type("main.fj::x", "lib.fj::MyStruct");

        assert_eq!(
            g.cross_module_types.get("main.fj::x"),
            Some(&"lib.fj::MyStruct".to_string())
        );
    }

    // ── I2.9: DOT visualization ──

    #[test]
    fn i2_9_dot_output() {
        let mut g = FineGrainedGraph::new();
        g.add_function("a.fj", "main", "fn main()", "hello()", &["hello"]);
        g.add_function("a.fj", "hello", "fn hello() -> str", "\"hi\"", &[]);

        let dot = g.to_dot();
        assert!(dot.contains("digraph dependencies"));
        assert!(dot.contains("a.fj::main"));
        assert!(dot.contains("a.fj::hello"));
        assert!(dot.contains("->"));
    }

    // ── I2.10: Integration — full change detection pipeline ──

    #[test]
    fn i2_10_full_change_detection() {
        // Old state: 3 functions
        let mut old = FineGrainedGraph::new();
        old.add_function("m.fj", "a", "fn a() -> i64", "1", &[]);
        old.add_function("m.fj", "b", "fn b() -> i64", "a()", &["a"]);
        old.add_function("m.fj", "c", "fn c() -> i64", "b()", &["b"]);
        old.add_type(
            "m.fj",
            "Point",
            SymbolKind::Struct,
            "struct Point { x: f64 }",
        );

        // New state: 'a' body changed, 'Point' field added
        let mut new = FineGrainedGraph::new();
        new.add_function("m.fj", "a", "fn a() -> i64", "2", &[]); // body only
        new.add_function("m.fj", "b", "fn b() -> i64", "a()", &["a"]);
        new.add_function("m.fj", "c", "fn c() -> i64", "b()", &["b"]);
        new.add_type(
            "m.fj",
            "Point",
            SymbolKind::Struct,
            "struct Point { x: f64, y: f64 }",
        ); // sig change

        let changes = new.detect_changes(&old);
        assert_eq!(changes.body_only_changed.len(), 1); // a body
        assert_eq!(changes.signature_changed.len(), 1); // Point sig

        let recomp = new.recompilation_set(&changes);
        assert!(recomp.contains("m.fj::a")); // body changed
        assert!(recomp.contains("m.fj::Point")); // sig changed
        assert!(!recomp.contains("m.fj::b")); // not affected by body-only change in a
        assert!(!recomp.contains("m.fj::c"));
    }

    #[test]
    fn i2_10_added_and_removed_symbols() {
        let mut old = FineGrainedGraph::new();
        old.add_function("m.fj", "old_fn", "fn old_fn()", "body", &[]);
        old.add_function("m.fj", "stable", "fn stable()", "body", &["old_fn"]);

        let mut new = FineGrainedGraph::new();
        new.add_function("m.fj", "new_fn", "fn new_fn()", "body", &[]);
        new.add_function("m.fj", "stable", "fn stable()", "body", &["old_fn"]);

        let changes = new.detect_changes(&old);
        assert!(changes.added.contains("m.fj::new_fn"));
        assert!(changes.removed.contains("m.fj::old_fn"));

        let recomp = new.recompilation_set(&changes);
        assert!(recomp.contains("m.fj::new_fn")); // new symbol
        assert!(recomp.contains("m.fj::stable")); // depended on removed old_fn
    }
}
