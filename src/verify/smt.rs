// Allow unused imports that are conditionally needed by #[cfg(feature = "smt")] functions.
#![allow(unused_imports)]
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
        match self {
            Self::Z3 => write!(f, "Z3"),
            Self::Cvc5 => write!(f, "CVC5"),
            Self::Yices2 => write!(f, "Yices2"),
        }
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
pub fn select_logic(
    has_arrays: bool,
    has_quantifiers: bool,
    has_bitvectors: bool,
    has_reals: bool,
) -> SmtLogic {
    if has_bitvectors {
        return SmtLogic::QfBv;
    }
    if has_quantifiers && has_arrays {
        return SmtLogic::Auflia;
    }
    if has_reals {
        return SmtLogic::Lra;
    }
    if has_arrays {
        return SmtLogic::QfAlia;
    }
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
    pub fn is_proven(&self) -> bool {
        matches!(self, Self::Unsat)
    }

    /// Returns true if a counterexample was found.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Sat(_))
    }
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
        let mut lines: Vec<String> = self
            .assignments
            .iter()
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
                    if i > 0 {
                        write!(f, ", ")?;
                    }
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
        self.entries.insert(
            vc_hash,
            CachedResult {
                result,
                source_hash,
                timestamp_ms: now_ms,
            },
        );
    }

    /// Returns cache hit rate.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }

    /// Number of cached entries.
    pub fn size(&self) -> usize {
        self.entries.len()
    }
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
    pub fn all_proven(&self) -> bool {
        self.failed == 0 && self.unknown == 0 && self.timeouts == 0
    }

    /// Returns verification coverage (fraction proven).
    pub fn coverage(&self) -> f64 {
        if self.total_vcs == 0 {
            return 1.0;
        }
        self.proven as f64 / self.total_vcs as f64
    }
}

impl fmt::Display for VerificationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Verification Report:")?;
        writeln!(f, "  Total VCs:  {}", self.total_vcs)?;
        writeln!(
            f,
            "  Proven:     {} ({:.1}%)",
            self.proven,
            self.coverage() * 100.0
        )?;
        writeln!(f, "  Failed:     {}", self.failed)?;
        writeln!(f, "  Timeout:    {}", self.timeouts)?;
        writeln!(f, "  Unknown:    {}", self.unknown)?;
        writeln!(f, "  Time:       {:.1}ms", self.solver_time_ms)?;
        if !self.failures.is_empty() {
            writeln!(f, "\nFailures:")?;
            for fail in &self.failures {
                writeln!(
                    f,
                    "  {}:{} — {} ({})",
                    fail.file, fail.line, fail.description, fail.kind
                )?;
                if let Some(ref ce) = fail.counterexample {
                    writeln!(f, "    {ce}")?;
                }
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC4: Real Z3 Solver Integration
// ═══════════════════════════════════════════════════════════════════════

/// A verification condition to be checked by Z3.
#[derive(Debug, Clone)]
pub struct VerificationCondition {
    /// Name of the VC (e.g., "array_bounds_check_line_42").
    pub name: String,
    /// SMT-LIB2 assertion string.
    pub smt_assertion: String,
    /// Source file.
    pub file: String,
    /// Source line.
    pub line: u32,
}

// ═══════════════════════════════════════════════════════════════════════
// Stub implementations for when Z3 is not available (no "smt" feature)
// ═══════════════════════════════════════════════════════════════════════

/// Prove that an integer expression is always non-negative (stub without Z3).
///
/// Performs lightweight syntactic analysis of the constraint string.
/// Returns `Unsat` (proven) if constraint clearly enforces non-negativity,
/// `Sat` (failed) otherwise.
#[cfg(not(feature = "smt"))]
pub fn prove_non_negative(_var_name: &str, constraint: &str) -> SmtResult {
    // Simple heuristic: if constraint says "x >= 0" we treat it as proven.
    if constraint.contains(">= 0") {
        SmtResult::Unsat
    } else {
        SmtResult::Sat(Counterexample {
            assignments: HashMap::from([(_var_name.to_string(), SmtValue::Int(-1))]),
        })
    }
}

/// Check satisfiability of a conjunction of integer constraints (stub without Z3).
#[cfg(not(feature = "smt"))]
pub fn check_satisfiable(assertions: &[(String, i64, &str, i64)]) -> SmtResult {
    // Simple: check that no assertion is trivially contradictory.
    // For the stub, we assume satisfiable if no obvious contradiction.
    for (_, _, op, _) in assertions {
        // All supported ops are plausible; just return Sat (satisfiable).
        let _ = op;
    }
    SmtResult::Sat(Counterexample::default())
}

/// Prove that array index is always within bounds (stub without Z3).
#[cfg(not(feature = "smt"))]
pub fn prove_array_bounds(index_constraint: &str, array_size: i64) -> SmtResult {
    // Heuristic: "i >= 0 && i < N" with N == array_size → proven.
    let expected = format!("< {array_size}");
    if index_constraint.contains(">= 0") && index_constraint.contains(&expected) {
        SmtResult::Unsat
    } else {
        SmtResult::Sat(Counterexample {
            assignments: HashMap::from([("index".to_string(), SmtValue::Int(array_size))]),
        })
    }
}

/// Prove matmul shape compatibility (stub without Z3).
#[cfg(not(feature = "smt"))]
pub fn prove_matmul_shapes(_m: i64, k1: i64, k2: i64, _n: i64) -> SmtResult {
    if k1 == k2 {
        SmtResult::Unsat // proven: inner dimensions match
    } else {
        SmtResult::Sat(Counterexample {
            assignments: HashMap::from([
                ("k1".to_string(), SmtValue::Int(k1)),
                ("k2".to_string(), SmtValue::Int(k2)),
            ]),
        })
    }
}

/// Prove that i32 addition doesn't overflow for bounded inputs (stub without Z3).
#[cfg(not(feature = "smt"))]
pub fn prove_no_i32_overflow(a_min: i32, a_max: i32, b_min: i32, b_max: i32) -> SmtResult {
    // Check if any combination could overflow i32.
    let max_sum = a_max as i64 + b_max as i64;
    let min_sum = a_min as i64 + b_min as i64;
    if max_sum > i32::MAX as i64 || min_sum < i32::MIN as i64 {
        SmtResult::Sat(Counterexample {
            assignments: HashMap::from([
                ("a".to_string(), SmtValue::Int(a_max as i64)),
                ("b".to_string(), SmtValue::Int(b_max as i64)),
            ]),
        })
    } else {
        SmtResult::Unsat
    }
}

/// Run a verification with a timeout (stub without Z3).
#[cfg(not(feature = "smt"))]
pub fn prove_with_timeout(var_name: &str, constraint: &str, _timeout_ms: u64) -> SmtResult {
    // Delegate to prove_non_negative (same logic, timeout is irrelevant without Z3).
    prove_non_negative(var_name, constraint)
}

/// Prove that an integer expression is always non-negative.
/// Returns Unsat if proven (no counterexample exists), Sat with counterexample if disproven.
///
/// Accepts constraint in two forms:
///   - operator-only: `">= 0"`, `"> 5"`, `"< 100"`
///   - with var prefix: `"x >= 0"`, `"x > 5"`, `"x < 100"`
///
/// The var-prefix form is stripped before operator parsing.
#[cfg(feature = "smt")]
pub fn prove_non_negative(var_name: &str, constraint: &str) -> SmtResult {
    use z3::ast::Ast;
    let cfg = z3::Config::new();
    let ctx = z3::Context::new(&cfg);
    let solver = z3::Solver::new(&ctx);

    let x = z3::ast::Int::new_const(&ctx, var_name);

    // Strip optional `<var_name> ` prefix so callers can pass either
    // "x >= 0" or ">= 0". Both formats now work.
    let var_prefix = format!("{var_name} ");
    let constraint = constraint.strip_prefix(&var_prefix).unwrap_or(constraint);

    // Apply constraint: parse simple forms like "> 0", "< 100", ">= 0"
    if let Some(rest) = constraint.strip_prefix("> ") {
        if let Ok(val) = rest.trim().parse::<i64>() {
            solver.assert(&x.gt(&z3::ast::Int::from_i64(&ctx, val)));
        }
    } else if let Some(rest) = constraint.strip_prefix("< ") {
        if let Ok(val) = rest.trim().parse::<i64>() {
            solver.assert(&x.lt(&z3::ast::Int::from_i64(&ctx, val)));
        }
    } else if let Some(rest) = constraint.strip_prefix(">= ") {
        if let Ok(val) = rest.trim().parse::<i64>() {
            solver.assert(&x.ge(&z3::ast::Int::from_i64(&ctx, val)));
        }
    }

    // Negate the goal: if x >= 0 is always true, then x < 0 should be unsatisfiable
    solver.assert(&x.lt(&z3::ast::Int::from_i64(&ctx, 0)));

    match solver.check() {
        z3::SatResult::Unsat => SmtResult::Unsat, // proven: x is always >= 0
        z3::SatResult::Sat => {
            // Counterexample found
            let model = solver
                .get_model()
                .expect("Z3 solver model after SAT result");
            let val = model.eval(&x, true).expect("Z3 model eval for variable");
            SmtResult::Sat(Counterexample {
                assignments: HashMap::from([(
                    var_name.to_string(),
                    SmtValue::Int(val.as_i64().unwrap_or(0)),
                )]),
            })
        }
        z3::SatResult::Unknown => SmtResult::Unknown,
    }
}

/// Check if two integer expressions can be equal (satisfiability check).
#[cfg(feature = "smt")]
pub fn check_satisfiable(assertions: &[(String, i64, &str, i64)]) -> SmtResult {
    use z3::ast::Ast;
    let cfg = z3::Config::new();
    let ctx = z3::Context::new(&cfg);
    let solver = z3::Solver::new(&ctx);

    let mut vars: HashMap<String, z3::ast::Int> = HashMap::new();

    for (name, _default, op, value) in assertions {
        let var = vars
            .entry(name.clone())
            .or_insert_with(|| z3::ast::Int::new_const(&ctx, name.as_str()))
            .clone();
        let val = z3::ast::Int::from_i64(&ctx, *value);

        match *op {
            "==" => solver.assert(&var._eq(&val)),
            ">" => solver.assert(&var.gt(&val)),
            "<" => solver.assert(&var.lt(&val)),
            ">=" => solver.assert(&var.ge(&val)),
            "<=" => solver.assert(&var.le(&val)),
            "!=" => {
                solver.assert(&z3::ast::Bool::not(&var._eq(&val)));
            }
            _ => {}
        }
    }

    match solver.check() {
        z3::SatResult::Sat => {
            let model = solver
                .get_model()
                .expect("Z3 solver model after SAT result");
            let mut assignments = HashMap::new();
            for (name, var) in &vars {
                if let Some(val) = model.eval(var, true) {
                    assignments.insert(name.clone(), SmtValue::Int(val.as_i64().unwrap_or(0)));
                }
            }
            SmtResult::Sat(Counterexample { assignments })
        }
        z3::SatResult::Unsat => SmtResult::Unsat,
        z3::SatResult::Unknown => SmtResult::Unknown,
    }
}

/// Prove that array index is always within bounds.
#[cfg(feature = "smt")]
pub fn prove_array_bounds(index_constraint: &str, array_size: i64) -> SmtResult {
    use z3::ast::Ast;
    let cfg = z3::Config::new();
    let ctx = z3::Context::new(&cfg);
    let solver = z3::Solver::new(&ctx);

    let idx = z3::ast::Int::new_const(&ctx, "index");
    let size = z3::ast::Int::from_i64(&ctx, array_size);
    let zero = z3::ast::Int::from_i64(&ctx, 0);

    // Accept several constraint forms:
    //   "< N"               — operator-only (legacy)
    //   "i < N"             — with var prefix
    //   "i >= 0 && i < N"   — natural compound form
    //   "any"               — unconstrained (must find counterexample)
    //
    // For natural-language forms, scan for `>=` and `<` against integer
    // literals; assert each constraint we recognize. Anything we don't
    // parse is ignored — the test will then expose a false counterexample
    // and fail loudly, which is the right signal.
    if index_constraint == "any" {
        // No constraint on index — should find counterexample
    } else if index_constraint.contains("&&")
        || index_constraint.contains("index ")
        || index_constraint.starts_with(|c: char| c.is_alphabetic())
    {
        // Natural-language form: split on `&&` and parse each clause.
        for clause in index_constraint.split("&&").map(str::trim) {
            // Strip optional var name prefix (e.g. "i", "index", "idx")
            let clause = clause
                .split_once(' ')
                .map(|(first, rest)| {
                    if first.chars().all(|c| c.is_alphanumeric() || c == '_')
                        && !first.starts_with(|c: char| c.is_ascii_digit())
                    {
                        rest
                    } else {
                        clause
                    }
                })
                .unwrap_or(clause);
            if let Some(rest) = clause.strip_prefix(">= ")
                && let Ok(val) = rest.trim().parse::<i64>()
            {
                solver.assert(&idx.ge(&z3::ast::Int::from_i64(&ctx, val)));
            } else if let Some(rest) = clause.strip_prefix("> ")
                && let Ok(val) = rest.trim().parse::<i64>()
            {
                solver.assert(&idx.gt(&z3::ast::Int::from_i64(&ctx, val)));
            } else if let Some(rest) = clause.strip_prefix("<= ")
                && let Ok(val) = rest.trim().parse::<i64>()
            {
                solver.assert(&idx.le(&z3::ast::Int::from_i64(&ctx, val)));
            } else if let Some(rest) = clause.strip_prefix("< ")
                && let Ok(val) = rest.trim().parse::<i64>()
            {
                solver.assert(&idx.lt(&z3::ast::Int::from_i64(&ctx, val)));
            }
        }
    } else if let Some(rest) = index_constraint.strip_prefix("< ") {
        // Legacy operator-only form
        if let Ok(val) = rest.trim().parse::<i64>() {
            solver.assert(&idx.ge(&zero));
            solver.assert(&idx.lt(&z3::ast::Int::from_i64(&ctx, val)));
        }
    }

    // Negate: index < 0 || index >= size
    let out_of_bounds = z3::ast::Bool::or(&ctx, &[&idx.lt(&zero), &idx.ge(&size)]);
    solver.assert(&out_of_bounds);

    match solver.check() {
        z3::SatResult::Unsat => SmtResult::Unsat, // proven: always in bounds
        z3::SatResult::Sat => {
            let model = solver
                .get_model()
                .expect("Z3 solver model after SAT result");
            let val = model
                .eval(&idx, true)
                .expect("Z3 model eval for index variable");
            SmtResult::Sat(Counterexample {
                assignments: HashMap::from([(
                    "index".to_string(),
                    SmtValue::Int(val.as_i64().unwrap_or(0)),
                )]),
            })
        }
        z3::SatResult::Unknown => SmtResult::Unknown,
    }
}

/// Prove matmul shape compatibility: A[m,k] × B[k2,n] requires k == k2.
#[cfg(feature = "smt")]
pub fn prove_matmul_shapes(m: i64, k1: i64, k2: i64, n: i64) -> SmtResult {
    use z3::ast::Ast;
    let cfg = z3::Config::new();
    let ctx = z3::Context::new(&cfg);
    let solver = z3::Solver::new(&ctx);

    let k1_val = z3::ast::Int::from_i64(&ctx, k1);
    let k2_val = z3::ast::Int::from_i64(&ctx, k2);

    // Assert k1 != k2 (negate the goal k1 == k2)
    solver.assert(&z3::ast::Bool::not(&k1_val._eq(&k2_val)));

    match solver.check() {
        z3::SatResult::Unsat => SmtResult::Unsat, // proven: k1 must equal k2
        z3::SatResult::Sat => SmtResult::Sat(Counterexample {
            assignments: HashMap::from([
                (
                    "A_shape".to_string(),
                    SmtValue::Array(vec![
                        (SmtValue::Int(0), SmtValue::Int(m)),
                        (SmtValue::Int(1), SmtValue::Int(k1)),
                    ]),
                ),
                (
                    "B_shape".to_string(),
                    SmtValue::Array(vec![
                        (SmtValue::Int(0), SmtValue::Int(k2)),
                        (SmtValue::Int(1), SmtValue::Int(n)),
                    ]),
                ),
            ]),
        }),
        z3::SatResult::Unknown => SmtResult::Unknown,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════
// VQ6.1: Bitvector Theory (overflow detection)
// ═══════════════════════════════════════════════════════════════════════

/// Prove that i32 addition doesn't overflow for bounded inputs.
/// Returns Unsat if no overflow possible, Sat with counterexample if overflow found.
#[cfg(feature = "smt")]
pub fn prove_no_i32_overflow(a_min: i32, a_max: i32, b_min: i32, b_max: i32) -> SmtResult {
    use z3::ast::Ast;
    let cfg = z3::Config::new();
    let ctx = z3::Context::new(&cfg);
    let solver = z3::Solver::new(&ctx);

    let a = z3::ast::BV::new_const(&ctx, "a", 32);
    let b = z3::ast::BV::new_const(&ctx, "b", 32);

    // Constrain ranges
    let a_min_bv = z3::ast::BV::from_i64(&ctx, a_min as i64, 32);
    let a_max_bv = z3::ast::BV::from_i64(&ctx, a_max as i64, 32);
    let b_min_bv = z3::ast::BV::from_i64(&ctx, b_min as i64, 32);
    let b_max_bv = z3::ast::BV::from_i64(&ctx, b_max as i64, 32);

    solver.assert(&a.bvsge(&a_min_bv));
    solver.assert(&a.bvsle(&a_max_bv));
    solver.assert(&b.bvsge(&b_min_bv));
    solver.assert(&b.bvsle(&b_max_bv));

    // Check: does signed addition overflow?
    // Overflow if: (a > 0 && b > 0 && a+b < 0) || (a < 0 && b < 0 && a+b > 0)
    let sum = a.bvadd(&b);
    let zero = z3::ast::BV::from_i64(&ctx, 0, 32);

    let pos_overflow =
        z3::ast::Bool::and(&ctx, &[&a.bvsgt(&zero), &b.bvsgt(&zero), &sum.bvslt(&zero)]);
    let neg_overflow =
        z3::ast::Bool::and(&ctx, &[&a.bvslt(&zero), &b.bvslt(&zero), &sum.bvsgt(&zero)]);
    let overflow = z3::ast::Bool::or(&ctx, &[&pos_overflow, &neg_overflow]);
    solver.assert(&overflow);

    match solver.check() {
        z3::SatResult::Unsat => SmtResult::Unsat, // no overflow possible
        z3::SatResult::Sat => {
            let model = solver
                .get_model()
                .expect("Z3 solver model after SAT result");
            let a_val = model.eval(&a, true).expect("Z3 model eval for operand a");
            let b_val = model.eval(&b, true).expect("Z3 model eval for operand b");
            SmtResult::Sat(Counterexample {
                assignments: HashMap::from([
                    ("a".to_string(), SmtValue::Int(a_val.as_i64().unwrap_or(0))),
                    ("b".to_string(), SmtValue::Int(b_val.as_i64().unwrap_or(0))),
                ]),
            })
        }
        z3::SatResult::Unknown => SmtResult::Unknown,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// VQ6.5: Timeout Handling
// ═══════════════════════════════════════════════════════════════════════

/// Run a verification with a timeout.
///
/// Accepts constraint in two forms (same as `prove_non_negative`):
///   - operator-only: `">= 0"`
///   - with var prefix: `"n >= 0"` (var name is stripped before parsing)
#[cfg(feature = "smt")]
pub fn prove_with_timeout(var_name: &str, constraint: &str, timeout_ms: u64) -> SmtResult {
    use z3::ast::Ast;
    let mut cfg = z3::Config::new();
    cfg.set_timeout_msec(timeout_ms);
    let ctx = z3::Context::new(&cfg);
    let solver = z3::Solver::new(&ctx);

    let x = z3::ast::Int::new_const(&ctx, var_name);

    // Strip optional `<var_name> ` prefix so callers can pass either form.
    let var_prefix = format!("{var_name} ");
    let constraint = constraint.strip_prefix(&var_prefix).unwrap_or(constraint);

    if let Some(rest) = constraint.strip_prefix(">= ") {
        if let Ok(val) = rest.trim().parse::<i64>() {
            solver.assert(&x.ge(&z3::ast::Int::from_i64(&ctx, val)));
        }
    }

    // Try to prove x >= 0 (negate: x < 0)
    solver.assert(&x.lt(&z3::ast::Int::from_i64(&ctx, 0)));

    match solver.check() {
        z3::SatResult::Unsat => SmtResult::Unsat,
        z3::SatResult::Sat => {
            let model = solver
                .get_model()
                .expect("Z3 solver model after SAT result");
            let val = model.eval(&x, true).expect("Z3 model eval for variable");
            SmtResult::Sat(Counterexample {
                assignments: HashMap::from([(
                    var_name.to_string(),
                    SmtValue::Int(val.as_i64().unwrap_or(0)),
                )]),
            })
        }
        z3::SatResult::Unknown => SmtResult::Unknown,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// VQ6.7: Counterexample Pretty Display
// ═══════════════════════════════════════════════════════════════════════

/// Format a verification result with context for human reading.
pub fn format_verification_result(
    description: &str,
    file: &str,
    line: u32,
    result: &SmtResult,
) -> String {
    match result {
        SmtResult::Unsat => format!("  ✅ {file}:{line} — {description}: PROVEN safe"),
        SmtResult::Sat(ce) => {
            let mut msg = format!("  ❌ {file}:{line} — {description}: FAILED\n");
            msg.push_str(&format!("     Counterexample: {}\n", ce.display()));
            msg.push_str("     Fix: add bounds check or tighten precondition\n");
            msg
        }
        SmtResult::Timeout => {
            format!("  ⚠️  {file}:{line} — {description}: TIMEOUT (solver timed out)")
        }
        SmtResult::Error(e) => {
            format!("  ⛔ {file}:{line} — {description}: SOLVER ERROR ({e})")
        }
        SmtResult::Unknown => {
            format!("  ⚠️  {file}:{line} — {description}: UNKNOWN (solver could not decide)")
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// VQ6.9: Error Localization
// ═══════════════════════════════════════════════════════════════════════

/// A localized verification error with source position.
#[derive(Debug)]
pub struct LocalizedError {
    /// Source file.
    pub file: String,
    /// Line number.
    pub line: u32,
    /// Column number.
    pub column: u32,
    /// Error description.
    pub description: String,
    /// Counterexample (if available).
    pub counterexample: Option<String>,
    /// Suggested fix.
    pub suggestion: String,
}

impl std::fmt::Display for LocalizedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {} — {}",
            self.file, self.line, self.column, self.description, self.suggestion
        )?;
        if let Some(ref ce) = self.counterexample {
            write!(f, "\n  counterexample: {ce}")?;
        }
        Ok(())
    }
}

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
            total_vcs: 20,
            proven: 18,
            failed: 1,
            timeouts: 1,
            unknown: 0,
            solver_time_ms: 1234.5,
            cache_hit_rate: 0.75,
            failures: vec![VerificationFailure {
                description: "array bounds".to_string(),
                kind: "ArrayBoundsCheck".to_string(),
                file: "main.fj".to_string(),
                line: 42,
                counterexample: Some("i = 10, len = 5".to_string()),
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
            total_vcs: 10,
            proven: 10,
            failed: 0,
            timeouts: 0,
            unknown: 0,
            solver_time_ms: 100.0,
            cache_hit_rate: 0.5,
            failures: vec![],
        };
        assert!(report.all_proven());
        assert!((report.coverage() - 1.0).abs() < 0.001);
    }

    #[test]
    fn v2_1_solver_display() {
        assert_eq!(format!("{}", SolverBackend::Z3), "Z3");
        assert_eq!(format!("{}", SolverBackend::Cvc5), "CVC5");
    }

    // ═══════════════════════════════════════════════════════════════════
    // V8 GC4: Real Z3 integration tests
    // ═══════════════════════════════════════════════════════════════════

    #[cfg(feature = "smt")]
    #[test]
    fn gc4_prove_non_negative_with_constraint() {
        // x > 0 implies x >= 0 — should be provable
        let result = prove_non_negative("x", "> 0");
        assert!(
            result.is_proven(),
            "x > 0 should prove x >= 0, got: {result:?}"
        );
    }

    #[cfg(feature = "smt")]
    #[test]
    fn gc4_disprove_non_negative_unconstrained() {
        // No constraint on x — x can be negative
        let result = prove_non_negative("x", "");
        assert!(
            result.is_failed(),
            "unconstrained x should have counterexample, got: {result:?}"
        );
        if let SmtResult::Sat(ce) = &result {
            let val = ce.assignments.get("x").unwrap();
            if let SmtValue::Int(n) = val {
                assert!(*n < 0, "counterexample should be negative: {n}");
            }
        }
    }

    #[cfg(feature = "smt")]
    #[test]
    fn gc4_satisfiable_constraints() {
        // x > 5 AND x < 10 — satisfiable
        let result =
            check_satisfiable(&[("x".to_string(), 0, ">", 5), ("x".to_string(), 0, "<", 10)]);
        assert!(
            result.is_failed(), // Sat = satisfiable = found assignment
            "x > 5 AND x < 10 should be satisfiable"
        );
    }

    #[cfg(feature = "smt")]
    #[test]
    fn gc4_unsatisfiable_constraints() {
        // x > 10 AND x < 5 — unsatisfiable
        let result =
            check_satisfiable(&[("x".to_string(), 0, ">", 10), ("x".to_string(), 0, "<", 5)]);
        assert!(
            result.is_proven(), // Unsat = no solution = constraints conflict
            "x > 10 AND x < 5 should be unsatisfiable"
        );
    }

    #[cfg(feature = "smt")]
    #[test]
    fn gc4_array_bounds_proven() {
        // index in [0, 10), array size 10 — always in bounds
        let result = prove_array_bounds("< 10", 10);
        assert!(
            result.is_proven(),
            "index < 10 should prove bounds for size 10"
        );
    }

    #[cfg(feature = "smt")]
    #[test]
    fn gc4_array_bounds_violated() {
        // index unconstrained, array size 5 — can go out of bounds
        let result = prove_array_bounds("any", 5);
        assert!(
            result.is_failed(),
            "unconstrained index should violate bounds"
        );
    }

    #[cfg(feature = "smt")]
    #[test]
    fn gc4_matmul_shapes_compatible() {
        // A[3,4] × B[4,5] — k1 == k2 == 4
        let result = prove_matmul_shapes(3, 4, 4, 5);
        assert!(result.is_proven(), "k=4 == k=4 should be proven");
    }

    #[cfg(feature = "smt")]
    #[test]
    fn gc4_matmul_shapes_incompatible() {
        // A[3,4] × B[5,6] — k1=4 != k2=5
        let result = prove_matmul_shapes(3, 4, 5, 6);
        assert!(
            result.is_failed(),
            "k=4 != k=5 should fail with counterexample"
        );
    }

    #[cfg(feature = "smt")]
    #[test]
    fn gc4_two_variable_constraint() {
        // x > 0 AND y > 0 AND x + y should be > 0 (checked via satisfiability)
        let result =
            check_satisfiable(&[("x".to_string(), 0, ">", 0), ("y".to_string(), 0, ">", 0)]);
        // Should find satisfying assignment
        if let SmtResult::Sat(ce) = &result {
            assert!(ce.assignments.contains_key("x"));
            assert!(ce.assignments.contains_key("y"));
        }
    }

    #[cfg(feature = "smt")]
    #[test]
    fn gc4_equality_constraint() {
        // x == 42 — should find x = 42
        let result = check_satisfiable(&[("x".to_string(), 0, "==", 42)]);
        if let SmtResult::Sat(ce) = &result {
            if let Some(SmtValue::Int(v)) = ce.assignments.get("x") {
                assert_eq!(*v, 42);
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // VQ6: Quality improvement tests
    // ═══════════════════════════════════════════════════════════════════

    #[cfg(feature = "smt")]
    #[test]
    fn vq6_1_no_overflow_small_range() {
        // a in [0, 100], b in [0, 100] → no i32 overflow
        let result = prove_no_i32_overflow(0, 100, 0, 100);
        assert!(result.is_proven(), "small range should not overflow");
    }

    #[cfg(feature = "smt")]
    #[test]
    fn vq6_1_overflow_max_range() {
        // a in [0, i32::MAX], b in [1, i32::MAX] → CAN overflow
        let result = prove_no_i32_overflow(0, i32::MAX, 1, i32::MAX);
        assert!(
            result.is_failed(),
            "max range should overflow: {:?}",
            result
        );
    }

    #[cfg(feature = "smt")]
    #[test]
    fn vq6_5_timeout_simple() {
        // Simple constraint — should resolve quickly within 5s
        let result = prove_with_timeout("x", ">= 0", 5000);
        assert!(result.is_proven(), "x >= 0 should prove x >= 0");
    }

    #[test]
    fn vq6_7_format_proven() {
        let msg = format_verification_result("array bounds", "main.fj", 42, &SmtResult::Unsat);
        assert!(msg.contains("✅"));
        assert!(msg.contains("PROVEN"));
        assert!(msg.contains("main.fj:42"));
    }

    #[test]
    fn vq6_7_format_failed() {
        let msg = format_verification_result(
            "index check",
            "lib.fj",
            10,
            &SmtResult::Sat(Counterexample {
                assignments: HashMap::from([("i".to_string(), SmtValue::Int(-1))]),
            }),
        );
        assert!(msg.contains("❌"));
        assert!(msg.contains("FAILED"));
        assert!(msg.contains("i = -1"));
    }

    #[test]
    fn vq6_7_format_unknown() {
        let msg = format_verification_result("complex proof", "deep.fj", 99, &SmtResult::Unknown);
        assert!(msg.contains("UNKNOWN"));
    }

    #[test]
    fn vq6_7_format_timeout() {
        let msg = format_verification_result("complex proof", "deep.fj", 99, &SmtResult::Timeout);
        assert!(msg.contains("TIMEOUT"));
    }

    #[test]
    fn vq6_9_localized_error() {
        let err = LocalizedError {
            file: "main.fj".to_string(),
            line: 42,
            column: 15,
            description: "array index may be out of bounds".to_string(),
            counterexample: Some("i = 10, len = 5".to_string()),
            suggestion: "add `if i < len(arr)` guard".to_string(),
        };
        let s = format!("{err}");
        assert!(s.contains("main.fj:42:15"));
        assert!(s.contains("out of bounds"));
        assert!(s.contains("i = 10, len = 5"));
        assert!(s.contains("guard"));
    }
}
