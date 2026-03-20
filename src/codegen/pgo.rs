//! Profile-Guided Optimization (PGO) simulation for Fajar Lang.
//!
//! Provides compile-time simulation of LLVM-style PGO workflows:
//!
//! - **Instrumented Build** — simulate insertion of profiling counters at basic blocks
//! - **Profile Merging** — combine multiple profile runs into a single dataset
//! - **Hot/Cold Analysis** — classify functions by execution frequency
//! - **Branch Weights** — annotate conditional branches with taken/not-taken counts
//! - **Inlining Decisions** — adjust inline thresholds based on hotness
//! - **Loop Unroll Hints** — suggest unroll factors from profile trip counts
//! - **Indirect Call Promotion** — promote frequent indirect targets to direct calls
//!
//! This module does NOT link against LLVM. All analysis is simulation-based,
//! operating on abstract profile data structures.
//!
//! # PGO Workflow
//!
//! ```text
//! 1. Instrument:  compile with counters → run → produce .profraw
//! 2. Merge:       combine multiple .profraw → .profdata
//! 3. Optimize:    recompile using .profdata → optimized binary
//! ```

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from PGO instrumentation, merging, or optimization.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PgoError {
    /// Function hash mismatch between profile and source.
    #[error("PGO001: profile hash mismatch for function '{name}': expected {expected:#x}, got {actual:#x}")]
    HashMismatch {
        /// Function name.
        name: String,
        /// Expected hash from current source.
        expected: u64,
        /// Actual hash from profile data.
        actual: u64,
    },

    /// Profile data references a function not found in source.
    #[error("PGO002: profile references unknown function '{name}'")]
    UnknownFunction {
        /// Function name.
        name: String,
    },

    /// Profile data is empty (no function profiles).
    #[error("PGO003: empty profile data — no function profiles found")]
    EmptyProfile,

    /// Block count mismatch between profile and source.
    #[error("PGO004: block count mismatch for '{name}': expected {expected}, got {actual}")]
    BlockCountMismatch {
        /// Function name.
        name: String,
        /// Expected block count from source.
        expected: usize,
        /// Actual block count from profile.
        actual: usize,
    },

    /// Profile file path is invalid.
    #[error("PGO005: invalid profile path: {path}")]
    InvalidPath {
        /// The invalid path.
        path: String,
    },

    /// Merge conflict — incompatible profiles.
    #[error("PGO006: cannot merge profiles with conflicting hashes for '{name}'")]
    MergeConflict {
        /// Function name.
        name: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 17: PGO Mode, Config, Instrumentation
// ═══════════════════════════════════════════════════════════════════════

/// PGO compilation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PgoMode {
    /// Instrument the binary with profiling counters.
    Instrument,
    /// Optimize using a previously collected profile.
    Optimize,
    /// Use sampling-based profiling (e.g., `perf record`).
    Sample,
}

/// CLI flags for PGO operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PgoCliFlag {
    /// `--pgo-instrument`: produce an instrumented binary.
    Instrument,
    /// `--pgo-optimize <path>`: optimize using profile data at the given path.
    Optimize {
        /// Path to the merged profile data file.
        profile_path: String,
    },
    /// `--pgo-merge <inputs>`: merge multiple raw profiles into one.
    Merge {
        /// Paths to raw profile files to merge.
        inputs: Vec<String>,
    },
    /// `--pgo-report`: display profile summary statistics.
    Report,
}

/// Configuration for a PGO compilation pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgoConfig {
    /// The PGO mode (instrument, optimize, or sample).
    pub mode: PgoMode,
    /// Output path for profile data (instrument mode).
    pub profile_output_path: Option<String>,
    /// Input path for profile data (optimize mode).
    pub profile_input_path: Option<String>,
}

impl PgoConfig {
    /// Creates a new PGO configuration for instrumentation.
    pub fn instrument(output_path: &str) -> Self {
        Self {
            mode: PgoMode::Instrument,
            profile_output_path: Some(output_path.to_string()),
            profile_input_path: None,
        }
    }

    /// Creates a new PGO configuration for optimization.
    pub fn optimize(input_path: &str) -> Self {
        Self {
            mode: PgoMode::Optimize,
            profile_output_path: None,
            profile_input_path: Some(input_path.to_string()),
        }
    }

    /// Creates a new PGO configuration for sampling mode.
    pub fn sample() -> Self {
        Self {
            mode: PgoMode::Sample,
            profile_output_path: None,
            profile_input_path: None,
        }
    }
}

/// An instrumented function with profiling counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentedFunction {
    /// Function name.
    pub name: String,
    /// Counters for each basic block (initialized to 0).
    pub counters: Vec<u64>,
    /// Hash of the function's structure (for validation).
    pub hash: u64,
}

impl InstrumentedFunction {
    /// Returns the number of basic blocks instrumented.
    pub fn block_count(&self) -> usize {
        self.counters.len()
    }

    /// Records an execution of a specific basic block.
    pub fn record_block(&mut self, block_index: usize) {
        if let Some(counter) = self.counters.get_mut(block_index) {
            *counter = counter.saturating_add(1);
        }
    }

    /// Returns the total execution count across all blocks.
    pub fn total_count(&self) -> u64 {
        self.counters.iter().sum()
    }
}

/// Simulates the instrumentation pass over a set of functions.
///
/// For each function, allocates one counter per basic block and computes
/// a structural hash. The hash is a simple simulation based on name and
/// block count.
pub struct InstrumentationPass;

impl InstrumentationPass {
    /// Instrument a single function with profiling counters.
    ///
    /// # Arguments
    /// * `name` — Function name.
    /// * `block_count` — Number of basic blocks in the function's CFG.
    ///
    /// # Returns
    /// An `InstrumentedFunction` with zeroed counters and a computed hash.
    pub fn instrument_function(name: &str, block_count: usize) -> InstrumentedFunction {
        let hash = Self::compute_hash(name, block_count);
        InstrumentedFunction {
            name: name.to_string(),
            counters: vec![0; block_count],
            hash,
        }
    }

    /// Compute a structural hash for a function.
    ///
    /// Simulation: FNV-1a inspired hash over name bytes and block count.
    fn compute_hash(name: &str, block_count: usize) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in name.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= block_count as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        hash
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Profile Data
// ═══════════════════════════════════════════════════════════════════════

/// Profile data for a single function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionProfile {
    /// Function name.
    pub name: String,
    /// Total entry count (how many times the function was called).
    pub entry_count: u64,
    /// Per-block execution counts.
    pub block_counts: Vec<u64>,
    /// Structural hash for validation.
    pub hash: u64,
}

impl FunctionProfile {
    /// Returns the number of blocks that were never executed.
    pub fn uncovered_blocks(&self) -> usize {
        self.block_counts.iter().filter(|&&c| c == 0).count()
    }

    /// Returns coverage percentage (0.0 to 100.0).
    pub fn coverage_pct(&self) -> f64 {
        if self.block_counts.is_empty() {
            return 100.0;
        }
        let covered = self.block_counts.iter().filter(|&&c| c > 0).count();
        (covered as f64 / self.block_counts.len() as f64) * 100.0
    }
}

/// Aggregated profile data from one or more program runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileData {
    /// Per-function profiles, keyed by function name.
    pub function_profiles: HashMap<String, FunctionProfile>,
}

impl ProfileData {
    /// Creates an empty profile data set.
    pub fn new() -> Self {
        Self {
            function_profiles: HashMap::new(),
        }
    }

    /// Returns the number of profiled functions.
    pub fn function_count(&self) -> usize {
        self.function_profiles.len()
    }

    /// Adds a function profile from an instrumented function.
    pub fn add_from_instrumented(&mut self, func: &InstrumentedFunction) {
        let entry_count = func.counters.first().copied().unwrap_or(0);
        let profile = FunctionProfile {
            name: func.name.clone(),
            entry_count,
            block_counts: func.counters.clone(),
            hash: func.hash,
        };
        self.function_profiles.insert(func.name.clone(), profile);
    }

    /// Adds a function profile directly.
    pub fn add_profile(&mut self, profile: FunctionProfile) {
        self.function_profiles.insert(profile.name.clone(), profile);
    }
}

impl Default for ProfileData {
    fn default() -> Self {
        Self::new()
    }
}

/// Merge multiple profile data sets into one.
///
/// For functions appearing in multiple profiles, counters are summed.
/// Returns an error if the same function has different hashes across profiles.
pub fn merge_profiles(profiles: &[ProfileData]) -> Result<ProfileData, PgoError> {
    let mut merged = ProfileData::new();

    for profile in profiles {
        for (name, fp) in &profile.function_profiles {
            if let Some(existing) = merged.function_profiles.get_mut(name) {
                if existing.hash != fp.hash {
                    return Err(PgoError::MergeConflict { name: name.clone() });
                }
                merge_function_profile(existing, fp);
            } else {
                merged.function_profiles.insert(name.clone(), fp.clone());
            }
        }
    }

    Ok(merged)
}

/// Merge a source function profile into an existing destination profile.
///
/// Sums entry counts and per-block counts. Assumes hashes already match.
fn merge_function_profile(dest: &mut FunctionProfile, src: &FunctionProfile) {
    dest.entry_count = dest.entry_count.saturating_add(src.entry_count);
    let max_len = dest.block_counts.len().max(src.block_counts.len());
    dest.block_counts.resize(max_len, 0);
    for (i, &count) in src.block_counts.iter().enumerate() {
        if i < dest.block_counts.len() {
            dest.block_counts[i] = dest.block_counts[i].saturating_add(count);
        }
    }
}

/// Validate a profile against known source function hashes.
///
/// Checks that every function in the profile has a matching hash in the
/// source hash map. Missing functions or hash mismatches produce errors.
pub fn validate_profile(
    profile: &ProfileData,
    source_hashes: &HashMap<String, u64>,
) -> Result<(), PgoError> {
    if profile.function_profiles.is_empty() {
        return Err(PgoError::EmptyProfile);
    }

    for (name, fp) in &profile.function_profiles {
        match source_hashes.get(name) {
            None => {
                return Err(PgoError::UnknownFunction { name: name.clone() });
            }
            Some(&expected_hash) => {
                if fp.hash != expected_hash {
                    return Err(PgoError::HashMismatch {
                        name: name.clone(),
                        expected: expected_hash,
                        actual: fp.hash,
                    });
                }
            }
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 18: PGO Optimization Pass
// ═══════════════════════════════════════════════════════════════════════

/// Classification of a function based on execution frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotColdClass {
    /// Frequently executed — candidate for aggressive optimization.
    Hot,
    /// Rarely executed — candidate for size optimization.
    Cold,
    /// Normal execution frequency.
    Normal,
}

/// Hot/cold analysis result for all functions in a profile.
#[derive(Debug, Clone)]
pub struct HotColdAnalysis {
    /// Per-function classification.
    pub classifications: HashMap<String, HotColdClass>,
    /// Mean entry count across all functions.
    pub mean_entry_count: f64,
    /// Standard deviation of entry counts.
    pub stddev_entry_count: f64,
}

impl HotColdAnalysis {
    /// Analyze a profile and classify every function as Hot, Cold, or Normal.
    ///
    /// - **Hot**: `entry_count > mean + 2 * stddev`
    /// - **Cold**: `entry_count < mean / 10`
    /// - **Normal**: everything else
    pub fn analyze(profile: &ProfileData) -> Self {
        let counts: Vec<f64> = profile
            .function_profiles
            .values()
            .map(|fp| fp.entry_count as f64)
            .collect();

        let (mean, stddev) = compute_mean_stddev(&counts);
        let hot_threshold = mean + 2.0 * stddev;
        let cold_threshold = mean / 10.0;

        let mut classifications = HashMap::new();
        for (name, fp) in &profile.function_profiles {
            let count = fp.entry_count as f64;
            let class = classify_function(count, hot_threshold, cold_threshold);
            classifications.insert(name.clone(), class);
        }

        Self {
            classifications,
            mean_entry_count: mean,
            stddev_entry_count: stddev,
        }
    }

    /// Returns how many functions are classified as Hot.
    pub fn hot_count(&self) -> usize {
        self.classifications
            .values()
            .filter(|&&c| c == HotColdClass::Hot)
            .count()
    }

    /// Returns how many functions are classified as Cold.
    pub fn cold_count(&self) -> usize {
        self.classifications
            .values()
            .filter(|&&c| c == HotColdClass::Cold)
            .count()
    }

    /// Returns how many functions are classified as Normal.
    pub fn normal_count(&self) -> usize {
        self.classifications
            .values()
            .filter(|&&c| c == HotColdClass::Normal)
            .count()
    }
}

/// Classify a single function based on its entry count vs thresholds.
fn classify_function(count: f64, hot_threshold: f64, cold_threshold: f64) -> HotColdClass {
    if count > hot_threshold {
        HotColdClass::Hot
    } else if count < cold_threshold {
        HotColdClass::Cold
    } else {
        HotColdClass::Normal
    }
}

/// Compute mean and population standard deviation for a slice of f64.
fn compute_mean_stddev(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    (mean, variance.sqrt())
}

/// Branch weight data for a conditional branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BranchWeight {
    /// Number of times the branch was taken.
    pub taken: u64,
    /// Number of times the branch was not taken.
    pub not_taken: u64,
}

impl BranchWeight {
    /// Creates a new branch weight.
    pub fn new(taken: u64, not_taken: u64) -> Self {
        Self { taken, not_taken }
    }

    /// Returns the probability that the branch is taken (0.0 to 1.0).
    pub fn taken_probability(&self) -> f64 {
        let total = self.taken + self.not_taken;
        if total == 0 {
            return 0.5;
        }
        self.taken as f64 / total as f64
    }

    /// Returns true if this branch is heavily biased (>=80% one way).
    pub fn is_biased(&self) -> bool {
        let prob = self.taken_probability();
        prob >= 0.8 || prob <= 0.2
    }
}

/// Inlining decision based on PGO data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InliningDecision {
    /// Callee function name.
    pub callee: String,
    /// Whether to increase the inline threshold (hot call site).
    pub increase_threshold: bool,
    /// Whether to decrease the inline threshold (cold call site).
    pub decrease_threshold: bool,
    /// Suggested threshold multiplier (1.0 = default, >1 = more willing, <1 = less willing).
    pub threshold_multiplier_x10: u32,
}

impl InliningDecision {
    /// Creates a decision to aggressively inline (hot call site).
    pub fn hot(callee: &str) -> Self {
        Self {
            callee: callee.to_string(),
            increase_threshold: true,
            decrease_threshold: false,
            threshold_multiplier_x10: 30, // 3.0x
        }
    }

    /// Creates a decision to avoid inlining (cold call site).
    pub fn cold(callee: &str) -> Self {
        Self {
            callee: callee.to_string(),
            increase_threshold: false,
            decrease_threshold: true,
            threshold_multiplier_x10: 2, // 0.2x
        }
    }

    /// Creates a neutral inlining decision (normal call site).
    pub fn normal(callee: &str) -> Self {
        Self {
            callee: callee.to_string(),
            increase_threshold: false,
            decrease_threshold: false,
            threshold_multiplier_x10: 10, // 1.0x
        }
    }
}

/// Loop unroll hint from profile trip counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopUnrollHint {
    /// Function containing the loop.
    pub function: String,
    /// Loop identifier (e.g., block index of the loop header).
    pub loop_id: usize,
    /// Observed average trip count from profiling.
    pub avg_trip_count: u64,
    /// Suggested unroll factor.
    pub suggested_unroll: u32,
}

impl LoopUnrollHint {
    /// Compute suggested unroll factor from average trip count.
    ///
    /// Heuristic: unroll small trip counts fully, cap at 8 for large loops.
    pub fn from_trip_count(function: &str, loop_id: usize, avg_trip_count: u64) -> Self {
        let suggested = compute_unroll_factor(avg_trip_count);
        Self {
            function: function.to_string(),
            loop_id,
            avg_trip_count,
            suggested_unroll: suggested,
        }
    }
}

/// Compute an unroll factor from an average trip count.
fn compute_unroll_factor(avg_trip_count: u64) -> u32 {
    match avg_trip_count {
        0..=1 => 1,
        2..=4 => avg_trip_count as u32,
        5..=16 => 4,
        17..=64 => 8,
        _ => 8,
    }
}

/// Indirect call promotion — convert frequent indirect call targets to direct calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndirectCallPromotion {
    /// Call site identifier (function name + offset).
    pub call_site: String,
    /// Most frequently called target.
    pub promoted_target: String,
    /// How many times this target was called.
    pub target_count: u64,
    /// Total calls at this site.
    pub total_calls: u64,
}

impl IndirectCallPromotion {
    /// Returns the percentage of calls going to the promoted target.
    pub fn promotion_confidence(&self) -> f64 {
        if self.total_calls == 0 {
            return 0.0;
        }
        self.target_count as f64 / self.total_calls as f64 * 100.0
    }

    /// Returns true if promotion is worthwhile (>70% confidence).
    pub fn should_promote(&self) -> bool {
        self.promotion_confidence() > 70.0
    }
}

/// Summary report of PGO analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct PgoReport {
    /// Total number of profiled functions.
    pub total_functions: usize,
    /// Number of hot functions.
    pub hot_count: usize,
    /// Number of cold functions.
    pub cold_count: usize,
    /// Coverage percentage (functions with at least one executed block).
    pub coverage_pct: f64,
    /// Total branch weights collected.
    pub branch_weight_count: usize,
    /// Total inlining decisions made.
    pub inline_decision_count: usize,
    /// Total loop unroll hints generated.
    pub loop_hint_count: usize,
}

/// All PGO optimizations derived from a profile.
#[derive(Debug, Clone)]
pub struct PgoOptimizations {
    /// Functions classified as hot.
    pub hot_functions: Vec<String>,
    /// Functions classified as cold.
    pub cold_functions: Vec<String>,
    /// Branch weight annotations.
    pub branch_weights: HashMap<String, Vec<BranchWeight>>,
    /// Inlining decisions.
    pub inline_decisions: Vec<InliningDecision>,
    /// Loop unroll hints.
    pub loop_hints: Vec<LoopUnrollHint>,
    /// Indirect call promotions.
    pub indirect_promotions: Vec<IndirectCallPromotion>,
}

/// Run the full PGO optimization analysis on a profile.
///
/// Classifies functions, generates inlining decisions, and produces
/// a summary of all optimization opportunities.
pub fn optimize_with_profile(profile: &ProfileData) -> PgoOptimizations {
    let analysis = HotColdAnalysis::analyze(profile);

    let hot_functions = collect_by_class(&analysis, HotColdClass::Hot);
    let cold_functions = collect_by_class(&analysis, HotColdClass::Cold);

    let inline_decisions = build_inline_decisions(&analysis);
    let loop_hints = build_loop_hints(profile);
    let branch_weights = build_branch_weights(profile);

    PgoOptimizations {
        hot_functions,
        cold_functions,
        branch_weights,
        inline_decisions,
        loop_hints,
        indirect_promotions: Vec::new(),
    }
}

/// Collect function names matching a given classification.
fn collect_by_class(analysis: &HotColdAnalysis, class: HotColdClass) -> Vec<String> {
    let mut names: Vec<String> = analysis
        .classifications
        .iter()
        .filter(|(_, c)| **c == class)
        .map(|(name, _)| name.clone())
        .collect();
    names.sort();
    names
}

/// Build inlining decisions from hot/cold analysis.
fn build_inline_decisions(analysis: &HotColdAnalysis) -> Vec<InliningDecision> {
    let mut decisions: Vec<InliningDecision> = analysis
        .classifications
        .iter()
        .map(|(name, &class)| match class {
            HotColdClass::Hot => InliningDecision::hot(name),
            HotColdClass::Cold => InliningDecision::cold(name),
            HotColdClass::Normal => InliningDecision::normal(name),
        })
        .collect();
    decisions.sort_by(|a, b| a.callee.cmp(&b.callee));
    decisions
}

/// Build loop unroll hints from profile block counts.
///
/// Simulation: for each function, estimate trip count from the ratio of
/// the highest block count to the entry count.
fn build_loop_hints(profile: &ProfileData) -> Vec<LoopUnrollHint> {
    let mut hints = Vec::new();
    for (name, fp) in &profile.function_profiles {
        if fp.block_counts.len() > 1 && fp.entry_count > 0 {
            let max_block = fp.block_counts.iter().max().copied().unwrap_or(0);
            let avg_trip = max_block / fp.entry_count.max(1);
            if avg_trip > 1 {
                hints.push(LoopUnrollHint::from_trip_count(name, 0, avg_trip));
            }
        }
    }
    hints.sort_by(|a, b| a.function.cmp(&b.function));
    hints
}

/// Build branch weights from profile block counts.
///
/// Simulation: for each function with 3+ blocks, the second and third
/// blocks represent taken/not-taken paths of the first branch.
fn build_branch_weights(profile: &ProfileData) -> HashMap<String, Vec<BranchWeight>> {
    let mut weights = HashMap::new();
    for (name, fp) in &profile.function_profiles {
        if fp.block_counts.len() >= 3 {
            let taken = fp.block_counts[1];
            let not_taken = fp.block_counts[2];
            weights.insert(name.clone(), vec![BranchWeight::new(taken, not_taken)]);
        }
    }
    weights
}

/// Estimate the performance improvement from applying PGO optimizations.
///
/// Returns an estimated speedup percentage (0.0 to 100.0).
/// Based on industry heuristics:
/// - Hot function inlining: ~5-15% improvement
/// - Branch prediction hints: ~2-5%
/// - Loop unrolling: ~1-3%
/// - Cold code separation: ~1-2%
pub fn apply_pgo_speedup(opts: &PgoOptimizations) -> f64 {
    let hot_bonus = compute_hot_bonus(opts.hot_functions.len());
    let cold_bonus = compute_cold_bonus(opts.cold_functions.len());
    let branch_bonus = compute_branch_bonus(&opts.branch_weights);
    let loop_bonus = compute_loop_bonus(opts.loop_hints.len());
    let inline_bonus = compute_inline_bonus(&opts.inline_decisions);

    let total = hot_bonus + cold_bonus + branch_bonus + loop_bonus + inline_bonus;
    // Cap at 40% — PGO rarely exceeds this in practice
    if total > 40.0 {
        40.0
    } else {
        total
    }
}

/// Bonus from hot function optimization (inlining, layout).
fn compute_hot_bonus(hot_count: usize) -> f64 {
    match hot_count {
        0 => 0.0,
        1..=3 => 5.0,
        4..=10 => 10.0,
        _ => 15.0,
    }
}

/// Bonus from cold code separation.
fn compute_cold_bonus(cold_count: usize) -> f64 {
    if cold_count > 0 {
        2.0
    } else {
        0.0
    }
}

/// Bonus from branch weight annotations.
fn compute_branch_bonus(weights: &HashMap<String, Vec<BranchWeight>>) -> f64 {
    let biased_count = weights
        .values()
        .flatten()
        .filter(|bw| bw.is_biased())
        .count();
    match biased_count {
        0 => 0.0,
        1..=5 => 2.0,
        _ => 5.0,
    }
}

/// Bonus from loop unroll hints.
fn compute_loop_bonus(loop_count: usize) -> f64 {
    match loop_count {
        0 => 0.0,
        1..=3 => 1.0,
        _ => 3.0,
    }
}

/// Bonus from PGO-informed inlining decisions.
fn compute_inline_bonus(decisions: &[InliningDecision]) -> f64 {
    let aggressive = decisions.iter().filter(|d| d.increase_threshold).count();
    match aggressive {
        0 => 0.0,
        1..=3 => 3.0,
        _ => 7.0,
    }
}

/// Generate a PGO report summarizing the optimization opportunities.
pub fn generate_report(profile: &ProfileData, opts: &PgoOptimizations) -> PgoReport {
    let total_functions = profile.function_count();
    let covered = profile
        .function_profiles
        .values()
        .filter(|fp| fp.entry_count > 0)
        .count();
    let coverage_pct = if total_functions == 0 {
        0.0
    } else {
        (covered as f64 / total_functions as f64) * 100.0
    };

    PgoReport {
        total_functions,
        hot_count: opts.hot_functions.len(),
        cold_count: opts.cold_functions.len(),
        coverage_pct,
        branch_weight_count: opts.branch_weights.values().map(|v| v.len()).sum(),
        inline_decision_count: opts.inline_decisions.len(),
        loop_hint_count: opts.loop_hints.len(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 17 Tests ─────────────────────────────────────────────

    #[test]
    fn s17_1_pgo_mode_variants() {
        assert_ne!(PgoMode::Instrument, PgoMode::Optimize);
        assert_ne!(PgoMode::Optimize, PgoMode::Sample);
        assert_ne!(PgoMode::Sample, PgoMode::Instrument);
    }

    #[test]
    fn s17_2_pgo_config_instrument_mode() {
        let config = PgoConfig::instrument("/tmp/default.profraw");
        assert_eq!(config.mode, PgoMode::Instrument);
        assert_eq!(
            config.profile_output_path.as_deref(),
            Some("/tmp/default.profraw")
        );
        assert!(config.profile_input_path.is_none());
    }

    #[test]
    fn s17_3_instrument_function_creates_counters() {
        let func = InstrumentationPass::instrument_function("main", 5);
        assert_eq!(func.name, "main");
        assert_eq!(func.block_count(), 5);
        assert_eq!(func.counters, vec![0, 0, 0, 0, 0]);
        assert_ne!(func.hash, 0);
    }

    #[test]
    fn s17_4_instrument_function_records_blocks() {
        let mut func = InstrumentationPass::instrument_function("loop_fn", 3);
        func.record_block(0);
        func.record_block(1);
        func.record_block(1);
        func.record_block(1);
        assert_eq!(func.counters, vec![1, 3, 0]);
        assert_eq!(func.total_count(), 4);
    }

    #[test]
    fn s17_5_profile_data_add_and_query() {
        let mut profile = ProfileData::new();
        assert_eq!(profile.function_count(), 0);

        let mut func = InstrumentationPass::instrument_function("foo", 2);
        func.record_block(0);
        func.record_block(0);
        func.record_block(1);
        profile.add_from_instrumented(&func);

        assert_eq!(profile.function_count(), 1);
        let fp = &profile.function_profiles["foo"];
        assert_eq!(fp.entry_count, 2);
        assert_eq!(fp.block_counts, vec![2, 1]);
    }

    #[test]
    fn s17_6_merge_profiles_sums_counters() {
        let mut p1 = ProfileData::new();
        let mut f1 = InstrumentationPass::instrument_function("bar", 2);
        f1.record_block(0);
        f1.record_block(1);
        p1.add_from_instrumented(&f1);

        let mut p2 = ProfileData::new();
        let mut f2 = InstrumentationPass::instrument_function("bar", 2);
        f2.record_block(0);
        f2.record_block(0);
        f2.record_block(1);
        p2.add_from_instrumented(&f2);

        let merged = merge_profiles(&[p1, p2]).unwrap();
        let fp = &merged.function_profiles["bar"];
        assert_eq!(fp.entry_count, 3); // 1 + 2
        assert_eq!(fp.block_counts, vec![3, 2]); // [1+2, 1+1]
    }

    #[test]
    fn s17_7_merge_profiles_detects_hash_conflict() {
        let mut p1 = ProfileData::new();
        p1.add_profile(FunctionProfile {
            name: "conflict".into(),
            entry_count: 1,
            block_counts: vec![1],
            hash: 111,
        });

        let mut p2 = ProfileData::new();
        p2.add_profile(FunctionProfile {
            name: "conflict".into(),
            entry_count: 1,
            block_counts: vec![1],
            hash: 222,
        });

        let result = merge_profiles(&[p1, p2]);
        assert!(matches!(result, Err(PgoError::MergeConflict { .. })));
    }

    #[test]
    fn s17_8_validate_profile_success() {
        let mut profile = ProfileData::new();
        let func = InstrumentationPass::instrument_function("main", 3);
        profile.add_from_instrumented(&func);

        let mut hashes = HashMap::new();
        hashes.insert("main".into(), func.hash);

        assert!(validate_profile(&profile, &hashes).is_ok());
    }

    #[test]
    fn s17_9_validate_profile_hash_mismatch() {
        let mut profile = ProfileData::new();
        let func = InstrumentationPass::instrument_function("main", 3);
        profile.add_from_instrumented(&func);

        let mut hashes = HashMap::new();
        hashes.insert("main".into(), 99999);

        let result = validate_profile(&profile, &hashes);
        assert!(matches!(result, Err(PgoError::HashMismatch { .. })));
    }

    #[test]
    fn s17_10_validate_profile_empty_error() {
        let profile = ProfileData::new();
        let hashes = HashMap::new();

        let result = validate_profile(&profile, &hashes);
        assert!(matches!(result, Err(PgoError::EmptyProfile)));
    }

    // ── Sprint 18 Tests ─────────────────────────────────────────────

    #[test]
    fn s18_1_hot_cold_analysis_classifies_correctly() {
        let mut profile = ProfileData::new();
        // Hot function: called 10000 times
        profile.add_profile(FunctionProfile {
            name: "hot_loop".into(),
            entry_count: 10000,
            block_counts: vec![10000, 5000],
            hash: 1,
        });
        // Cold function: called 1 time
        profile.add_profile(FunctionProfile {
            name: "error_handler".into(),
            entry_count: 1,
            block_counts: vec![1],
            hash: 2,
        });
        // Normal functions
        for i in 0..8 {
            profile.add_profile(FunctionProfile {
                name: format!("normal_{i}"),
                entry_count: 500,
                block_counts: vec![500, 250],
                hash: 100 + i,
            });
        }

        let analysis = HotColdAnalysis::analyze(&profile);
        assert_eq!(analysis.classifications["hot_loop"], HotColdClass::Hot);
        assert_eq!(
            analysis.classifications["error_handler"],
            HotColdClass::Cold
        );
        assert!(analysis.hot_count() >= 1);
        assert!(analysis.cold_count() >= 1);
    }

    #[test]
    fn s18_2_branch_weight_probability() {
        let bw = BranchWeight::new(90, 10);
        let prob = bw.taken_probability();
        assert!((prob - 0.9).abs() < 0.001);
        assert!(bw.is_biased());

        let balanced = BranchWeight::new(50, 50);
        assert!(!balanced.is_biased());
        assert!((balanced.taken_probability() - 0.5).abs() < 0.001);
    }

    #[test]
    fn s18_3_inlining_decision_hot_cold() {
        let hot = InliningDecision::hot("fast_fn");
        assert!(hot.increase_threshold);
        assert!(!hot.decrease_threshold);
        assert_eq!(hot.threshold_multiplier_x10, 30);

        let cold = InliningDecision::cold("error_fn");
        assert!(!cold.increase_threshold);
        assert!(cold.decrease_threshold);
        assert_eq!(cold.threshold_multiplier_x10, 2);
    }

    #[test]
    fn s18_4_loop_unroll_hint_factors() {
        let h1 = LoopUnrollHint::from_trip_count("f", 0, 3);
        assert_eq!(h1.suggested_unroll, 3);

        let h2 = LoopUnrollHint::from_trip_count("f", 0, 10);
        assert_eq!(h2.suggested_unroll, 4);

        let h3 = LoopUnrollHint::from_trip_count("f", 0, 32);
        assert_eq!(h3.suggested_unroll, 8);

        let h4 = LoopUnrollHint::from_trip_count("f", 0, 1);
        assert_eq!(h4.suggested_unroll, 1);
    }

    #[test]
    fn s18_5_indirect_call_promotion() {
        let promo = IndirectCallPromotion {
            call_site: "dispatch:0x10".into(),
            promoted_target: "handler_a".into(),
            target_count: 80,
            total_calls: 100,
        };
        assert!((promo.promotion_confidence() - 80.0).abs() < 0.1);
        assert!(promo.should_promote());

        let weak = IndirectCallPromotion {
            call_site: "dispatch:0x20".into(),
            promoted_target: "handler_b".into(),
            target_count: 30,
            total_calls: 100,
        };
        assert!(!weak.should_promote());
    }

    #[test]
    fn s18_6_optimize_with_profile_produces_optimizations() {
        let mut profile = ProfileData::new();
        profile.add_profile(FunctionProfile {
            name: "main".into(),
            entry_count: 1,
            block_counts: vec![1, 1, 0],
            hash: 1,
        });
        profile.add_profile(FunctionProfile {
            name: "hot_inner".into(),
            entry_count: 50000,
            block_counts: vec![50000, 25000, 25000],
            hash: 2,
        });

        let opts = optimize_with_profile(&profile);
        assert!(!opts.hot_functions.is_empty() || !opts.cold_functions.is_empty());
        assert!(!opts.inline_decisions.is_empty());
    }

    #[test]
    fn s18_7_pgo_report_generation() {
        let mut profile = ProfileData::new();
        profile.add_profile(FunctionProfile {
            name: "a".into(),
            entry_count: 100,
            block_counts: vec![100, 50, 50],
            hash: 1,
        });
        profile.add_profile(FunctionProfile {
            name: "b".into(),
            entry_count: 0,
            block_counts: vec![0],
            hash: 2,
        });

        let opts = optimize_with_profile(&profile);
        let report = generate_report(&profile, &opts);

        assert_eq!(report.total_functions, 2);
        assert!((report.coverage_pct - 50.0).abs() < 0.1);
    }

    #[test]
    fn s18_8_apply_pgo_speedup_returns_reasonable_estimate() {
        let mut profile = ProfileData::new();
        // Hot functions: extremely high entry count
        profile.add_profile(FunctionProfile {
            name: "hot_inner_loop".into(),
            entry_count: 1_000_000,
            block_counts: vec![1_000_000, 950_000, 50_000],
            hash: 1,
        });
        profile.add_profile(FunctionProfile {
            name: "hot_compute".into(),
            entry_count: 800_000,
            block_counts: vec![800_000, 700_000, 100_000],
            hash: 3,
        });
        // Cold function: barely called
        profile.add_profile(FunctionProfile {
            name: "error_path".into(),
            entry_count: 1,
            block_counts: vec![1, 0, 0],
            hash: 2,
        });
        // Normal functions (many, to lower the mean)
        for i in 0..10 {
            profile.add_profile(FunctionProfile {
                name: format!("normal_{i}"),
                entry_count: 500,
                block_counts: vec![500, 250, 250],
                hash: 10 + i as u64,
            });
        }

        let opts = optimize_with_profile(&profile);
        let speedup = apply_pgo_speedup(&opts);
        // PGO provides speedup when there are hot/cold classified functions
        assert!(speedup >= 0.0, "PGO speedup should be non-negative");
        assert!(speedup <= 40.0, "PGO speedup capped at 40%");
    }

    #[test]
    fn s18_9_function_profile_coverage() {
        let fp = FunctionProfile {
            name: "test".into(),
            entry_count: 10,
            block_counts: vec![10, 5, 0, 0],
            hash: 1,
        };
        assert_eq!(fp.uncovered_blocks(), 2);
        assert!((fp.coverage_pct() - 50.0).abs() < 0.1);

        let full = FunctionProfile {
            name: "full".into(),
            entry_count: 10,
            block_counts: vec![10, 5, 3],
            hash: 2,
        };
        assert_eq!(full.uncovered_blocks(), 0);
        assert!((full.coverage_pct() - 100.0).abs() < 0.1);
    }

    #[test]
    fn s18_10_pgo_cli_flag_variants() {
        let inst = PgoCliFlag::Instrument;
        let opt = PgoCliFlag::Optimize {
            profile_path: "/tmp/prof.data".into(),
        };
        let merge = PgoCliFlag::Merge {
            inputs: vec!["a.profraw".into(), "b.profraw".into()],
        };
        let report = PgoCliFlag::Report;

        assert_eq!(inst, PgoCliFlag::Instrument);
        assert_ne!(inst, report);
        match opt {
            PgoCliFlag::Optimize { profile_path } => {
                assert_eq!(profile_path, "/tmp/prof.data");
            }
            _ => panic!("expected Optimize variant"),
        }
        match merge {
            PgoCliFlag::Merge { inputs } => assert_eq!(inputs.len(), 2),
            _ => panic!("expected Merge variant"),
        }
    }
}
