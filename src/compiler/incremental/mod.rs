//! # Incremental Compilation — Dependency Graph
//!
//! Tracks file-level dependencies, detects changes via content hashing,
//! computes transitive dependents for minimal recompilation, and provides
//! topological ordering for correct build sequencing.
//!
//! ## Architecture
//!
//! ```text
//! Source files → DependencyGraph → detect_changes → transitive_dependents
//!                                                  → topological_sort
//! ```

pub mod bench;
pub mod cache;
pub mod disk;
pub mod edge_cases;
pub mod fine_grained;
pub mod integration;
pub mod ir_cache;
pub mod parallel;
pub mod pipeline;
pub mod rebuild_bench;

use std::collections::{HashMap, HashSet, VecDeque};

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors arising from incremental compilation operations.
#[derive(Debug, thiserror::Error)]
pub enum IncrementalError {
    /// A circular dependency was detected among source files.
    #[error("circular dependency detected: {cycle}")]
    CyclicDependency {
        /// Human-readable description of the cycle (e.g. "a.fj -> b.fj -> a.fj").
        cycle: String,
    },

    /// A referenced file was not found in the dependency graph.
    #[error("file not found in dependency graph: {path}")]
    FileNotFound {
        /// The missing file path.
        path: String,
    },

    /// An I/O error occurred while reading source files or cache.
    #[error("io error: {message}")]
    IoError {
        /// Description of the I/O failure.
        message: String,
    },

    /// The artifact cache is corrupted or contains invalid data.
    #[error("cache corruption: {message}")]
    CacheCorruption {
        /// Description of the corruption.
        message: String,
    },

    /// A hashing operation failed.
    #[error("hash error: {message}")]
    HashError {
        /// Description of the hash failure.
        message: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// FileNode
// ═══════════════════════════════════════════════════════════════════════

/// Represents a single source file in the dependency graph.
///
/// Each node stores metadata about the file including its content hash
/// for change detection, its import dependencies, and its exported symbols.
#[derive(Debug, Clone)]
pub struct FileNode {
    /// The file path (relative to project root).
    pub path: String,
    /// Deterministic hash of the file's normalized content.
    pub content_hash: String,
    /// Paths of files this file imports via `use` or `mod` statements.
    pub dependencies: Vec<String>,
    /// Symbols exported by this file (function/struct/enum names).
    pub exports: Vec<String>,
}

impl FileNode {
    /// Creates a new `FileNode` with the given metadata.
    pub fn new(
        path: String,
        content_hash: String,
        dependencies: Vec<String>,
        exports: Vec<String>,
    ) -> Self {
        Self {
            path,
            content_hash,
            dependencies,
            exports,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DependencyGraph
// ═══════════════════════════════════════════════════════════════════════

/// A directed graph of file-level dependencies for incremental compilation.
///
/// Nodes are source files; edges point from importer to importee.
/// Supports change detection, transitive dependent analysis, topological
/// sorting, and cycle detection.
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// Map from file path to its node metadata.
    pub nodes: HashMap<String, FileNode>,
    /// Forward edges: file -> files it depends on.
    pub edges: HashMap<String, Vec<String>>,
    /// Reverse edges: file -> files that depend on it.
    pub reverse_edges: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    /// Creates an empty dependency graph.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
        }
    }

    /// Adds a file node to the graph and registers its dependency edges.
    ///
    /// If a file with the same path already exists, it is replaced.
    pub fn add_file(
        &mut self,
        path: String,
        hash: String,
        deps: Vec<String>,
        exports: Vec<String>,
    ) {
        // Remove old reverse edges if replacing
        if let Some(old_node) = self.nodes.get(&path) {
            for dep in &old_node.dependencies {
                if let Some(rev) = self.reverse_edges.get_mut(dep) {
                    rev.retain(|p| p != &path);
                }
            }
        }

        // Register forward edges
        self.edges.insert(path.clone(), deps.clone());

        // Register reverse edges
        for dep in &deps {
            self.reverse_edges
                .entry(dep.clone())
                .or_default()
                .push(path.clone());
        }

        let node = FileNode::new(path.clone(), hash, deps, exports);
        self.nodes.insert(path, node);
    }

    /// Returns the list of files that directly depend on (import) the given file.
    ///
    /// These are the files that would need recompilation if `path` changes.
    pub fn dependents(&self, path: &str) -> Vec<String> {
        self.reverse_edges.get(path).cloned().unwrap_or_default()
    }

    /// Returns the list of files that the given file directly imports.
    pub fn dependencies(&self, path: &str) -> Vec<String> {
        self.edges.get(path).cloned().unwrap_or_default()
    }

    /// Returns the total number of files in the graph.
    pub fn file_count(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Content hashing
// ═══════════════════════════════════════════════════════════════════════

/// Computes a deterministic hash of normalized source content.
///
/// Normalization strips trailing whitespace and normalizes line endings
/// to ensure consistent hashing across platforms.
pub fn compute_content_hash(source: &str) -> String {
    // Simple but deterministic: FNV-1a inspired hash
    // We normalize: trim trailing whitespace per line, unify line endings
    let normalized = normalize_source(source);
    let hash = fnv1a_hash(normalized.as_bytes());
    format!("{hash:016x}")
}

/// Normalizes source text for consistent hashing.
fn normalize_source(source: &str) -> String {
    source
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// FNV-1a 64-bit hash for deterministic content hashing.
fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x00000100000001B3);
    }
    hash
}

// ═══════════════════════════════════════════════════════════════════════
// Dependency extraction
// ═══════════════════════════════════════════════════════════════════════

/// Builds a dependency graph from a set of source files.
///
/// Parses `use` and `mod` statements from each file's source to determine
/// import relationships. Files are provided as `(path, source_content)` pairs.
pub fn build_dependency_graph(files: &[(String, String)]) -> DependencyGraph {
    let mut graph = DependencyGraph::new();

    for (path, source) in files {
        let hash = compute_content_hash(source);
        let deps = extract_dependencies(source);
        let exports = extract_exports(source);
        graph.add_file(path.clone(), hash, deps, exports);
    }

    graph
}

/// Extracts dependency paths from `use` and `mod` statements in source.
fn extract_dependencies(source: &str) -> Vec<String> {
    let mut deps = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("use ") {
            if let Some(module_path) = parse_use_path(rest) {
                deps.push(module_path);
            }
        } else if let Some(rest) = trimmed.strip_prefix("mod ") {
            if let Some(module_name) = parse_mod_decl(rest) {
                deps.push(module_name);
            }
        }
    }

    deps
}

/// Parses a use statement to extract the module file path.
///
/// Converts `use std::math` -> `std/math.fj`
fn parse_use_path(rest: &str) -> Option<String> {
    let path_str = rest.trim_end_matches(';').trim();
    if path_str.is_empty() {
        return None;
    }
    // Convert :: separators to /
    let file_path = path_str.replace("::", "/");
    Some(format!("{file_path}.fj"))
}

/// Parses a mod declaration to extract the module name.
fn parse_mod_decl(rest: &str) -> Option<String> {
    let name = rest
        .trim_end_matches(';')
        .trim()
        .trim_end_matches(" {")
        .trim();
    if name.is_empty() || name.contains(' ') {
        return None;
    }
    Some(format!("{name}.fj"))
}

/// Extracts exported symbol names (fn, struct, enum with pub).
fn extract_exports(source: &str) -> Vec<String> {
    let mut exports = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        // pub fn name(...)
        if let Some(rest) = trimmed.strip_prefix("pub fn ") {
            if let Some(name) = rest.split('(').next() {
                let name = name.split('<').next().unwrap_or(name).trim();
                if !name.is_empty() {
                    exports.push(name.to_string());
                }
            }
        }
        // pub struct Name
        else if let Some(rest) = trimmed.strip_prefix("pub struct ") {
            if let Some(name) = extract_type_name(rest) {
                exports.push(name);
            }
        }
        // pub enum Name
        else if let Some(rest) = trimmed.strip_prefix("pub enum ") {
            if let Some(name) = extract_type_name(rest) {
                exports.push(name);
            }
        }
        // fn name(...) — also exported in Fajar Lang (pub is default for legacy)
        else if let Some(rest) = trimmed.strip_prefix("fn ") {
            if let Some(name) = rest.split('(').next() {
                let name = name.split('<').next().unwrap_or(name).trim();
                if !name.is_empty() {
                    exports.push(name.to_string());
                }
            }
        }
    }

    exports
}

/// Extracts a type name from the rest of a struct/enum declaration.
fn extract_type_name(rest: &str) -> Option<String> {
    let name = rest
        .split(|c: char| c == '{' || c == '<' || c == '(' || c.is_whitespace())
        .next()?
        .trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

// ═══════════════════════════════════════════════════════════════════════
// Change detection
// ═══════════════════════════════════════════════════════════════════════

/// Detects files that have changed between two snapshots of the dependency graph.
///
/// A file is considered changed if its content hash differs or if it
/// exists in only one of the two graphs.
pub fn detect_changes(old: &DependencyGraph, new: &DependencyGraph) -> Vec<String> {
    let mut changed = Vec::new();

    // Files in new that differ from old (or are new)
    for (path, new_node) in &new.nodes {
        match old.nodes.get(path) {
            Some(old_node) if old_node.content_hash == new_node.content_hash => {}
            _ => changed.push(path.clone()),
        }
    }

    // Files removed in new
    for path in old.nodes.keys() {
        if !new.nodes.contains_key(path) {
            changed.push(path.clone());
        }
    }

    changed.sort();
    changed
}

/// Computes all files that transitively depend on the changed files.
///
/// Uses BFS over reverse edges to find every file that needs recompilation
/// when the given set of files changes.
pub fn transitive_dependents(graph: &DependencyGraph, changed: &[String]) -> Vec<String> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    // Seed with direct changed files
    for path in changed {
        if !visited.contains(path) {
            visited.insert(path.clone());
            queue.push_back(path.clone());
        }
    }

    // BFS over reverse edges
    while let Some(current) = queue.pop_front() {
        let deps = graph.dependents(&current);
        for dep in deps {
            if visited.insert(dep.clone()) {
                queue.push_back(dep);
            }
        }
    }

    let mut result: Vec<String> = visited.into_iter().collect();
    result.sort();
    result
}

/// Returns a topological ordering of files for correct compilation order.
///
/// Files with no dependencies come first. Returns an error if the graph
/// contains cycles.
pub fn topological_sort(graph: &DependencyGraph) -> Result<Vec<String>, IncrementalError> {
    let (mut in_degree, adj) = build_topo_structures(graph);

    // Seed queue with zero in-degree nodes (sorted for determinism)
    let mut seeds: Vec<String> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(path, _)| path.clone())
        .collect();
    seeds.sort();

    let mut queue: VecDeque<String> = seeds.into_iter().collect();
    let mut result = Vec::new();

    while let Some(node) = queue.pop_front() {
        result.push(node.clone());
        process_topo_neighbors(&node, &adj, &mut in_degree, &mut queue);
    }

    if result.len() != graph.nodes.len() {
        let cycle_desc = find_cycle_description(graph);
        return Err(IncrementalError::CyclicDependency { cycle: cycle_desc });
    }

    Ok(result)
}

/// Builds in-degree map and adjacency list for topological sort.
fn build_topo_structures(
    graph: &DependencyGraph,
) -> (HashMap<String, usize>, HashMap<String, Vec<String>>) {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();

    for path in graph.nodes.keys() {
        in_degree.entry(path.clone()).or_insert(0);
        adj.entry(path.clone()).or_default();
    }

    for (file, deps) in &graph.edges {
        for dep in deps {
            if graph.nodes.contains_key(dep) {
                adj.entry(dep.clone()).or_default().push(file.clone());
                *in_degree.entry(file.clone()).or_insert(0) += 1;
            }
        }
    }

    (in_degree, adj)
}

/// Processes neighbors of a node during topological sort (Kahn's algorithm).
fn process_topo_neighbors(
    node: &str,
    adj: &HashMap<String, Vec<String>>,
    in_degree: &mut HashMap<String, usize>,
    queue: &mut VecDeque<String>,
) {
    if let Some(neighbors) = adj.get(node) {
        let mut sorted = neighbors.clone();
        sorted.sort();
        for neighbor in sorted {
            if let Some(deg) = in_degree.get_mut(&neighbor) {
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(neighbor);
                }
            }
        }
    }
}

/// Finds a cycle in the graph and returns a human-readable description.
fn find_cycle_description(graph: &DependencyGraph) -> String {
    let mut visited: HashSet<String> = HashSet::new();
    let mut on_stack: HashSet<String> = HashSet::new();
    let mut parent: HashMap<String, String> = HashMap::new();

    for start in graph.nodes.keys() {
        if visited.contains(start) {
            continue;
        }
        let mut stack = vec![start.clone()];
        while let Some(node) = stack.pop() {
            if on_stack.contains(&node) {
                // Found cycle — reconstruct it
                return reconstruct_cycle(&node, &parent);
            }
            if visited.contains(&node) {
                continue;
            }
            visited.insert(node.clone());
            on_stack.insert(node.clone());

            if let Some(deps) = graph.edges.get(&node) {
                for dep in deps {
                    if graph.nodes.contains_key(dep) && !visited.contains(dep) {
                        parent.insert(dep.clone(), node.clone());
                        stack.push(dep.clone());
                    } else if on_stack.contains(dep) {
                        parent.insert(dep.clone(), node.clone());
                        return reconstruct_cycle(dep, &parent);
                    }
                }
            }
        }
        on_stack.clear();
    }

    "unknown cycle".to_string()
}

/// Reconstructs a cycle path from parent links.
fn reconstruct_cycle(cycle_node: &str, parent: &HashMap<String, String>) -> String {
    let mut path = vec![cycle_node.to_string()];
    let mut current = cycle_node.to_string();

    for _ in 0..parent.len() {
        if let Some(p) = parent.get(&current) {
            if p == cycle_node {
                path.push(p.clone());
                break;
            }
            path.push(p.clone());
            current = p.clone();
        } else {
            break;
        }
    }

    path.reverse();
    path.join(" -> ")
}

// ═══════════════════════════════════════════════════════════════════════
// Function-level dependency tracking
// ═══════════════════════════════════════════════════════════════════════

/// A function node in the function-level dependency graph.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionNode {
    /// Fully-qualified function name (e.g., `"main"`, `"math::add"`).
    pub name: String,
    /// Containing file path.
    pub file: String,
    /// Content hash of the function body (for change detection).
    pub body_hash: String,
    /// Functions this function calls (callees).
    pub calls: Vec<String>,
    /// Whether this function is a `const fn` / `comptime fn`.
    pub is_const: bool,
}

/// Function-level dependency graph for fine-grained incremental compilation.
///
/// Tracks which functions call which, allowing recompilation of only the
/// functions affected by a change rather than entire files.
#[derive(Debug, Clone)]
pub struct FunctionGraph {
    /// Function nodes by name.
    pub functions: HashMap<String, FunctionNode>,
    /// Reverse call graph: function → callers.
    pub callers: HashMap<String, Vec<String>>,
}

impl FunctionGraph {
    /// Creates an empty function graph.
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            callers: HashMap::new(),
        }
    }

    /// Adds a function to the graph.
    pub fn add_function(&mut self, node: FunctionNode) {
        let name = node.name.clone();
        // Build reverse edges
        for callee in &node.calls {
            self.callers
                .entry(callee.clone())
                .or_default()
                .push(name.clone());
        }
        self.functions.insert(name, node);
    }

    /// Returns functions that directly call the given function.
    pub fn callers_of(&self, name: &str) -> Vec<String> {
        self.callers.get(name).cloned().unwrap_or_default()
    }

    /// Detects which functions changed between two snapshots.
    pub fn detect_changed_functions(&self, old: &FunctionGraph) -> Vec<String> {
        let mut changed = Vec::new();
        for (name, node) in &self.functions {
            match old.functions.get(name) {
                Some(old_node) if old_node.body_hash == node.body_hash => {}
                _ => changed.push(name.clone()),
            }
        }
        // Removed functions
        for name in old.functions.keys() {
            if !self.functions.contains_key(name) {
                changed.push(name.clone());
            }
        }
        changed.sort();
        changed
    }

    /// Returns all functions transitively affected by changes to the given functions.
    pub fn transitive_callers(&self, changed: &[String]) -> Vec<String> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        for name in changed {
            if visited.insert(name.clone()) {
                queue.push_back(name.clone());
            }
        }

        while let Some(current) = queue.pop_front() {
            for caller in self.callers_of(&current) {
                if visited.insert(caller.clone()) {
                    queue.push_back(caller);
                }
            }
        }

        let mut result: Vec<String> = visited.into_iter().collect();
        result.sort();
        result
    }

    /// Returns the number of functions in the graph.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }
}

impl Default for FunctionGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds a function graph from source code by extracting function definitions and calls.
pub fn build_function_graph(files: &[(String, String)]) -> FunctionGraph {
    let mut graph = FunctionGraph::new();

    for (file_path, source) in files {
        extract_function_nodes(file_path, source, &mut graph);
    }

    graph
}

/// Extracts function definitions and their call dependencies from source.
fn extract_function_nodes(file_path: &str, source: &str, graph: &mut FunctionGraph) {
    let mut current_fn: Option<(String, usize, bool)> = None; // (name, brace_depth, is_const)
    let mut brace_depth: usize = 0;
    let mut fn_body = String::new();
    let mut fn_calls = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();

        // Track brace depth
        let open_count = trimmed.chars().filter(|c| *c == '{').count();
        let close_count = trimmed.chars().filter(|c| *c == '}').count();

        // Detect function start
        if current_fn.is_none() {
            let is_const = trimmed.starts_with("const fn ")
                || trimmed.starts_with("pub const fn ")
                || trimmed.starts_with("comptime fn ")
                || trimmed.starts_with("pub comptime fn ");

            let fn_name = extract_fn_name(trimmed);
            if let Some(name) = fn_name {
                current_fn = Some((name, brace_depth, is_const));
                fn_body.clear();
                fn_calls.clear();
            }
        }

        brace_depth += open_count;
        brace_depth = brace_depth.saturating_sub(close_count);

        if let Some((ref name, start_depth, is_const)) = current_fn {
            fn_body.push_str(line);
            fn_body.push('\n');

            // Extract function calls from this line
            extract_calls_from_line(trimmed, &mut fn_calls);

            // Function ends when we return to the starting brace depth
            if brace_depth <= start_depth && fn_body.contains('{') {
                let body_hash = compute_content_hash(&fn_body);
                let node = FunctionNode {
                    name: name.clone(),
                    file: file_path.to_string(),
                    body_hash,
                    calls: fn_calls.clone(),
                    is_const,
                };
                graph.add_function(node);
                current_fn = None;
            }
        }
    }
}

/// Extracts function name from a line like `fn foo(...)` or `pub fn bar(...)`.
fn extract_fn_name(line: &str) -> Option<String> {
    // Strip annotations and modifiers
    let stripped = line
        .trim_start_matches("pub ")
        .trim_start_matches("const ")
        .trim_start_matches("comptime ")
        .trim_start_matches("async ");

    if let Some(rest) = stripped.strip_prefix("fn ") {
        let name = rest.split(['(', '<']).next()?.trim();
        if !name.is_empty() {
            let first = name.as_bytes()[0];
            if first.is_ascii_alphabetic() || first == b'_' {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Extracts function call names from a source line (simple heuristic).
fn extract_calls_from_line(line: &str, calls: &mut Vec<String>) {
    // Simple pattern: identifier followed by (
    let chars = line.chars();
    let mut current_ident = String::new();

    for ch in chars {
        if ch.is_alphanumeric() || ch == '_' {
            current_ident.push(ch);
        } else {
            if ch == '(' && !current_ident.is_empty() {
                // Skip keywords that look like calls
                if !matches!(
                    current_ident.as_str(),
                    "if" | "while"
                        | "for"
                        | "match"
                        | "loop"
                        | "return"
                        | "let"
                        | "fn"
                        | "struct"
                        | "enum"
                ) {
                    if !calls.contains(&current_ident) {
                        calls.push(current_ident.clone());
                    }
                }
            }
            current_ident.clear();
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Disk-persistent cache
// ═══════════════════════════════════════════════════════════════════════

/// Saves a dependency graph snapshot to disk as JSON.
pub fn save_graph_snapshot(
    graph: &DependencyGraph,
    cache_dir: &str,
) -> Result<(), IncrementalError> {
    let dir = std::path::Path::new(cache_dir);
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| IncrementalError::IoError {
            message: format!("cannot create cache dir: {e}"),
        })?;
    }

    let path = dir.join("dep_graph.json");
    let mut data = String::from("{\n");

    // Serialize nodes as simple JSON
    data.push_str("  \"files\": {\n");
    let mut first = true;
    for (file_path, node) in &graph.nodes {
        if !first {
            data.push_str(",\n");
        }
        first = false;
        data.push_str(&format!(
            "    \"{}\": {{\"hash\": \"{}\", \"deps\": [{}], \"exports\": [{}]}}",
            file_path,
            node.content_hash,
            node.dependencies
                .iter()
                .map(|d| format!("\"{d}\""))
                .collect::<Vec<_>>()
                .join(", "),
            node.exports
                .iter()
                .map(|e| format!("\"{e}\""))
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    data.push_str("\n  }\n}\n");

    std::fs::write(&path, &data).map_err(|e| IncrementalError::IoError {
        message: format!("cannot write graph snapshot: {e}"),
    })?;

    Ok(())
}

/// Loads a dependency graph snapshot from disk.
pub fn load_graph_snapshot(cache_dir: &str) -> Result<DependencyGraph, IncrementalError> {
    let path = std::path::Path::new(cache_dir).join("dep_graph.json");
    if !path.exists() {
        return Ok(DependencyGraph::new());
    }

    let data = std::fs::read_to_string(&path).map_err(|e| IncrementalError::IoError {
        message: format!("cannot read graph snapshot: {e}"),
    })?;

    // Simple JSON parser for our format (avoid serde_json dependency for this)
    parse_graph_json(&data)
}

/// Parses the simple JSON format for dependency graph snapshots.
fn parse_graph_json(json: &str) -> Result<DependencyGraph, IncrementalError> {
    let mut graph = DependencyGraph::new();

    // Extract file entries using simple string parsing
    // Format: "filepath": {"hash": "...", "deps": [...], "exports": [...]}
    for line in json.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if trimmed.starts_with('"') && trimmed.contains("\"hash\"") {
            if let Some((path, rest)) = trimmed.split_once(": {") {
                let file_path = path.trim().trim_matches('"').to_string();
                let rest = rest.trim_end_matches('}');

                let hash = extract_json_string(rest, "hash").unwrap_or_default();
                let deps = extract_json_array(rest, "deps");
                let exports = extract_json_array(rest, "exports");

                graph.add_file(file_path, hash, deps, exports);
            }
        }
    }

    Ok(graph)
}

/// Extracts a string value from simple JSON.
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\": \"");
    let start = json.find(&pattern)? + pattern.len();
    let end = json[start..].find('"')? + start;
    Some(json[start..end].to_string())
}

/// Extracts a string array from simple JSON.
fn extract_json_array(json: &str, key: &str) -> Vec<String> {
    let pattern = format!("\"{key}\": [");
    let start = match json.find(&pattern) {
        Some(s) => s + pattern.len(),
        None => return Vec::new(),
    };
    let end = match json[start..].find(']') {
        Some(e) => e + start,
        None => return Vec::new(),
    };
    let inner = &json[start..end];
    inner
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s9_1_file_node_creation() {
        let node = FileNode::new(
            "main.fj".to_string(),
            "abc123".to_string(),
            vec!["math.fj".to_string()],
            vec!["main".to_string()],
        );
        assert_eq!(node.path, "main.fj");
        assert_eq!(node.content_hash, "abc123");
        assert_eq!(node.dependencies, vec!["math.fj"]);
        assert_eq!(node.exports, vec!["main"]);
    }

    #[test]
    fn s9_2_add_file_and_query_dependents() {
        let mut graph = DependencyGraph::new();
        graph.add_file(
            "math.fj".to_string(),
            "h1".to_string(),
            vec![],
            vec!["add".to_string()],
        );
        graph.add_file(
            "main.fj".to_string(),
            "h2".to_string(),
            vec!["math.fj".to_string()],
            vec!["main".to_string()],
        );

        // math.fj has main.fj as a dependent
        assert_eq!(graph.dependents("math.fj"), vec!["main.fj"]);
        // main.fj depends on math.fj
        assert_eq!(graph.dependencies("main.fj"), vec!["math.fj"]);
        // math.fj has no dependencies
        assert!(graph.dependencies("math.fj").is_empty());
    }

    #[test]
    fn s9_3_build_dependency_graph_from_source() {
        let files = vec![
            ("main.fj".to_string(), "use math\nfn main() { }".to_string()),
            (
                "math.fj".to_string(),
                "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            ),
        ];
        let graph = build_dependency_graph(&files);

        assert_eq!(graph.file_count(), 2);
        assert!(graph.nodes.contains_key("main.fj"));
        assert!(graph.nodes.contains_key("math.fj"));

        // main.fj should have a dependency on math.fj
        let main_deps = graph.dependencies("main.fj");
        assert_eq!(main_deps, vec!["math.fj"]);

        // math.fj exports "add"
        let math_node = graph.nodes.get("math.fj").expect("math.fj exists");
        assert!(math_node.exports.contains(&"add".to_string()));
    }

    #[test]
    fn s9_4_content_hash_deterministic() {
        let source = "fn main() {\n    let x = 42\n}\n";
        let hash1 = compute_content_hash(source);
        let hash2 = compute_content_hash(source);
        assert_eq!(hash1, hash2);

        // Trailing whitespace should not affect hash
        let source_trailing = "fn main() {  \n    let x = 42  \n}  \n";
        let hash3 = compute_content_hash(source_trailing);
        assert_eq!(hash1, hash3);
    }

    #[test]
    fn s9_5_content_hash_differs_for_different_content() {
        let hash1 = compute_content_hash("let x = 1");
        let hash2 = compute_content_hash("let x = 2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn s9_6_detect_changes_between_graphs() {
        let files_v1 = vec![
            ("a.fj".to_string(), "let x = 1".to_string()),
            ("b.fj".to_string(), "let y = 2".to_string()),
        ];
        let files_v2 = vec![
            ("a.fj".to_string(), "let x = 1".to_string()),
            ("b.fj".to_string(), "let y = 99".to_string()), // changed
            ("c.fj".to_string(), "let z = 3".to_string()),  // new
        ];

        let old = build_dependency_graph(&files_v1);
        let new = build_dependency_graph(&files_v2);
        let changed = detect_changes(&old, &new);

        assert!(changed.contains(&"b.fj".to_string()));
        assert!(changed.contains(&"c.fj".to_string()));
        assert!(!changed.contains(&"a.fj".to_string()));
    }

    #[test]
    fn s9_7_transitive_dependents_bfs() {
        let mut graph = DependencyGraph::new();
        // c depends on b, b depends on a
        graph.add_file("a.fj".into(), "h1".into(), vec![], vec![]);
        graph.add_file("b.fj".into(), "h2".into(), vec!["a.fj".into()], vec![]);
        graph.add_file("c.fj".into(), "h3".into(), vec!["b.fj".into()], vec![]);

        // Changing a.fj should trigger recompile of a, b, and c
        let deps = transitive_dependents(&graph, &["a.fj".to_string()]);
        assert!(deps.contains(&"a.fj".to_string()));
        assert!(deps.contains(&"b.fj".to_string()));
        assert!(deps.contains(&"c.fj".to_string()));
        assert_eq!(deps.len(), 3);
    }

    #[test]
    fn s9_8_topological_sort_correct_order() {
        let mut graph = DependencyGraph::new();
        graph.add_file("a.fj".into(), "h1".into(), vec![], vec![]);
        graph.add_file("b.fj".into(), "h2".into(), vec!["a.fj".into()], vec![]);
        graph.add_file("c.fj".into(), "h3".into(), vec!["b.fj".into()], vec![]);

        let order = topological_sort(&graph).expect("no cycles");

        // a.fj must come before b.fj, b.fj must come before c.fj
        let pos_a = order.iter().position(|p| p == "a.fj").expect("a present");
        let pos_b = order.iter().position(|p| p == "b.fj").expect("b present");
        let pos_c = order.iter().position(|p| p == "c.fj").expect("c present");
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn s9_9_cycle_detection_returns_error() {
        let mut graph = DependencyGraph::new();
        graph.add_file("a.fj".into(), "h1".into(), vec!["b.fj".into()], vec![]);
        graph.add_file("b.fj".into(), "h2".into(), vec!["a.fj".into()], vec![]);

        let result = topological_sort(&graph);
        assert!(result.is_err());
        match result {
            Err(IncrementalError::CyclicDependency { cycle }) => {
                assert!(cycle.contains("a.fj") || cycle.contains("b.fj"));
            }
            _ => panic!("expected CyclicDependency error"),
        }
    }

    #[test]
    fn s9_10_empty_graph_operations() {
        let graph = DependencyGraph::new();
        assert_eq!(graph.file_count(), 0);
        assert!(graph.dependents("nonexistent.fj").is_empty());
        assert!(graph.dependencies("nonexistent.fj").is_empty());

        let order = topological_sort(&graph).expect("empty graph is valid");
        assert!(order.is_empty());

        let empty_old = DependencyGraph::new();
        let empty_new = DependencyGraph::new();
        let changes = detect_changes(&empty_old, &empty_new);
        assert!(changes.is_empty());
    }
}
