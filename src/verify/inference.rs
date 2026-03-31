//! Automated Property Inference — Sprint V6 (10 tasks).
//!
//! Automatically infers safety properties from Fajar Lang code without manual
//! annotations. Covers null safety, bounds checking, overflow detection,
//! division safety, resource cleanup, unreachable code, type narrowing,
//! pattern exhaustiveness, and purity inference.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V6.1: Inferred Property Types
// ═══════════════════════════════════════════════════════════════════════

/// A property automatically inferred from code analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct InferredProperty {
    /// Unique identifier for this property.
    pub id: u64,
    /// The kind of inferred property.
    pub kind: InferredPropertyKind,
    /// Confidence level (0.0 = speculative, 1.0 = certain).
    pub confidence: f64,
    /// Human-readable description.
    pub description: String,
    /// Source file.
    pub file: String,
    /// Source line.
    pub line: u32,
    /// The function this property applies to.
    pub function: String,
    /// Whether the property has been verified by SMT solver.
    pub verified: bool,
}

/// Kinds of automatically inferred properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InferredPropertyKind {
    /// Variable is never null (Option is always Some).
    NullSafety,
    /// Array index is always within bounds.
    BoundsCheck,
    /// Arithmetic operation does not overflow.
    NoOverflow,
    /// Divisor is never zero.
    DivisionSafety,
    /// Resource (file, lock, connection) is always cleaned up.
    ResourceCleanup,
    /// Code path is unreachable.
    Unreachable,
    /// Type is narrowed after a conditional check.
    TypeNarrowing,
    /// Match/switch is exhaustive over all variants.
    PatternExhaustiveness,
    /// Function is pure (no side effects).
    Purity,
    /// Loop terminates.
    Termination,
}

impl fmt::Display for InferredPropertyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NullSafety => write!(f, "null-safety"),
            Self::BoundsCheck => write!(f, "bounds-check"),
            Self::NoOverflow => write!(f, "no-overflow"),
            Self::DivisionSafety => write!(f, "division-safety"),
            Self::ResourceCleanup => write!(f, "resource-cleanup"),
            Self::Unreachable => write!(f, "unreachable"),
            Self::TypeNarrowing => write!(f, "type-narrowing"),
            Self::PatternExhaustiveness => write!(f, "pattern-exhaustiveness"),
            Self::Purity => write!(f, "purity"),
            Self::Termination => write!(f, "termination"),
        }
    }
}

impl fmt::Display for InferredProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.verified { "verified" } else { "inferred" };
        write!(
            f,
            "[{status}] {}:{} in {}: {} ({}, confidence={:.0}%)",
            self.file,
            self.line,
            self.function,
            self.description,
            self.kind,
            self.confidence * 100.0,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V6.2: Null Safety Inference
// ═══════════════════════════════════════════════════════════════════════

/// Represents a variable's null status in dataflow analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullStatus {
    /// Definitely not null (e.g., after `if x != null`).
    NonNull,
    /// Possibly null (default for Option types).
    MaybeNull,
    /// Definitely null (e.g., assigned `None`).
    Null,
    /// Unknown (not yet analyzed).
    Unknown,
}

/// Infers null safety for variables used without null checks.
pub fn infer_null_safety(
    var_name: &str,
    is_option_type: bool,
    has_null_check: bool,
    has_unwrap: bool,
    file: &str,
    line: u32,
    function: &str,
) -> Option<InferredProperty> {
    if !is_option_type {
        return None;
    }

    if has_null_check {
        return Some(InferredProperty {
            id: 0,
            kind: InferredPropertyKind::NullSafety,
            confidence: 1.0,
            description: format!("{var_name} is guarded by null check"),
            file: file.to_string(),
            line,
            function: function.to_string(),
            verified: false,
        });
    }

    if has_unwrap {
        return Some(InferredProperty {
            id: 0,
            kind: InferredPropertyKind::NullSafety,
            confidence: 0.3,
            description: format!("{var_name} is unwrapped without null check — potential panic"),
            file: file.to_string(),
            line,
            function: function.to_string(),
            verified: false,
        });
    }

    None
}

// ═══════════════════════════════════════════════════════════════════════
// V6.3: Bounds Check Inference
// ═══════════════════════════════════════════════════════════════════════

/// Represents known bounds for an integer variable.
#[derive(Debug, Clone, PartialEq)]
pub struct IntegerBounds {
    /// Variable name.
    pub name: String,
    /// Lower bound (inclusive).
    pub min: Option<i64>,
    /// Upper bound (inclusive).
    pub max: Option<i64>,
}

impl IntegerBounds {
    /// Creates unbounded.
    pub fn unbounded(name: &str) -> Self {
        Self {
            name: name.to_string(),
            min: None,
            max: None,
        }
    }

    /// Creates with known range.
    pub fn bounded(name: &str, min: i64, max: i64) -> Self {
        Self {
            name: name.to_string(),
            min: Some(min),
            max: Some(max),
        }
    }

    /// Returns true if the index is provably within [0, array_len).
    pub fn is_safe_index(&self, array_len: usize) -> bool {
        match (self.min, self.max) {
            (Some(min), Some(max)) => min >= 0 && (max as usize) < array_len,
            _ => false,
        }
    }
}

/// Infers bounds safety for an array access.
pub fn infer_bounds_check(
    index_bounds: &IntegerBounds,
    array_len: usize,
    file: &str,
    line: u32,
    function: &str,
) -> InferredProperty {
    let safe = index_bounds.is_safe_index(array_len);
    let confidence = if safe { 1.0 } else { 0.5 };

    InferredProperty {
        id: 0,
        kind: InferredPropertyKind::BoundsCheck,
        confidence,
        description: if safe {
            format!(
                "index {} is always in [0, {})",
                index_bounds.name, array_len
            )
        } else {
            format!(
                "index {} may be out of bounds for array of length {}",
                index_bounds.name, array_len
            )
        },
        file: file.to_string(),
        line,
        function: function.to_string(),
        verified: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V6.4: Overflow Inference
// ═══════════════════════════════════════════════════════════════════════

/// Integer type with its range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

impl IntType {
    /// Returns the (min, max) range for this type.
    pub fn range(self) -> (i128, i128) {
        match self {
            Self::I8 => (i8::MIN as i128, i8::MAX as i128),
            Self::I16 => (i16::MIN as i128, i16::MAX as i128),
            Self::I32 => (i32::MIN as i128, i32::MAX as i128),
            Self::I64 => (i64::MIN as i128, i64::MAX as i128),
            Self::U8 => (0, u8::MAX as i128),
            Self::U16 => (0, u16::MAX as i128),
            Self::U32 => (0, u32::MAX as i128),
            Self::U64 => (0, u64::MAX as i128),
        }
    }

    /// Returns the bit width.
    pub fn bits(self) -> u32 {
        match self {
            Self::I8 | Self::U8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::I32 | Self::U32 => 32,
            Self::I64 | Self::U64 => 64,
        }
    }
}

impl fmt::Display for IntType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I8 => write!(f, "i8"),
            Self::I16 => write!(f, "i16"),
            Self::I32 => write!(f, "i32"),
            Self::I64 => write!(f, "i64"),
            Self::U8 => write!(f, "u8"),
            Self::U16 => write!(f, "u16"),
            Self::U32 => write!(f, "u32"),
            Self::U64 => write!(f, "u64"),
        }
    }
}

/// Arithmetic operation for overflow analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
}

impl fmt::Display for ArithOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => write!(f, "+"),
            Self::Sub => write!(f, "-"),
            Self::Mul => write!(f, "*"),
        }
    }
}

/// Infers whether an arithmetic operation can overflow.
pub fn infer_overflow(
    op: ArithOp,
    lhs_bounds: &IntegerBounds,
    rhs_bounds: &IntegerBounds,
    int_type: IntType,
    file: &str,
    line: u32,
    function: &str,
) -> InferredProperty {
    let (type_min, type_max) = int_type.range();

    let can_overflow = match (lhs_bounds.min, lhs_bounds.max, rhs_bounds.min, rhs_bounds.max) {
        (Some(a_min), Some(a_max), Some(b_min), Some(b_max)) => {
            let (result_min, result_max) = match op {
                ArithOp::Add => (a_min as i128 + b_min as i128, a_max as i128 + b_max as i128),
                ArithOp::Sub => (a_min as i128 - b_max as i128, a_max as i128 - b_min as i128),
                ArithOp::Mul => {
                    let products = [
                        a_min as i128 * b_min as i128,
                        a_min as i128 * b_max as i128,
                        a_max as i128 * b_min as i128,
                        a_max as i128 * b_max as i128,
                    ];
                    let min_p = products.iter().copied().min().unwrap_or(0);
                    let max_p = products.iter().copied().max().unwrap_or(0);
                    (min_p, max_p)
                }
            };
            result_min < type_min || result_max > type_max
        }
        _ => true, // unknown bounds, conservatively assume overflow possible
    };

    let confidence = if can_overflow { 0.6 } else { 1.0 };

    InferredProperty {
        id: 0,
        kind: InferredPropertyKind::NoOverflow,
        confidence,
        description: if can_overflow {
            format!(
                "{} {} {} may overflow {} range",
                lhs_bounds.name, op, rhs_bounds.name, int_type
            )
        } else {
            format!(
                "{} {} {} is safe within {} range",
                lhs_bounds.name, op, rhs_bounds.name, int_type
            )
        },
        file: file.to_string(),
        line,
        function: function.to_string(),
        verified: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V6.5: Division Safety Inference
// ═══════════════════════════════════════════════════════════════════════

/// Infers whether a division is safe (divisor is never zero).
pub fn infer_division_safety(
    divisor_bounds: &IntegerBounds,
    file: &str,
    line: u32,
    function: &str,
) -> InferredProperty {
    let safe = match (divisor_bounds.min, divisor_bounds.max) {
        (Some(min), Some(max)) => min > 0 || max < 0,
        (Some(min), None) => min > 0,
        (None, Some(max)) => max < 0,
        _ => false,
    };

    InferredProperty {
        id: 0,
        kind: InferredPropertyKind::DivisionSafety,
        confidence: if safe { 1.0 } else { 0.4 },
        description: if safe {
            format!("divisor {} is never zero", divisor_bounds.name)
        } else {
            format!("divisor {} may be zero", divisor_bounds.name)
        },
        file: file.to_string(),
        line,
        function: function.to_string(),
        verified: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V6.6: Resource Cleanup Inference
// ═══════════════════════════════════════════════════════════════════════

/// A tracked resource (file handle, mutex guard, connection, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedResource {
    /// Resource name/variable.
    pub name: String,
    /// Resource type.
    pub resource_type: ResourceType,
    /// Line where the resource is acquired.
    pub acquire_line: u32,
    /// Line where the resource is released (None = not released).
    pub release_line: Option<u32>,
}

/// Types of trackable resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    FileHandle,
    MutexGuard,
    Connection,
    Allocation,
    TempFile,
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileHandle => write!(f, "file handle"),
            Self::MutexGuard => write!(f, "mutex guard"),
            Self::Connection => write!(f, "connection"),
            Self::Allocation => write!(f, "allocation"),
            Self::TempFile => write!(f, "temp file"),
        }
    }
}

/// Infers resource cleanup properties.
pub fn infer_resource_cleanup(
    resource: &TrackedResource,
    has_drop_impl: bool,
    file: &str,
    function: &str,
) -> InferredProperty {
    let cleaned = resource.release_line.is_some() || has_drop_impl;
    InferredProperty {
        id: 0,
        kind: InferredPropertyKind::ResourceCleanup,
        confidence: if cleaned { 0.9 } else { 0.5 },
        description: if cleaned {
            format!(
                "{} ({}) is properly cleaned up",
                resource.name, resource.resource_type
            )
        } else {
            format!(
                "{} ({}) may leak — no release found",
                resource.name, resource.resource_type
            )
        },
        file: file.to_string(),
        line: resource.acquire_line,
        function: function.to_string(),
        verified: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V6.7: Unreachable Code Inference
// ═══════════════════════════════════════════════════════════════════════

/// Reason why code is unreachable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnreachableReason {
    /// After a `return` statement.
    AfterReturn,
    /// After a `break` or `continue`.
    AfterBreakContinue,
    /// After a `panic!()` or `todo!()`.
    AfterDiverge,
    /// Condition is always false.
    DeadBranch(String),
    /// Match arm is impossible (type system guarantees).
    ImpossiblePattern(String),
}

impl fmt::Display for UnreachableReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AfterReturn => write!(f, "after return statement"),
            Self::AfterBreakContinue => write!(f, "after break/continue"),
            Self::AfterDiverge => write!(f, "after diverging expression"),
            Self::DeadBranch(cond) => write!(f, "condition '{cond}' is always false"),
            Self::ImpossiblePattern(pat) => write!(f, "pattern '{pat}' is impossible"),
        }
    }
}

/// Infers that code is unreachable.
pub fn infer_unreachable(
    reason: UnreachableReason,
    file: &str,
    line: u32,
    function: &str,
) -> InferredProperty {
    let confidence = match &reason {
        UnreachableReason::AfterReturn
        | UnreachableReason::AfterBreakContinue
        | UnreachableReason::AfterDiverge => 1.0,
        UnreachableReason::DeadBranch(_) => 0.8,
        UnreachableReason::ImpossiblePattern(_) => 0.9,
    };

    InferredProperty {
        id: 0,
        kind: InferredPropertyKind::Unreachable,
        confidence,
        description: format!("code is unreachable: {reason}"),
        file: file.to_string(),
        line,
        function: function.to_string(),
        verified: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V6.8: Type Narrowing Inference
// ═══════════════════════════════════════════════════════════════════════

/// A type narrowing event (e.g., after `if x is Some`).
#[derive(Debug, Clone, PartialEq)]
pub struct TypeNarrowingEvent {
    /// Variable being narrowed.
    pub variable: String,
    /// Original type.
    pub original_type: String,
    /// Narrowed type.
    pub narrowed_type: String,
    /// Condition that causes narrowing.
    pub condition: String,
}

/// Infers type narrowing after conditional checks.
pub fn infer_type_narrowing(
    event: &TypeNarrowingEvent,
    file: &str,
    line: u32,
    function: &str,
) -> InferredProperty {
    InferredProperty {
        id: 0,
        kind: InferredPropertyKind::TypeNarrowing,
        confidence: 1.0,
        description: format!(
            "{} narrowed from {} to {} after '{}'",
            event.variable, event.original_type, event.narrowed_type, event.condition
        ),
        file: file.to_string(),
        line,
        function: function.to_string(),
        verified: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V6.9: Pattern Exhaustiveness Inference
// ═══════════════════════════════════════════════════════════════════════

/// Result of pattern exhaustiveness analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct ExhaustivenessResult {
    /// Whether the match is exhaustive.
    pub is_exhaustive: bool,
    /// Missing patterns (if not exhaustive).
    pub missing_patterns: Vec<String>,
    /// Total variant count.
    pub total_variants: usize,
    /// Covered variant count.
    pub covered_variants: usize,
}

/// Infers pattern exhaustiveness for a match expression.
pub fn infer_pattern_exhaustiveness(
    match_var: &str,
    all_variants: &[String],
    covered_variants: &[String],
    has_wildcard: bool,
    file: &str,
    line: u32,
    function: &str,
) -> (InferredProperty, ExhaustivenessResult) {
    let missing: Vec<String> = if has_wildcard {
        vec![]
    } else {
        all_variants
            .iter()
            .filter(|v| !covered_variants.contains(v))
            .cloned()
            .collect()
    };

    let is_exhaustive = missing.is_empty();
    let confidence = if is_exhaustive { 1.0 } else { 0.9 };

    let prop = InferredProperty {
        id: 0,
        kind: InferredPropertyKind::PatternExhaustiveness,
        confidence,
        description: if is_exhaustive {
            format!("match on {match_var} is exhaustive")
        } else {
            format!(
                "match on {match_var} is not exhaustive — missing: {}",
                missing.join(", ")
            )
        },
        file: file.to_string(),
        line,
        function: function.to_string(),
        verified: false,
    };

    let result = ExhaustivenessResult {
        is_exhaustive,
        missing_patterns: missing,
        total_variants: all_variants.len(),
        covered_variants: covered_variants.len() + if has_wildcard { 1 } else { 0 },
    };

    (prop, result)
}

// ═══════════════════════════════════════════════════════════════════════
// V6.10: Purity Inference
// ═══════════════════════════════════════════════════════════════════════

/// Side effects detected in a function.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SideEffect {
    /// Reads from I/O.
    IoRead,
    /// Writes to I/O.
    IoWrite,
    /// Mutates a global/static variable.
    GlobalMutation(String),
    /// Allocates heap memory.
    HeapAllocation,
    /// Calls an impure function.
    ImpureCall(String),
    /// Modifies mutable reference parameter.
    MutRefWrite(String),
}

impl fmt::Display for SideEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoRead => write!(f, "I/O read"),
            Self::IoWrite => write!(f, "I/O write"),
            Self::GlobalMutation(name) => write!(f, "mutates global '{name}'"),
            Self::HeapAllocation => write!(f, "heap allocation"),
            Self::ImpureCall(name) => write!(f, "calls impure fn '{name}'"),
            Self::MutRefWrite(name) => write!(f, "writes via &mut {name}"),
        }
    }
}

/// Infers function purity.
pub fn infer_purity(
    function_name: &str,
    side_effects: &[SideEffect],
    file: &str,
    line: u32,
) -> InferredProperty {
    let is_pure = side_effects.is_empty();

    InferredProperty {
        id: 0,
        kind: InferredPropertyKind::Purity,
        confidence: if is_pure { 0.95 } else { 1.0 },
        description: if is_pure {
            format!("fn {function_name} is pure (no side effects)")
        } else {
            let effects: Vec<String> = side_effects.iter().map(|e| format!("{e}")).collect();
            format!(
                "fn {function_name} is impure: {}",
                effects.join(", ")
            )
        },
        file: file.to_string(),
        line,
        function: function_name.to_string(),
        verified: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V6 Engine: Inference Engine
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for the inference engine.
#[derive(Debug, Clone)]
pub struct InferenceConfig {
    /// Enable null safety inference.
    pub null_safety: bool,
    /// Enable bounds inference.
    pub bounds_check: bool,
    /// Enable overflow inference.
    pub overflow: bool,
    /// Enable division safety inference.
    pub division_safety: bool,
    /// Enable resource cleanup inference.
    pub resource_cleanup: bool,
    /// Enable unreachable code inference.
    pub unreachable: bool,
    /// Enable type narrowing inference.
    pub type_narrowing: bool,
    /// Enable pattern exhaustiveness inference.
    pub exhaustiveness: bool,
    /// Enable purity inference.
    pub purity: bool,
    /// Minimum confidence threshold (skip below this).
    pub min_confidence: f64,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            null_safety: true,
            bounds_check: true,
            overflow: true,
            division_safety: true,
            resource_cleanup: true,
            unreachable: true,
            type_narrowing: true,
            exhaustiveness: true,
            purity: true,
            min_confidence: 0.3,
        }
    }
}

/// The main inference engine that combines all property inference passes.
#[derive(Debug)]
pub struct InferenceEngine {
    /// Configuration.
    pub config: InferenceConfig,
    /// All inferred properties (accumulated across calls).
    properties: Vec<InferredProperty>,
    /// Next property ID.
    next_id: u64,
    /// Statistics per kind.
    stats: HashMap<InferredPropertyKind, u64>,
}

impl InferenceEngine {
    /// Creates a new inference engine with default configuration.
    pub fn new() -> Self {
        Self {
            config: InferenceConfig::default(),
            properties: Vec::new(),
            next_id: 1,
            stats: HashMap::new(),
        }
    }

    /// Creates a new inference engine with custom configuration.
    pub fn with_config(config: InferenceConfig) -> Self {
        Self {
            config,
            properties: Vec::new(),
            next_id: 1,
            stats: HashMap::new(),
        }
    }

    /// Adds an inferred property, assigning it a unique ID.
    pub fn add_property(&mut self, mut prop: InferredProperty) {
        if prop.confidence < self.config.min_confidence {
            return;
        }
        prop.id = self.next_id;
        self.next_id += 1;
        *self.stats.entry(prop.kind).or_insert(0) += 1;
        self.properties.push(prop);
    }

    /// Returns all inferred properties.
    pub fn infer_all(&self) -> Vec<InferredProperty> {
        self.properties.clone()
    }

    /// Returns properties filtered by kind.
    pub fn properties_by_kind(&self, kind: InferredPropertyKind) -> Vec<&InferredProperty> {
        self.properties.iter().filter(|p| p.kind == kind).collect()
    }

    /// Returns properties for a specific file.
    pub fn properties_for_file(&self, file: &str) -> Vec<&InferredProperty> {
        self.properties.iter().filter(|p| p.file == file).collect()
    }

    /// Returns properties for a specific function.
    pub fn properties_for_function(&self, function: &str) -> Vec<&InferredProperty> {
        self.properties
            .iter()
            .filter(|p| p.function == function)
            .collect()
    }

    /// Returns the total number of inferred properties.
    pub fn total_count(&self) -> usize {
        self.properties.len()
    }

    /// Returns count of high-confidence (>= 0.8) properties.
    pub fn high_confidence_count(&self) -> usize {
        self.properties
            .iter()
            .filter(|p| p.confidence >= 0.8)
            .count()
    }

    /// Returns count per kind.
    pub fn count_by_kind(&self) -> &HashMap<InferredPropertyKind, u64> {
        &self.stats
    }

    /// Marks a property as verified by SMT solver.
    pub fn mark_verified(&mut self, id: u64) -> bool {
        for prop in &mut self.properties {
            if prop.id == id {
                prop.verified = true;
                return true;
            }
        }
        false
    }

    /// Returns summary statistics.
    pub fn summary(&self) -> InferenceSummary {
        let total = self.properties.len() as u64;
        let verified = self.properties.iter().filter(|p| p.verified).count() as u64;
        let high_confidence = self
            .properties
            .iter()
            .filter(|p| p.confidence >= 0.8)
            .count() as u64;
        let potential_issues = self
            .properties
            .iter()
            .filter(|p| p.confidence < 0.6)
            .count() as u64;

        InferenceSummary {
            total,
            verified,
            high_confidence,
            potential_issues,
            by_kind: self.stats.clone(),
        }
    }

    /// Clears all accumulated properties.
    pub fn clear(&mut self) {
        self.properties.clear();
        self.stats.clear();
        self.next_id = 1;
    }
}

impl Default for InferenceEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of inference results.
#[derive(Debug, Clone)]
pub struct InferenceSummary {
    /// Total inferred properties.
    pub total: u64,
    /// Properties verified by SMT.
    pub verified: u64,
    /// High-confidence properties (>= 0.8).
    pub high_confidence: u64,
    /// Potential issues (confidence < 0.6).
    pub potential_issues: u64,
    /// Breakdown by kind.
    pub by_kind: HashMap<InferredPropertyKind, u64>,
}

impl fmt::Display for InferenceSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Inference Summary:")?;
        writeln!(f, "  Total properties:   {}", self.total)?;
        writeln!(f, "  Verified by SMT:    {}", self.verified)?;
        writeln!(f, "  High confidence:    {}", self.high_confidence)?;
        writeln!(f, "  Potential issues:   {}", self.potential_issues)?;
        if !self.by_kind.is_empty() {
            writeln!(f, "  By kind:")?;
            let mut kinds: Vec<_> = self.by_kind.iter().collect();
            kinds.sort_by_key(|(k, _)| format!("{k}"));
            for (kind, count) in kinds {
                writeln!(f, "    {kind}: {count}")?;
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
    fn v6_1_inferred_property_kind_display() {
        assert_eq!(format!("{}", InferredPropertyKind::NullSafety), "null-safety");
        assert_eq!(
            format!("{}", InferredPropertyKind::BoundsCheck),
            "bounds-check"
        );
        assert_eq!(
            format!("{}", InferredPropertyKind::NoOverflow),
            "no-overflow"
        );
        assert_eq!(
            format!("{}", InferredPropertyKind::DivisionSafety),
            "division-safety"
        );
        assert_eq!(
            format!("{}", InferredPropertyKind::ResourceCleanup),
            "resource-cleanup"
        );
        assert_eq!(
            format!("{}", InferredPropertyKind::Purity),
            "purity"
        );
    }

    #[test]
    fn v6_1_inferred_property_display() {
        let prop = InferredProperty {
            id: 1,
            kind: InferredPropertyKind::NullSafety,
            confidence: 0.95,
            description: "x is never null".to_string(),
            file: "main.fj".to_string(),
            line: 10,
            function: "process".to_string(),
            verified: true,
        };
        let s = format!("{prop}");
        assert!(s.contains("verified"));
        assert!(s.contains("main.fj:10"));
        assert!(s.contains("null-safety"));
        assert!(s.contains("95%"));
    }

    #[test]
    fn v6_2_null_safety_with_check() {
        let prop = infer_null_safety("opt_val", true, true, false, "safe.fj", 5, "process");
        assert!(prop.is_some());
        let p = prop.expect("expected property");
        assert_eq!(p.kind, InferredPropertyKind::NullSafety);
        assert!((p.confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v6_2_null_safety_unwrap_without_check() {
        let prop = infer_null_safety("opt_val", true, false, true, "risky.fj", 12, "handle");
        assert!(prop.is_some());
        let p = prop.expect("expected property");
        assert!(p.confidence < 0.5);
        assert!(p.description.contains("panic"));
    }

    #[test]
    fn v6_2_null_safety_non_option() {
        let prop = infer_null_safety("x", false, false, false, "safe.fj", 1, "main");
        assert!(prop.is_none());
    }

    #[test]
    fn v6_3_bounds_safe() {
        let bounds = IntegerBounds::bounded("i", 0, 9);
        let prop = infer_bounds_check(&bounds, 10, "loop.fj", 20, "iterate");
        assert_eq!(prop.kind, InferredPropertyKind::BoundsCheck);
        assert!((prop.confidence - 1.0).abs() < f64::EPSILON);
        assert!(prop.description.contains("always in"));
    }

    #[test]
    fn v6_3_bounds_unsafe() {
        let bounds = IntegerBounds::bounded("i", 0, 15);
        let prop = infer_bounds_check(&bounds, 10, "loop.fj", 20, "iterate");
        assert!(prop.confidence < 1.0);
        assert!(prop.description.contains("out of bounds"));
    }

    #[test]
    fn v6_3_bounds_unbounded() {
        let bounds = IntegerBounds::unbounded("idx");
        assert!(!bounds.is_safe_index(100));
    }

    #[test]
    fn v6_4_overflow_safe_add() {
        let a = IntegerBounds::bounded("a", 0, 100);
        let b = IntegerBounds::bounded("b", 0, 100);
        let prop = infer_overflow(ArithOp::Add, &a, &b, IntType::I32, "math.fj", 5, "calc");
        assert_eq!(prop.kind, InferredPropertyKind::NoOverflow);
        assert!((prop.confidence - 1.0).abs() < f64::EPSILON);
        assert!(prop.description.contains("safe"));
    }

    #[test]
    fn v6_4_overflow_unsafe_mul() {
        let a = IntegerBounds::bounded("a", 0, i32::MAX as i64);
        let b = IntegerBounds::bounded("b", 2, 3);
        let prop = infer_overflow(ArithOp::Mul, &a, &b, IntType::I32, "math.fj", 10, "calc");
        assert!(prop.confidence < 1.0);
        assert!(prop.description.contains("overflow"));
    }

    #[test]
    fn v6_4_int_type_range() {
        let (min, max) = IntType::I8.range();
        assert_eq!(min, -128);
        assert_eq!(max, 127);

        let (min, max) = IntType::U8.range();
        assert_eq!(min, 0);
        assert_eq!(max, 255);
    }

    #[test]
    fn v6_4_int_type_display_and_bits() {
        assert_eq!(format!("{}", IntType::I32), "i32");
        assert_eq!(IntType::I32.bits(), 32);
        assert_eq!(IntType::U64.bits(), 64);
    }

    #[test]
    fn v6_5_division_safe() {
        let divisor = IntegerBounds::bounded("d", 1, 100);
        let prop = infer_division_safety(&divisor, "div.fj", 8, "safe_div");
        assert!((prop.confidence - 1.0).abs() < f64::EPSILON);
        assert!(prop.description.contains("never zero"));
    }

    #[test]
    fn v6_5_division_unsafe() {
        let divisor = IntegerBounds::bounded("d", -5, 5);
        let prop = infer_division_safety(&divisor, "div.fj", 8, "risky_div");
        assert!(prop.confidence < 1.0);
        assert!(prop.description.contains("may be zero"));
    }

    #[test]
    fn v6_6_resource_cleanup_with_release() {
        let resource = TrackedResource {
            name: "file".to_string(),
            resource_type: ResourceType::FileHandle,
            acquire_line: 10,
            release_line: Some(25),
        };
        let prop = infer_resource_cleanup(&resource, false, "io.fj", "read_data");
        assert!(prop.confidence >= 0.9);
        assert!(prop.description.contains("properly cleaned"));
    }

    #[test]
    fn v6_6_resource_leak() {
        let resource = TrackedResource {
            name: "conn".to_string(),
            resource_type: ResourceType::Connection,
            acquire_line: 5,
            release_line: None,
        };
        let prop = infer_resource_cleanup(&resource, false, "net.fj", "connect");
        assert!(prop.confidence < 0.9);
        assert!(prop.description.contains("leak"));
    }

    #[test]
    fn v6_6_resource_type_display() {
        assert_eq!(format!("{}", ResourceType::FileHandle), "file handle");
        assert_eq!(format!("{}", ResourceType::MutexGuard), "mutex guard");
    }

    #[test]
    fn v6_7_unreachable_after_return() {
        let prop = infer_unreachable(
            UnreachableReason::AfterReturn,
            "main.fj",
            15,
            "process",
        );
        assert!((prop.confidence - 1.0).abs() < f64::EPSILON);
        assert!(prop.description.contains("after return"));
    }

    #[test]
    fn v6_7_unreachable_dead_branch() {
        let prop = infer_unreachable(
            UnreachableReason::DeadBranch("false".to_string()),
            "main.fj",
            20,
            "process",
        );
        assert!(prop.confidence < 1.0);
        assert!(prop.confidence >= 0.8);
    }

    #[test]
    fn v6_8_type_narrowing() {
        let event = TypeNarrowingEvent {
            variable: "x".to_string(),
            original_type: "Option<i32>".to_string(),
            narrowed_type: "i32".to_string(),
            condition: "x is Some".to_string(),
        };
        let prop = infer_type_narrowing(&event, "narrow.fj", 30, "handle");
        assert_eq!(prop.kind, InferredPropertyKind::TypeNarrowing);
        assert!(prop.description.contains("narrowed from Option<i32> to i32"));
    }

    #[test]
    fn v6_9_pattern_exhaustive() {
        let all = vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()];
        let covered = vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()];
        let (prop, result) =
            infer_pattern_exhaustiveness("color", &all, &covered, false, "ui.fj", 40, "render");
        assert!(result.is_exhaustive);
        assert!(result.missing_patterns.is_empty());
        assert_eq!(result.total_variants, 3);
        assert!(prop.description.contains("exhaustive"));
    }

    #[test]
    fn v6_9_pattern_not_exhaustive() {
        let all = vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()];
        let covered = vec!["Red".to_string()];
        let (prop, result) =
            infer_pattern_exhaustiveness("color", &all, &covered, false, "ui.fj", 40, "render");
        assert!(!result.is_exhaustive);
        assert_eq!(result.missing_patterns.len(), 2);
        assert!(prop.description.contains("Green"));
        assert!(prop.description.contains("Blue"));
    }

    #[test]
    fn v6_9_pattern_wildcard_makes_exhaustive() {
        let all = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let covered = vec!["A".to_string()];
        let (_, result) =
            infer_pattern_exhaustiveness("x", &all, &covered, true, "m.fj", 50, "dispatch");
        assert!(result.is_exhaustive);
    }

    #[test]
    fn v6_10_purity_pure_function() {
        let prop = infer_purity("add", &[], "math.fj", 1);
        assert_eq!(prop.kind, InferredPropertyKind::Purity);
        assert!(prop.description.contains("pure"));
    }

    #[test]
    fn v6_10_purity_impure_function() {
        let effects = vec![SideEffect::IoWrite, SideEffect::GlobalMutation("counter".to_string())];
        let prop = infer_purity("log", &effects, "io.fj", 5);
        assert!(prop.description.contains("impure"));
        assert!(prop.description.contains("I/O write"));
    }

    #[test]
    fn v6_10_side_effect_display() {
        assert_eq!(format!("{}", SideEffect::IoRead), "I/O read");
        assert_eq!(
            format!("{}", SideEffect::ImpureCall("println".to_string())),
            "calls impure fn 'println'"
        );
    }

    #[test]
    fn v6_engine_new_and_default() {
        let engine = InferenceEngine::new();
        assert_eq!(engine.total_count(), 0);
        assert!(engine.config.null_safety);

        let engine2 = InferenceEngine::default();
        assert_eq!(engine2.total_count(), 0);
    }

    #[test]
    fn v6_engine_add_and_filter() {
        let mut engine = InferenceEngine::new();

        let prop1 = InferredProperty {
            id: 0,
            kind: InferredPropertyKind::NullSafety,
            confidence: 1.0,
            description: "x is non-null".to_string(),
            file: "a.fj".to_string(),
            line: 1,
            function: "foo".to_string(),
            verified: false,
        };
        let prop2 = InferredProperty {
            id: 0,
            kind: InferredPropertyKind::BoundsCheck,
            confidence: 0.8,
            description: "i in bounds".to_string(),
            file: "b.fj".to_string(),
            line: 5,
            function: "bar".to_string(),
            verified: false,
        };
        engine.add_property(prop1);
        engine.add_property(prop2);

        assert_eq!(engine.total_count(), 2);
        assert_eq!(engine.properties_by_kind(InferredPropertyKind::NullSafety).len(), 1);
        assert_eq!(engine.properties_for_file("a.fj").len(), 1);
        assert_eq!(engine.properties_for_function("bar").len(), 1);
    }

    #[test]
    fn v6_engine_mark_verified() {
        let mut engine = InferenceEngine::new();
        engine.add_property(InferredProperty {
            id: 0,
            kind: InferredPropertyKind::Purity,
            confidence: 1.0,
            description: "pure".to_string(),
            file: "a.fj".to_string(),
            line: 1,
            function: "f".to_string(),
            verified: false,
        });

        assert!(engine.mark_verified(1));
        assert!(!engine.mark_verified(999));

        let all = engine.infer_all();
        assert!(all[0].verified);
    }

    #[test]
    fn v6_engine_summary() {
        let mut engine = InferenceEngine::new();
        engine.add_property(InferredProperty {
            id: 0,
            kind: InferredPropertyKind::NullSafety,
            confidence: 1.0,
            description: "safe".to_string(),
            file: "a.fj".to_string(),
            line: 1,
            function: "f".to_string(),
            verified: true,
        });
        engine.add_property(InferredProperty {
            id: 0,
            kind: InferredPropertyKind::BoundsCheck,
            confidence: 0.4,
            description: "maybe".to_string(),
            file: "a.fj".to_string(),
            line: 2,
            function: "f".to_string(),
            verified: false,
        });

        let summary = engine.summary();
        assert_eq!(summary.total, 2);
        assert_eq!(summary.verified, 1);
        assert_eq!(summary.high_confidence, 1);
        assert_eq!(summary.potential_issues, 1);

        let s = format!("{summary}");
        assert!(s.contains("Total properties:   2"));
    }

    #[test]
    fn v6_engine_clear() {
        let mut engine = InferenceEngine::new();
        engine.add_property(InferredProperty {
            id: 0,
            kind: InferredPropertyKind::Purity,
            confidence: 1.0,
            description: "pure".to_string(),
            file: "a.fj".to_string(),
            line: 1,
            function: "f".to_string(),
            verified: false,
        });
        assert_eq!(engine.total_count(), 1);

        engine.clear();
        assert_eq!(engine.total_count(), 0);
    }

    #[test]
    fn v6_engine_min_confidence_filter() {
        let config = InferenceConfig {
            min_confidence: 0.5,
            ..InferenceConfig::default()
        };
        let mut engine = InferenceEngine::with_config(config);

        // This property has confidence 0.3 — should be filtered out
        engine.add_property(InferredProperty {
            id: 0,
            kind: InferredPropertyKind::NullSafety,
            confidence: 0.3,
            description: "low".to_string(),
            file: "a.fj".to_string(),
            line: 1,
            function: "f".to_string(),
            verified: false,
        });
        assert_eq!(engine.total_count(), 0);

        // This property has confidence 0.5 — should pass
        engine.add_property(InferredProperty {
            id: 0,
            kind: InferredPropertyKind::NullSafety,
            confidence: 0.5,
            description: "ok".to_string(),
            file: "a.fj".to_string(),
            line: 2,
            function: "f".to_string(),
            verified: false,
        });
        assert_eq!(engine.total_count(), 1);
    }

    #[test]
    fn v6_engine_high_confidence_count() {
        let mut engine = InferenceEngine::new();
        for conf in [0.3, 0.5, 0.7, 0.8, 0.9, 1.0] {
            engine.add_property(InferredProperty {
                id: 0,
                kind: InferredPropertyKind::Purity,
                confidence: conf,
                description: format!("conf={conf}"),
                file: "a.fj".to_string(),
                line: 1,
                function: "f".to_string(),
                verified: false,
            });
        }
        assert_eq!(engine.high_confidence_count(), 3); // 0.8, 0.9, 1.0
    }

    #[test]
    fn v6_unreachable_reason_display() {
        assert_eq!(
            format!("{}", UnreachableReason::AfterReturn),
            "after return statement"
        );
        assert_eq!(
            format!("{}", UnreachableReason::DeadBranch("x > 100".to_string())),
            "condition 'x > 100' is always false"
        );
        assert_eq!(
            format!("{}", UnreachableReason::ImpossiblePattern("Foo::Bar".to_string())),
            "pattern 'Foo::Bar' is impossible"
        );
    }

    #[test]
    fn v6_arith_op_display() {
        assert_eq!(format!("{}", ArithOp::Add), "+");
        assert_eq!(format!("{}", ArithOp::Sub), "-");
        assert_eq!(format!("{}", ArithOp::Mul), "*");
    }

    #[test]
    fn v6_null_status_variants() {
        assert_ne!(NullStatus::NonNull, NullStatus::MaybeNull);
        assert_ne!(NullStatus::Null, NullStatus::Unknown);
    }
}
