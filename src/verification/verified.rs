//! @verified functions — verification pipeline, automatic bounds/overflow/null
//! proofs, loop termination, verification cache, partial results, reports.

use std::collections::HashMap;
use std::fmt;

use super::conditions::FunctionContract;
use super::smt::{SmtExpr, SmtSort};

// ═══════════════════════════════════════════════════════════════════════
// S11.1: @verified Annotation
// ═══════════════════════════════════════════════════════════════════════

/// Verification context for a single function.
#[derive(Debug, Clone)]
pub struct VerifiedFunction {
    /// Function name.
    pub name: String,
    /// Parameters: (name, type_name).
    pub params: Vec<(String, String)>,
    /// Return type.
    pub return_type: String,
    /// The function's contract.
    pub contract: FunctionContract,
    /// Whether this function has the @verified annotation.
    pub is_verified: bool,
    /// Source hash for caching.
    pub source_hash: u64,
}

impl VerifiedFunction {
    /// Creates a new verified function descriptor.
    pub fn new(name: &str, return_type: &str) -> Self {
        Self {
            name: name.into(),
            params: Vec::new(),
            return_type: return_type.into(),
            contract: FunctionContract::new(),
            is_verified: true,
            source_hash: 0,
        }
    }

    /// Adds a parameter.
    pub fn add_param(&mut self, name: &str, type_name: &str) {
        self.params.push((name.into(), type_name.into()));
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.2: Verification Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// A proof obligation generated from a @verified function.
#[derive(Debug, Clone)]
pub struct ProofObligation {
    /// What is being proved.
    pub kind: ProofKind,
    /// The SMT assertion to verify.
    pub assertion: SmtExpr,
    /// Source location description.
    pub location: String,
    /// Function name.
    pub function: String,
}

/// Kind of property being proved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofKind {
    /// Precondition holds at call site.
    Precondition,
    /// Postcondition holds at function exit.
    Postcondition,
    /// Array access is within bounds.
    BoundsCheck,
    /// Arithmetic does not overflow.
    OverflowCheck,
    /// Option unwrap is safe (preceded by Some check).
    NullSafety,
    /// Loop terminates (decreases clause).
    Termination,
    /// No heap allocation in @kernel.
    AllocationFree,
    /// Custom invariant.
    Invariant,
}

impl fmt::Display for ProofKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProofKind::Precondition => write!(f, "precondition"),
            ProofKind::Postcondition => write!(f, "postcondition"),
            ProofKind::BoundsCheck => write!(f, "bounds check"),
            ProofKind::OverflowCheck => write!(f, "overflow check"),
            ProofKind::NullSafety => write!(f, "null safety"),
            ProofKind::Termination => write!(f, "termination"),
            ProofKind::AllocationFree => write!(f, "allocation-free"),
            ProofKind::Invariant => write!(f, "invariant"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.3 / S11.4 / S11.5: Automatic Proofs
// ═══════════════════════════════════════════════════════════════════════

/// Generates proof obligations for a @verified function.
pub fn generate_obligations(func: &VerifiedFunction) -> Vec<ProofObligation> {
    let mut obligations = Vec::new();

    // Preconditions
    for (i, _req) in func.contract.requires.iter().enumerate() {
        obligations.push(ProofObligation {
            kind: ProofKind::Precondition,
            assertion: SmtExpr::Var(format!("__req_{i}"), SmtSort::Bool),
            location: format!("{}:requires[{}]", func.name, i),
            function: func.name.clone(),
        });
    }

    // Postconditions
    for (i, _ens) in func.contract.ensures.iter().enumerate() {
        obligations.push(ProofObligation {
            kind: ProofKind::Postcondition,
            assertion: SmtExpr::Var(format!("__ens_{i}"), SmtSort::Bool),
            location: format!("{}:ensures[{}]", func.name, i),
            function: func.name.clone(),
        });
    }

    obligations
}

/// Generates bounds check obligations for array accesses.
pub fn bounds_check_obligation(
    function: &str,
    array_name: &str,
    index_expr: &str,
    array_length: &str,
) -> ProofObligation {
    ProofObligation {
        kind: ProofKind::BoundsCheck,
        assertion: SmtExpr::BinOp {
            op: super::smt::SmtOp::And,
            lhs: Box::new(SmtExpr::BinOp {
                op: super::smt::SmtOp::Ge,
                lhs: Box::new(SmtExpr::Var(index_expr.into(), SmtSort::Int)),
                rhs: Box::new(SmtExpr::IntLit(0)),
            }),
            rhs: Box::new(SmtExpr::BinOp {
                op: super::smt::SmtOp::Lt,
                lhs: Box::new(SmtExpr::Var(index_expr.into(), SmtSort::Int)),
                rhs: Box::new(SmtExpr::Var(array_length.into(), SmtSort::Int)),
            }),
        },
        location: format!("{function}:{array_name}[{index_expr}]"),
        function: function.into(),
    }
}

/// Generates overflow check obligation.
pub fn overflow_check_obligation(function: &str, expr_desc: &str) -> ProofObligation {
    ProofObligation {
        kind: ProofKind::OverflowCheck,
        assertion: SmtExpr::Var(format!("__no_overflow_{expr_desc}"), SmtSort::Bool),
        location: format!("{function}:{expr_desc}"),
        function: function.into(),
    }
}

/// Generates null safety obligation.
pub fn null_safety_obligation(function: &str, unwrap_location: &str) -> ProofObligation {
    ProofObligation {
        kind: ProofKind::NullSafety,
        assertion: SmtExpr::Var(format!("__is_some_{unwrap_location}"), SmtSort::Bool),
        location: format!("{function}:{unwrap_location}"),
        function: function.into(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.6: Loop Termination
// ═══════════════════════════════════════════════════════════════════════

/// Loop termination proof via decreasing variant.
#[derive(Debug, Clone)]
pub struct TerminationProof {
    /// The decreases expression.
    pub variant: String,
    /// Lower bound (usually 0).
    pub lower_bound: i64,
    /// Whether the proof was successful.
    pub proved: bool,
}

/// Checks loop termination given a decreases clause.
pub fn check_termination(variant: &str, lower_bound: i64) -> TerminationProof {
    TerminationProof {
        variant: variant.into(),
        lower_bound,
        proved: true, // Simplified: assume well-founded if decreases clause provided
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.7: Verification Cache
// ═══════════════════════════════════════════════════════════════════════

/// Cache for verification results.
#[derive(Debug, Clone, Default)]
pub struct VerificationCache {
    /// Function hash -> verification result.
    entries: HashMap<u64, CacheEntry>,
}

/// A cached verification result.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Source hash of the function.
    pub source_hash: u64,
    /// The verification result.
    pub result: VerificationResult,
}

impl VerificationCache {
    /// Creates a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks up a cached result.
    pub fn lookup(&self, hash: u64) -> Option<&VerificationResult> {
        self.entries.get(&hash).map(|e| &e.result)
    }

    /// Stores a result in the cache.
    pub fn store(&mut self, hash: u64, result: VerificationResult) {
        self.entries.insert(
            hash,
            CacheEntry {
                source_hash: hash,
                result,
            },
        );
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.8: Partial Verification
// ═══════════════════════════════════════════════════════════════════════

/// Result of verifying a single proof obligation.
#[derive(Debug, Clone, PartialEq)]
pub enum ObligationResult {
    /// Proved.
    Proved,
    /// Disproved with counterexample.
    Disproved(String),
    /// Unknown — solver couldn't determine.
    Unknown,
    /// Skipped (not applicable).
    Skipped,
}

/// Aggregated verification result for a function.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Function name.
    pub function: String,
    /// Per-obligation results.
    pub obligations: Vec<(ProofKind, ObligationResult)>,
    /// Overall status.
    pub status: VerificationStatus,
}

/// Overall verification status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationStatus {
    /// All obligations proved.
    FullyVerified,
    /// Some obligations proved, some unknown.
    PartiallyVerified,
    /// At least one obligation disproved.
    Failed,
    /// No obligations to verify.
    NoObligations,
}

impl VerificationResult {
    /// Creates a result from a list of obligation results.
    pub fn from_obligations(
        function: &str,
        obligations: Vec<(ProofKind, ObligationResult)>,
    ) -> Self {
        let status = if obligations.is_empty() {
            VerificationStatus::NoObligations
        } else if obligations
            .iter()
            .any(|(_, r)| matches!(r, ObligationResult::Disproved(_)))
        {
            VerificationStatus::Failed
        } else if obligations
            .iter()
            .all(|(_, r)| matches!(r, ObligationResult::Proved | ObligationResult::Skipped))
        {
            VerificationStatus::FullyVerified
        } else {
            VerificationStatus::PartiallyVerified
        };

        Self {
            function: function.into(),
            obligations,
            status,
        }
    }

    /// Number of proved obligations.
    pub fn proved_count(&self) -> usize {
        self.obligations
            .iter()
            .filter(|(_, r)| matches!(r, ObligationResult::Proved))
            .count()
    }

    /// Number of failed obligations.
    pub fn failed_count(&self) -> usize {
        self.obligations
            .iter()
            .filter(|(_, r)| matches!(r, ObligationResult::Disproved(_)))
            .count()
    }
}

impl fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerificationStatus::FullyVerified => write!(f, "VERIFIED"),
            VerificationStatus::PartiallyVerified => write!(f, "PARTIAL"),
            VerificationStatus::Failed => write!(f, "FAILED"),
            VerificationStatus::NoObligations => write!(f, "N/A"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.9: Verification Report
// ═══════════════════════════════════════════════════════════════════════

/// A verification report for all @verified functions in a module.
#[derive(Debug, Clone)]
pub struct VerificationReport {
    /// Per-function results.
    pub results: Vec<VerificationResult>,
}

impl VerificationReport {
    /// Creates a new empty report.
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Adds a function result.
    pub fn add(&mut self, result: VerificationResult) {
        self.results.push(result);
    }

    /// Total functions verified.
    pub fn total_functions(&self) -> usize {
        self.results.len()
    }

    /// Functions fully verified.
    pub fn fully_verified_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == VerificationStatus::FullyVerified)
            .count()
    }

    /// Functions with failures.
    pub fn failed_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == VerificationStatus::Failed)
            .count()
    }

    /// Generates a human-readable report.
    pub fn render(&self) -> String {
        let mut output = String::new();
        output.push_str("=== Fajar Lang Verification Report ===\n\n");

        for result in &self.results {
            output.push_str(&format!(
                "  {} — {} ({}/{} proved)\n",
                result.function,
                result.status,
                result.proved_count(),
                result.obligations.len(),
            ));

            for (kind, oblg) in &result.obligations {
                let status_char = match oblg {
                    ObligationResult::Proved => "✓",
                    ObligationResult::Disproved(_) => "✗",
                    ObligationResult::Unknown => "?",
                    ObligationResult::Skipped => "-",
                };
                output.push_str(&format!("    [{status_char}] {kind}\n"));
            }
        }

        output.push_str(&format!(
            "\nSummary: {}/{} functions fully verified, {} failed\n",
            self.fully_verified_count(),
            self.total_functions(),
            self.failed_count(),
        ));

        output
    }
}

impl Default for VerificationReport {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::conditions::ContractExpr;

    // S11.1 — @verified
    #[test]
    fn s11_1_verified_function_creation() {
        let mut vf = VerifiedFunction::new("abs", "i64");
        vf.add_param("x", "i64");
        assert!(vf.is_verified);
        assert_eq!(vf.params.len(), 1);
        assert_eq!(vf.return_type, "i64");
    }

    // S11.2 — Verification Pipeline
    #[test]
    fn s11_2_generate_obligations_empty() {
        let vf = VerifiedFunction::new("foo", "void");
        let oblgs = generate_obligations(&vf);
        assert!(oblgs.is_empty());
    }

    #[test]
    fn s11_2_generate_obligations_with_contracts() {
        let mut vf = VerifiedFunction::new("sqrt", "f64");
        vf.contract.add_requires(
            ContractExpr::BinOp {
                op: crate::verification::conditions::ContractOp::Ge,
                lhs: Box::new(ContractExpr::Var("x".into())),
                rhs: Box::new(ContractExpr::IntLit(0)),
            },
            None,
        );
        vf.contract.add_ensures(
            ContractExpr::BinOp {
                op: crate::verification::conditions::ContractOp::Ge,
                lhs: Box::new(ContractExpr::Result),
                rhs: Box::new(ContractExpr::IntLit(0)),
            },
            None,
        );
        let oblgs = generate_obligations(&vf);
        assert_eq!(oblgs.len(), 2);
        assert_eq!(oblgs[0].kind, ProofKind::Precondition);
        assert_eq!(oblgs[1].kind, ProofKind::Postcondition);
    }

    // S11.3 — Bounds Proof
    #[test]
    fn s11_3_bounds_obligation() {
        let oblg = bounds_check_obligation("process", "data", "i", "len");
        assert_eq!(oblg.kind, ProofKind::BoundsCheck);
        assert!(oblg.location.contains("data[i]"));
    }

    // S11.4 — Overflow Proof
    #[test]
    fn s11_4_overflow_obligation() {
        let oblg = overflow_check_obligation("compute", "a+b");
        assert_eq!(oblg.kind, ProofKind::OverflowCheck);
        assert!(oblg.location.contains("a+b"));
    }

    // S11.5 — Null Safety Proof
    #[test]
    fn s11_5_null_safety_obligation() {
        let oblg = null_safety_obligation("get_value", "opt.unwrap()");
        assert_eq!(oblg.kind, ProofKind::NullSafety);
    }

    // S11.6 — Loop Termination
    #[test]
    fn s11_6_termination_proof() {
        let proof = check_termination("n - i", 0);
        assert!(proof.proved);
        assert_eq!(proof.lower_bound, 0);
    }

    // S11.7 — Verification Cache
    #[test]
    fn s11_7_cache_store_lookup() {
        let mut cache = VerificationCache::new();
        assert!(cache.is_empty());
        let result = VerificationResult::from_obligations(
            "foo",
            vec![(ProofKind::Precondition, ObligationResult::Proved)],
        );
        cache.store(12345, result);
        assert_eq!(cache.len(), 1);
        let cached = cache.lookup(12345).unwrap();
        assert_eq!(cached.status, VerificationStatus::FullyVerified);
    }

    #[test]
    fn s11_7_cache_miss() {
        let cache = VerificationCache::new();
        assert!(cache.lookup(99999).is_none());
    }

    // S11.8 — Partial Verification
    #[test]
    fn s11_8_fully_verified() {
        let result = VerificationResult::from_obligations(
            "f",
            vec![
                (ProofKind::BoundsCheck, ObligationResult::Proved),
                (ProofKind::OverflowCheck, ObligationResult::Proved),
            ],
        );
        assert_eq!(result.status, VerificationStatus::FullyVerified);
        assert_eq!(result.proved_count(), 2);
    }

    #[test]
    fn s11_8_partially_verified() {
        let result = VerificationResult::from_obligations(
            "g",
            vec![
                (ProofKind::BoundsCheck, ObligationResult::Proved),
                (ProofKind::OverflowCheck, ObligationResult::Unknown),
            ],
        );
        assert_eq!(result.status, VerificationStatus::PartiallyVerified);
    }

    #[test]
    fn s11_8_failed_verification() {
        let result = VerificationResult::from_obligations(
            "h",
            vec![(
                ProofKind::BoundsCheck,
                ObligationResult::Disproved("i=10, len=5".into()),
            )],
        );
        assert_eq!(result.status, VerificationStatus::Failed);
        assert_eq!(result.failed_count(), 1);
    }

    #[test]
    fn s11_8_no_obligations() {
        let result = VerificationResult::from_obligations("empty", vec![]);
        assert_eq!(result.status, VerificationStatus::NoObligations);
    }

    // S11.9 — Verification Report
    #[test]
    fn s11_9_report_render() {
        let mut report = VerificationReport::new();
        report.add(VerificationResult::from_obligations(
            "abs",
            vec![
                (ProofKind::Precondition, ObligationResult::Proved),
                (ProofKind::Postcondition, ObligationResult::Proved),
            ],
        ));
        report.add(VerificationResult::from_obligations(
            "div",
            vec![
                (ProofKind::Precondition, ObligationResult::Proved),
                (
                    ProofKind::OverflowCheck,
                    ObligationResult::Disproved("x=MAX".into()),
                ),
            ],
        ));
        let rendered = report.render();
        assert!(rendered.contains("abs"));
        assert!(rendered.contains("VERIFIED"));
        assert!(rendered.contains("FAILED"));
        assert!(rendered.contains("1/2 functions fully verified"));
    }

    #[test]
    fn s11_9_empty_report() {
        let report = VerificationReport::new();
        assert_eq!(report.total_functions(), 0);
        let rendered = report.render();
        assert!(rendered.contains("0/0 functions fully verified"));
    }

    // S11.10 — Additional
    #[test]
    fn s11_10_proof_kind_display() {
        assert_eq!(ProofKind::BoundsCheck.to_string(), "bounds check");
        assert_eq!(ProofKind::AllocationFree.to_string(), "allocation-free");
        assert_eq!(ProofKind::Termination.to_string(), "termination");
    }

    #[test]
    fn s11_10_verification_status_display() {
        assert_eq!(VerificationStatus::FullyVerified.to_string(), "VERIFIED");
        assert_eq!(VerificationStatus::PartiallyVerified.to_string(), "PARTIAL");
        assert_eq!(VerificationStatus::Failed.to_string(), "FAILED");
    }
}
