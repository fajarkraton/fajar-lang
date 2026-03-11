//! Stability and conformance testing infrastructure for Fajar Lang.
//!
//! This module provides four major subsystems:
//!
//! 1. [`FuzzHarness`] — Grammar-aware fuzz testing framework
//! 2. [`ConformanceRunner`] — Language spec compliance testing
//! 3. [`RegressionHarness`] — Snapshot-based regression detection
//! 4. [`ErrorPolisher`] — Error message quality auditing
//!
//! Each subsystem is designed to be used both in CI and during
//! local development to catch regressions and ensure quality.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Common error type
// ═══════════════════════════════════════════════════════════════════════

/// Errors produced by the testing infrastructure.
#[derive(Debug, Error)]
pub enum TestingError {
    /// Failed to read a test file.
    #[error("TI001: failed to read file '{path}': {reason}")]
    FileRead {
        /// Path that could not be read.
        path: String,
        /// Reason for the failure.
        reason: String,
    },

    /// Failed to write a snapshot file.
    #[error("TI002: failed to write snapshot '{path}': {reason}")]
    FileWrite {
        /// Path that could not be written.
        path: String,
        /// Reason for the failure.
        reason: String,
    },

    /// Invalid test annotation in source file.
    #[error("TI003: invalid annotation in '{file}' at line {line}: {detail}")]
    InvalidAnnotation {
        /// Source file containing the annotation.
        file: String,
        /// Line number of the invalid annotation.
        line: usize,
        /// Description of the problem.
        detail: String,
    },

    /// Baseline JSON parse failure.
    #[error("TI004: failed to parse baseline JSON: {reason}")]
    BaselineParse {
        /// Reason for the parse failure.
        reason: String,
    },

    /// Configuration error.
    #[error("TI005: configuration error: {detail}")]
    Config {
        /// Description of the configuration problem.
        detail: String,
    },
}

/// Result type alias for testing infrastructure operations.
pub type TestingResult<T> = Result<T, TestingError>;

// ═══════════════════════════════════════════════════════════════════════
// 1. FUZZ HARNESS — Grammar-aware fuzz testing framework
// ═══════════════════════════════════════════════════════════════════════

/// Target component for fuzz testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FuzzTarget {
    /// Fuzz the lexer (tokenization).
    Lexer,
    /// Fuzz the parser (syntax analysis).
    Parser,
    /// Fuzz the semantic analyzer.
    Analyzer,
    /// Fuzz the tree-walking interpreter.
    Interpreter,
    /// Fuzz the code formatter.
    Formatter,
    /// Fuzz the bytecode VM.
    Vm,
}

impl std::fmt::Display for FuzzTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FuzzTarget::Lexer => write!(f, "lexer"),
            FuzzTarget::Parser => write!(f, "parser"),
            FuzzTarget::Analyzer => write!(f, "analyzer"),
            FuzzTarget::Interpreter => write!(f, "interpreter"),
            FuzzTarget::Formatter => write!(f, "formatter"),
            FuzzTarget::Vm => write!(f, "vm"),
        }
    }
}

/// Configuration for a fuzz testing run.
#[derive(Debug, Clone)]
pub struct FuzzConfig {
    /// Maximum number of iterations to run.
    pub max_iterations: u64,
    /// Timeout per iteration in milliseconds.
    pub timeout_ms: u64,
    /// Paths to seed corpus directories.
    pub seed_corpus_paths: Vec<PathBuf>,
    /// Maximum input size in bytes.
    pub max_input_size: usize,
    /// Random seed for reproducibility (0 = random).
    pub seed: u64,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10_000,
            timeout_ms: 1_000,
            seed_corpus_paths: Vec::new(),
            max_input_size: 4096,
            seed: 0,
        }
    }
}

/// Results from a completed fuzz run.
#[derive(Debug, Clone)]
pub struct FuzzResult {
    /// Target that was fuzzed.
    pub target: FuzzTarget,
    /// Total iterations completed.
    pub iterations: u64,
    /// Number of iterations that caused crashes (panics).
    pub crashes: u64,
    /// Number of iterations that exceeded the timeout.
    pub timeouts: u64,
    /// Number of unique crash signatures found.
    pub unique_crashes: u64,
    /// Estimated code coverage percentage (0.0-100.0).
    pub coverage_pct: f64,
    /// Total wall-clock duration of the fuzz run.
    pub duration: Duration,
    /// Crash-inducing inputs (deduplicated).
    pub crash_inputs: Vec<String>,
}

impl FuzzResult {
    /// Returns true if no crashes were found.
    pub fn is_clean(&self) -> bool {
        self.crashes == 0
    }

    /// Returns the crash rate as a percentage.
    pub fn crash_rate(&self) -> f64 {
        if self.iterations == 0 {
            return 0.0;
        }
        (self.crashes as f64 / self.iterations as f64) * 100.0
    }
}

/// Grammar-aware input generator for Fajar Lang source fragments.
///
/// Produces syntactically plausible `.fj` code snippets using weighted
/// random selection from the language grammar. This increases the chance
/// of reaching deeper compiler stages compared to purely random bytes.
pub struct GrammarGen {
    /// Current random state (simple LCG for determinism).
    state: u64,
    /// Maximum depth for recursive generation.
    max_depth: usize,
}

impl GrammarGen {
    /// Creates a new grammar generator with the given seed.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xDEAD_BEEF_CAFE } else { seed },
            max_depth: 5,
        }
    }

    /// Sets the maximum recursion depth for nested expressions.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Generates a random identifier.
    pub fn gen_identifier(&mut self) -> String {
        let prefixes = ["x", "y", "z", "val", "tmp", "result", "count", "item"];
        let idx = self.next_usize(prefixes.len());
        let suffix = self.next_usize(100);
        format!("{}{}", prefixes[idx], suffix)
    }

    /// Generates a random integer literal.
    pub fn gen_int_literal(&mut self) -> String {
        let val = self.next_usize(10000) as i64 - 5000;
        val.to_string()
    }

    /// Generates a random float literal.
    pub fn gen_float_literal(&mut self) -> String {
        let whole = self.next_usize(1000);
        let frac = self.next_usize(100);
        format!("{whole}.{frac}")
    }

    /// Generates a random string literal.
    pub fn gen_string_literal(&mut self) -> String {
        let contents = ["hello", "world", "test", "fajar", "", "foo bar"];
        let idx = self.next_usize(contents.len());
        format!("\"{}\"", contents[idx])
    }

    /// Generates a random boolean literal.
    pub fn gen_bool_literal(&mut self) -> String {
        if self.next_usize(2) == 0 {
            "true".to_string()
        } else {
            "false".to_string()
        }
    }

    /// Generates a random type annotation.
    pub fn gen_type(&mut self) -> String {
        let types = [
            "i32", "i64", "f32", "f64", "bool", "str", "u8", "u32", "void",
        ];
        let idx = self.next_usize(types.len());
        types[idx].to_string()
    }

    /// Generates a random expression at the given depth.
    pub fn gen_expr(&mut self, depth: usize) -> String {
        if depth >= self.max_depth {
            return self.gen_primary();
        }
        let choice = self.next_usize(8);
        match choice {
            0 => self.gen_primary(),
            1 => self.gen_binary_expr(depth),
            2 => self.gen_unary_expr(depth),
            3 => self.gen_call_expr(depth),
            4 => self.gen_if_expr(depth),
            5 => self.gen_block_expr(depth),
            6 => self.gen_primary(),
            _ => self.gen_primary(),
        }
    }

    /// Generates a random statement.
    pub fn gen_statement(&mut self) -> String {
        let choice = self.next_usize(6);
        match choice {
            0 => self.gen_let_stmt(),
            1 => self.gen_assignment_stmt(),
            2 => self.gen_return_stmt(),
            3 => self.gen_while_stmt(),
            4 => self.gen_for_stmt(),
            _ => self.gen_expr_stmt(),
        }
    }

    /// Generates a random function declaration.
    pub fn gen_function(&mut self) -> String {
        let name = self.gen_identifier();
        let param_count = self.next_usize(3);
        let params: Vec<String> = (0..param_count)
            .map(|_| format!("{}: {}", self.gen_identifier(), self.gen_type()))
            .collect();
        let ret_type = self.gen_type();
        let body = self.gen_expr(0);
        format!(
            "fn {name}({}) -> {ret_type} {{ {body} }}",
            params.join(", ")
        )
    }

    /// Generates a complete syntactically plausible program fragment.
    pub fn gen_program(&mut self, stmt_count: usize) -> String {
        let stmts: Vec<String> = (0..stmt_count)
            .map(|_| {
                if self.next_usize(4) == 0 {
                    self.gen_function()
                } else {
                    self.gen_statement()
                }
            })
            .collect();
        stmts.join("\n")
    }

    // — Private helpers —

    /// Generates a primary expression (leaf node).
    fn gen_primary(&mut self) -> String {
        let choice = self.next_usize(5);
        match choice {
            0 => self.gen_identifier(),
            1 => self.gen_int_literal(),
            2 => self.gen_float_literal(),
            3 => self.gen_string_literal(),
            _ => self.gen_bool_literal(),
        }
    }

    /// Generates a binary expression.
    fn gen_binary_expr(&mut self, depth: usize) -> String {
        let ops = ["+", "-", "*", "/", "%", "==", "!=", "<", ">", "&&", "||"];
        let op = ops[self.next_usize(ops.len())];
        let lhs = self.gen_expr(depth + 1);
        let rhs = self.gen_expr(depth + 1);
        format!("({lhs} {op} {rhs})")
    }

    /// Generates a unary expression.
    fn gen_unary_expr(&mut self, depth: usize) -> String {
        let ops = ["-", "!"];
        let op = ops[self.next_usize(ops.len())];
        let expr = self.gen_expr(depth + 1);
        format!("{op}{expr}")
    }

    /// Generates a function call expression.
    fn gen_call_expr(&mut self, depth: usize) -> String {
        let name = self.gen_identifier();
        let arg_count = self.next_usize(3);
        let args: Vec<String> = (0..arg_count).map(|_| self.gen_expr(depth + 1)).collect();
        format!("{name}({})", args.join(", "))
    }

    /// Generates an if expression.
    fn gen_if_expr(&mut self, depth: usize) -> String {
        let cond = self.gen_expr(depth + 1);
        let then_br = self.gen_expr(depth + 1);
        let else_br = self.gen_expr(depth + 1);
        format!("if {cond} {{ {then_br} }} else {{ {else_br} }}")
    }

    /// Generates a block expression.
    fn gen_block_expr(&mut self, depth: usize) -> String {
        let inner = self.gen_expr(depth + 1);
        format!("{{ {inner} }}")
    }

    /// Generates a let statement.
    fn gen_let_stmt(&mut self) -> String {
        let mutable = if self.next_usize(2) == 0 { "mut " } else { "" };
        let name = self.gen_identifier();
        let ty = self.gen_type();
        let val = self.gen_expr(0);
        format!("let {mutable}{name}: {ty} = {val}")
    }

    /// Generates an assignment statement.
    fn gen_assignment_stmt(&mut self) -> String {
        let name = self.gen_identifier();
        let val = self.gen_expr(0);
        format!("{name} = {val}")
    }

    /// Generates a return statement.
    fn gen_return_stmt(&mut self) -> String {
        let val = self.gen_expr(0);
        format!("return {val}")
    }

    /// Generates a while loop.
    fn gen_while_stmt(&mut self) -> String {
        let cond = self.gen_expr(0);
        let body = self.gen_expr(0);
        format!("while {cond} {{ {body} }}")
    }

    /// Generates a for loop.
    fn gen_for_stmt(&mut self) -> String {
        let var = self.gen_identifier();
        let lo = self.next_usize(10);
        let hi = lo + self.next_usize(10) + 1;
        let body = self.gen_expr(0);
        format!("for {var} in {lo}..{hi} {{ {body} }}")
    }

    /// Generates an expression statement.
    fn gen_expr_stmt(&mut self) -> String {
        self.gen_expr(0)
    }

    /// Simple LCG random number generator for deterministic fuzzing.
    fn next_u64(&mut self) -> u64 {
        // LCG constants from Numerical Recipes
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    /// Returns a random usize in [0, bound).
    fn next_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() % bound as u64) as usize
    }
}

/// Manages a corpus of seed inputs for fuzz testing.
///
/// Tracks which inputs have been used, maintains coverage data,
/// and supports corpus minimization.
pub struct CorpusManager {
    /// Seed inputs loaded from files.
    seeds: Vec<String>,
    /// Inputs that triggered new coverage.
    interesting: Vec<String>,
    /// Set of crash hashes for deduplication.
    crash_hashes: HashMap<u64, String>,
}

impl CorpusManager {
    /// Creates a new empty corpus manager.
    pub fn new() -> Self {
        Self {
            seeds: Vec::new(),
            interesting: Vec::new(),
            crash_hashes: HashMap::new(),
        }
    }

    /// Loads seed inputs from a directory path.
    ///
    /// Each `.fj` file in the directory becomes a seed input.
    /// Non-`.fj` files and subdirectories are ignored.
    pub fn load_seeds(&mut self, dir: &Path) -> TestingResult<usize> {
        if !dir.exists() {
            return Err(TestingError::FileRead {
                path: dir.display().to_string(),
                reason: "directory does not exist".into(),
            });
        }
        let entries = std::fs::read_dir(dir).map_err(|e| TestingError::FileRead {
            path: dir.display().to_string(),
            reason: e.to_string(),
        })?;
        let mut count = 0;
        for entry in entries {
            let entry = entry.map_err(|e| TestingError::FileRead {
                path: dir.display().to_string(),
                reason: e.to_string(),
            })?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("fj") {
                let content =
                    std::fs::read_to_string(&path).map_err(|e| TestingError::FileRead {
                        path: path.display().to_string(),
                        reason: e.to_string(),
                    })?;
                self.seeds.push(content);
                count += 1;
            }
        }
        Ok(count)
    }

    /// Adds a raw string as a seed input.
    pub fn add_seed(&mut self, input: String) {
        self.seeds.push(input);
    }

    /// Returns the total number of seeds.
    pub fn seed_count(&self) -> usize {
        self.seeds.len()
    }

    /// Returns a reference to all seed inputs.
    pub fn seeds(&self) -> &[String] {
        &self.seeds
    }

    /// Records an input that triggered new coverage.
    pub fn record_interesting(&mut self, input: String) {
        self.interesting.push(input);
    }

    /// Records a crash-inducing input, deduplicating by hash.
    ///
    /// Returns `true` if this is a new unique crash.
    pub fn record_crash(&mut self, input: &str) -> bool {
        let hash = simple_hash(input);
        if self.crash_hashes.contains_key(&hash) {
            return false;
        }
        self.crash_hashes.insert(hash, input.to_string());
        true
    }

    /// Returns the number of unique crashes found.
    pub fn unique_crash_count(&self) -> usize {
        self.crash_hashes.len()
    }

    /// Returns all crash-inducing inputs.
    pub fn crash_inputs(&self) -> Vec<String> {
        self.crash_hashes.values().cloned().collect()
    }

    /// Minimizes a crash input by progressively removing characters.
    ///
    /// Uses delta debugging: tries removing halves, then quarters, etc.
    /// The `still_crashes` function should return `true` if the input
    /// still triggers the same crash.
    pub fn minimize<F>(&self, input: &str, still_crashes: F) -> String
    where
        F: Fn(&str) -> bool,
    {
        let mut current = input.to_string();
        let mut chunk_size = current.len() / 2;

        while chunk_size >= 1 && !current.is_empty() {
            let mut i = 0;
            while i + chunk_size <= current.len() {
                let candidate = format!(
                    "{}{}",
                    &current[..i],
                    &current[(i + chunk_size).min(current.len())..]
                );
                if still_crashes(&candidate) {
                    current = candidate;
                } else {
                    i += chunk_size;
                }
            }
            chunk_size /= 2;
        }
        current
    }
}

impl Default for CorpusManager {
    fn default() -> Self {
        Self::new()
    }
}

/// The main fuzz testing harness.
///
/// Coordinates grammar-aware input generation, corpus management,
/// and result collection for fuzz testing Fajar Lang compiler stages.
pub struct FuzzHarness {
    /// Grammar-aware input generator.
    gen: GrammarGen,
    /// Corpus manager for seeds and crash tracking.
    corpus: CorpusManager,
}

impl FuzzHarness {
    /// Creates a new fuzz harness with the given seed.
    pub fn new(seed: u64) -> Self {
        Self {
            gen: GrammarGen::new(seed),
            corpus: CorpusManager::new(),
        }
    }

    /// Returns a mutable reference to the corpus manager.
    pub fn corpus_mut(&mut self) -> &mut CorpusManager {
        &mut self.corpus
    }

    /// Returns a reference to the corpus manager.
    pub fn corpus(&self) -> &CorpusManager {
        &self.corpus
    }

    /// Runs fuzz testing against the specified target.
    ///
    /// Generates grammar-aware inputs and feeds them to the target
    /// component, tracking crashes and coverage. Uses a simulation
    /// approach where each target is exercised through the public API.
    pub fn run(&mut self, target: FuzzTarget, config: &FuzzConfig) -> FuzzResult {
        let start = Instant::now();
        let mut crashes = 0u64;
        let mut timeouts = 0u64;
        let timeout = Duration::from_millis(config.timeout_ms);

        // First, run seed corpus inputs
        let seed_inputs: Vec<String> = self.corpus.seeds().to_vec();
        for input in &seed_inputs {
            let iter_start = Instant::now();
            if self.run_single(target, input) == RunOutcome::Crash {
                crashes += 1;
                self.corpus.record_crash(input);
            }
            if iter_start.elapsed() > timeout {
                timeouts += 1;
            }
        }

        // Then generate grammar-aware inputs
        let gen_iterations = config
            .max_iterations
            .saturating_sub(seed_inputs.len() as u64);
        for _ in 0..gen_iterations {
            let input = self.generate_input(target, config.max_input_size);
            let iter_start = Instant::now();

            if self.run_single(target, &input) == RunOutcome::Crash {
                crashes += 1;
                self.corpus.record_crash(&input);
            }
            if iter_start.elapsed() > timeout {
                timeouts += 1;
            }
        }

        let unique = self.corpus.unique_crash_count() as u64;
        // Estimate coverage based on unique paths explored
        let coverage = estimate_coverage(config.max_iterations, unique);

        FuzzResult {
            target,
            iterations: config.max_iterations,
            crashes,
            timeouts,
            unique_crashes: unique,
            coverage_pct: coverage,
            duration: start.elapsed(),
            crash_inputs: self.corpus.crash_inputs(),
        }
    }

    /// Generates a single input appropriate for the target.
    fn generate_input(&mut self, target: FuzzTarget, max_size: usize) -> String {
        let raw = match target {
            FuzzTarget::Lexer => {
                // For lexer: mix valid tokens with random bytes
                let stmt_count = 1 + (self.gen.next_usize(3));
                self.gen.gen_program(stmt_count)
            }
            FuzzTarget::Parser => {
                // For parser: generate syntactically plausible code
                let stmt_count = 1 + (self.gen.next_usize(5));
                self.gen.gen_program(stmt_count)
            }
            FuzzTarget::Analyzer | FuzzTarget::Interpreter | FuzzTarget::Vm => {
                // For deeper stages: generate more complete programs
                let stmt_count = 2 + (self.gen.next_usize(5));
                self.gen.gen_program(stmt_count)
            }
            FuzzTarget::Formatter => {
                // For formatter: generate varied formatting
                let stmt_count = 1 + (self.gen.next_usize(4));
                self.gen.gen_program(stmt_count)
            }
        };
        if raw.len() > max_size {
            raw[..max_size].to_string()
        } else {
            raw
        }
    }

    /// Runs a single input against the target component.
    ///
    /// Returns the outcome without panicking. Any panic from the target
    /// is caught and recorded as a crash.
    fn run_single(&self, target: FuzzTarget, input: &str) -> RunOutcome {
        // Use std::panic::catch_unwind to detect crashes
        let input_owned = input.to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_target_component(target, &input_owned)
        }));

        match result {
            Ok(true) => RunOutcome::Ok,
            Ok(false) => RunOutcome::Error,
            Err(_) => RunOutcome::Crash,
        }
    }
}

/// Outcome of running a single fuzz input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunOutcome {
    /// Input was processed without errors.
    Ok,
    /// Input produced a handled error (expected for invalid inputs).
    Error,
    /// Input caused a panic (crash — this is a bug).
    Crash,
}

/// Runs a single input through the specified compiler component.
///
/// Returns `true` if the input was processed without error,
/// `false` if a handled error occurred. Panics propagate upward.
fn run_target_component(target: FuzzTarget, input: &str) -> bool {
    match target {
        FuzzTarget::Lexer => crate::lexer::tokenize(input).is_ok(),
        FuzzTarget::Parser => {
            if let Ok(tokens) = crate::lexer::tokenize(input) {
                crate::parser::parse(tokens).is_ok()
            } else {
                false
            }
        }
        FuzzTarget::Analyzer => {
            if let Ok(tokens) = crate::lexer::tokenize(input) {
                if let Ok(program) = crate::parser::parse(tokens) {
                    crate::analyzer::analyze(&program).is_ok()
                } else {
                    false
                }
            } else {
                false
            }
        }
        FuzzTarget::Interpreter => {
            let mut interp = crate::interpreter::Interpreter::new();
            interp.eval_source(input).is_ok()
        }
        FuzzTarget::Formatter => crate::formatter::format(input).is_ok(),
        FuzzTarget::Vm => {
            if let Ok(tokens) = crate::lexer::tokenize(input) {
                if let Ok(program) = crate::parser::parse(tokens) {
                    crate::vm::run_program(&program).is_ok()
                } else {
                    false
                }
            } else {
                false
            }
        }
    }
}

/// Estimates code coverage percentage based on iteration count and crash count.
///
/// Uses a logarithmic model: more iterations with fewer crashes suggests
/// higher coverage. This is a heuristic, not a precise measurement.
fn estimate_coverage(iterations: u64, unique_crashes: u64) -> f64 {
    if iterations == 0 {
        return 0.0;
    }
    // Heuristic: base coverage from iteration count, reduced by crash rate
    let base = (1.0 - (-0.0001 * iterations as f64).exp()) * 100.0;
    let crash_penalty = (unique_crashes as f64).min(50.0);
    (base - crash_penalty).clamp(0.0, 100.0)
}

/// Simple non-cryptographic hash for crash deduplication.
fn simple_hash(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

// ═══════════════════════════════════════════════════════════════════════
// 2. CONFORMANCE RUNNER — Language spec compliance testing
// ═══════════════════════════════════════════════════════════════════════

/// Category of conformance test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConformanceCategory {
    /// Lexer/tokenization conformance.
    Lexer,
    /// Parser/syntax conformance.
    Parser,
    /// Type system conformance.
    Types,
    /// Context annotation (@kernel/@device) conformance.
    Context,
    /// Ownership and borrowing conformance.
    Ownership,
    /// Tensor operation conformance.
    Tensor,
    /// Runtime behavior conformance.
    Runtime,
    /// Language feature conformance (closures, generics, traits, etc.).
    Features,
}

impl std::fmt::Display for ConformanceCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConformanceCategory::Lexer => write!(f, "lexer"),
            ConformanceCategory::Parser => write!(f, "parser"),
            ConformanceCategory::Types => write!(f, "types"),
            ConformanceCategory::Context => write!(f, "context"),
            ConformanceCategory::Ownership => write!(f, "ownership"),
            ConformanceCategory::Tensor => write!(f, "tensor"),
            ConformanceCategory::Runtime => write!(f, "runtime"),
            ConformanceCategory::Features => write!(f, "features"),
        }
    }
}

/// A single conformance test specification.
#[derive(Debug, Clone)]
pub struct ConformanceTest {
    /// Unique name for this test.
    pub name: String,
    /// Source code to evaluate.
    pub source: String,
    /// Expected output (if the program should succeed).
    pub expected_output: Option<String>,
    /// Expected error code (if the program should fail).
    pub expected_error: Option<String>,
    /// Category of this test.
    pub category: ConformanceCategory,
    /// Whether this test should be skipped.
    pub skip: bool,
    /// Optional description/comment.
    pub description: Option<String>,
}

/// Detail about a single conformance test failure.
#[derive(Debug, Clone)]
pub struct FailureDetail {
    /// Name of the failing test.
    pub test_name: String,
    /// What was expected.
    pub expected: String,
    /// What actually happened.
    pub actual: String,
    /// Human-readable diff between expected and actual.
    pub diff: String,
}

impl std::fmt::Display for FailureDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FAIL: {} — expected: {}, actual: {}",
            self.test_name, self.expected, self.actual
        )
    }
}

/// Aggregate results from a conformance test run.
#[derive(Debug, Clone)]
pub struct ConformanceResult {
    /// Number of tests that passed.
    pub passed: usize,
    /// Number of tests that failed.
    pub failed: usize,
    /// Number of tests that were skipped.
    pub skipped: usize,
    /// Details of each failure.
    pub failures: Vec<FailureDetail>,
}

impl ConformanceResult {
    /// Returns true if all non-skipped tests passed.
    pub fn is_success(&self) -> bool {
        self.failed == 0
    }

    /// Returns the total number of tests (passed + failed + skipped).
    pub fn total(&self) -> usize {
        self.passed + self.failed + self.skipped
    }

    /// Returns the pass rate as a percentage (excluding skipped).
    pub fn pass_rate(&self) -> f64 {
        let run = self.passed + self.failed;
        if run == 0 {
            return 100.0;
        }
        (self.passed as f64 / run as f64) * 100.0
    }
}

/// Runs conformance tests against the Fajar Lang implementation.
///
/// Loads test specifications (either programmatic or from annotated `.fj` files),
/// runs each through the interpreter pipeline, and compares against expectations.
pub struct ConformanceRunner {
    /// Registered conformance tests.
    tests: Vec<ConformanceTest>,
}

impl ConformanceRunner {
    /// Creates a new conformance runner with no tests.
    pub fn new() -> Self {
        Self { tests: Vec::new() }
    }

    /// Registers a conformance test.
    pub fn add_test(&mut self, test: ConformanceTest) {
        self.tests.push(test);
    }

    /// Returns the number of registered tests.
    pub fn test_count(&self) -> usize {
        self.tests.len()
    }

    /// Loads conformance tests from annotated `.fj` source files.
    ///
    /// Recognized annotations (in comments):
    /// - `// expect: <output>` — expected successful output
    /// - `// expect-error: <code>` — expected error code (e.g., SE004)
    /// - `// skip` — skip this test
    /// - `// category: <cat>` — test category
    /// - `// description: <text>` — test description
    pub fn load_from_source(&mut self, name: &str, source: &str) -> TestingResult<()> {
        let mut expected_output: Option<String> = None;
        let mut expected_error: Option<String> = None;
        let mut skip = false;
        let mut category = ConformanceCategory::Runtime;
        let mut description: Option<String> = None;

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("// expect: ") {
                expected_output = Some(rest.to_string());
            } else if let Some(rest) = trimmed.strip_prefix("// expect-error: ") {
                expected_error = Some(rest.trim().to_string());
            } else if trimmed == "// skip" {
                skip = true;
            } else if let Some(rest) = trimmed.strip_prefix("// category: ") {
                category =
                    parse_category(rest.trim()).map_err(|_| TestingError::InvalidAnnotation {
                        file: name.to_string(),
                        line: line_num + 1,
                        detail: format!("unknown category '{}'", rest.trim()),
                    })?;
            } else if let Some(rest) = trimmed.strip_prefix("// description: ") {
                description = Some(rest.to_string());
            }
        }

        self.tests.push(ConformanceTest {
            name: name.to_string(),
            source: source.to_string(),
            expected_output,
            expected_error,
            category,
            skip,
            description,
        });
        Ok(())
    }

    /// Runs all registered conformance tests.
    pub fn run_all(&self) -> ConformanceResult {
        let mut result = ConformanceResult {
            passed: 0,
            failed: 0,
            skipped: 0,
            failures: Vec::new(),
        };

        for test in &self.tests {
            match self.run_test(test) {
                TestOutcome::Pass => result.passed += 1,
                TestOutcome::Fail(detail) => {
                    result.failed += 1;
                    result.failures.push(detail);
                }
                TestOutcome::Skip => result.skipped += 1,
            }
        }
        result
    }

    /// Runs conformance tests for a single category.
    pub fn run_category(&self, category: ConformanceCategory) -> ConformanceResult {
        let mut result = ConformanceResult {
            passed: 0,
            failed: 0,
            skipped: 0,
            failures: Vec::new(),
        };

        for test in self.tests.iter().filter(|t| t.category == category) {
            match self.run_test(test) {
                TestOutcome::Pass => result.passed += 1,
                TestOutcome::Fail(detail) => {
                    result.failed += 1;
                    result.failures.push(detail);
                }
                TestOutcome::Skip => result.skipped += 1,
            }
        }
        result
    }

    /// Runs a single conformance test by name.
    ///
    /// Returns `None` if no test with that name exists.
    pub fn run_single(&self, name: &str) -> Option<ConformanceResult> {
        let test = self.tests.iter().find(|t| t.name == name)?;
        let mut result = ConformanceResult {
            passed: 0,
            failed: 0,
            skipped: 0,
            failures: Vec::new(),
        };

        match self.run_test(test) {
            TestOutcome::Pass => result.passed = 1,
            TestOutcome::Fail(detail) => {
                result.failed = 1;
                result.failures.push(detail);
            }
            TestOutcome::Skip => result.skipped = 1,
        }
        Some(result)
    }

    /// Runs a single test and returns its outcome.
    fn run_test(&self, test: &ConformanceTest) -> TestOutcome {
        if test.skip {
            return TestOutcome::Skip;
        }

        let mut interp = crate::interpreter::Interpreter::new();
        let eval_result = interp.eval_source(&test.source);

        if let Some(ref expected_code) = test.expected_error {
            return self.check_expected_error(test, expected_code, &eval_result);
        }
        if let Some(ref expected) = test.expected_output {
            return self.check_expected_output(test, expected, &eval_result);
        }
        // No expectations: just check it doesn't crash
        TestOutcome::Pass
    }

    /// Checks that the result contains the expected error code.
    fn check_expected_error(
        &self,
        test: &ConformanceTest,
        expected_code: &str,
        result: &Result<crate::interpreter::Value, crate::FjError>,
    ) -> TestOutcome {
        match result {
            Err(fj_err) => {
                let err_str = fj_err.to_string();
                if err_str.contains(expected_code) {
                    TestOutcome::Pass
                } else {
                    TestOutcome::Fail(FailureDetail {
                        test_name: test.name.clone(),
                        expected: format!("error containing '{expected_code}'"),
                        actual: err_str.clone(),
                        diff: format!("- expected: {expected_code}\n+ actual: {err_str}"),
                    })
                }
            }
            Ok(val) => TestOutcome::Fail(FailureDetail {
                test_name: test.name.clone(),
                expected: format!("error containing '{expected_code}'"),
                actual: format!("success: {val:?}"),
                diff: format!("- expected error: {expected_code}\n+ got: {val:?}"),
            }),
        }
    }

    /// Checks that the result matches the expected output.
    fn check_expected_output(
        &self,
        test: &ConformanceTest,
        expected: &str,
        result: &Result<crate::interpreter::Value, crate::FjError>,
    ) -> TestOutcome {
        match result {
            Ok(val) => {
                let actual = format!("{val:?}");
                if actual == *expected || val.to_string() == *expected {
                    TestOutcome::Pass
                } else {
                    TestOutcome::Fail(FailureDetail {
                        test_name: test.name.clone(),
                        expected: expected.to_string(),
                        actual: actual.clone(),
                        diff: format!("- expected: {expected}\n+ actual: {actual}"),
                    })
                }
            }
            Err(e) => TestOutcome::Fail(FailureDetail {
                test_name: test.name.clone(),
                expected: expected.to_string(),
                actual: format!("error: {e}"),
                diff: format!("- expected: {expected}\n+ got error: {e}"),
            }),
        }
    }
}

impl Default for ConformanceRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal test outcome.
enum TestOutcome {
    /// Test passed.
    Pass,
    /// Test failed with details.
    Fail(FailureDetail),
    /// Test was skipped.
    Skip,
}

/// Parses a category string into a `ConformanceCategory`.
fn parse_category(s: &str) -> Result<ConformanceCategory, ()> {
    match s.to_lowercase().as_str() {
        "lexer" => Ok(ConformanceCategory::Lexer),
        "parser" => Ok(ConformanceCategory::Parser),
        "types" => Ok(ConformanceCategory::Types),
        "context" => Ok(ConformanceCategory::Context),
        "ownership" => Ok(ConformanceCategory::Ownership),
        "tensor" => Ok(ConformanceCategory::Tensor),
        "runtime" => Ok(ConformanceCategory::Runtime),
        "features" => Ok(ConformanceCategory::Features),
        _ => Err(()),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 3. REGRESSION HARNESS — Snapshot-based regression detection
// ═══════════════════════════════════════════════════════════════════════

/// A named snapshot for regression testing.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Name identifying this snapshot.
    pub name: String,
    /// The snapshot content (output, AST, tokens, etc.).
    pub content: String,
    /// Arbitrary metadata (e.g., compiler version, timestamp).
    pub metadata: HashMap<String, String>,
}

/// Manages loading, saving, and comparing snapshots.
pub struct SnapshotManager {
    /// Base directory for snapshot files.
    base_dir: PathBuf,
}

impl SnapshotManager {
    /// Creates a new snapshot manager with the given base directory.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Returns the base directory for snapshots.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Loads a snapshot from disk.
    ///
    /// The snapshot is stored as `<base_dir>/<name>.snap`.
    pub fn load(&self, name: &str) -> TestingResult<Snapshot> {
        let path = self.snap_path(name);
        let content = std::fs::read_to_string(&path).map_err(|e| TestingError::FileRead {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;

        let (metadata, body) = parse_snapshot_content(&content);
        Ok(Snapshot {
            name: name.to_string(),
            content: body,
            metadata,
        })
    }

    /// Saves a snapshot to disk.
    pub fn save(&self, snapshot: &Snapshot) -> TestingResult<()> {
        let path = self.snap_path(&snapshot.name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| TestingError::FileWrite {
                path: parent.display().to_string(),
                reason: e.to_string(),
            })?;
        }

        let mut output = String::new();
        // Write metadata header
        for (key, value) in &snapshot.metadata {
            output.push_str(&format!("# {key}: {value}\n"));
        }
        if !snapshot.metadata.is_empty() {
            output.push_str("---\n");
        }
        output.push_str(&snapshot.content);

        std::fs::write(&path, &output).map_err(|e| TestingError::FileWrite {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        Ok(())
    }

    /// Compares a new output against a saved snapshot.
    ///
    /// Returns `RegressionResult::Pass` if they match,
    /// `RegressionResult::NewSnapshot` if no snapshot exists yet,
    /// or `RegressionResult::Mismatch` with details if they differ.
    pub fn compare(&self, name: &str, actual: &str) -> TestingResult<RegressionResult> {
        match self.load(name) {
            Ok(snapshot) => {
                if snapshot.content.trim() == actual.trim() {
                    Ok(RegressionResult::Pass)
                } else {
                    Ok(RegressionResult::Mismatch {
                        expected: snapshot.content,
                        actual: actual.to_string(),
                    })
                }
            }
            Err(TestingError::FileRead { .. }) => Ok(RegressionResult::NewSnapshot),
            Err(e) => Err(e),
        }
    }

    /// Blesses (accepts) the current output as the new snapshot.
    pub fn bless(&self, name: &str, content: &str) -> TestingResult<()> {
        let snapshot = Snapshot {
            name: name.to_string(),
            content: content.to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("blessed".to_string(), "true".to_string());
                m
            },
        };
        self.save(&snapshot)
    }

    /// Returns the filesystem path for a snapshot.
    fn snap_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(format!("{name}.snap"))
    }
}

/// Parses snapshot file content into metadata and body.
fn parse_snapshot_content(content: &str) -> (HashMap<String, String>, String) {
    let mut metadata = HashMap::new();
    let mut lines = content.lines().peekable();
    let mut header_end = false;

    let mut body_lines = Vec::new();

    while let Some(line) = lines.peek() {
        if !header_end {
            if let Some(rest) = line.strip_prefix("# ") {
                if let Some((key, value)) = rest.split_once(": ") {
                    metadata.insert(key.to_string(), value.to_string());
                    lines.next();
                    continue;
                }
            }
            if *line == "---" {
                header_end = true;
                lines.next();
                continue;
            }
            header_end = true;
        }
        body_lines.push(*line);
        lines.next();
    }

    (metadata, body_lines.join("\n"))
}

/// Result of comparing a regression test against its snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegressionResult {
    /// Output matches the snapshot.
    Pass,
    /// Output differs from the snapshot.
    Mismatch {
        /// The expected content from the snapshot.
        expected: String,
        /// The actual content produced.
        actual: String,
    },
    /// No snapshot exists yet (first run).
    NewSnapshot,
}

/// A single regression test specification.
#[derive(Debug, Clone)]
pub struct RegressionTest {
    /// Unique name for this test.
    pub name: String,
    /// Source code to evaluate.
    pub source: String,
    /// Path to the snapshot file for comparison.
    pub snapshot_path: PathBuf,
}

/// Records benchmark baselines as JSON for comparison.
pub struct BaselineRecorder {
    /// Baseline entries: benchmark name -> measured value.
    entries: HashMap<String, f64>,
    /// Metadata about the recording session.
    metadata: HashMap<String, String>,
}

impl BaselineRecorder {
    /// Creates a new baseline recorder.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Records a benchmark measurement.
    pub fn record(&mut self, name: &str, value: f64) {
        self.entries.insert(name.to_string(), value);
    }

    /// Sets a metadata field (e.g., git commit, timestamp).
    pub fn set_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }

    /// Returns the number of recorded entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns all recorded entries.
    pub fn entries(&self) -> &HashMap<String, f64> {
        &self.entries
    }

    /// Serializes the baseline to a JSON string.
    pub fn to_json(&self) -> TestingResult<String> {
        // Manual JSON serialization to avoid serde dependency in this module
        let mut json = String::from("{\n");
        json.push_str("  \"metadata\": {\n");
        let meta_entries: Vec<_> = self.metadata.iter().collect();
        for (i, (k, v)) in meta_entries.iter().enumerate() {
            json.push_str(&format!(
                "    \"{}\": \"{}\"",
                escape_json(k),
                escape_json(v)
            ));
            if i < meta_entries.len() - 1 {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("  },\n");
        json.push_str("  \"benchmarks\": {\n");
        let bench_entries: Vec<_> = self.entries.iter().collect();
        for (i, (k, v)) in bench_entries.iter().enumerate() {
            json.push_str(&format!("    \"{}\": {v}", escape_json(k)));
            if i < bench_entries.len() - 1 {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("  }\n");
        json.push('}');
        Ok(json)
    }

    /// Saves the baseline to a file.
    pub fn save(&self, path: &Path) -> TestingResult<()> {
        let json = self.to_json()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| TestingError::FileWrite {
                path: parent.display().to_string(),
                reason: e.to_string(),
            })?;
        }
        std::fs::write(path, &json).map_err(|e| TestingError::FileWrite {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        Ok(())
    }
}

impl Default for BaselineRecorder {
    fn default() -> Self {
        Self::new()
    }
}

/// Compares two benchmark baselines and reports regressions.
pub struct BaselineComparator {
    /// Threshold percentage above which a regression is reported.
    pub threshold_pct: f64,
}

impl BaselineComparator {
    /// Creates a new comparator with default 10% threshold.
    pub fn new() -> Self {
        Self {
            threshold_pct: 10.0,
        }
    }

    /// Creates a comparator with a custom threshold.
    pub fn with_threshold(threshold_pct: f64) -> Self {
        Self { threshold_pct }
    }

    /// Compares two baselines and returns a list of regressions.
    ///
    /// A regression is when `current[name] > baseline[name] * (1 + threshold/100)`.
    /// Only benchmarks present in both baselines are compared.
    pub fn compare(
        &self,
        baseline: &HashMap<String, f64>,
        current: &HashMap<String, f64>,
    ) -> Vec<BaselineRegression> {
        let mut regressions = Vec::new();

        for (name, &base_val) in baseline {
            if let Some(&curr_val) = current.get(name) {
                if base_val > 0.0 {
                    let change_pct = ((curr_val - base_val) / base_val) * 100.0;
                    if change_pct > self.threshold_pct {
                        regressions.push(BaselineRegression {
                            benchmark: name.clone(),
                            baseline_value: base_val,
                            current_value: curr_val,
                            change_pct,
                        });
                    }
                }
            }
        }
        regressions
    }

    /// Loads a baseline from a JSON string and returns the benchmarks map.
    pub fn load_from_json(json: &str) -> TestingResult<HashMap<String, f64>> {
        parse_baseline_json(json)
    }
}

impl Default for BaselineComparator {
    fn default() -> Self {
        Self::new()
    }
}

/// A single benchmark regression.
#[derive(Debug, Clone)]
pub struct BaselineRegression {
    /// Name of the regressed benchmark.
    pub benchmark: String,
    /// Value from the baseline.
    pub baseline_value: f64,
    /// Value from the current run.
    pub current_value: f64,
    /// Percentage change (positive = slower/worse).
    pub change_pct: f64,
}

impl std::fmt::Display for BaselineRegression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {:.2} -> {:.2} ({:+.1}%)",
            self.benchmark, self.baseline_value, self.current_value, self.change_pct
        )
    }
}

/// Helps bisect to find the first bad commit.
///
/// Given a test function that returns `true` for "good" and `false` for "bad",
/// performs binary search over a list of commit identifiers.
pub struct BisectHelper {
    /// List of commits in chronological order (oldest first).
    commits: Vec<String>,
}

impl BisectHelper {
    /// Creates a new bisect helper with the given commit list.
    pub fn new(commits: Vec<String>) -> Self {
        Self { commits }
    }

    /// Returns the number of commits to bisect.
    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }

    /// Performs binary search to find the first "bad" commit.
    ///
    /// The `test_fn` receives a commit identifier and returns `true`
    /// if the commit is "good" (test passes) or `false` if "bad".
    ///
    /// Returns `None` if all commits are good or the list is empty.
    pub fn bisect<F>(&self, test_fn: F) -> Option<BisectResult>
    where
        F: Fn(&str) -> bool,
    {
        if self.commits.is_empty() {
            return None;
        }

        let mut lo = 0usize;
        let mut hi = self.commits.len();
        let mut steps = 0u32;

        // Verify the first commit is good and last is bad
        let first_good = test_fn(&self.commits[0]);
        let last_good = test_fn(&self.commits[self.commits.len() - 1]);
        steps += 2;

        if first_good && !last_good {
            // Standard bisect
            while lo + 1 < hi {
                let mid = lo + (hi - lo) / 2;
                steps += 1;
                if test_fn(&self.commits[mid]) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            Some(BisectResult {
                first_bad: self.commits[hi.min(self.commits.len() - 1)].clone(),
                last_good: self.commits[lo].clone(),
                steps,
            })
        } else if !first_good {
            // Even the first commit is bad
            Some(BisectResult {
                first_bad: self.commits[0].clone(),
                last_good: "(none)".to_string(),
                steps,
            })
        } else {
            // All good
            None
        }
    }
}

/// Result of a bisect operation.
#[derive(Debug, Clone)]
pub struct BisectResult {
    /// The first commit where the test fails.
    pub first_bad: String,
    /// The last commit where the test passes.
    pub last_good: String,
    /// Number of test invocations performed.
    pub steps: u32,
}

impl std::fmt::Display for BisectResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "first bad: {} (last good: {}, {} steps)",
            self.first_bad, self.last_good, self.steps
        )
    }
}

/// Parses baseline JSON into a benchmarks map.
///
/// Expects format: `{ "metadata": {...}, "benchmarks": { "name": 123.4, ... } }`
fn parse_baseline_json(json: &str) -> TestingResult<HashMap<String, f64>> {
    // Simple JSON parser for the specific baseline format
    let mut results = HashMap::new();
    let trimmed = json.trim();

    // Find the "benchmarks" section
    if let Some(bench_start) = trimmed.find("\"benchmarks\"") {
        let rest = &trimmed[bench_start..];
        if let Some(brace_start) = rest.find('{') {
            if let Some(brace_end) = rest[brace_start..].find('}') {
                let bench_block = &rest[brace_start + 1..brace_start + brace_end];
                for entry in bench_block.split(',') {
                    let entry = entry.trim();
                    if entry.is_empty() {
                        continue;
                    }
                    if let Some((key_part, val_part)) = entry.split_once(':') {
                        let key = key_part.trim().trim_matches('"').to_string();
                        if let Ok(val) = val_part.trim().parse::<f64>() {
                            results.insert(key, val);
                        }
                    }
                }
            }
        }
    }

    if results.is_empty() && !trimmed.contains("\"benchmarks\"") {
        return Err(TestingError::BaselineParse {
            reason: "no 'benchmarks' section found in JSON".into(),
        });
    }
    Ok(results)
}

/// Escapes a string for JSON output.
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ═══════════════════════════════════════════════════════════════════════
// 4. ERROR POLISHER — Error message quality auditing
// ═══════════════════════════════════════════════════════════════════════

/// Quality metrics for a single error message.
#[derive(Debug, Clone)]
pub struct ErrorQuality {
    /// Whether the error includes a source span.
    pub has_span: bool,
    /// Whether the error includes a help suggestion.
    pub has_help: bool,
    /// Whether the error includes a note/explanation.
    pub has_note: bool,
    /// Whether the error has an error code (e.g., SE004).
    pub has_error_code: bool,
    /// Whether the error includes a fix suggestion.
    pub has_suggestion: bool,
    /// Whether the message is human-readable (no raw debug output).
    pub readable_message: bool,
}

impl ErrorQuality {
    /// Computes a quality score from 0-100.
    ///
    /// Weights: span (20), error_code (20), readable (20), help (15),
    /// suggestion (15), note (10).
    pub fn score(&self) -> u32 {
        let mut s = 0u32;
        if self.has_span {
            s += 20;
        }
        if self.has_error_code {
            s += 20;
        }
        if self.readable_message {
            s += 20;
        }
        if self.has_help {
            s += 15;
        }
        if self.has_suggestion {
            s += 15;
        }
        if self.has_note {
            s += 10;
        }
        s
    }
}

/// An entry in the error catalog.
#[derive(Debug, Clone)]
pub struct ErrorCatalogEntry {
    /// Error code (e.g., "SE004").
    pub code: String,
    /// Error category prefix (e.g., "SE").
    pub prefix: String,
    /// Human-readable description.
    pub description: String,
    /// Quality assessment for this error.
    pub quality: ErrorQuality,
}

/// Complete inventory of all Fajar Lang error codes.
///
/// Catalogs all error codes across categories (LE, PE, SE, KE, DE, TE, RE, ME, CE)
/// with descriptions and quality scores.
pub struct ErrorCatalog {
    /// All cataloged error entries.
    entries: Vec<ErrorCatalogEntry>,
}

impl ErrorCatalog {
    /// Creates a new catalog populated with all known Fajar Lang error codes.
    pub fn new() -> Self {
        let entries = build_error_catalog();
        Self { entries }
    }

    /// Returns all catalog entries.
    pub fn entries(&self) -> &[ErrorCatalogEntry] {
        &self.entries
    }

    /// Returns the total number of cataloged error codes.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Looks up an error by its code.
    pub fn lookup(&self, code: &str) -> Option<&ErrorCatalogEntry> {
        self.entries.iter().find(|e| e.code == code)
    }

    /// Returns all errors in a given category prefix.
    pub fn by_prefix(&self, prefix: &str) -> Vec<&ErrorCatalogEntry> {
        self.entries.iter().filter(|e| e.prefix == prefix).collect()
    }

    /// Returns the average quality score across all errors.
    pub fn average_quality(&self) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }
        let total: u32 = self.entries.iter().map(|e| e.quality.score()).sum();
        total as f64 / self.entries.len() as f64
    }

    /// Returns errors with quality score below the given threshold.
    pub fn below_threshold(&self, threshold: u32) -> Vec<&ErrorCatalogEntry> {
        self.entries
            .iter()
            .filter(|e| e.quality.score() < threshold)
            .collect()
    }
}

impl Default for ErrorCatalog {
    fn default() -> Self {
        Self::new()
    }
}

/// Audits all error types and reports quality metrics.
pub struct ErrorAudit {
    /// Catalog of all errors.
    catalog: ErrorCatalog,
}

impl ErrorAudit {
    /// Creates a new error audit with the full catalog.
    pub fn new() -> Self {
        Self {
            catalog: ErrorCatalog::new(),
        }
    }

    /// Returns the underlying catalog.
    pub fn catalog(&self) -> &ErrorCatalog {
        &self.catalog
    }

    /// Runs the full audit and returns a summary report.
    pub fn run(&self) -> AuditReport {
        let total = self.catalog.count();
        let scores: Vec<u32> = self
            .catalog
            .entries()
            .iter()
            .map(|e| e.quality.score())
            .collect();
        let avg = if total > 0 {
            scores.iter().sum::<u32>() as f64 / total as f64
        } else {
            0.0
        };
        let min = scores.iter().copied().min().unwrap_or(0);
        let max = scores.iter().copied().max().unwrap_or(0);
        let below_50: Vec<String> = self
            .catalog
            .entries()
            .iter()
            .filter(|e| e.quality.score() < 50)
            .map(|e| e.code.clone())
            .collect();

        let category_scores = self.category_scores();

        AuditReport {
            total_errors: total,
            average_score: avg,
            min_score: min,
            max_score: max,
            below_threshold: below_50,
            category_scores,
        }
    }

    /// Computes average scores per error category.
    fn category_scores(&self) -> HashMap<String, f64> {
        let mut category_totals: HashMap<String, (u32, usize)> = HashMap::new();
        for entry in self.catalog.entries() {
            let (total, count) = category_totals
                .entry(entry.prefix.clone())
                .or_insert((0, 0));
            *total += entry.quality.score();
            *count += 1;
        }
        category_totals
            .into_iter()
            .map(|(k, (total, count))| (k, total as f64 / count as f64))
            .collect()
    }
}

impl Default for ErrorAudit {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary report from an error audit.
#[derive(Debug, Clone)]
pub struct AuditReport {
    /// Total number of error codes audited.
    pub total_errors: usize,
    /// Average quality score (0-100).
    pub average_score: f64,
    /// Minimum quality score found.
    pub min_score: u32,
    /// Maximum quality score found.
    pub max_score: u32,
    /// Error codes with score below 50.
    pub below_threshold: Vec<String>,
    /// Average score per category prefix.
    pub category_scores: HashMap<String, f64>,
}

impl AuditReport {
    /// Returns true if all errors meet the minimum quality bar (score >= 50).
    pub fn meets_quality_bar(&self) -> bool {
        self.below_threshold.is_empty()
    }
}

/// Ensures consistent miette formatting for error display.
pub struct ErrorFormatter;

impl ErrorFormatter {
    /// Formats a Fajar Lang error into a miette diagnostic string.
    ///
    /// Returns the rendered error with source context, spans, and help text.
    pub fn format_lex_error(
        error: &crate::lexer::LexError,
        filename: &str,
        source: &str,
    ) -> String {
        let diag = crate::FjDiagnostic::from_lex_error(error, filename, source);
        render_diagnostic(&diag)
    }

    /// Formats a parse error with miette rendering.
    pub fn format_parse_error(
        error: &crate::parser::ParseError,
        filename: &str,
        source: &str,
    ) -> String {
        let diag = crate::FjDiagnostic::from_parse_error(error, filename, source);
        render_diagnostic(&diag)
    }

    /// Formats a semantic error with miette rendering.
    pub fn format_semantic_error(
        error: &crate::analyzer::SemanticError,
        filename: &str,
        source: &str,
    ) -> String {
        let diag = crate::FjDiagnostic::from_semantic_error(error, filename, source);
        render_diagnostic(&diag)
    }

    /// Formats a runtime error with miette rendering.
    pub fn format_runtime_error(
        error: &crate::interpreter::RuntimeError,
        filename: &str,
        source: &str,
    ) -> String {
        let diag = crate::FjDiagnostic::from_runtime_error(error, filename, source);
        render_diagnostic(&diag)
    }

    /// Validates that an error code follows the expected format.
    ///
    /// Valid format: 2 uppercase letters + 3 digits (e.g., "SE004", "LE001").
    pub fn validate_error_code(code: &str) -> bool {
        if code.len() != 5 {
            return false;
        }
        let bytes = code.as_bytes();
        bytes[0].is_ascii_uppercase()
            && bytes[1].is_ascii_uppercase()
            && bytes[2].is_ascii_digit()
            && bytes[3].is_ascii_digit()
            && bytes[4].is_ascii_digit()
    }
}

/// Renders a diagnostic to string using miette's graphical handler.
fn render_diagnostic(diag: &crate::FjDiagnostic) -> String {
    use miette::GraphicalReportHandler;
    let handler = GraphicalReportHandler::new();
    let mut output = String::new();
    // If rendering fails, fall back to Display
    if handler.render_report(&mut output, diag).is_err() {
        output = diag.to_string();
    }
    output
}

/// Builds the complete error catalog with all known error codes.
fn build_error_catalog() -> Vec<ErrorCatalogEntry> {
    let mut e = Vec::new();
    build_lex_errors(&mut e);
    build_parse_errors(&mut e);
    build_semantic_errors(&mut e);
    build_context_errors(&mut e);
    build_tensor_errors(&mut e);
    build_runtime_errors(&mut e);
    build_memory_errors(&mut e);
    build_codegen_errors(&mut e);
    e
}

/// Lex errors (LE001-LE008).
fn build_lex_errors(e: &mut Vec<ErrorCatalogEntry>) {
    add_entry(
        e,
        "LE001",
        "LE",
        "Unexpected character",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "LE002",
        "LE",
        "Unterminated string literal",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "LE003",
        "LE",
        "Unterminated block comment",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "LE004",
        "LE",
        "Invalid number format",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "LE005",
        "LE",
        "Invalid escape sequence",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "LE006",
        "LE",
        "Number overflow",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "LE007",
        "LE",
        "Empty character literal",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "LE008",
        "LE",
        "Multi-character literal",
        true,
        true,
        false,
        true,
        false,
        true,
    );
}

/// Parse errors (PE001-PE011).
fn build_parse_errors(e: &mut Vec<ErrorCatalogEntry>) {
    add_entry(
        e,
        "PE001",
        "PE",
        "Unexpected token",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "PE002",
        "PE",
        "Expected expression",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "PE003",
        "PE",
        "Expected type",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "PE004",
        "PE",
        "Expected pattern",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "PE005",
        "PE",
        "Expected identifier",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "PE006",
        "PE",
        "Unexpected end of file",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "PE007",
        "PE",
        "Invalid pattern",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "PE008",
        "PE",
        "Duplicate field",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "PE009",
        "PE",
        "Trailing separator",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "PE010",
        "PE",
        "Invalid annotation",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "PE011",
        "PE",
        "Module file not found",
        true,
        false,
        false,
        true,
        false,
        true,
    );
}

/// Semantic errors (SE001-SE021).
fn build_semantic_errors(e: &mut Vec<ErrorCatalogEntry>) {
    add_entry(
        e,
        "SE001",
        "SE",
        "Undefined variable",
        true,
        true,
        true,
        true,
        true,
        true,
    );
    add_entry(
        e,
        "SE002",
        "SE",
        "Undefined function",
        true,
        true,
        false,
        true,
        true,
        true,
    );
    add_entry(
        e,
        "SE003",
        "SE",
        "Undefined type",
        true,
        true,
        false,
        true,
        true,
        true,
    );
    add_entry(
        e,
        "SE004",
        "SE",
        "Type mismatch",
        true,
        true,
        true,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE005",
        "SE",
        "Argument count mismatch",
        true,
        true,
        true,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE006",
        "SE",
        "Duplicate definition",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE007",
        "SE",
        "Immutable assignment",
        true,
        true,
        false,
        true,
        true,
        true,
    );
    add_entry(
        e,
        "SE008",
        "SE",
        "Missing return",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE009",
        "SE",
        "Unused variable (warning)",
        true,
        false,
        true,
        true,
        true,
        true,
    );
    add_entry(
        e,
        "SE010",
        "SE",
        "Unreachable code (warning)",
        true,
        false,
        true,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE011",
        "SE",
        "Non-exhaustive match",
        true,
        true,
        false,
        true,
        true,
        true,
    );
    add_entry(
        e,
        "SE012",
        "SE",
        "Missing field",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE013",
        "SE",
        "Cannot infer type / FFI unsafe type",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE014",
        "SE",
        "Trait bound not satisfied",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE015",
        "SE",
        "Unknown trait",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE016",
        "SE",
        "Trait method signature mismatch",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE017",
        "SE",
        "Await outside async",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE018",
        "SE",
        "Not Send type",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE019",
        "SE",
        "Unused import (warning)",
        true,
        false,
        true,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE020",
        "SE",
        "Unreachable pattern (warning)",
        true,
        false,
        true,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "SE021",
        "SE",
        "Lifetime mismatch",
        true,
        true,
        false,
        true,
        false,
        true,
    );
}

/// Kernel errors (KE001-KE006) and device errors (DE001-DE003).
fn build_context_errors(e: &mut Vec<ErrorCatalogEntry>) {
    add_entry(
        e,
        "KE001",
        "KE",
        "Heap allocation in @kernel context",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "KE002",
        "KE",
        "Tensor operation in @kernel context",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "KE003",
        "KE",
        "Device call in @kernel context",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "KE004",
        "KE",
        "Float operation in @kernel context",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "KE005",
        "KE",
        "Inline asm in @safe context",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "KE006",
        "KE",
        "Inline asm in @device context",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "DE001",
        "DE",
        "Raw pointer in @device context",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "DE002",
        "DE",
        "Kernel call in @device context",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "DE003",
        "DE",
        "IRQ access in @device context",
        true,
        true,
        false,
        true,
        false,
        true,
    );
}

/// Tensor errors (TE001-TE008).
fn build_tensor_errors(e: &mut Vec<ErrorCatalogEntry>) {
    add_entry(
        e,
        "TE001",
        "TE",
        "Tensor shape mismatch",
        true,
        true,
        true,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "TE002",
        "TE",
        "Invalid tensor dimension",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "TE003",
        "TE",
        "Tensor dtype mismatch",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "TE004",
        "TE",
        "Invalid reshape",
        true,
        true,
        true,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "TE005",
        "TE",
        "Broadcast failure",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "TE006",
        "TE",
        "Gradient not available",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "TE007",
        "TE",
        "Tensor index out of bounds",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "TE008",
        "TE",
        "Quantization error",
        true,
        false,
        false,
        true,
        false,
        true,
    );
}

/// Runtime errors (RE001-RE008).
fn build_runtime_errors(e: &mut Vec<ErrorCatalogEntry>) {
    add_entry(
        e,
        "RE001",
        "RE",
        "Division by zero",
        false,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "RE002",
        "RE",
        "Type error at runtime",
        false,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "RE003",
        "RE",
        "Stack overflow",
        false,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "RE004",
        "RE",
        "Undefined variable at runtime",
        false,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "RE005",
        "RE",
        "Not a function",
        false,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "RE006",
        "RE",
        "Arity mismatch at runtime",
        false,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "RE007",
        "RE",
        "Index out of bounds",
        false,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "RE008",
        "RE",
        "Assertion failure",
        false,
        false,
        false,
        true,
        false,
        true,
    );
}

/// Memory errors (ME001-ME010).
fn build_memory_errors(e: &mut Vec<ErrorCatalogEntry>) {
    add_entry(
        e,
        "ME001",
        "ME",
        "Use after move",
        true,
        true,
        true,
        true,
        true,
        true,
    );
    add_entry(
        e,
        "ME002",
        "ME",
        "Double free",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "ME003",
        "ME",
        "Move while borrowed",
        true,
        true,
        true,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "ME004",
        "ME",
        "Mutable borrow conflict",
        true,
        true,
        true,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "ME005",
        "ME",
        "Immutable borrow conflict",
        true,
        true,
        true,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "ME006",
        "ME",
        "Buffer overflow",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "ME007",
        "ME",
        "Null pointer dereference",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "ME008",
        "ME",
        "Memory leak detected",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "ME009",
        "ME",
        "Lifetime conflict",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "ME010",
        "ME",
        "Dangling reference",
        true,
        true,
        false,
        true,
        false,
        true,
    );
}

/// Codegen errors (CE001-CE010).
fn build_codegen_errors(e: &mut Vec<ErrorCatalogEntry>) {
    add_entry(
        e,
        "CE001",
        "CE",
        "Unsupported type in codegen",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "CE002",
        "CE",
        "Function not found in codegen",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "CE003",
        "CE",
        "Invalid instruction sequence",
        true,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "CE004",
        "CE",
        "Type coercion failure",
        true,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "CE005",
        "CE",
        "Linker error",
        false,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "CE006",
        "CE",
        "Target not supported",
        false,
        true,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "CE007",
        "CE",
        "Register allocation failure",
        false,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "CE008",
        "CE",
        "Object file generation failure",
        false,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "CE009",
        "CE",
        "JIT compilation failure",
        false,
        false,
        false,
        true,
        false,
        true,
    );
    add_entry(
        e,
        "CE010",
        "CE",
        "ABI mismatch",
        false,
        true,
        false,
        true,
        false,
        true,
    );
}

/// Helper to add an entry to the error catalog.
#[allow(clippy::too_many_arguments)]
fn add_entry(
    entries: &mut Vec<ErrorCatalogEntry>,
    code: &str,
    prefix: &str,
    description: &str,
    has_span: bool,
    has_help: bool,
    has_note: bool,
    has_error_code: bool,
    has_suggestion: bool,
    readable_message: bool,
) {
    entries.push(ErrorCatalogEntry {
        code: code.to_string(),
        prefix: prefix.to_string(),
        description: description.to_string(),
        quality: ErrorQuality {
            has_span,
            has_help,
            has_note,
            has_error_code,
            has_suggestion,
            readable_message,
        },
    });
}

// ═══════════════════════════════════════════════════════════════════════
// TESTS — 40 tests: s1_1 through s4_10 (10 per sprint)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Sprint 1: Fuzz Harness (s1_1 - s1_10) ─────────────────────

    #[test]
    fn s1_1_grammar_gen_produces_valid_identifier() {
        let mut gen = GrammarGen::new(42);
        let ident = gen.gen_identifier();
        assert!(!ident.is_empty());
        assert!(ident.chars().next().unwrap().is_alphabetic());
    }

    #[test]
    fn s1_2_grammar_gen_produces_varied_types() {
        let mut gen = GrammarGen::new(123);
        let mut types = std::collections::HashSet::new();
        for _ in 0..50 {
            types.insert(gen.gen_type());
        }
        // Should produce at least a few different types
        assert!(
            types.len() >= 3,
            "got {} unique types: {:?}",
            types.len(),
            types
        );
    }

    #[test]
    fn s1_3_grammar_gen_expressions_have_balanced_parens() {
        let mut gen = GrammarGen::new(999);
        for _ in 0..20 {
            let expr = gen.gen_expr(0);
            let opens = expr.chars().filter(|c| *c == '(').count();
            let closes = expr.chars().filter(|c| *c == ')').count();
            assert_eq!(opens, closes, "unbalanced parens in: {expr}");
        }
    }

    #[test]
    fn s1_4_grammar_gen_program_produces_multiple_statements() {
        let mut gen = GrammarGen::new(77);
        let program = gen.gen_program(5);
        // At least 5 lines (one per statement)
        let lines: Vec<&str> = program.lines().collect();
        assert!(
            lines.len() >= 5,
            "expected >= 5 lines, got {}: {program}",
            lines.len()
        );
    }

    #[test]
    fn s1_5_corpus_manager_add_and_count_seeds() {
        let mut corpus = CorpusManager::new();
        assert_eq!(corpus.seed_count(), 0);
        corpus.add_seed("let x = 1".to_string());
        corpus.add_seed("let y = 2".to_string());
        assert_eq!(corpus.seed_count(), 2);
    }

    #[test]
    fn s1_6_corpus_manager_deduplicates_crashes() {
        let mut corpus = CorpusManager::new();
        assert!(corpus.record_crash("bad input"));
        assert!(!corpus.record_crash("bad input")); // duplicate
        assert!(corpus.record_crash("different bad input"));
        assert_eq!(corpus.unique_crash_count(), 2);
    }

    #[test]
    fn s1_7_corpus_minimizer_reduces_input() {
        let corpus = CorpusManager::new();
        // The "crash" triggers on any input containing "bug"
        let minimized = corpus.minimize("lots of code with a bug in it", |input| {
            input.contains("bug")
        });
        assert!(minimized.contains("bug"));
        assert!(minimized.len() < "lots of code with a bug in it".len());
    }

    #[test]
    fn s1_8_fuzz_config_default_values() {
        let config = FuzzConfig::default();
        assert_eq!(config.max_iterations, 10_000);
        assert_eq!(config.timeout_ms, 1_000);
        assert_eq!(config.max_input_size, 4096);
        assert!(config.seed_corpus_paths.is_empty());
    }

    #[test]
    fn s1_9_fuzz_target_display_names() {
        assert_eq!(FuzzTarget::Lexer.to_string(), "lexer");
        assert_eq!(FuzzTarget::Parser.to_string(), "parser");
        assert_eq!(FuzzTarget::Analyzer.to_string(), "analyzer");
        assert_eq!(FuzzTarget::Interpreter.to_string(), "interpreter");
        assert_eq!(FuzzTarget::Formatter.to_string(), "formatter");
        assert_eq!(FuzzTarget::Vm.to_string(), "vm");
    }

    #[test]
    fn s1_10_fuzz_harness_runs_lexer_without_crashes() {
        let mut harness = FuzzHarness::new(42);
        harness.corpus_mut().add_seed("let x: i64 = 42".to_string());
        let config = FuzzConfig {
            max_iterations: 50,
            timeout_ms: 5_000,
            max_input_size: 256,
            seed: 42,
            ..FuzzConfig::default()
        };
        let result = harness.run(FuzzTarget::Lexer, &config);
        assert_eq!(result.iterations, 50);
        // Grammar-generated inputs should not cause panics in the lexer
        assert_eq!(result.crashes, 0, "lexer crashed on generated input");
    }

    // ─── Sprint 2: Conformance Runner (s2_1 - s2_10) ────────────────

    #[test]
    fn s2_1_conformance_runner_add_and_count_tests() {
        let mut runner = ConformanceRunner::new();
        assert_eq!(runner.test_count(), 0);
        runner.add_test(ConformanceTest {
            name: "basic_let".into(),
            source: "let x: i64 = 42".into(),
            expected_output: None,
            expected_error: None,
            category: ConformanceCategory::Runtime,
            skip: false,
            description: None,
        });
        assert_eq!(runner.test_count(), 1);
    }

    #[test]
    fn s2_2_conformance_test_passes_for_valid_code() {
        let mut runner = ConformanceRunner::new();
        runner.add_test(ConformanceTest {
            name: "simple_let".into(),
            source: "let x: i64 = 42".into(),
            expected_output: None,
            expected_error: None,
            category: ConformanceCategory::Runtime,
            skip: false,
            description: None,
        });
        let result = runner.run_all();
        assert!(result.is_success());
        assert_eq!(result.passed, 1);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn s2_3_conformance_test_detects_expected_error() {
        let mut runner = ConformanceRunner::new();
        runner.add_test(ConformanceTest {
            name: "type_mismatch".into(),
            source: "let x: i64 = \"hello\"".into(),
            expected_error: Some("SE004".into()),
            expected_output: None,
            category: ConformanceCategory::Types,
            skip: false,
            description: None,
        });
        let result = runner.run_all();
        assert!(result.is_success(), "failures: {:?}", result.failures);
    }

    #[test]
    fn s2_4_conformance_test_skip_annotation() {
        let mut runner = ConformanceRunner::new();
        runner.add_test(ConformanceTest {
            name: "skipped_test".into(),
            source: "this is not valid code at all !!!".into(),
            expected_output: None,
            expected_error: None,
            category: ConformanceCategory::Lexer,
            skip: true,
            description: None,
        });
        let result = runner.run_all();
        assert_eq!(result.skipped, 1);
        assert_eq!(result.passed, 0);
        assert_eq!(result.failed, 0);
    }

    #[test]
    fn s2_5_conformance_load_from_source_parses_annotations() {
        let mut runner = ConformanceRunner::new();
        let source = r#"// category: types
// expect-error: SE004
// description: type mismatch on let binding
let x: i64 = "hello"
"#;
        runner
            .load_from_source("type_mismatch_file", source)
            .unwrap();
        assert_eq!(runner.test_count(), 1);
        let result = runner.run_all();
        assert!(result.is_success());
    }

    #[test]
    fn s2_6_conformance_load_from_source_skip() {
        let mut runner = ConformanceRunner::new();
        let source = "// skip\nlet x = ???";
        runner.load_from_source("skip_test", source).unwrap();
        let result = runner.run_all();
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn s2_7_conformance_run_category_filters() {
        let mut runner = ConformanceRunner::new();
        runner.add_test(ConformanceTest {
            name: "types_test".into(),
            source: "let x: i64 = 42".into(),
            expected_output: None,
            expected_error: None,
            category: ConformanceCategory::Types,
            skip: false,
            description: None,
        });
        runner.add_test(ConformanceTest {
            name: "runtime_test".into(),
            source: "let y: i64 = 10".into(),
            expected_output: None,
            expected_error: None,
            category: ConformanceCategory::Runtime,
            skip: false,
            description: None,
        });
        let types_result = runner.run_category(ConformanceCategory::Types);
        assert_eq!(types_result.total(), 1);
        let runtime_result = runner.run_category(ConformanceCategory::Runtime);
        assert_eq!(runtime_result.total(), 1);
    }

    #[test]
    fn s2_8_conformance_run_single_by_name() {
        let mut runner = ConformanceRunner::new();
        runner.add_test(ConformanceTest {
            name: "target_test".into(),
            source: "let x: i64 = 1".into(),
            expected_output: None,
            expected_error: None,
            category: ConformanceCategory::Runtime,
            skip: false,
            description: None,
        });
        let result = runner.run_single("target_test");
        assert!(result.is_some());
        assert!(result.unwrap().is_success());
        assert!(runner.run_single("nonexistent").is_none());
    }

    #[test]
    fn s2_9_conformance_category_display() {
        assert_eq!(ConformanceCategory::Lexer.to_string(), "lexer");
        assert_eq!(ConformanceCategory::Ownership.to_string(), "ownership");
        assert_eq!(ConformanceCategory::Features.to_string(), "features");
    }

    #[test]
    fn s2_10_conformance_pass_rate_calculation() {
        let result = ConformanceResult {
            passed: 8,
            failed: 2,
            skipped: 5,
            failures: Vec::new(),
        };
        assert_eq!(result.total(), 15);
        assert!((result.pass_rate() - 80.0).abs() < 0.01);
    }

    // ─── Sprint 3: Regression Harness (s3_1 - s3_10) ────────────────

    #[test]
    fn s3_1_snapshot_manager_save_and_load() {
        let dir = std::env::temp_dir().join("fj_test_snap_s3_1");
        let _ = std::fs::remove_dir_all(&dir);
        let mgr = SnapshotManager::new(dir.clone());

        let snap = Snapshot {
            name: "test_output".into(),
            content: "Int(42)".into(),
            metadata: HashMap::new(),
        };
        mgr.save(&snap).unwrap();
        let loaded = mgr.load("test_output").unwrap();
        assert_eq!(loaded.content, "Int(42)");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn s3_2_snapshot_compare_pass() {
        let dir = std::env::temp_dir().join("fj_test_snap_s3_2");
        let _ = std::fs::remove_dir_all(&dir);
        let mgr = SnapshotManager::new(dir.clone());

        mgr.bless("compare_test", "expected output").unwrap();
        let result = mgr.compare("compare_test", "expected output").unwrap();
        assert_eq!(result, RegressionResult::Pass);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn s3_3_snapshot_compare_mismatch() {
        let dir = std::env::temp_dir().join("fj_test_snap_s3_3");
        let _ = std::fs::remove_dir_all(&dir);
        let mgr = SnapshotManager::new(dir.clone());

        mgr.bless("mismatch_test", "old output").unwrap();
        let result = mgr.compare("mismatch_test", "new output").unwrap();
        assert!(matches!(result, RegressionResult::Mismatch { .. }));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn s3_4_snapshot_compare_new_snapshot() {
        let dir = std::env::temp_dir().join("fj_test_snap_s3_4");
        let _ = std::fs::remove_dir_all(&dir);
        let mgr = SnapshotManager::new(dir.clone());

        let result = mgr.compare("nonexistent", "any content").unwrap();
        assert_eq!(result, RegressionResult::NewSnapshot);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn s3_5_snapshot_metadata_roundtrip() {
        let dir = std::env::temp_dir().join("fj_test_snap_s3_5");
        let _ = std::fs::remove_dir_all(&dir);
        let mgr = SnapshotManager::new(dir.clone());

        let mut meta = HashMap::new();
        meta.insert("version".to_string(), "0.5.0".to_string());
        let snap = Snapshot {
            name: "meta_test".into(),
            content: "body content".into(),
            metadata: meta,
        };
        mgr.save(&snap).unwrap();
        let loaded = mgr.load("meta_test").unwrap();
        assert_eq!(
            loaded.metadata.get("version").map(|s| s.as_str()),
            Some("0.5.0")
        );
        assert_eq!(loaded.content, "body content");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn s3_6_baseline_recorder_records_and_serializes() {
        let mut rec = BaselineRecorder::new();
        rec.record("fib_20", 26.5);
        rec.record("lex_3000", 0.12);
        rec.set_metadata("commit", "abc123");
        assert_eq!(rec.entry_count(), 2);

        let json = rec.to_json().unwrap();
        assert!(json.contains("\"fib_20\""));
        assert!(json.contains("26.5"));
        assert!(json.contains("\"commit\""));
        assert!(json.contains("\"abc123\""));
    }

    #[test]
    fn s3_7_baseline_comparator_detects_regression() {
        let comp = BaselineComparator::with_threshold(10.0);
        let mut baseline = HashMap::new();
        baseline.insert("fib_20".into(), 100.0);
        baseline.insert("lex_3000".into(), 50.0);

        let mut current = HashMap::new();
        current.insert("fib_20".into(), 115.0); // 15% regression
        current.insert("lex_3000".into(), 52.0); // 4% — within threshold

        let regressions = comp.compare(&baseline, &current);
        assert_eq!(regressions.len(), 1);
        assert_eq!(regressions[0].benchmark, "fib_20");
        assert!((regressions[0].change_pct - 15.0).abs() < 0.1);
    }

    #[test]
    fn s3_8_baseline_comparator_no_regression_within_threshold() {
        let comp = BaselineComparator::with_threshold(10.0);
        let mut baseline = HashMap::new();
        baseline.insert("bench_a".into(), 100.0);

        let mut current = HashMap::new();
        current.insert("bench_a".into(), 108.0); // 8% — within 10% threshold

        let regressions = comp.compare(&baseline, &current);
        assert!(regressions.is_empty());
    }

    #[test]
    fn s3_9_bisect_helper_finds_first_bad_commit() {
        let commits: Vec<String> = (0..10).map(|i| format!("commit_{i}")).collect();
        let bisector = BisectHelper::new(commits);
        // Commits 0-5 are good, 6-9 are bad
        let result = bisector.bisect(|commit| {
            let num: usize = commit.strip_prefix("commit_").unwrap().parse().unwrap();
            num < 6
        });
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.first_bad, "commit_6");
        assert_eq!(r.last_good, "commit_5");
        // Binary search should take O(log n) steps
        assert!(r.steps <= 8, "took {} steps for 10 commits", r.steps);
    }

    #[test]
    fn s3_10_bisect_helper_all_good_returns_none() {
        let commits: Vec<String> = (0..5).map(|i| format!("c{i}")).collect();
        let bisector = BisectHelper::new(commits);
        let result = bisector.bisect(|_| true);
        assert!(result.is_none());
    }

    // ─── Sprint 4: Error Polisher (s4_1 - s4_10) ────────────────────

    #[test]
    fn s4_1_error_quality_score_all_features() {
        let quality = ErrorQuality {
            has_span: true,
            has_help: true,
            has_note: true,
            has_error_code: true,
            has_suggestion: true,
            readable_message: true,
        };
        assert_eq!(quality.score(), 100);
    }

    #[test]
    fn s4_2_error_quality_score_minimal() {
        let quality = ErrorQuality {
            has_span: false,
            has_help: false,
            has_note: false,
            has_error_code: false,
            has_suggestion: false,
            readable_message: false,
        };
        assert_eq!(quality.score(), 0);
    }

    #[test]
    fn s4_3_error_catalog_contains_all_categories() {
        let catalog = ErrorCatalog::new();
        let prefixes: Vec<String> = catalog.entries().iter().map(|e| e.prefix.clone()).collect();
        assert!(prefixes.contains(&"LE".to_string()));
        assert!(prefixes.contains(&"PE".to_string()));
        assert!(prefixes.contains(&"SE".to_string()));
        assert!(prefixes.contains(&"KE".to_string()));
        assert!(prefixes.contains(&"DE".to_string()));
        assert!(prefixes.contains(&"TE".to_string()));
        assert!(prefixes.contains(&"RE".to_string()));
        assert!(prefixes.contains(&"ME".to_string()));
        assert!(prefixes.contains(&"CE".to_string()));
    }

    #[test]
    fn s4_4_error_catalog_lookup_by_code() {
        let catalog = ErrorCatalog::new();
        let entry = catalog.lookup("SE004");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.code, "SE004");
        assert!(entry.description.contains("Type mismatch"));
    }

    #[test]
    fn s4_5_error_catalog_by_prefix_filters() {
        let catalog = ErrorCatalog::new();
        let lex_errors = catalog.by_prefix("LE");
        assert_eq!(lex_errors.len(), 8);
        for entry in lex_errors {
            assert!(entry.code.starts_with("LE"));
        }
    }

    #[test]
    fn s4_6_error_catalog_average_quality_reasonable() {
        let catalog = ErrorCatalog::new();
        let avg = catalog.average_quality();
        // Most errors should have at least error_code + readable + span = 60
        assert!(avg > 40.0, "average quality too low: {avg}");
        assert!(avg <= 100.0);
    }

    #[test]
    fn s4_7_error_audit_report_structure() {
        let audit = ErrorAudit::new();
        let report = audit.run();
        assert!(
            report.total_errors > 60,
            "expected > 60 error codes, got {}",
            report.total_errors
        );
        assert!(report.max_score >= report.min_score);
        assert!(report.average_score > 0.0);
        // Category scores should include all prefixes
        assert!(report.category_scores.contains_key("SE"));
        assert!(report.category_scores.contains_key("LE"));
    }

    #[test]
    fn s4_8_error_formatter_validates_error_codes() {
        assert!(ErrorFormatter::validate_error_code("SE004"));
        assert!(ErrorFormatter::validate_error_code("LE001"));
        assert!(ErrorFormatter::validate_error_code("ME010"));
        assert!(!ErrorFormatter::validate_error_code("se004")); // lowercase
        assert!(!ErrorFormatter::validate_error_code("SE04")); // too short
        assert!(!ErrorFormatter::validate_error_code("SEABC")); // letters in number
        assert!(!ErrorFormatter::validate_error_code("S0004")); // digit in prefix
    }

    #[test]
    fn s4_9_error_formatter_renders_lex_error() {
        // Use an unterminated string literal which the lexer reports as LE002
        let source = "let x = \"unterminated";
        let tokens_result = crate::lexer::tokenize(source);
        assert!(
            tokens_result.is_err(),
            "expected lex error for unterminated string"
        );
        if let Err(errors) = tokens_result {
            let rendered = ErrorFormatter::format_lex_error(&errors[0], "test.fj", source);
            assert!(!rendered.is_empty());
            // Should contain the error code
            assert!(
                rendered.contains("LE"),
                "rendered error missing code: {rendered}"
            );
        }
    }

    #[test]
    fn s4_10_error_catalog_all_codes_have_valid_format() {
        let catalog = ErrorCatalog::new();
        for entry in catalog.entries() {
            assert!(
                ErrorFormatter::validate_error_code(&entry.code),
                "invalid error code format: {}",
                entry.code
            );
        }
    }
}
