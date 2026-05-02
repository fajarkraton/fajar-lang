//! Proof Caching & Incrementality — Sprint V5: 10 tasks.
//!
//! Provides per-function proof result caching with hash-based invalidation,
//! incremental verification (only re-verify changed functions + dependents),
//! parallel verification (simulated), timeout management, proof persistence,
//! visualization report, counterexample display, and verification statistics.
//! All simulated (no real Z3 or filesystem persistence).

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V5.1: Proof Result
// ═══════════════════════════════════════════════════════════════════════

/// The result of a single proof obligation.
#[derive(Debug, Clone, PartialEq)]
pub enum ProofResult {
    /// Proven safe.
    Verified,
    /// Counterexample found.
    Falsified(CounterexampleInfo),
    /// Solver timed out.
    Timeout { elapsed_ms: u64 },
    /// Unknown (solver could not decide).
    Unknown(String),
    /// Error during verification.
    Error(String),
}

impl ProofResult {
    /// Returns true if the proof is verified.
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified)
    }

    /// Returns true if the proof was falsified.
    pub fn is_falsified(&self) -> bool {
        matches!(self, Self::Falsified(_))
    }
}

impl fmt::Display for ProofResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verified => write!(f, "VERIFIED"),
            Self::Falsified(ce) => write!(f, "FALSIFIED: {ce}"),
            Self::Timeout { elapsed_ms } => write!(f, "TIMEOUT ({elapsed_ms}ms)"),
            Self::Unknown(msg) => write!(f, "UNKNOWN: {msg}"),
            Self::Error(msg) => write!(f, "ERROR: {msg}"),
        }
    }
}

/// Counterexample information.
#[derive(Debug, Clone, PartialEq)]
pub struct CounterexampleInfo {
    /// Variable assignments.
    pub assignments: HashMap<String, String>,
    /// The property that was violated.
    pub violated_property: String,
}

impl fmt::Display for CounterexampleInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "property '{}' violated", self.violated_property)?;
        if !self.assignments.is_empty() {
            let mut sorted: Vec<_> = self.assignments.iter().collect();
            sorted.sort_by_key(|(k, _)| (*k).clone());
            write!(f, " with ")?;
            for (i, (k, v)) in sorted.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{k}={v}")?;
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V5.2: Per-Function Proof Cache Entry
// ═══════════════════════════════════════════════════════════════════════

/// A cached proof result for a single function.
#[derive(Debug, Clone)]
pub struct CachedProof {
    /// Function name.
    pub function_name: String,
    /// Hash of the function source code.
    pub source_hash: u64,
    /// Hash of the function's dependency signatures.
    pub deps_hash: u64,
    /// Proof results for each obligation.
    pub results: Vec<(String, ProofResult)>,
    /// Timestamp when this proof was cached (simulated ms).
    pub timestamp_ms: u64,
    /// Time spent verifying (ms).
    pub verify_time_ms: u64,
}

impl CachedProof {
    /// Returns true if all obligations in this proof are verified.
    pub fn all_verified(&self) -> bool {
        self.results.iter().all(|(_, r)| r.is_verified())
    }

    /// Returns the number of verified obligations.
    pub fn verified_count(&self) -> usize {
        self.results.iter().filter(|(_, r)| r.is_verified()).count()
    }

    /// Returns the number of falsified obligations.
    pub fn falsified_count(&self) -> usize {
        self.results
            .iter()
            .filter(|(_, r)| r.is_falsified())
            .count()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V5.3: Proof Cache with Hash-Based Invalidation
// ═══════════════════════════════════════════════════════════════════════

/// Cache for function-level proof results with hash-based invalidation.
#[derive(Debug, Clone, Default)]
pub struct ProofCache {
    /// Function name -> cached proof.
    entries: HashMap<String, CachedProof>,
    /// Cache statistics.
    pub stats: CacheStats,
}

/// Cache statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache lookups that returned a valid entry.
    pub hits: u64,
    /// Number of cache lookups that missed.
    pub misses: u64,
    /// Number of cache entries invalidated due to hash change.
    pub invalidations: u64,
    /// Number of entries inserted.
    pub insertions: u64,
}

impl ProofCache {
    /// Creates an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks up a cached proof for a function. Returns the cached proof if the
    /// source hash and dependency hash both match; otherwise returns None.
    pub fn get(
        &mut self,
        function_name: &str,
        source_hash: u64,
        deps_hash: u64,
    ) -> Option<&CachedProof> {
        if let Some(entry) = self.entries.get(function_name) {
            if entry.source_hash == source_hash && entry.deps_hash == deps_hash {
                self.stats.hits += 1;
                return Some(entry);
            }
            // Hash mismatch -- entry is stale
            self.stats.invalidations += 1;
        }
        self.stats.misses += 1;
        None
    }

    /// Inserts or updates a cached proof.
    pub fn insert(&mut self, proof: CachedProof) {
        self.stats.insertions += 1;
        self.entries.insert(proof.function_name.clone(), proof);
    }

    /// Removes a specific entry.
    pub fn remove(&mut self, function_name: &str) -> Option<CachedProof> {
        self.entries.remove(function_name)
    }

    /// Clears the entire cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns the number of cached entries.
    pub fn size(&self) -> usize {
        self.entries.len()
    }

    /// Returns the cache hit rate.
    pub fn hit_rate(&self) -> f64 {
        let total = self.stats.hits + self.stats.misses;
        if total == 0 {
            return 0.0;
        }
        self.stats.hits as f64 / total as f64
    }

    /// Returns all cached function names.
    pub fn cached_functions(&self) -> Vec<&str> {
        self.entries.keys().map(|k| k.as_str()).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V5.4: Incremental Verification
// ═══════════════════════════════════════════════════════════════════════

/// Dependency graph for incremental verification.
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    /// Function name -> list of functions it depends on.
    dependencies: HashMap<String, Vec<String>>,
    /// Reverse: function name -> list of functions that depend on it.
    dependents: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    /// Creates an empty dependency graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a dependency: `function` depends on `dependency`.
    pub fn add_dependency(&mut self, function: &str, dependency: &str) {
        self.dependencies
            .entry(function.to_string())
            .or_default()
            .push(dependency.to_string());
        self.dependents
            .entry(dependency.to_string())
            .or_default()
            .push(function.to_string());
    }

    /// Returns the direct dependencies of a function.
    pub fn get_dependencies(&self, function: &str) -> &[String] {
        self.dependencies
            .get(function)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns the functions that directly depend on the given function.
    pub fn get_dependents(&self, function: &str) -> &[String] {
        self.dependents
            .get(function)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Computes the transitive closure of dependents for a set of changed functions.
    /// Returns all functions that need re-verification.
    pub fn affected_functions(&self, changed: &[String]) -> Vec<String> {
        let mut affected = std::collections::HashSet::new();
        let mut queue: Vec<String> = changed.to_vec();

        while let Some(func) = queue.pop() {
            if affected.insert(func.clone()) {
                for dep in self.get_dependents(&func) {
                    if !affected.contains(dep) {
                        queue.push(dep.clone());
                    }
                }
            }
        }

        let mut result: Vec<String> = affected.into_iter().collect();
        result.sort();
        result
    }

    /// Returns the total number of functions in the graph.
    pub fn function_count(&self) -> usize {
        let mut all = std::collections::HashSet::new();
        for (k, deps) in &self.dependencies {
            all.insert(k.clone());
            for d in deps {
                all.insert(d.clone());
            }
        }
        all.len()
    }
}

/// Incremental verification planner: determines which functions need re-verification.
#[derive(Debug)]
pub struct IncrementalVerifier {
    /// Proof cache.
    pub cache: ProofCache,
    /// Dependency graph.
    pub deps: DependencyGraph,
    /// Current source hashes (function name -> hash).
    pub source_hashes: HashMap<String, u64>,
}

impl IncrementalVerifier {
    /// Creates a new incremental verifier.
    pub fn new() -> Self {
        Self {
            cache: ProofCache::new(),
            deps: DependencyGraph::new(),
            source_hashes: HashMap::new(),
        }
    }

    /// Updates the source hash for a function.
    pub fn update_hash(&mut self, function: &str, hash: u64) {
        self.source_hashes.insert(function.to_string(), hash);
    }

    /// Determines which functions need re-verification based on changed hashes.
    pub fn plan_verification(&mut self, new_hashes: &HashMap<String, u64>) -> VerificationPlan {
        let mut changed = Vec::new();
        let mut unchanged = Vec::new();

        for (func, new_hash) in new_hashes {
            let old_hash = self.source_hashes.get(func).copied();
            if old_hash != Some(*new_hash) {
                changed.push(func.clone());
            } else {
                unchanged.push(func.clone());
            }
        }

        // Find all affected (transitive dependents of changed)
        let affected = self.deps.affected_functions(&changed);

        // Filter: only re-verify affected functions that are not already cached
        let to_verify: Vec<String> = affected
            .iter()
            .filter(|f| {
                let hash = new_hashes.get(*f).copied().unwrap_or(0);
                self.cache.get(f, hash, 0).is_none()
            })
            .cloned()
            .collect();

        let skipped: Vec<String> = new_hashes
            .keys()
            .filter(|f| !affected.contains(f))
            .cloned()
            .collect();

        VerificationPlan {
            to_verify,
            skipped,
            changed,
            total_functions: new_hashes.len(),
        }
    }
}

impl Default for IncrementalVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// A verification plan: which functions to verify, which to skip.
#[derive(Debug, Clone)]
pub struct VerificationPlan {
    /// Functions that need verification.
    pub to_verify: Vec<String>,
    /// Functions that can be skipped (cached proof still valid).
    pub skipped: Vec<String>,
    /// Functions that changed (source hash different).
    pub changed: Vec<String>,
    /// Total number of functions.
    pub total_functions: usize,
}

impl fmt::Display for VerificationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Verification Plan:")?;
        writeln!(
            f,
            "  Total: {}, To verify: {}, Skipped: {}",
            self.total_functions,
            self.to_verify.len(),
            self.skipped.len()
        )?;
        if !self.to_verify.is_empty() {
            writeln!(f, "  Re-verify: {}", self.to_verify.join(", "))?;
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V5.5: Parallel Verification (Simulated)
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for parallel verification.
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Number of simulated threads.
    pub num_threads: u32,
    /// Timeout per function (ms).
    pub timeout_per_fn_ms: u64,
    /// Maximum total verification time (ms).
    pub total_timeout_ms: u64,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            num_threads: 4,
            timeout_per_fn_ms: 5000,
            total_timeout_ms: 60000,
        }
    }
}

/// Simulates parallel verification by distributing functions across threads.
pub fn simulate_parallel_verify(
    functions: &[String],
    config: &ParallelConfig,
) -> ParallelVerifyResult {
    let num_threads = config.num_threads.max(1) as usize;
    let chunk_size = functions.len().div_ceil(num_threads);

    let mut thread_assignments: Vec<Vec<String>> = Vec::new();
    for chunk in functions.chunks(chunk_size) {
        thread_assignments.push(chunk.to_vec());
    }

    // Simulate timing: each function takes ~timeout_per_fn_ms / 10 (simulated fast)
    let simulated_per_fn = config.timeout_per_fn_ms / 10;
    let max_chunk_size = thread_assignments
        .iter()
        .map(|c| c.len())
        .max()
        .unwrap_or(0);
    let estimated_time = max_chunk_size as u64 * simulated_per_fn;

    let timed_out = estimated_time > config.total_timeout_ms;

    ParallelVerifyResult {
        thread_assignments,
        estimated_time_ms: estimated_time,
        timed_out,
        functions_verified: if timed_out {
            (config.total_timeout_ms / simulated_per_fn) as usize
        } else {
            functions.len()
        },
        total_functions: functions.len(),
    }
}

/// Result of simulated parallel verification.
#[derive(Debug, Clone)]
pub struct ParallelVerifyResult {
    /// Per-thread function assignments.
    pub thread_assignments: Vec<Vec<String>>,
    /// Estimated total time (ms).
    pub estimated_time_ms: u64,
    /// Whether the total timeout was exceeded.
    pub timed_out: bool,
    /// Number of functions actually verified.
    pub functions_verified: usize,
    /// Total number of functions.
    pub total_functions: usize,
}

impl fmt::Display for ParallelVerifyResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Parallel verification: {} threads, {}/{} functions, ~{}ms",
            self.thread_assignments.len(),
            self.functions_verified,
            self.total_functions,
            self.estimated_time_ms
        )?;
        if self.timed_out {
            writeln!(f, "  WARNING: total timeout exceeded")?;
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V5.6: Timeout Management
// ═══════════════════════════════════════════════════════════════════════

/// Timeout manager for proof obligations.
#[derive(Debug, Clone)]
pub struct TimeoutManager {
    /// Default timeout per obligation (ms).
    pub default_timeout_ms: u64,
    /// Per-function timeout overrides.
    pub overrides: HashMap<String, u64>,
    /// Total elapsed time (simulated).
    pub total_elapsed_ms: u64,
    /// Total budget (ms).
    pub total_budget_ms: u64,
}

impl TimeoutManager {
    /// Creates a new timeout manager.
    pub fn new(default_ms: u64, total_budget_ms: u64) -> Self {
        Self {
            default_timeout_ms: default_ms,
            overrides: HashMap::new(),
            total_elapsed_ms: 0,
            total_budget_ms,
        }
    }

    /// Gets the timeout for a specific function.
    pub fn get_timeout(&self, function: &str) -> u64 {
        self.overrides
            .get(function)
            .copied()
            .unwrap_or(self.default_timeout_ms)
    }

    /// Sets a timeout override for a specific function.
    pub fn set_timeout(&mut self, function: &str, timeout_ms: u64) {
        self.overrides.insert(function.to_string(), timeout_ms);
    }

    /// Records time spent on a verification.
    pub fn record_elapsed(&mut self, elapsed_ms: u64) {
        self.total_elapsed_ms = self.total_elapsed_ms.saturating_add(elapsed_ms);
    }

    /// Returns true if the total budget is exhausted.
    pub fn budget_exhausted(&self) -> bool {
        self.total_elapsed_ms >= self.total_budget_ms
    }

    /// Returns remaining budget (ms).
    pub fn remaining_ms(&self) -> u64 {
        self.total_budget_ms.saturating_sub(self.total_elapsed_ms)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V5.7: Proof Persistence (Simulated)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated proof persistence: serializes proof cache to a string representation.
#[derive(Debug, Clone, Default)]
pub struct ProofPersistence {
    /// Serialized proof data (simulated as string map).
    stored: HashMap<String, String>,
}

impl ProofPersistence {
    /// Creates a new persistence store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Saves a proof cache entry (simulated serialization).
    pub fn save(&mut self, function_name: &str, proof: &CachedProof) {
        let serialized = format!(
            "fn={},hash={},deps={},results={},time={}ms",
            proof.function_name,
            proof.source_hash,
            proof.deps_hash,
            proof.results.len(),
            proof.verify_time_ms
        );
        self.stored.insert(function_name.to_string(), serialized);
    }

    /// Loads a saved proof entry (simulated deserialization).
    pub fn load(&self, function_name: &str) -> Option<&str> {
        self.stored.get(function_name).map(|s| s.as_str())
    }

    /// Returns the number of persisted entries.
    pub fn size(&self) -> usize {
        self.stored.len()
    }

    /// Clears all persisted data.
    pub fn clear(&mut self) {
        self.stored.clear();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V5.8-V5.9: Visualization Report & Counterexample Display
// ═══════════════════════════════════════════════════════════════════════

/// Complete verification report with visualization support.
#[derive(Debug, Clone)]
pub struct VerificationReport {
    /// Per-function results.
    pub function_results: Vec<FunctionVerifyResult>,
    /// Total verification time (ms).
    pub total_time_ms: u64,
    /// Cache hit rate.
    pub cache_hit_rate: f64,
    /// Number of threads used.
    pub threads_used: u32,
}

/// Per-function verification result.
#[derive(Debug, Clone)]
pub struct FunctionVerifyResult {
    /// Function name.
    pub name: String,
    /// Proof results for each obligation.
    pub obligations: Vec<(String, ProofResult)>,
    /// Time spent (ms).
    pub time_ms: u64,
    /// Whether the result came from cache.
    pub from_cache: bool,
}

impl FunctionVerifyResult {
    /// Returns true if all obligations are verified.
    pub fn all_verified(&self) -> bool {
        self.obligations.iter().all(|(_, r)| r.is_verified())
    }

    /// Returns the counterexamples found.
    pub fn counterexamples(&self) -> Vec<&CounterexampleInfo> {
        self.obligations
            .iter()
            .filter_map(|(_, r)| {
                if let ProofResult::Falsified(ce) = r {
                    Some(ce)
                } else {
                    None
                }
            })
            .collect()
    }
}

impl VerificationReport {
    /// Returns total number of obligations.
    pub fn total_obligations(&self) -> usize {
        self.function_results
            .iter()
            .map(|f| f.obligations.len())
            .sum()
    }

    /// Returns number of verified obligations.
    pub fn verified_count(&self) -> usize {
        self.function_results
            .iter()
            .flat_map(|f| f.obligations.iter())
            .filter(|(_, r)| r.is_verified())
            .count()
    }

    /// Returns number of falsified obligations.
    pub fn falsified_count(&self) -> usize {
        self.function_results
            .iter()
            .flat_map(|f| f.obligations.iter())
            .filter(|(_, r)| r.is_falsified())
            .count()
    }

    /// Returns number of functions that are fully verified.
    pub fn functions_clean(&self) -> usize {
        self.function_results
            .iter()
            .filter(|f| f.all_verified())
            .count()
    }

    /// Returns verification coverage (fraction of obligations verified).
    pub fn coverage(&self) -> f64 {
        let total = self.total_obligations();
        if total == 0 {
            return 1.0;
        }
        self.verified_count() as f64 / total as f64
    }

    /// All counterexamples across all functions.
    pub fn all_counterexamples(&self) -> Vec<(&str, &CounterexampleInfo)> {
        self.function_results
            .iter()
            .flat_map(|f| {
                f.obligations.iter().filter_map(move |(_, r)| {
                    if let ProofResult::Falsified(ce) = r {
                        Some((f.name.as_str(), ce))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }
}

impl fmt::Display for VerificationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Verification Report ===")?;
        writeln!(
            f,
            "Functions: {} ({} clean)",
            self.function_results.len(),
            self.functions_clean()
        )?;
        writeln!(
            f,
            "Obligations: {} ({} verified, {} falsified)",
            self.total_obligations(),
            self.verified_count(),
            self.falsified_count()
        )?;
        writeln!(f, "Coverage: {:.1}%", self.coverage() * 100.0)?;
        writeln!(f, "Time: {}ms", self.total_time_ms)?;
        writeln!(f, "Cache hit rate: {:.1}%", self.cache_hit_rate * 100.0)?;

        let ces = self.all_counterexamples();
        if !ces.is_empty() {
            writeln!(f, "\nCounterexamples:")?;
            for (func, ce) in &ces {
                writeln!(f, "  {func}: {ce}")?;
            }
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V5.10: Verification Statistics
// ═══════════════════════════════════════════════════════════════════════

/// Aggregate verification statistics across sessions.
#[derive(Debug, Clone, Default)]
pub struct VerificationStatistics {
    /// Total proofs attempted.
    pub total_proofs: u64,
    /// Total proofs verified.
    pub total_verified: u64,
    /// Total proofs falsified.
    pub total_falsified: u64,
    /// Total proofs timed out.
    pub total_timeouts: u64,
    /// Total proofs with unknown result.
    pub total_unknown: u64,
    /// Total verification time (ms).
    pub total_time_ms: u64,
    /// Average proof time (ms).
    pub avg_proof_time_ms: f64,
    /// Maximum proof time (ms).
    pub max_proof_time_ms: u64,
    /// Cache hits.
    pub cache_hits: u64,
    /// Cache misses.
    pub cache_misses: u64,
    /// Number of incremental re-verifications saved.
    pub incremental_savings: u64,
}

impl VerificationStatistics {
    /// Creates empty statistics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a proof result.
    pub fn record(&mut self, result: &ProofResult, time_ms: u64) {
        self.total_proofs += 1;
        self.total_time_ms += time_ms;
        if time_ms > self.max_proof_time_ms {
            self.max_proof_time_ms = time_ms;
        }
        self.avg_proof_time_ms = self.total_time_ms as f64 / self.total_proofs as f64;

        match result {
            ProofResult::Verified => self.total_verified += 1,
            ProofResult::Falsified(_) => self.total_falsified += 1,
            ProofResult::Timeout { .. } => self.total_timeouts += 1,
            ProofResult::Unknown(_) => self.total_unknown += 1,
            ProofResult::Error(_) => {} // Errors not counted in any bucket
        }
    }

    /// Records a cache hit.
    pub fn record_cache_hit(&mut self) {
        self.cache_hits += 1;
    }

    /// Records a cache miss.
    pub fn record_cache_miss(&mut self) {
        self.cache_misses += 1;
    }

    /// Records an incremental saving (function not re-verified).
    pub fn record_incremental_saving(&mut self) {
        self.incremental_savings += 1;
    }

    /// Returns the overall proof success rate.
    pub fn success_rate(&self) -> f64 {
        if self.total_proofs == 0 {
            return 0.0;
        }
        self.total_verified as f64 / self.total_proofs as f64
    }

    /// Returns the cache hit rate.
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }
}

impl fmt::Display for VerificationStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Verification Statistics:")?;
        writeln!(
            f,
            "  Proofs: {} total, {} verified, {} falsified, {} timeout, {} unknown",
            self.total_proofs,
            self.total_verified,
            self.total_falsified,
            self.total_timeouts,
            self.total_unknown
        )?;
        writeln!(f, "  Success rate: {:.1}%", self.success_rate() * 100.0)?;
        writeln!(
            f,
            "  Time: {}ms total, {:.1}ms avg, {}ms max",
            self.total_time_ms, self.avg_proof_time_ms, self.max_proof_time_ms
        )?;
        writeln!(
            f,
            "  Cache: {:.1}% hit rate ({} hits, {} misses)",
            self.cache_hit_rate() * 100.0,
            self.cache_hits,
            self.cache_misses
        )?;
        writeln!(
            f,
            "  Incremental savings: {} re-verifications avoided",
            self.incremental_savings
        )?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // --- V5.1: ProofResult ---

    #[test]
    fn v5_1_proof_result_verified() {
        let r = ProofResult::Verified;
        assert!(r.is_verified());
        assert!(!r.is_falsified());
        assert_eq!(format!("{r}"), "VERIFIED");
    }

    #[test]
    fn v5_1_proof_result_falsified() {
        let ce = CounterexampleInfo {
            assignments: HashMap::from([("x".to_string(), "-1".to_string())]),
            violated_property: "x >= 0".to_string(),
        };
        let r = ProofResult::Falsified(ce);
        assert!(r.is_falsified());
        assert!(!r.is_verified());
        let s = format!("{r}");
        assert!(s.contains("FALSIFIED"));
        assert!(s.contains("x >= 0"));
    }

    #[test]
    fn v5_1_proof_result_timeout() {
        let r = ProofResult::Timeout { elapsed_ms: 5000 };
        assert_eq!(format!("{r}"), "TIMEOUT (5000ms)");
    }

    // --- V5.2: CachedProof ---

    #[test]
    fn v5_2_cached_proof_all_verified() {
        let proof = CachedProof {
            function_name: "add".to_string(),
            source_hash: 123,
            deps_hash: 456,
            results: vec![
                ("precondition".to_string(), ProofResult::Verified),
                ("postcondition".to_string(), ProofResult::Verified),
            ],
            timestamp_ms: 1000,
            verify_time_ms: 50,
        };
        assert!(proof.all_verified());
        assert_eq!(proof.verified_count(), 2);
        assert_eq!(proof.falsified_count(), 0);
    }

    #[test]
    fn v5_2_cached_proof_with_failure() {
        let proof = CachedProof {
            function_name: "div".to_string(),
            source_hash: 789,
            deps_hash: 0,
            results: vec![
                ("precondition".to_string(), ProofResult::Verified),
                (
                    "no_div_zero".to_string(),
                    ProofResult::Falsified(CounterexampleInfo {
                        assignments: HashMap::from([("d".to_string(), "0".to_string())]),
                        violated_property: "d != 0".to_string(),
                    }),
                ),
            ],
            timestamp_ms: 2000,
            verify_time_ms: 100,
        };
        assert!(!proof.all_verified());
        assert_eq!(proof.verified_count(), 1);
        assert_eq!(proof.falsified_count(), 1);
    }

    // --- V5.3: ProofCache ---

    #[test]
    fn v5_3_cache_hit() {
        let mut cache = ProofCache::new();
        cache.insert(CachedProof {
            function_name: "f".to_string(),
            source_hash: 100,
            deps_hash: 200,
            results: vec![("p".to_string(), ProofResult::Verified)],
            timestamp_ms: 0,
            verify_time_ms: 10,
        });
        assert_eq!(cache.size(), 1);
        let entry = cache.get("f", 100, 200);
        assert!(entry.is_some());
        assert_eq!(cache.stats.hits, 1);
    }

    #[test]
    fn v5_3_cache_miss() {
        let mut cache = ProofCache::new();
        let entry = cache.get("nonexistent", 0, 0);
        assert!(entry.is_none());
        assert_eq!(cache.stats.misses, 1);
    }

    #[test]
    fn v5_3_cache_invalidation() {
        let mut cache = ProofCache::new();
        cache.insert(CachedProof {
            function_name: "g".to_string(),
            source_hash: 100,
            deps_hash: 200,
            results: vec![],
            timestamp_ms: 0,
            verify_time_ms: 0,
        });
        // Different hash -> invalidation
        let entry = cache.get("g", 999, 200);
        assert!(entry.is_none());
        assert_eq!(cache.stats.invalidations, 1);
    }

    #[test]
    fn v5_3_cache_hit_rate() {
        let mut cache = ProofCache::new();
        cache.insert(CachedProof {
            function_name: "h".to_string(),
            source_hash: 1,
            deps_hash: 2,
            results: vec![],
            timestamp_ms: 0,
            verify_time_ms: 0,
        });
        cache.get("h", 1, 2); // hit
        cache.get("x", 0, 0); // miss
        assert!((cache.hit_rate() - 0.5).abs() < 0.001);
    }

    // --- V5.4: DependencyGraph ---

    #[test]
    fn v5_4_dependency_graph() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("train", "forward");
        graph.add_dependency("train", "backward");
        graph.add_dependency("forward", "matmul");

        assert_eq!(graph.get_dependencies("train"), &["forward", "backward"]);
        assert_eq!(graph.get_dependents("forward"), &["train"]);
    }

    #[test]
    fn v5_4_affected_functions() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("train", "forward");
        graph.add_dependency("forward", "matmul");
        graph.add_dependency("forward", "relu");

        // If matmul changes, forward and train are affected
        let affected = graph.affected_functions(&["matmul".to_string()]);
        assert!(affected.contains(&"matmul".to_string()));
        assert!(affected.contains(&"forward".to_string()));
        assert!(affected.contains(&"train".to_string()));
    }

    #[test]
    fn v5_4_incremental_plan() {
        let mut verifier = IncrementalVerifier::new();
        verifier.update_hash("f1", 100);
        verifier.update_hash("f2", 200);
        verifier.deps.add_dependency("f2", "f1");

        let new_hashes = HashMap::from([
            ("f1".to_string(), 101u64), // changed
            ("f2".to_string(), 200u64), // unchanged but depends on f1
        ]);
        let plan = verifier.plan_verification(&new_hashes);
        assert!(!plan.changed.is_empty());
        assert!(plan.to_verify.contains(&"f1".to_string()));
    }

    // --- V5.5: Parallel Verification ---

    #[test]
    fn v5_5_parallel_verify() {
        let functions: Vec<String> = (0..8).map(|i| format!("fn_{i}")).collect();
        let config = ParallelConfig {
            num_threads: 4,
            timeout_per_fn_ms: 1000,
            total_timeout_ms: 60000,
        };
        let result = simulate_parallel_verify(&functions, &config);
        assert_eq!(result.total_functions, 8);
        assert_eq!(result.thread_assignments.len(), 4);
        assert!(!result.timed_out);
    }

    #[test]
    fn v5_5_parallel_timeout() {
        let functions: Vec<String> = (0..100).map(|i| format!("fn_{i}")).collect();
        let config = ParallelConfig {
            num_threads: 1,
            timeout_per_fn_ms: 10000,
            total_timeout_ms: 100, // Very tight budget
        };
        let result = simulate_parallel_verify(&functions, &config);
        assert!(result.timed_out);
    }

    // --- V5.6: Timeout Management ---

    #[test]
    fn v5_6_timeout_manager() {
        let mut tm = TimeoutManager::new(5000, 60000);
        assert_eq!(tm.get_timeout("any_fn"), 5000);
        tm.set_timeout("complex_fn", 10000);
        assert_eq!(tm.get_timeout("complex_fn"), 10000);
        assert!(!tm.budget_exhausted());
        assert_eq!(tm.remaining_ms(), 60000);
        tm.record_elapsed(50000);
        assert_eq!(tm.remaining_ms(), 10000);
        tm.record_elapsed(15000);
        assert!(tm.budget_exhausted());
    }

    // --- V5.7: Proof Persistence ---

    #[test]
    fn v5_7_persistence() {
        let mut store = ProofPersistence::new();
        let proof = CachedProof {
            function_name: "f".to_string(),
            source_hash: 42,
            deps_hash: 0,
            results: vec![("p".to_string(), ProofResult::Verified)],
            timestamp_ms: 0,
            verify_time_ms: 100,
        };
        store.save("f", &proof);
        assert_eq!(store.size(), 1);
        let loaded = store.load("f");
        assert!(loaded.is_some());
        assert!(loaded.is_some_and(|s| s.contains("hash=42")));
        assert!(store.load("nonexistent").is_none());
    }

    // --- V5.8: Verification Report ---

    #[test]
    fn v5_8_report() {
        let report = VerificationReport {
            function_results: vec![
                FunctionVerifyResult {
                    name: "add".to_string(),
                    obligations: vec![("pre".to_string(), ProofResult::Verified)],
                    time_ms: 10,
                    from_cache: false,
                },
                FunctionVerifyResult {
                    name: "div".to_string(),
                    obligations: vec![
                        ("pre".to_string(), ProofResult::Verified),
                        (
                            "no_zero".to_string(),
                            ProofResult::Falsified(CounterexampleInfo {
                                assignments: HashMap::from([("d".to_string(), "0".to_string())]),
                                violated_property: "d != 0".to_string(),
                            }),
                        ),
                    ],
                    time_ms: 50,
                    from_cache: false,
                },
            ],
            total_time_ms: 60,
            cache_hit_rate: 0.0,
            threads_used: 1,
        };
        assert_eq!(report.total_obligations(), 3);
        assert_eq!(report.verified_count(), 2);
        assert_eq!(report.falsified_count(), 1);
        assert_eq!(report.functions_clean(), 1);
        let s = format!("{report}");
        assert!(s.contains("Verification Report"));
        assert!(s.contains("Counterexamples"));
    }

    #[test]
    fn v5_8_report_all_clean() {
        let report = VerificationReport {
            function_results: vec![FunctionVerifyResult {
                name: "safe".to_string(),
                obligations: vec![("p".to_string(), ProofResult::Verified)],
                time_ms: 5,
                from_cache: true,
            }],
            total_time_ms: 5,
            cache_hit_rate: 1.0,
            threads_used: 1,
        };
        assert!((report.coverage() - 1.0).abs() < 0.001);
        assert_eq!(report.functions_clean(), 1);
    }

    // --- V5.9: Counterexample Display ---

    #[test]
    fn v5_9_counterexample_display() {
        let ce = CounterexampleInfo {
            assignments: HashMap::from([
                ("x".to_string(), "-1".to_string()),
                ("y".to_string(), "0".to_string()),
            ]),
            violated_property: "x >= 0 && y > 0".to_string(),
        };
        let s = format!("{ce}");
        assert!(s.contains("x >= 0 && y > 0"));
        assert!(s.contains("x=-1"));
        assert!(s.contains("y=0"));
    }

    // --- V5.10: Statistics ---

    #[test]
    fn v5_10_statistics() {
        let mut stats = VerificationStatistics::new();
        stats.record(&ProofResult::Verified, 10);
        stats.record(&ProofResult::Verified, 20);
        stats.record(
            &ProofResult::Falsified(CounterexampleInfo {
                assignments: HashMap::new(),
                violated_property: "p".to_string(),
            }),
            30,
        );
        stats.record(&ProofResult::Timeout { elapsed_ms: 5000 }, 5000);

        assert_eq!(stats.total_proofs, 4);
        assert_eq!(stats.total_verified, 2);
        assert_eq!(stats.total_falsified, 1);
        assert_eq!(stats.total_timeouts, 1);
        assert!((stats.success_rate() - 0.5).abs() < 0.001);
        assert_eq!(stats.max_proof_time_ms, 5000);

        stats.record_cache_hit();
        stats.record_cache_hit();
        stats.record_cache_miss();
        assert!((stats.cache_hit_rate() - 2.0 / 3.0).abs() < 0.01);

        stats.record_incremental_saving();
        assert_eq!(stats.incremental_savings, 1);

        let s = format!("{stats}");
        assert!(s.contains("4 total"));
        assert!(s.contains("2 verified"));
    }

    #[test]
    fn v5_10_statistics_empty() {
        let stats = VerificationStatistics::new();
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.cache_hit_rate(), 0.0);
    }

    #[test]
    fn v5_10_plan_display() {
        let plan = VerificationPlan {
            to_verify: vec!["f1".to_string(), "f2".to_string()],
            skipped: vec!["f3".to_string()],
            changed: vec!["f1".to_string()],
            total_functions: 3,
        };
        let s = format!("{plan}");
        assert!(s.contains("Total: 3"));
        assert!(s.contains("To verify: 2"));
    }
}
