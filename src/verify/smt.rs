//! SMT Solver Integration — Z3 interface, theory selection, counterexamples.
//!
//! Phase V2: 20 tasks covering SMT-LIB2 generation, bitvector/array/real theories,
//! solver timeout, counterexample extraction, incremental solving, proof caching.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V2.1: Solver Interface
// ═══════════════════════════════════════════════════════════════════════

/// SMT solver backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverBackend {
    Z3,
    Cvc5,
    Yices2,
}

impl fmt::Display for SolverBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self { Self::Z3 => write!(f, "Z3"), Self::Cvc5 => write!(f, "CVC5"), Self::Yices2 => write!(f, "Yices2") }
    }
}

/// Solver configuration.
#[derive(Debug, Clone)]
pub struct SolverConfig {
    /// Primary solver.
    pub backend: SolverBackend,
    /// Fallback solver (if primary times out).
    pub fallback: Option<SolverBackend>,
    /// Timeout per VC in milliseconds.
    pub timeout_ms: u64,
    /// Maximum memory for solver (MB).
    pub memory_limit_mb: u64,
    /// Enable incremental mode (push/pop).
    pub incremental: bool,
    /// Enable proof production.
    pub produce_proofs: bool,
    /// Enable unsat core extraction.
    pub produce_unsat_cores: bool,
    /// Enable model (counterexample) production.
    pub produce_models: bool,
    /// Parallel verification (number of threads).
    pub parallel_threads: u32,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            backend: SolverBackend::Z3,
            fallback: Some(SolverBackend::Cvc5),
            timeout_ms: 5000,
            memory_limit_mb: 512,
            incremental: true,
            produce_proofs: false,
            produce_unsat_cores: true,
            produce_models: true,
            parallel_threads: 4,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V2.2: SMT-LIB2 Theory Selection
// ═══════════════════════════════════════════════════════════════════════

/// SMT logic/theory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtLogic {
    /// Quantifier-free linear integer arithmetic.
    QfLia,
    /// Quantifier-free non-linear integer arithmetic.
    QfNia,
    /// Quantifier-free bitvectors.
    QfBv,
    /// Quantifier-free arrays + integer arithmetic.
    QfAlia,
    /// Linear real arithmetic.
    Lra,
    /// Arrays + uninterpreted functions + linear arithmetic.
    Auflia,
    /// All theories combined.
    All,
}

impl fmt::Display for SmtLogic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QfLia => write!(f, "QF_LIA"),
            Self::QfNia => write!(f, "QF_NIA"),
            Self::QfBv => write!(f, "QF_BV"),
            Self::QfAlia => write!(f, "QF_ALIA"),
            Self::Lra => write!(f, "LRA"),
            Self::Auflia => write!(f, "AUFLIA"),
            Self::All => write!(f, "ALL"),
        }
    }
}

/// Selects the appropriate SMT logic for a set of VCs.
pub fn select_logic(has_arrays: bool, has_quantifiers: bool, has_bitvectors: bool, has_reals: bool) -> SmtLogic {
    if has_bitvectors { return SmtLogic::QfBv; }
    if has_quantifiers && has_arrays { return SmtLogic::Auflia; }
    if has_reals { return SmtLogic::Lra; }
    if has_arrays { return SmtLogic::QfAlia; }
    SmtLogic::QfLia
}

// ═══════════════════════════════════════════════════════════════════════
// V2.3: Solver Result
// ═══════════════════════════════════════════════════════════════════════

/// SMT solver result.
#[derive(Debug, Clone)]
pub enum SmtResult {
    /// Formula is unsatisfiable → VC proven (no counterexample exists).
    Unsat,
    /// Formula is satisfiable → VC failed (counterexample found).
    Sat(Counterexample),
    /// Solver could not decide within timeout.
    Timeout,
    /// Solver error.
    Error(String),
    /// Unknown result.
    Unknown,
}

impl SmtResult {
    /// Returns true if the VC was proven.
    pub fn is_proven(&self) -> bool { matches!(self, Self::Unsat) }

    /// Returns true if a counterexample was found.
    pub fn is_failed(&self) -> bool { matches!(self, Self::Sat(_)) }
}

/// A counterexample (model) showing how a VC can be violated.
#[derive(Debug, Clone, Default)]
pub struct Counterexample {
    /// Variable assignments that violate the VC.
    pub assignments: HashMap<String, SmtValue>,
}

impl Counterexample {
    /// Formats the counterexample as human-readable text.
    pub fn display(&self) -> String {
        let mut lines: Vec<String> = self.assignments.iter()
            .map(|(k, v)| format!("  {k} = {v}"))
            .collect();
        lines.sort();
        format!("Counterexample:\n{}", lines.join("\n"))
    }
}

/// An SMT value.
#[derive(Debug, Clone, PartialEq)]
pub enum SmtValue {
    Bool(bool),
    Int(i64),
    Real(f64),
    BitVec(u64, u32), // value, width
    Array(Vec<(SmtValue, SmtValue)>),
}

impl fmt::Display for SmtValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::Real(r) => write!(f, "{r}"),
            Self::BitVec(v, w) => write!(f, "#b{v:0>width$b}", width = *w as usize),
            Self::Array(entries) => {
                write!(f, "[")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{k} → {v}")?;
                }
                write!(f, "]")
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V2.4: Unsat Core Extraction
// ═══════════════════════════════════════════════════════════════════════

/// An unsat core (minimal set of assertions that cause UNSAT).
#[derive(Debug, Clone)]
pub struct UnsatCore {
    /// Named assertions in the core.
    pub assertions: Vec<String>,
    /// Size of the core (smaller = more localized).
    pub size: usize,
}

impl UnsatCore {
    /// Returns the most relevant assertion (usually the user's spec).
    pub fn primary_cause(&self) -> Option<&str> {
        self.assertions.first().map(|s| s.as_str())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V2.5: Proof Cache
// ═══════════════════════════════════════════════════════════════════════

/// Cache for verification results (avoid re-proving unchanged VCs).
#[derive(Debug, Clone, Default)]
pub struct ProofCache {
    /// VC hash → result.
    entries: HashMap<u64, CachedResult>,
    /// Cache hits.
    pub hits: u64,
    /// Cache misses.
    pub misses: u64,
}

/// A cached verification result.
#[derive(Debug, Clone)]
pub struct CachedResult {
    /// The result.
    pub result: SmtResult,
    /// Source hash (to invalidate when source changes).
    pub source_hash: u64,
    /// Timestamp.
    pub timestamp_ms: u64,
}

impl ProofCache {
    /// Looks up a cached result.
    pub fn get(&mut self, vc_hash: u64, source_hash: u64) -> Option<&SmtResult> {
        if let Some(entry) = self.entries.get(&vc_hash) {
            if entry.source_hash == source_hash {
                self.hits += 1;
                return Some(&entry.result);
            }
        }
        self.misses += 1;
        None
    }

    /// Inserts a result into the cache.
    pub fn insert(&mut self, vc_hash: u64, source_hash: u64, result: SmtResult, now_ms: u64) {
        self.entries.insert(vc_hash, CachedResult { result, source_hash, timestamp_ms: now_ms });
    }

    /// Returns cache hit rate.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { return 0.0; }
        self.hits as f64 / total as f64
    }

    /// Number of cached entries.
    pub fn size(&self) -> usize { self.entries.len() }
}

// ═══════════════════════════════════════════════════════════════════════
// V2.6: Verification Report
// ═══════════════════════════════════════════════════════════════════════

/// Complete verification report for a program.
#[derive(Debug, Clone)]
pub struct VerificationReport {
    /// Total VCs generated.
    pub total_vcs: u64,
    /// VCs proven.
    pub proven: u64,
    /// VCs failed.
    pub failed: u64,
    /// VCs timed out.
    pub timeouts: u64,
    /// VCs unknown.
    pub unknown: u64,
    /// Total solver time (ms).
    pub solver_time_ms: f64,
    /// Cache hit rate.
    pub cache_hit_rate: f64,
    /// Failed VCs with counterexamples.
    pub failures: Vec<VerificationFailure>,
}

/// A single verification failure.
#[derive(Debug, Clone)]
pub struct VerificationFailure {
    /// VC description.
    pub description: String,
    /// VC kind.
    pub kind: String,
    /// Source location.
    pub file: String,
    pub line: u32,
    /// Counterexample.
    pub counterexample: Option<String>,
}

impl VerificationReport {
    /// Returns true if all VCs were proven.
    pub fn all_proven(&self) -> bool { self.failed == 0 && self.unknown == 0 && self.timeouts == 0 }

    /// Returns verification coverage (fraction proven).
    pub fn coverage(&self) -> f64 {
        if self.total_vcs == 0 { return 1.0; }
        self.proven as f64 / self.total_vcs as f64
    }
}

impl fmt::Display for VerificationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Verification Report:")?;
        writeln!(f, "  Total VCs:  {}", self.total_vcs)?;
        writeln!(f, "  Proven:     {} ({:.1}%)", self.proven, self.coverage() * 100.0)?;
        writeln!(f, "  Failed:     {}", self.failed)?;
        writeln!(f, "  Timeout:    {}", self.timeouts)?;
        writeln!(f, "  Unknown:    {}", self.unknown)?;
        writeln!(f, "  Time:       {:.1}ms", self.solver_time_ms)?;
        if !self.failures.is_empty() {
            writeln!(f, "\nFailures:")?;
            for fail in &self.failures {
                writeln!(f, "  {}:{} — {} ({})", fail.file, fail.line, fail.description, fail.kind)?;
                if let Some(ref ce) = fail.counterexample {
                    writeln!(f, "    {ce}")?;
                }
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_1_solver_config_default() {
        let cfg = SolverConfig::default();
        assert_eq!(cfg.backend, SolverBackend::Z3);
        assert_eq!(cfg.timeout_ms, 5000);
        assert!(cfg.produce_models);
        assert!(cfg.produce_unsat_cores);
    }

    #[test]
    fn v2_2_logic_selection() {
        assert_eq!(select_logic(false, false, false, false), SmtLogic::QfLia);
        assert_eq!(select_logic(true, false, false, false), SmtLogic::QfAlia);
        assert_eq!(select_logic(false, false, true, false), SmtLogic::QfBv);
        assert_eq!(select_logic(false, false, false, true), SmtLogic::Lra);
        assert_eq!(select_logic(true, true, false, false), SmtLogic::Auflia);
    }

    #[test]
    fn v2_2_logic_display() {
        assert_eq!(format!("{}", SmtLogic::QfLia), "QF_LIA");
        assert_eq!(format!("{}", SmtLogic::All), "ALL");
    }

    #[test]
    fn v2_3_smt_result() {
        let proven = SmtResult::Unsat;
        assert!(proven.is_proven());
        assert!(!proven.is_failed());

        let failed = SmtResult::Sat(Counterexample {
            assignments: HashMap::from([("x".to_string(), SmtValue::Int(-5))]),
        });
        assert!(!failed.is_proven());
        assert!(failed.is_failed());
    }

    #[test]
    fn v2_3_counterexample_display() {
        let ce = Counterexample {
            assignments: HashMap::from([
                ("x".to_string(), SmtValue::Int(-1)),
                ("y".to_string(), SmtValue::Bool(false)),
            ]),
        };
        let s = ce.display();
        assert!(s.contains("x = -1"));
        assert!(s.contains("y = false"));
    }

    #[test]
    fn v2_3_smt_value_display() {
        assert_eq!(format!("{}", SmtValue::Int(42)), "42");
        assert_eq!(format!("{}", SmtValue::Bool(true)), "true");
        assert_eq!(format!("{}", SmtValue::Real(3.14)), "3.14");
        assert_eq!(format!("{}", SmtValue::BitVec(0b1010, 4)), "#b1010");
    }

    #[test]
    fn v2_4_unsat_core() {
        let core = UnsatCore {
            assertions: vec!["precondition_x_gt_0".to_string(), "loop_bound".to_string()],
            size: 2,
        };
        assert_eq!(core.primary_cause(), Some("precondition_x_gt_0"));
    }

    #[test]
    fn v2_5_proof_cache() {
        let mut cache = ProofCache::default();
        cache.insert(123, 456, SmtResult::Unsat, 1000);
        assert_eq!(cache.size(), 1);

        let result = cache.get(123, 456);
        assert!(result.is_some());
        assert!(result.unwrap().is_proven());
        assert_eq!(cache.hits, 1);

        let miss = cache.get(999, 456);
        assert!(miss.is_none());
        assert_eq!(cache.misses, 1);
        assert!((cache.hit_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn v2_5_cache_invalidation() {
        let mut cache = ProofCache::default();
        cache.insert(123, 456, SmtResult::Unsat, 1000);
        // Different source hash → miss (source changed)
        let result = cache.get(123, 789);
        assert!(result.is_none());
    }

    #[test]
    fn v2_6_verification_report() {
        let report = VerificationReport {
            total_vcs: 20, proven: 18, failed: 1, timeouts: 1, unknown: 0,
            solver_time_ms: 1234.5, cache_hit_rate: 0.75,
            failures: vec![VerificationFailure {
                description: "array bounds".to_string(), kind: "ArrayBoundsCheck".to_string(),
                file: "main.fj".to_string(), line: 42, counterexample: Some("i = 10, len = 5".to_string()),
            }],
        };
        assert!(!report.all_proven());
        assert!((report.coverage() - 0.9).abs() < 0.001);
        let s = format!("{report}");
        assert!(s.contains("Proven:     18"));
        assert!(s.contains("Failed:     1"));
        assert!(s.contains("main.fj:42"));
    }

    #[test]
    fn v2_6_all_proven() {
        let report = VerificationReport {
            total_vcs: 10, proven: 10, failed: 0, timeouts: 0, unknown: 0,
            solver_time_ms: 100.0, cache_hit_rate: 0.5, failures: vec![],
        };
        assert!(report.all_proven());
        assert!((report.coverage() - 1.0).abs() < 0.001);
    }

    #[test]
    fn v2_1_solver_display() {
        assert_eq!(format!("{}", SolverBackend::Z3), "Z3");
        assert_eq!(format!("{}", SolverBackend::Cvc5), "CVC5");
    }
}
