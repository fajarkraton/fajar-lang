//! On-Stack Replacement — OSR entry points, state capture, mid-loop
//! transition, frame construction, deoptimization, nested loops.

use std::collections::HashMap;
use std::fmt;

use super::counters::ExecutionTier;

// ═══════════════════════════════════════════════════════════════════════
// S16.1: OSR Entry Points
// ═══════════════════════════════════════════════════════════════════════

/// An OSR entry point in compiled code.
#[derive(Debug, Clone)]
pub struct OsrEntryPoint {
    /// Unique ID for this entry point.
    pub id: u32,
    /// Loop nesting depth (0 = outermost).
    pub loop_depth: u32,
    /// Offset in compiled code where OSR can enter.
    pub code_offset: usize,
    /// Local variables live at this point.
    pub live_locals: Vec<LiveLocal>,
}

/// A local variable that is live at an OSR point.
#[derive(Debug, Clone)]
pub struct LiveLocal {
    /// Local variable index.
    pub index: usize,
    /// Variable name (for debugging).
    pub name: String,
    /// Type tag.
    pub type_tag: OsrType,
}

/// Type of a value at an OSR point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsrType {
    /// 64-bit integer.
    Int,
    /// 64-bit float.
    Float,
    /// Boolean.
    Bool,
    /// Pointer (string, array, struct, etc.).
    Ptr,
}

impl fmt::Display for OsrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OsrType::Int => write!(f, "i64"),
            OsrType::Float => write!(f, "f64"),
            OsrType::Bool => write!(f, "bool"),
            OsrType::Ptr => write!(f, "ptr"),
        }
    }
}

/// Identifies loop headers as OSR entry points.
pub fn identify_osr_points(
    loop_headers: &[(u32, u32)], // (loop_id, nesting_depth)
    live_locals_per_loop: &HashMap<u32, Vec<LiveLocal>>,
) -> Vec<OsrEntryPoint> {
    loop_headers
        .iter()
        .enumerate()
        .map(|(i, (loop_id, depth))| OsrEntryPoint {
            id: *loop_id,
            loop_depth: *depth,
            code_offset: i * 64, // simplified: 64-byte alignment
            live_locals: live_locals_per_loop
                .get(loop_id)
                .cloned()
                .unwrap_or_default(),
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S16.2: State Capture
// ═══════════════════════════════════════════════════════════════════════

/// Captured interpreter state at an OSR point.
#[derive(Debug, Clone)]
pub struct CapturedState {
    /// Function name.
    pub function: String,
    /// OSR entry point ID.
    pub osr_point_id: u32,
    /// Local variable values (index -> value).
    pub locals: HashMap<usize, CapturedValue>,
    /// Current loop iteration count.
    pub iteration: u64,
}

/// A captured value for OSR state transfer.
#[derive(Debug, Clone, PartialEq)]
pub enum CapturedValue {
    /// Integer value.
    Int(i64),
    /// Float value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// Pointer value (opaque).
    Ptr(u64),
}

impl CapturedValue {
    /// Returns the OSR type of this value.
    pub fn osr_type(&self) -> OsrType {
        match self {
            CapturedValue::Int(_) => OsrType::Int,
            CapturedValue::Float(_) => OsrType::Float,
            CapturedValue::Bool(_) => OsrType::Bool,
            CapturedValue::Ptr(_) => OsrType::Ptr,
        }
    }
}

/// Captures interpreter state for OSR transition.
pub fn capture_state(
    function: &str,
    osr_point_id: u32,
    locals: HashMap<usize, CapturedValue>,
    iteration: u64,
) -> CapturedState {
    CapturedState {
        function: function.into(),
        osr_point_id,
        locals,
        iteration,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S16.3 / S16.4: Mid-Loop Transition & Frame Construction
// ═══════════════════════════════════════════════════════════════════════

/// Result of an OSR transition.
#[derive(Debug, Clone)]
pub enum OsrTransition {
    /// Successfully transitioned to JIT.
    Success {
        function: String,
        from_tier: ExecutionTier,
        to_tier: ExecutionTier,
        locals_transferred: usize,
    },
    /// OSR not possible (e.g., no compiled code).
    NotAvailable { reason: String },
    /// Type mismatch in state transfer.
    TypeMismatch {
        local_index: usize,
        expected: OsrType,
        actual: OsrType,
    },
}

/// Validates that captured state matches the OSR entry point's expectations.
pub fn validate_osr_state(
    state: &CapturedState,
    entry: &OsrEntryPoint,
) -> Result<(), OsrTransition> {
    for live in &entry.live_locals {
        if let Some(value) = state.locals.get(&live.index) {
            if value.osr_type() != live.type_tag {
                return Err(OsrTransition::TypeMismatch {
                    local_index: live.index,
                    expected: live.type_tag,
                    actual: value.osr_type(),
                });
            }
        }
    }
    Ok(())
}

/// Performs an OSR transition from interpreter to JIT.
pub fn osr_enter(state: &CapturedState, entry: &OsrEntryPoint) -> OsrTransition {
    if let Err(e) = validate_osr_state(state, entry) {
        return e;
    }

    OsrTransition::Success {
        function: state.function.clone(),
        from_tier: ExecutionTier::Interpreter,
        to_tier: ExecutionTier::BaselineJIT,
        locals_transferred: state.locals.len(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S16.5 / S16.6: Deoptimization (JIT to Interpreter)
// ═══════════════════════════════════════════════════════════════════════

/// Deoptimization metadata for reconstructing interpreter state.
#[derive(Debug, Clone)]
pub struct DeoptMetadata {
    /// OSR point where deopt occurs.
    pub osr_point_id: u32,
    /// JIT register -> interpreter local index mapping.
    pub register_mapping: Vec<(u32, usize)>,
    /// Stack slot -> interpreter local index mapping.
    pub stack_mapping: Vec<(u32, usize)>,
}

/// Result of deoptimization.
#[derive(Debug, Clone)]
pub struct DeoptResult {
    /// Function being deoptimized.
    pub function: String,
    /// Reason for deoptimization.
    pub reason: DeoptReason,
    /// Reconstructed interpreter state.
    pub restored_locals: usize,
    /// Target tier.
    pub target_tier: ExecutionTier,
}

/// Reason for deoptimization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeoptReason {
    /// Type guard failed.
    TypeGuardFailure { expected: String, actual: String },
    /// Overflow detected.
    Overflow,
    /// Bounds check failed.
    BoundsCheckFailure,
    /// Explicit deopt (e.g., debugging).
    Explicit,
}

impl fmt::Display for DeoptReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeoptReason::TypeGuardFailure { expected, actual } => {
                write!(f, "type guard: expected {expected}, got {actual}")
            }
            DeoptReason::Overflow => write!(f, "overflow"),
            DeoptReason::BoundsCheckFailure => write!(f, "bounds check"),
            DeoptReason::Explicit => write!(f, "explicit"),
        }
    }
}

/// Performs deoptimization from JIT to interpreter.
pub fn deoptimize(function: &str, reason: DeoptReason, metadata: &DeoptMetadata) -> DeoptResult {
    let restored = metadata.register_mapping.len() + metadata.stack_mapping.len();
    DeoptResult {
        function: function.into(),
        reason,
        restored_locals: restored,
        target_tier: ExecutionTier::Interpreter,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S16.7: Nested Loop OSR
// ═══════════════════════════════════════════════════════════════════════

/// Nested loop OSR configuration.
#[derive(Debug, Clone)]
pub struct NestedLoopOsr {
    /// Outer loop OSR point.
    pub outer_osr: u32,
    /// Inner loop OSR point.
    pub inner_osr: u32,
    /// Whether inner loop can be promoted independently.
    pub independent_promotion: bool,
}

/// Checks whether a nested loop can have independent OSR.
pub fn can_independent_osr(
    inner_loop_back_edges: u64,
    outer_loop_back_edges: u64,
    threshold: u64,
) -> bool {
    inner_loop_back_edges >= threshold && outer_loop_back_edges < threshold
}

// ═══════════════════════════════════════════════════════════════════════
// S16.8: OSR Threshold
// ═══════════════════════════════════════════════════════════════════════

/// Checks if OSR should be triggered for a loop.
pub fn should_trigger_osr(
    back_edge_count: u64,
    osr_threshold: u64,
    already_compiled: bool,
) -> bool {
    !already_compiled && back_edge_count >= osr_threshold
}

// ═══════════════════════════════════════════════════════════════════════
// S16.9: Performance Validation
// ═══════════════════════════════════════════════════════════════════════

/// OSR performance measurement.
#[derive(Debug, Clone)]
pub struct OsrBenchmark {
    /// Function name.
    pub function: String,
    /// Iterations before OSR.
    pub iterations_before_osr: u64,
    /// Time per iteration before OSR (interpreter).
    pub pre_osr_iter_ns: u64,
    /// Time per iteration after OSR (JIT).
    pub post_osr_iter_ns: u64,
}

impl OsrBenchmark {
    /// Speedup factor from OSR.
    pub fn speedup(&self) -> f64 {
        if self.post_osr_iter_ns == 0 {
            return f64::INFINITY;
        }
        self.pre_osr_iter_ns as f64 / self.post_osr_iter_ns as f64
    }

    /// Whether the OSR transition improved performance.
    pub fn is_beneficial(&self) -> bool {
        self.post_osr_iter_ns < self.pre_osr_iter_ns
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S16.1 — OSR Entry Points
    #[test]
    fn s16_1_identify_osr_points() {
        let headers = vec![(0, 0), (1, 1)];
        let mut locals_map = HashMap::new();
        locals_map.insert(
            0,
            vec![LiveLocal {
                index: 0,
                name: "i".into(),
                type_tag: OsrType::Int,
            }],
        );
        let points = identify_osr_points(&headers, &locals_map);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].loop_depth, 0);
        assert_eq!(points[1].loop_depth, 1);
        assert_eq!(points[0].live_locals.len(), 1);
    }

    #[test]
    fn s16_1_osr_type_display() {
        assert_eq!(OsrType::Int.to_string(), "i64");
        assert_eq!(OsrType::Float.to_string(), "f64");
        assert_eq!(OsrType::Ptr.to_string(), "ptr");
    }

    // S16.2 — State Capture
    #[test]
    fn s16_2_capture_state() {
        let mut locals = HashMap::new();
        locals.insert(0, CapturedValue::Int(42));
        locals.insert(1, CapturedValue::Float(3.14));
        let state = capture_state("loop_fn", 0, locals, 50);
        assert_eq!(state.iteration, 50);
        assert_eq!(state.locals.len(), 2);
    }

    #[test]
    fn s16_2_captured_value_type() {
        assert_eq!(CapturedValue::Int(0).osr_type(), OsrType::Int);
        assert_eq!(CapturedValue::Float(0.0).osr_type(), OsrType::Float);
        assert_eq!(CapturedValue::Bool(true).osr_type(), OsrType::Bool);
        assert_eq!(CapturedValue::Ptr(0).osr_type(), OsrType::Ptr);
    }

    // S16.3 — Mid-Loop Transition
    #[test]
    fn s16_3_successful_osr_enter() {
        let mut locals = HashMap::new();
        locals.insert(0, CapturedValue::Int(10));
        let state = capture_state("f", 0, locals, 100);
        let entry = OsrEntryPoint {
            id: 0,
            loop_depth: 0,
            code_offset: 0,
            live_locals: vec![LiveLocal {
                index: 0,
                name: "i".into(),
                type_tag: OsrType::Int,
            }],
        };
        match osr_enter(&state, &entry) {
            OsrTransition::Success {
                locals_transferred, ..
            } => assert_eq!(locals_transferred, 1),
            _ => panic!("expected success"),
        }
    }

    // S16.4 — Frame Construction (validated via osr_enter)
    #[test]
    fn s16_4_type_mismatch_rejected() {
        let mut locals = HashMap::new();
        locals.insert(0, CapturedValue::Float(1.0)); // Float, but Int expected
        let state = capture_state("f", 0, locals, 0);
        let entry = OsrEntryPoint {
            id: 0,
            loop_depth: 0,
            code_offset: 0,
            live_locals: vec![LiveLocal {
                index: 0,
                name: "i".into(),
                type_tag: OsrType::Int,
            }],
        };
        match osr_enter(&state, &entry) {
            OsrTransition::TypeMismatch {
                expected, actual, ..
            } => {
                assert_eq!(expected, OsrType::Int);
                assert_eq!(actual, OsrType::Float);
            }
            _ => panic!("expected type mismatch"),
        }
    }

    // S16.5 — Deoptimization
    #[test]
    fn s16_5_deoptimize() {
        let metadata = DeoptMetadata {
            osr_point_id: 0,
            register_mapping: vec![(0, 0), (1, 1)],
            stack_mapping: vec![(0, 2)],
        };
        let result = deoptimize(
            "hot_fn",
            DeoptReason::TypeGuardFailure {
                expected: "i64".into(),
                actual: "str".into(),
            },
            &metadata,
        );
        assert_eq!(result.restored_locals, 3);
        assert_eq!(result.target_tier, ExecutionTier::Interpreter);
    }

    // S16.6 — Deopt Metadata
    #[test]
    fn s16_6_deopt_reason_display() {
        let r = DeoptReason::TypeGuardFailure {
            expected: "i64".into(),
            actual: "str".into(),
        };
        assert!(r.to_string().contains("type guard"));
        assert_eq!(DeoptReason::Overflow.to_string(), "overflow");
        assert_eq!(DeoptReason::BoundsCheckFailure.to_string(), "bounds check");
    }

    // S16.7 — Nested Loop OSR
    #[test]
    fn s16_7_independent_inner_loop() {
        assert!(can_independent_osr(1000, 10, 500));
        assert!(!can_independent_osr(100, 10, 500));
        assert!(!can_independent_osr(1000, 1000, 500));
    }

    // S16.8 — OSR Threshold
    #[test]
    fn s16_8_osr_trigger() {
        assert!(should_trigger_osr(500, 500, false));
        assert!(!should_trigger_osr(499, 500, false));
        assert!(!should_trigger_osr(1000, 500, true)); // already compiled
    }

    // S16.9 — Performance Validation
    #[test]
    fn s16_9_osr_benchmark() {
        let bench = OsrBenchmark {
            function: "loop_fn".into(),
            iterations_before_osr: 100,
            pre_osr_iter_ns: 1000,
            post_osr_iter_ns: 100,
        };
        assert!((bench.speedup() - 10.0).abs() < 0.01);
        assert!(bench.is_beneficial());
    }

    #[test]
    fn s16_9_no_benefit() {
        let bench = OsrBenchmark {
            function: "cold".into(),
            iterations_before_osr: 50,
            pre_osr_iter_ns: 100,
            post_osr_iter_ns: 200,
        };
        assert!(!bench.is_beneficial());
    }

    // S16.10 — Additional
    #[test]
    fn s16_10_nested_loop_osr_config() {
        let config = NestedLoopOsr {
            outer_osr: 0,
            inner_osr: 1,
            independent_promotion: true,
        };
        assert!(config.independent_promotion);
    }

    #[test]
    fn s16_10_empty_osr_points() {
        let points = identify_osr_points(&[], &HashMap::new());
        assert!(points.is_empty());
    }

    #[test]
    fn s16_10_validate_matching_state() {
        let state = capture_state("f", 0, HashMap::new(), 0);
        let entry = OsrEntryPoint {
            id: 0,
            loop_depth: 0,
            code_offset: 0,
            live_locals: vec![],
        };
        assert!(validate_osr_state(&state, &entry).is_ok());
    }
}
