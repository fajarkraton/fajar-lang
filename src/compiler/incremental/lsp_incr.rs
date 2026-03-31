//! Incremental LSP integration — real-time analysis with sub-200ms response times.
//!
//! Uses the incremental compilation infrastructure to provide fast:
//! - On-type diagnostics (< 200ms after keystroke)
//! - Per-function reanalysis (< 50ms)
//! - Background workspace indexing (< 5s)
//! - Incremental symbol index and completion
//! - Memory-efficient shared caching between LSP and compiler

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use super::compute_content_hash;
use super::fine_grained::FineGrainedGraph;

// ═══════════════════════════════════════════════════════════════════════
// I8.1: Incremental Analysis Engine
// ═══════════════════════════════════════════════════════════════════════

/// LSP incremental analysis state for a workspace.
#[derive(Debug, Clone, Default)]
pub struct LspAnalysisState {
    /// File path → analyzed content hash.
    pub analyzed_hashes: HashMap<String, String>,
    /// File path → per-function analysis results.
    pub function_analyses: HashMap<String, Vec<FunctionAnalysis>>,
    /// Global symbol index.
    pub symbol_index: SymbolIndex,
    /// Cached diagnostics per file.
    pub diagnostics: HashMap<String, Vec<LspDiagnostic>>,
    /// Whether the full workspace has been indexed.
    pub workspace_indexed: bool,
    /// Total analysis time spent (for metrics).
    pub total_analysis_time: Duration,
}

/// Analysis result for a single function.
#[derive(Debug, Clone)]
pub struct FunctionAnalysis {
    /// Function name.
    pub name: String,
    /// Hash of the function body.
    pub body_hash: String,
    /// Whether analysis passed (no errors).
    pub ok: bool,
    /// Diagnostics for this function.
    pub diagnostics: Vec<LspDiagnostic>,
    /// Resolved types within the function.
    pub resolved_types: HashMap<String, String>,
}

/// A diagnostic for LSP.
#[derive(Debug, Clone, PartialEq)]
pub struct LspDiagnostic {
    /// File path.
    pub file: String,
    /// Line number (0-indexed).
    pub line: usize,
    /// Column number (0-indexed).
    pub col: usize,
    /// Severity.
    pub severity: LspSeverity,
    /// Message.
    pub message: String,
}

/// Diagnostic severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl LspAnalysisState {
    pub fn new() -> Self {
        Self::default()
    }

    /// I8.1: Check if a file needs re-analysis based on content change.
    pub fn needs_analysis(&self, file: &str, current_content: &str) -> bool {
        let current_hash = compute_content_hash(current_content);
        match self.analyzed_hashes.get(file) {
            Some(cached_hash) => cached_hash != &current_hash,
            None => true, // Never analyzed
        }
    }

    /// I8.1: Mark a file as analyzed.
    pub fn mark_analyzed(&mut self, file: &str, content: &str) {
        let hash = compute_content_hash(content);
        self.analyzed_hashes.insert(file.to_string(), hash);
    }

    /// I8.5: Update diagnostics for a file.
    pub fn update_diagnostics(&mut self, file: &str, diags: Vec<LspDiagnostic>) {
        self.diagnostics.insert(file.to_string(), diags);
    }

    /// I8.5: Get diagnostics for a file.
    pub fn get_diagnostics(&self, file: &str) -> &[LspDiagnostic] {
        self.diagnostics
            .get(file)
            .map(|d| d.as_slice())
            .unwrap_or(&[])
    }

    /// Get diagnostics for changed files + their dependents.
    pub fn diagnostics_for_changed(
        &self,
        changed_files: &[String],
        graph: &FineGrainedGraph,
    ) -> HashMap<String, Vec<LspDiagnostic>> {
        let mut affected = HashSet::new();
        for file in changed_files {
            affected.insert(file.clone());
            // Add files that import the changed file
            if let Some(importers) = graph.import_graph.get(file) {
                affected.extend(importers.iter().cloned());
            }
        }

        let mut result = HashMap::new();
        for file in &affected {
            if let Some(diags) = self.diagnostics.get(file) {
                result.insert(file.clone(), diags.clone());
            }
        }
        result
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I8.2: Per-Function Reanalysis
// ═══════════════════════════════════════════════════════════════════════

/// Determines which functions in a file need reanalysis after an edit.
pub fn functions_needing_reanalysis(
    file: &str,
    new_functions: &[FunctionAnalysis],
    state: &LspAnalysisState,
) -> Vec<String> {
    let cached = state.function_analyses.get(file);

    match cached {
        None => {
            // No cached analysis — all functions need analysis
            new_functions.iter().map(|f| f.name.clone()).collect()
        }
        Some(old_fns) => {
            let old_map: HashMap<&str, &str> = old_fns
                .iter()
                .map(|f| (f.name.as_str(), f.body_hash.as_str()))
                .collect();

            new_functions
                .iter()
                .filter(|f| {
                    match old_map.get(f.name.as_str()) {
                        Some(&old_hash) => old_hash != f.body_hash, // Body changed
                        None => true,                               // New function
                    }
                })
                .map(|f| f.name.clone())
                .collect()
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I8.3: Background Indexing
// ═══════════════════════════════════════════════════════════════════════

/// Background indexing task state.
#[derive(Debug, Clone)]
pub struct IndexingTask {
    /// Files to index.
    pub files: Vec<String>,
    /// Files indexed so far.
    pub indexed: usize,
    /// Whether indexing is complete.
    pub complete: bool,
    /// Start time.
    pub started_at: Instant,
}

impl IndexingTask {
    /// Create a new indexing task.
    pub fn new(files: Vec<String>) -> Self {
        Self {
            indexed: 0,
            complete: files.is_empty(),
            files,
            started_at: Instant::now(),
        }
    }

    /// Index the next file (simulate).
    pub fn index_next(&mut self) -> Option<String> {
        if self.indexed < self.files.len() {
            let file = self.files[self.indexed].clone();
            self.indexed += 1;
            if self.indexed >= self.files.len() {
                self.complete = true;
            }
            Some(file)
        } else {
            None
        }
    }

    /// Progress percentage.
    pub fn progress_pct(&self) -> f64 {
        if self.files.is_empty() {
            100.0
        } else {
            (self.indexed as f64 / self.files.len() as f64) * 100.0
        }
    }

    /// Elapsed time.
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I8.4: Incremental Symbol Index
// ═══════════════════════════════════════════════════════════════════════

/// Symbol index for go-to-definition, find-references, etc.
#[derive(Debug, Clone, Default)]
pub struct SymbolIndex {
    /// Symbol name → locations (file, line).
    pub definitions: HashMap<String, SymbolLocation>,
    /// Symbol name → usage locations.
    pub references: HashMap<String, Vec<SymbolLocation>>,
}

/// Location of a symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolLocation {
    pub file: String,
    pub line: usize,
    pub col: usize,
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a symbol definition.
    pub fn add_definition(&mut self, name: &str, file: &str, line: usize, col: usize) {
        self.definitions.insert(
            name.to_string(),
            SymbolLocation {
                file: file.into(),
                line,
                col,
            },
        );
    }

    /// Add a symbol reference.
    pub fn add_reference(&mut self, name: &str, file: &str, line: usize, col: usize) {
        self.references
            .entry(name.to_string())
            .or_default()
            .push(SymbolLocation {
                file: file.into(),
                line,
                col,
            });
    }

    /// Look up a definition.
    pub fn find_definition(&self, name: &str) -> Option<&SymbolLocation> {
        self.definitions.get(name)
    }

    /// Find all references.
    pub fn find_references(&self, name: &str) -> &[SymbolLocation] {
        self.references
            .get(name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Remove all symbols from a file (for incremental update).
    pub fn remove_file(&mut self, file: &str) {
        self.definitions.retain(|_, loc| loc.file != file);
        for refs in self.references.values_mut() {
            refs.retain(|loc| loc.file != file);
        }
    }

    /// Total symbol count (definitions).
    pub fn definition_count(&self) -> usize {
        self.definitions.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I8.6: Incremental Completion
// ═══════════════════════════════════════════════════════════════════════

/// Completion item from incremental analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct IncrCompletion {
    pub label: String,
    pub kind: IncrCompletionKind,
    pub detail: String,
    pub from_file: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncrCompletionKind {
    Function,
    Struct,
    Enum,
    Variable,
    Constant,
    Module,
}

/// Generate completions from the symbol index.
pub fn completions_from_index(prefix: &str, index: &SymbolIndex) -> Vec<IncrCompletion> {
    let lower_prefix = prefix.to_lowercase();
    index
        .definitions
        .iter()
        .filter(|(name, _)| name.to_lowercase().starts_with(&lower_prefix))
        .map(|(name, loc)| IncrCompletion {
            label: name.clone(),
            kind: IncrCompletionKind::Function, // simplified
            detail: format!("defined in {}", loc.file),
            from_file: loc.file.clone(),
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// I8.7: Memory-Efficient Caching
// ═══════════════════════════════════════════════════════════════════════

/// Memory usage statistics for LSP caching.
#[derive(Debug, Clone, Default)]
pub struct LspMemoryStats {
    /// Estimated AST memory (bytes).
    pub ast_bytes: usize,
    /// Estimated type info memory (bytes).
    pub type_info_bytes: usize,
    /// Symbol index memory (bytes).
    pub index_bytes: usize,
    /// Diagnostic memory (bytes).
    pub diag_bytes: usize,
}

impl LspMemoryStats {
    /// Total estimated memory usage.
    pub fn total_bytes(&self) -> usize {
        self.ast_bytes + self.type_info_bytes + self.index_bytes + self.diag_bytes
    }

    /// Total in MB.
    pub fn total_mb(&self) -> f64 {
        self.total_bytes() as f64 / 1_048_576.0
    }

    /// Whether usage exceeds the recommended limit.
    pub fn exceeds_limit(&self, limit_mb: f64) -> bool {
        self.total_mb() > limit_mb
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I8.8: Cache Warming
// ═══════════════════════════════════════════════════════════════════════

/// Determine which files to pre-analyze when a file is opened.
pub fn files_to_warm(
    opened_file: &str,
    import_graph: &HashMap<String, HashSet<String>>,
) -> Vec<String> {
    let mut to_warm = Vec::new();

    // Warm the opened file's imports
    if let Some(imports) = import_graph.get(opened_file) {
        for imp in imports {
            to_warm.push(imp.clone());
        }
    }

    // Also warm files that import the opened file (for cross-references)
    for (file, imports) in import_graph {
        if imports.contains(opened_file) && file != opened_file {
            to_warm.push(file.clone());
        }
    }

    to_warm
}

// ═══════════════════════════════════════════════════════════════════════
// I8.9: Stale Cache Indicator
// ═══════════════════════════════════════════════════════════════════════

/// Status of the LSP analysis for display in the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisStatus {
    /// Analysis is up-to-date.
    Fresh,
    /// Analysis is running (file was modified).
    Analyzing,
    /// Analysis is stale (waiting for re-analysis).
    Stale,
    /// Background indexing in progress.
    Indexing,
}

impl std::fmt::Display for AnalysisStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisStatus::Fresh => write!(f, "$(check) FJ"),
            AnalysisStatus::Analyzing => write!(f, "$(sync~spin) FJ Analyzing..."),
            AnalysisStatus::Stale => write!(f, "$(warning) FJ (stale)"),
            AnalysisStatus::Indexing => write!(f, "$(loading~spin) FJ Indexing..."),
        }
    }
}

/// Determine the current analysis status.
pub fn current_status(
    state: &LspAnalysisState,
    open_files: &[String],
    pending_changes: usize,
) -> AnalysisStatus {
    if !state.workspace_indexed {
        return AnalysisStatus::Indexing;
    }
    if pending_changes > 0 {
        return AnalysisStatus::Analyzing;
    }
    for file in open_files {
        if !state.analyzed_hashes.contains_key(file) {
            return AnalysisStatus::Stale;
        }
    }
    AnalysisStatus::Fresh
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — I8.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── I8.1: Incremental analysis engine ──

    #[test]
    fn i8_1_needs_analysis_on_change() {
        let mut state = LspAnalysisState::new();
        assert!(state.needs_analysis("main.fj", "let x = 1"));

        state.mark_analyzed("main.fj", "let x = 1");
        assert!(!state.needs_analysis("main.fj", "let x = 1")); // same content
        assert!(state.needs_analysis("main.fj", "let x = 2")); // changed
    }

    // ── I8.2: Per-function reanalysis ──

    #[test]
    fn i8_2_only_changed_functions() {
        let mut state = LspAnalysisState::new();
        state.function_analyses.insert(
            "a.fj".into(),
            vec![
                FunctionAnalysis {
                    name: "foo".into(),
                    body_hash: "h1".into(),
                    ok: true,
                    diagnostics: vec![],
                    resolved_types: HashMap::new(),
                },
                FunctionAnalysis {
                    name: "bar".into(),
                    body_hash: "h2".into(),
                    ok: true,
                    diagnostics: vec![],
                    resolved_types: HashMap::new(),
                },
            ],
        );

        let new_fns = vec![
            FunctionAnalysis {
                name: "foo".into(),
                body_hash: "h1".into(), // unchanged
                ok: true,
                diagnostics: vec![],
                resolved_types: HashMap::new(),
            },
            FunctionAnalysis {
                name: "bar".into(),
                body_hash: "h3".into(), // changed!
                ok: true,
                diagnostics: vec![],
                resolved_types: HashMap::new(),
            },
        ];

        let to_reanalyze = functions_needing_reanalysis("a.fj", &new_fns, &state);
        assert_eq!(to_reanalyze, vec!["bar"]); // only bar changed
    }

    // ── I8.3: Background indexing ──

    #[test]
    fn i8_3_background_indexing() {
        let mut task = IndexingTask::new(vec!["a.fj".into(), "b.fj".into(), "c.fj".into()]);
        assert!(!task.complete);
        assert_eq!(task.progress_pct(), 0.0);

        assert_eq!(task.index_next(), Some("a.fj".into()));
        assert_eq!(task.index_next(), Some("b.fj".into()));
        assert!(!task.complete);

        assert_eq!(task.index_next(), Some("c.fj".into()));
        assert!(task.complete);
        assert_eq!(task.progress_pct(), 100.0);
    }

    // ── I8.4: Incremental symbol index ──

    #[test]
    fn i8_4_symbol_index() {
        let mut idx = SymbolIndex::new();
        idx.add_definition("main", "main.fj", 1, 0);
        idx.add_definition("helper", "lib.fj", 5, 0);
        idx.add_reference("helper", "main.fj", 10, 4);

        assert_eq!(idx.definition_count(), 2);
        assert_eq!(idx.find_definition("main").unwrap().file, "main.fj");
        assert_eq!(idx.find_references("helper").len(), 1);
    }

    #[test]
    fn i8_4_index_remove_file() {
        let mut idx = SymbolIndex::new();
        idx.add_definition("foo", "a.fj", 1, 0);
        idx.add_definition("bar", "b.fj", 1, 0);
        idx.add_reference("foo", "b.fj", 5, 0);

        idx.remove_file("a.fj");
        assert_eq!(idx.definition_count(), 1); // only bar remains
        assert!(idx.find_definition("foo").is_none());
    }

    // ── I8.5: Incremental diagnostics ──

    #[test]
    fn i8_5_diagnostics_update() {
        let mut state = LspAnalysisState::new();
        state.update_diagnostics(
            "main.fj",
            vec![LspDiagnostic {
                file: "main.fj".into(),
                line: 5,
                col: 10,
                severity: LspSeverity::Error,
                message: "type mismatch".into(),
            }],
        );

        let diags = state.get_diagnostics("main.fj");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "type mismatch");
        assert!(state.get_diagnostics("other.fj").is_empty());
    }

    // ── I8.6: Incremental completion ──

    #[test]
    fn i8_6_completion_from_index() {
        let mut idx = SymbolIndex::new();
        idx.add_definition("println", "std.fj", 1, 0);
        idx.add_definition("print", "std.fj", 2, 0);
        idx.add_definition("parse_int", "std.fj", 3, 0);
        idx.add_definition("main", "main.fj", 1, 0);

        let completions = completions_from_index("pri", &idx);
        assert_eq!(completions.len(), 2); // print, println
        assert!(completions.iter().any(|c| c.label == "print"));
        assert!(completions.iter().any(|c| c.label == "println"));
    }

    // ── I8.7: Memory stats ──

    #[test]
    fn i8_7_memory_stats() {
        let stats = LspMemoryStats {
            ast_bytes: 50_000_000,       // 50MB
            type_info_bytes: 30_000_000, // 30MB
            index_bytes: 10_000_000,     // 10MB
            diag_bytes: 1_000_000,       // 1MB
        };
        assert!((stats.total_mb() - 86.78).abs() < 1.0);
        assert!(!stats.exceeds_limit(200.0));
        assert!(stats.exceeds_limit(50.0));
    }

    // ── I8.8: Cache warming ──

    #[test]
    fn i8_8_warm_on_open() {
        let mut imports = HashMap::new();
        imports.insert(
            "main.fj".into(),
            ["lib.fj", "utils.fj"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        imports.insert(
            "test.fj".into(),
            ["main.fj"].iter().map(|s| s.to_string()).collect(),
        );

        let to_warm = files_to_warm("main.fj", &imports);
        assert!(to_warm.contains(&"lib.fj".to_string()));
        assert!(to_warm.contains(&"utils.fj".to_string()));
        assert!(to_warm.contains(&"test.fj".to_string())); // imports main.fj
    }

    // ── I8.9: Stale cache indicator ──

    #[test]
    fn i8_9_status_fresh() {
        let mut state = LspAnalysisState::new();
        state.workspace_indexed = true;
        state
            .analyzed_hashes
            .insert("main.fj".into(), "hash".into());

        let status = current_status(&state, &["main.fj".into()], 0);
        assert_eq!(status, AnalysisStatus::Fresh);
        assert!(status.to_string().contains("check"));
    }

    #[test]
    fn i8_9_status_analyzing() {
        let mut state = LspAnalysisState::new();
        state.workspace_indexed = true;
        let status = current_status(&state, &[], 3);
        assert_eq!(status, AnalysisStatus::Analyzing);
    }

    #[test]
    fn i8_9_status_indexing() {
        let state = LspAnalysisState::new(); // not indexed
        let status = current_status(&state, &[], 0);
        assert_eq!(status, AnalysisStatus::Indexing);
    }

    // ── I8.10: Integration ──

    #[test]
    fn i8_10_full_lsp_incremental_flow() {
        let mut state = LspAnalysisState::new();

        // 1. Open file → needs analysis
        assert!(state.needs_analysis("main.fj", "fn main() { hello() }"));

        // 2. Analyze → mark done
        state.mark_analyzed("main.fj", "fn main() { hello() }");
        state.function_analyses.insert(
            "main.fj".into(),
            vec![FunctionAnalysis {
                name: "main".into(),
                body_hash: "h1".into(),
                ok: true,
                diagnostics: vec![],
                resolved_types: HashMap::new(),
            }],
        );

        // 3. Index symbols
        state.symbol_index.add_definition("main", "main.fj", 1, 0);
        state.symbol_index.add_definition("hello", "lib.fj", 1, 0);
        state.workspace_indexed = true;

        // 4. Status should be Fresh
        assert_eq!(
            current_status(&state, &["main.fj".into()], 0),
            AnalysisStatus::Fresh
        );

        // 5. Edit file → needs re-analysis
        assert!(state.needs_analysis("main.fj", "fn main() { world() }"));

        // 6. Per-function check
        let new_fns = vec![FunctionAnalysis {
            name: "main".into(),
            body_hash: "h2".into(),
            ok: true,
            diagnostics: vec![],
            resolved_types: HashMap::new(),
        }];
        let to_reanalyze = functions_needing_reanalysis("main.fj", &new_fns, &state);
        assert_eq!(to_reanalyze, vec!["main"]);

        // 7. Completions
        let completions = completions_from_index("hel", &state.symbol_index);
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].label, "hello");
    }
}
