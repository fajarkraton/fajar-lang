//! Optimizing JIT — inlining, CSE, DCE, LICM, speculative optimization,
//! guard failure handling, code replacement, optimization metrics.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use super::counters::ExecutionTier;

// ═══════════════════════════════════════════════════════════════════════
// S15.1: Optimizing Compiler Entry
// ═══════════════════════════════════════════════════════════════════════

/// Request for optimizing compilation.
#[derive(Debug, Clone)]
pub struct OptCompileRequest {
    /// Function name.
    pub name: String,
    /// IR instruction count.
    pub ir_size: usize,
    /// Inlining candidates from call graph.
    pub inline_candidates: Vec<InlineCandidate>,
    /// Type profile data for speculation.
    pub type_profiles: HashMap<usize, String>,
    /// Whether speculative optimization is enabled.
    pub speculate: bool,
}

/// A candidate for inlining.
#[derive(Debug, Clone)]
pub struct InlineCandidate {
    /// Callee function name.
    pub callee: String,
    /// Callee IR size.
    pub callee_ir_size: usize,
    /// Call frequency (from profiler).
    pub call_count: u64,
    /// Whether inlining is profitable.
    pub profitable: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// S15.2: Inlining Pass
// ═══════════════════════════════════════════════════════════════════════

/// Maximum IR size for inlining a callee.
pub const INLINE_SIZE_LIMIT: usize = 30;

/// Inlining decision for a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineDecision {
    /// Inline the call.
    Inline,
    /// Do not inline — callee too large.
    TooLarge,
    /// Do not inline — recursive call.
    Recursive,
    /// Do not inline — not hot enough.
    NotHot,
}

impl fmt::Display for InlineDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InlineDecision::Inline => write!(f, "inline"),
            InlineDecision::TooLarge => write!(f, "too-large"),
            InlineDecision::Recursive => write!(f, "recursive"),
            InlineDecision::NotHot => write!(f, "not-hot"),
        }
    }
}

/// Decides whether to inline a call site.
pub fn decide_inline(
    caller: &str,
    candidate: &InlineCandidate,
    min_call_count: u64,
) -> InlineDecision {
    if candidate.callee == caller {
        return InlineDecision::Recursive;
    }
    if candidate.callee_ir_size > INLINE_SIZE_LIMIT {
        return InlineDecision::TooLarge;
    }
    if candidate.call_count < min_call_count {
        return InlineDecision::NotHot;
    }
    InlineDecision::Inline
}

// ═══════════════════════════════════════════════════════════════════════
// S15.3: CSE in JIT
// ═══════════════════════════════════════════════════════════════════════

/// A common subexpression entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CseKey {
    /// Operation kind.
    pub op: String,
    /// Operand identifiers.
    pub operands: Vec<String>,
}

/// CSE optimization pass results.
#[derive(Debug, Clone)]
pub struct CseResult {
    /// Number of eliminated expressions.
    pub eliminated: usize,
    /// Estimated instruction reduction.
    pub instructions_saved: usize,
}

/// Performs CSE on a set of expressions (simplified).
pub fn cse_pass(expressions: &[(CseKey, String)]) -> CseResult {
    let mut seen: HashMap<&CseKey, usize> = HashMap::new();
    let mut eliminated = 0;
    for (key, _value_name) in expressions {
        let count = seen.entry(key).or_insert(0);
        if *count > 0 {
            eliminated += 1;
        }
        *count += 1;
    }
    CseResult {
        eliminated,
        instructions_saved: eliminated * 2, // ~2 instructions per eliminated expression
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S15.4: DCE in JIT
// ═══════════════════════════════════════════════════════════════════════

/// DCE optimization results.
#[derive(Debug, Clone)]
pub struct DceResult {
    /// Number of dead instructions removed.
    pub removed: usize,
    /// Dead basic blocks eliminated.
    pub dead_blocks: usize,
}

/// Performs DCE based on type profile data.
pub fn dce_pass(
    total_instructions: usize,
    unreachable_branches: usize,
    avg_branch_size: usize,
) -> DceResult {
    let removed = unreachable_branches * avg_branch_size;
    DceResult {
        removed: removed.min(total_instructions),
        dead_blocks: unreachable_branches,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S15.5: LICM in JIT
// ═══════════════════════════════════════════════════════════════════════

/// LICM optimization results.
#[derive(Debug, Clone)]
pub struct LicmResult {
    /// Number of invariant instructions hoisted.
    pub hoisted: usize,
    /// Loops processed.
    pub loops_processed: usize,
}

/// Performs LICM on loop bodies.
pub fn licm_pass(loop_count: usize, invariant_instructions_per_loop: &[usize]) -> LicmResult {
    let total_hoisted: usize = invariant_instructions_per_loop.iter().sum();
    LicmResult {
        hoisted: total_hoisted,
        loops_processed: loop_count,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S15.6: Speculative Optimization
// ═══════════════════════════════════════════════════════════════════════

/// A type guard inserted for speculative optimization.
#[derive(Debug, Clone)]
pub struct TypeGuard {
    /// Parameter index being guarded.
    pub param_index: usize,
    /// Expected type.
    pub expected_type: String,
    /// Deoptimization target on failure.
    pub deopt_target: DeoptTarget,
}

/// Where to deoptimize to on guard failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeoptTarget {
    /// Fall back to baseline JIT code.
    BaselineJIT,
    /// Fall back to interpreter.
    Interpreter,
}

impl fmt::Display for DeoptTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeoptTarget::BaselineJIT => write!(f, "baseline-jit"),
            DeoptTarget::Interpreter => write!(f, "interpreter"),
        }
    }
}

/// Creates type guards from profiling data.
pub fn create_type_guards(
    type_profiles: &HashMap<usize, String>,
    deopt_target: DeoptTarget,
) -> Vec<TypeGuard> {
    type_profiles
        .iter()
        .map(|(idx, ty)| TypeGuard {
            param_index: *idx,
            expected_type: ty.clone(),
            deopt_target,
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S15.7: Guard Failure Handling
// ═══════════════════════════════════════════════════════════════════════

/// Guard failure event.
#[derive(Debug, Clone)]
pub struct GuardFailure {
    /// Function name.
    pub function: String,
    /// Guard that failed.
    pub guard_index: usize,
    /// Actual type observed.
    pub actual_type: String,
    /// Expected type.
    pub expected_type: String,
}

/// Tracks guard failures for recompilation decisions.
#[derive(Debug, Clone, Default)]
pub struct GuardFailureTracker {
    /// All failures.
    pub failures: Vec<GuardFailure>,
    /// Failure counts per function.
    pub failure_counts: HashMap<String, u64>,
}

impl GuardFailureTracker {
    /// Creates a new tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a guard failure.
    pub fn record(&mut self, failure: GuardFailure) {
        *self
            .failure_counts
            .entry(failure.function.clone())
            .or_insert(0) += 1;
        self.failures.push(failure);
    }

    /// Whether a function should be recompiled (too many guard failures).
    pub fn should_recompile(&self, function: &str, threshold: u64) -> bool {
        self.failure_counts
            .get(function)
            .is_some_and(|c| *c >= threshold)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S15.8: Code Replacement
// ═══════════════════════════════════════════════════════════════════════

/// Code replacement event.
#[derive(Debug, Clone)]
pub struct CodeReplacement {
    /// Function name.
    pub function: String,
    /// Previous tier.
    pub from_tier: ExecutionTier,
    /// New tier.
    pub to_tier: ExecutionTier,
    /// Old code size.
    pub old_size: usize,
    /// New code size.
    pub new_size: usize,
}

// ═══════════════════════════════════════════════════════════════════════
// S15.9: Optimization Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Metrics for an optimizing compilation.
#[derive(Debug, Clone)]
pub struct OptMetrics {
    /// Function name.
    pub function: String,
    /// Compilation time.
    pub compile_time: Duration,
    /// Speedup ratio over baseline (estimated).
    pub estimated_speedup: f64,
    /// Code size before optimization.
    pub baseline_size: usize,
    /// Code size after optimization.
    pub optimized_size: usize,
    /// Number of inlined call sites.
    pub inlined_calls: usize,
    /// CSE eliminations.
    pub cse_eliminations: usize,
    /// DCE removals.
    pub dce_removals: usize,
    /// LICM hoists.
    pub licm_hoists: usize,
    /// Type guards inserted.
    pub type_guards: usize,
}

impl OptMetrics {
    /// Code size delta (positive = larger, negative = smaller).
    pub fn size_delta(&self) -> i64 {
        self.optimized_size as i64 - self.baseline_size as i64
    }

    /// Code size reduction percentage.
    pub fn size_reduction_pct(&self) -> f64 {
        if self.baseline_size == 0 {
            return 0.0;
        }
        ((self.baseline_size as f64 - self.optimized_size as f64) / self.baseline_size as f64)
            * 100.0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S15.1 — Optimizing Compiler Entry
    #[test]
    fn s15_1_opt_compile_request() {
        let req = OptCompileRequest {
            name: "hot_fn".into(),
            ir_size: 200,
            inline_candidates: vec![],
            type_profiles: HashMap::new(),
            speculate: true,
        };
        assert!(req.speculate);
        assert_eq!(req.ir_size, 200);
    }

    // S15.2 — Inlining
    #[test]
    fn s15_2_inline_small_function() {
        let candidate = InlineCandidate {
            callee: "helper".into(),
            callee_ir_size: 20,
            call_count: 500,
            profitable: true,
        };
        assert_eq!(
            decide_inline("main", &candidate, 100),
            InlineDecision::Inline
        );
    }

    #[test]
    fn s15_2_no_inline_large() {
        let candidate = InlineCandidate {
            callee: "big_fn".into(),
            callee_ir_size: 100,
            call_count: 1000,
            profitable: true,
        };
        assert_eq!(
            decide_inline("main", &candidate, 100),
            InlineDecision::TooLarge
        );
    }

    #[test]
    fn s15_2_no_inline_recursive() {
        let candidate = InlineCandidate {
            callee: "fib".into(),
            callee_ir_size: 10,
            call_count: 10000,
            profitable: true,
        };
        assert_eq!(
            decide_inline("fib", &candidate, 100),
            InlineDecision::Recursive
        );
    }

    #[test]
    fn s15_2_no_inline_cold() {
        let candidate = InlineCandidate {
            callee: "cold".into(),
            callee_ir_size: 5,
            call_count: 2,
            profitable: false,
        };
        assert_eq!(
            decide_inline("main", &candidate, 100),
            InlineDecision::NotHot
        );
    }

    // S15.3 — CSE
    #[test]
    fn s15_3_cse_eliminates_duplicates() {
        let key = CseKey {
            op: "add".into(),
            operands: vec!["a".into(), "b".into()],
        };
        let exprs = vec![
            (key.clone(), "v0".into()),
            (key.clone(), "v1".into()),
            (key, "v2".into()),
        ];
        let result = cse_pass(&exprs);
        assert_eq!(result.eliminated, 2);
        assert_eq!(result.instructions_saved, 4);
    }

    #[test]
    fn s15_3_cse_no_duplicates() {
        let k1 = CseKey {
            op: "add".into(),
            operands: vec!["a".into(), "b".into()],
        };
        let k2 = CseKey {
            op: "mul".into(),
            operands: vec!["c".into(), "d".into()],
        };
        let exprs = vec![(k1, "v0".into()), (k2, "v1".into())];
        let result = cse_pass(&exprs);
        assert_eq!(result.eliminated, 0);
    }

    // S15.4 — DCE
    #[test]
    fn s15_4_dce_removes_dead_branches() {
        let result = dce_pass(100, 3, 5);
        assert_eq!(result.removed, 15);
        assert_eq!(result.dead_blocks, 3);
    }

    // S15.5 — LICM
    #[test]
    fn s15_5_licm_hoists_invariants() {
        let result = licm_pass(3, &[5, 2, 8]);
        assert_eq!(result.hoisted, 15);
        assert_eq!(result.loops_processed, 3);
    }

    // S15.6 — Speculative Optimization
    #[test]
    fn s15_6_type_guards() {
        let mut profiles = HashMap::new();
        profiles.insert(0, "i64".into());
        profiles.insert(1, "f64".into());
        let guards = create_type_guards(&profiles, DeoptTarget::BaselineJIT);
        assert_eq!(guards.len(), 2);
    }

    #[test]
    fn s15_6_deopt_target_display() {
        assert_eq!(DeoptTarget::BaselineJIT.to_string(), "baseline-jit");
        assert_eq!(DeoptTarget::Interpreter.to_string(), "interpreter");
    }

    // S15.7 — Guard Failure
    #[test]
    fn s15_7_guard_failure_tracking() {
        let mut tracker = GuardFailureTracker::new();
        for _ in 0..5 {
            tracker.record(GuardFailure {
                function: "hot".into(),
                guard_index: 0,
                actual_type: "str".into(),
                expected_type: "i64".into(),
            });
        }
        assert!(tracker.should_recompile("hot", 5));
        assert!(!tracker.should_recompile("hot", 10));
        assert!(!tracker.should_recompile("cold", 1));
    }

    // S15.8 — Code Replacement
    #[test]
    fn s15_8_code_replacement_event() {
        let event = CodeReplacement {
            function: "foo".into(),
            from_tier: ExecutionTier::BaselineJIT,
            to_tier: ExecutionTier::OptimizingJIT,
            old_size: 400,
            new_size: 300,
        };
        assert!(event.new_size < event.old_size);
    }

    // S15.9 — Opt Metrics
    #[test]
    fn s15_9_metrics_size_delta() {
        let metrics = OptMetrics {
            function: "hot".into(),
            compile_time: Duration::from_millis(50),
            estimated_speedup: 3.2,
            baseline_size: 400,
            optimized_size: 300,
            inlined_calls: 2,
            cse_eliminations: 5,
            dce_removals: 10,
            licm_hoists: 3,
            type_guards: 1,
        };
        assert_eq!(metrics.size_delta(), -100);
        assert!((metrics.size_reduction_pct() - 25.0).abs() < 0.1);
    }

    // S15.10 — Additional
    #[test]
    fn s15_10_inline_decision_display() {
        assert_eq!(InlineDecision::Inline.to_string(), "inline");
        assert_eq!(InlineDecision::TooLarge.to_string(), "too-large");
        assert_eq!(InlineDecision::Recursive.to_string(), "recursive");
    }

    #[test]
    fn s15_10_empty_cse() {
        let result = cse_pass(&[]);
        assert_eq!(result.eliminated, 0);
    }
}
