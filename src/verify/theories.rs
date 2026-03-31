//! Advanced SMT Theories — Sprint V8 (10 tasks).
//!
//! Bitvector theory (u8-u64 overflow), array theory, floating-point (IEEE 754),
//! string theory, nonlinear arithmetic, separation logic, concurrent theory
//! (locks/atomics), theory combination (Nelson-Oppen), and custom theory plugins.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V8.1: Theory Selection Engine
// ═══════════════════════════════════════════════════════════════════════

/// SMT theory kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TheoryKind {
    /// Bitvectors (fixed-width integer overflow).
    Bitvector,
    /// Arrays (select/store, bounds).
    Array,
    /// IEEE 754 floating-point.
    FloatingPoint,
    /// String constraints (length, substring, regex).
    Strings,
    /// Nonlinear integer/real arithmetic.
    NonlinearArithmetic,
    /// Separation logic (heap reasoning).
    SeparationLogic,
    /// Concurrent theory (locks, atomics, happens-before).
    Concurrent,
    /// Linear integer arithmetic (default).
    LinearArithmetic,
    /// Uninterpreted functions.
    UninterpretedFunctions,
}

impl fmt::Display for TheoryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bitvector => write!(f, "QF_BV"),
            Self::Array => write!(f, "QF_AX"),
            Self::FloatingPoint => write!(f, "QF_FP"),
            Self::Strings => write!(f, "QF_S"),
            Self::NonlinearArithmetic => write!(f, "QF_NIA"),
            Self::SeparationLogic => write!(f, "SL"),
            Self::Concurrent => write!(f, "CONC"),
            Self::LinearArithmetic => write!(f, "QF_LIA"),
            Self::UninterpretedFunctions => write!(f, "QF_UF"),
        }
    }
}

/// The theory engine selects and combines SMT theories.
#[derive(Debug, Clone)]
pub struct SmtTheoryEngine {
    /// Active theories.
    pub active_theories: Vec<TheoryKind>,
    /// Custom theory plugins.
    pub custom_theories: Vec<CustomTheory>,
    /// Theory combination strategy.
    pub combination: CombinationStrategy,
    /// Theory-specific options.
    pub options: HashMap<String, String>,
}

impl SmtTheoryEngine {
    /// Creates a new engine with linear arithmetic only.
    pub fn new() -> Self {
        Self {
            active_theories: vec![TheoryKind::LinearArithmetic],
            custom_theories: Vec::new(),
            combination: CombinationStrategy::NelsonOppen,
            options: HashMap::new(),
        }
    }

    /// Selects theories based on code features present.
    pub fn select_theories(&mut self, features: &CodeFeatures) {
        self.active_theories.clear();

        // Always include linear arithmetic as baseline.
        self.active_theories.push(TheoryKind::LinearArithmetic);

        if features.has_bitvector_ops {
            self.active_theories.push(TheoryKind::Bitvector);
        }
        if features.has_array_access {
            self.active_theories.push(TheoryKind::Array);
        }
        if features.has_float_ops {
            self.active_theories.push(TheoryKind::FloatingPoint);
        }
        if features.has_string_ops {
            self.active_theories.push(TheoryKind::Strings);
        }
        if features.has_nonlinear_ops {
            self.active_theories.push(TheoryKind::NonlinearArithmetic);
        }
        if features.has_heap_ops {
            self.active_theories.push(TheoryKind::SeparationLogic);
        }
        if features.has_concurrency {
            self.active_theories.push(TheoryKind::Concurrent);
        }
        if features.has_function_calls {
            self.active_theories.push(TheoryKind::UninterpretedFunctions);
        }
    }

    /// Returns the combined SMT-LIB2 logic string.
    pub fn combined_logic(&self) -> String {
        if self.active_theories.len() <= 1 {
            return self
                .active_theories
                .first()
                .map(|t| format!("{t}"))
                .unwrap_or_else(|| "QF_LIA".to_string());
        }
        // Multiple theories → use ALL
        "ALL".to_string()
    }

    /// Adds a custom theory plugin.
    pub fn add_custom_theory(&mut self, theory: CustomTheory) {
        self.custom_theories.push(theory);
    }

    /// Sets a theory-specific option.
    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    /// Checks if a specific theory is active.
    pub fn has_theory(&self, kind: TheoryKind) -> bool {
        self.active_theories.contains(&kind)
    }
}

impl Default for SmtTheoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Code features detected that influence theory selection.
#[derive(Debug, Clone, Default)]
pub struct CodeFeatures {
    /// Uses bitvector operations (bit shifts, masks, overflow-sensitive).
    pub has_bitvector_ops: bool,
    /// Uses array/slice indexing.
    pub has_array_access: bool,
    /// Uses floating-point arithmetic.
    pub has_float_ops: bool,
    /// Uses string operations.
    pub has_string_ops: bool,
    /// Uses nonlinear arithmetic (multiplication of variables).
    pub has_nonlinear_ops: bool,
    /// Uses heap allocations (alloc, Box, Vec).
    pub has_heap_ops: bool,
    /// Uses concurrency primitives (mutex, atomic, channel).
    pub has_concurrency: bool,
    /// Calls functions that need uninterpreted function theory.
    pub has_function_calls: bool,
}

/// Theory combination strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombinationStrategy {
    /// Nelson-Oppen combination (standard, correct for convex theories).
    NelsonOppen,
    /// Shostak combination (efficient for specific theory pairs).
    Shostak,
    /// Delayed theory combination (lazy approach).
    Delayed,
}

impl fmt::Display for CombinationStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NelsonOppen => write!(f, "Nelson-Oppen"),
            Self::Shostak => write!(f, "Shostak"),
            Self::Delayed => write!(f, "Delayed"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8.2: Bitvector Theory
// ═══════════════════════════════════════════════════════════════════════

/// Bitvector sort (fixed-width integer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BvSort {
    /// Bit width.
    pub width: u32,
    /// Whether signed.
    pub signed: bool,
}

impl BvSort {
    /// Creates a sort for a Fajar Lang integer type.
    pub fn from_type_name(type_name: &str) -> Option<Self> {
        match type_name {
            "i8" => Some(Self { width: 8, signed: true }),
            "i16" => Some(Self { width: 16, signed: true }),
            "i32" => Some(Self { width: 32, signed: true }),
            "i64" => Some(Self { width: 64, signed: true }),
            "u8" => Some(Self { width: 8, signed: false }),
            "u16" => Some(Self { width: 16, signed: false }),
            "u32" => Some(Self { width: 32, signed: false }),
            "u64" => Some(Self { width: 64, signed: false }),
            _ => None,
        }
    }

    /// Generates SMT-LIB2 sort declaration.
    pub fn to_smtlib2(&self) -> String {
        format!("(_ BitVec {})", self.width)
    }

    /// Generates overflow detection assertion for addition.
    pub fn add_overflow_check(&self, a: &str, b: &str) -> String {
        if self.signed {
            // Signed overflow: (a > 0 && b > 0 && a+b < 0) || (a < 0 && b < 0 && a+b > 0)
            let zero = format!("(_ bv0 {})", self.width);
            format!(
                "(or (and (bvsgt {a} {zero}) (bvsgt {b} {zero}) (bvslt (bvadd {a} {b}) {zero})) \
                     (and (bvslt {a} {zero}) (bvslt {b} {zero}) (bvsgt (bvadd {a} {b}) {zero})))"
            )
        } else {
            // Unsigned overflow: a + b < a
            format!("(bvult (bvadd {a} {b}) {a})")
        }
    }

    /// Generates overflow detection assertion for multiplication.
    pub fn mul_overflow_check(&self, a: &str, b: &str) -> String {
        // Extend to double width, multiply, check if fits in original width.
        let dw = self.width * 2;
        if self.signed {
            format!(
                "(let ((prod (bvmul ((_ sign_extend {ext}) {a}) ((_ sign_extend {ext}) {b})))) \
                 (or (bvslt prod ((_ sign_extend {ext}) (bvneg (_ bv{max_abs} {w})))) \
                     (bvsgt prod ((_ sign_extend {ext}) (_ bv{max} {w})))))",
                ext = self.width,
                w = self.width,
                max = (1u64 << (self.width - 1)) - 1,
                max_abs = 1u64 << (self.width - 1),
            )
        } else {
            format!(
                "(distinct ((_ extract {high} {w}) (bvmul ((_ zero_extend {w}) {a}) ((_ zero_extend {w}) {b}))) (_ bv0 {w}))",
                high = dw - 1,
                w = self.width,
            )
        }
    }
}

impl fmt::Display for BvSort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = if self.signed { "i" } else { "u" };
        write!(f, "{prefix}{}", self.width)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8.3: Array Theory
// ═══════════════════════════════════════════════════════════════════════

/// SMT array sort: (Array IndexSort ElementSort).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArraySort {
    /// Index sort (usually Int or BitVec).
    pub index_sort: String,
    /// Element sort.
    pub element_sort: String,
}

impl ArraySort {
    /// Creates an integer-indexed integer array.
    pub fn int_array() -> Self {
        Self {
            index_sort: "Int".to_string(),
            element_sort: "Int".to_string(),
        }
    }

    /// Creates a bitvector-indexed array.
    pub fn bv_array(index_width: u32, element_width: u32) -> Self {
        Self {
            index_sort: format!("(_ BitVec {index_width})"),
            element_sort: format!("(_ BitVec {element_width})"),
        }
    }

    /// Generates SMT-LIB2 sort.
    pub fn to_smtlib2(&self) -> String {
        format!("(Array {} {})", self.index_sort, self.element_sort)
    }

    /// Generates bounds check assertion: `0 <= idx < size`.
    pub fn bounds_check(&self, _arr: &str, idx: &str, size: &str) -> String {
        format!("(and (>= {idx} 0) (< {idx} {size}))")
    }

    /// Generates a select (read) expression.
    pub fn select(&self, arr: &str, idx: &str) -> String {
        format!("(select {arr} {idx})")
    }

    /// Generates a store (write) expression.
    pub fn store(&self, arr: &str, idx: &str, val: &str) -> String {
        format!("(store {arr} {idx} {val})")
    }
}

impl fmt::Display for ArraySort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(Array {} {})", self.index_sort, self.element_sort)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8.4: Floating-Point Theory (IEEE 754)
// ═══════════════════════════════════════════════════════════════════════

/// IEEE 754 floating-point sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FpSort {
    /// Exponent bits.
    pub exponent_bits: u32,
    /// Significand bits (including hidden bit).
    pub significand_bits: u32,
}

impl FpSort {
    /// IEEE 754 single precision (f32): 8 exponent, 24 significand.
    pub fn float32() -> Self {
        Self {
            exponent_bits: 8,
            significand_bits: 24,
        }
    }

    /// IEEE 754 double precision (f64): 11 exponent, 53 significand.
    pub fn float64() -> Self {
        Self {
            exponent_bits: 11,
            significand_bits: 53,
        }
    }

    /// Generates SMT-LIB2 sort.
    pub fn to_smtlib2(&self) -> String {
        format!(
            "(_ FloatingPoint {} {})",
            self.exponent_bits, self.significand_bits
        )
    }

    /// Generates NaN check.
    pub fn is_nan_check(&self, x: &str) -> String {
        format!("(fp.isNaN {x})")
    }

    /// Generates infinity check.
    pub fn is_infinite_check(&self, x: &str) -> String {
        format!("(fp.isInfinite {x})")
    }

    /// Generates normal number assertion (not NaN, not Inf, not subnormal).
    pub fn is_normal_check(&self, x: &str) -> String {
        format!("(fp.isNormal {x})")
    }

    /// Total bit width.
    pub fn total_bits(&self) -> u32 {
        1 + self.exponent_bits + (self.significand_bits - 1) // sign + exp + mantissa
    }
}

impl fmt::Display for FpSort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total = self.total_bits();
        write!(f, "f{total}")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8.5: String Theory
// ═══════════════════════════════════════════════════════════════════════

/// String constraint for SMT solving.
#[derive(Debug, Clone, PartialEq)]
pub enum StringConstraint {
    /// String length constraint: `len(s) op value`.
    Length(String, StringCompareOp, usize),
    /// String contains substring.
    Contains(String, String),
    /// String prefix.
    Prefix(String, String),
    /// String suffix.
    Suffix(String, String),
    /// String equality.
    Equal(String, String),
    /// String regex match.
    Regex(String, String),
    /// Concatenation result.
    Concat(String, String, String), // a ++ b == result
}

/// Comparison operators for string length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringCompareOp {
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
}

impl fmt::Display for StringCompareOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eq => write!(f, "="),
            Self::Lt => write!(f, "<"),
            Self::Le => write!(f, "<="),
            Self::Gt => write!(f, ">"),
            Self::Ge => write!(f, ">="),
        }
    }
}

impl StringConstraint {
    /// Converts to SMT-LIB2.
    pub fn to_smtlib2(&self) -> String {
        match self {
            Self::Length(s, op, val) => format!("({op} (str.len {s}) {val})"),
            Self::Contains(s, sub) => format!("(str.contains {s} \"{sub}\")"),
            Self::Prefix(s, pre) => format!("(str.prefixof \"{pre}\" {s})"),
            Self::Suffix(s, suf) => format!("(str.suffixof \"{suf}\" {s})"),
            Self::Equal(a, b) => format!("(= {a} {b})"),
            Self::Regex(s, regex) => format!("(str.in_re {s} (re.from_str \"{regex}\"))" ),
            Self::Concat(a, b, result) => format!("(= (str.++ {a} {b}) {result})"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8.6: Nonlinear Arithmetic
// ═══════════════════════════════════════════════════════════════════════

/// A nonlinear arithmetic constraint.
#[derive(Debug, Clone, PartialEq)]
pub enum NonlinearConstraint {
    /// Polynomial: sum of terms, each term is (coefficient, [(variable, exponent)]).
    Polynomial(Vec<PolynomialTerm>),
    /// Absolute value: |x| op value.
    AbsValue(String, StringCompareOp, i64),
    /// Division with remainder: a = q * b + r, 0 <= r < |b|.
    DivMod(String, String, String, String),
}

/// A term in a polynomial.
#[derive(Debug, Clone, PartialEq)]
pub struct PolynomialTerm {
    /// Coefficient.
    pub coefficient: i64,
    /// Variables and their exponents.
    pub variables: Vec<(String, u32)>,
}

impl PolynomialTerm {
    /// Creates a constant term.
    pub fn constant(value: i64) -> Self {
        Self {
            coefficient: value,
            variables: Vec::new(),
        }
    }

    /// Creates a linear term (coefficient * variable).
    pub fn linear(coefficient: i64, variable: &str) -> Self {
        Self {
            coefficient,
            variables: vec![(variable.to_string(), 1)],
        }
    }

    /// Creates a quadratic term (coefficient * variable^2).
    pub fn quadratic(coefficient: i64, variable: &str) -> Self {
        Self {
            coefficient,
            variables: vec![(variable.to_string(), 2)],
        }
    }

    /// Converts to SMT-LIB2.
    pub fn to_smtlib2(&self) -> String {
        if self.variables.is_empty() {
            return format!("{}", self.coefficient);
        }
        let mut parts = Vec::new();
        if self.coefficient != 1 {
            parts.push(format!("{}", self.coefficient));
        }
        for (var, exp) in &self.variables {
            if *exp == 1 {
                parts.push(var.clone());
            } else {
                // x^n = (* x x ... x) n times
                let mut mul = var.clone();
                for _ in 1..*exp {
                    mul = format!("(* {mul} {var})");
                }
                parts.push(mul);
            }
        }
        if parts.len() == 1 {
            parts[0].clone()
        } else {
            let inner = parts.join(" ");
            format!("(* {inner})")
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8.7: Separation Logic
// ═══════════════════════════════════════════════════════════════════════

/// Separation logic assertion for heap reasoning.
#[derive(Debug, Clone, PartialEq)]
pub enum SepLogicAssertion {
    /// `emp` — empty heap.
    Emp,
    /// `x |-> v` — pointer x points to value v.
    PointsTo(String, String),
    /// `P * Q` — separating conjunction (disjoint heaps).
    SepConj(Box<SepLogicAssertion>, Box<SepLogicAssertion>),
    /// `P -* Q` — magic wand (if P added, then Q holds).
    MagicWand(Box<SepLogicAssertion>, Box<SepLogicAssertion>),
    /// `ls(x, y)` — list segment from x to y.
    ListSegment(String, String),
    /// `tree(x)` — binary tree rooted at x.
    Tree(String),
    /// Pure boolean constraint.
    Pure(String),
}

impl SepLogicAssertion {
    /// Generates a readable description.
    pub fn describe(&self) -> String {
        match self {
            Self::Emp => "empty heap".to_string(),
            Self::PointsTo(ptr, val) => format!("{ptr} -> {val}"),
            Self::SepConj(a, b) => format!("({} * {})", a.describe(), b.describe()),
            Self::MagicWand(a, b) => format!("({} -* {})", a.describe(), b.describe()),
            Self::ListSegment(from, to) => format!("ls({from}, {to})"),
            Self::Tree(root) => format!("tree({root})"),
            Self::Pure(expr) => expr.clone(),
        }
    }

    /// Returns true if this is the empty heap.
    pub fn is_emp(&self) -> bool {
        matches!(self, Self::Emp)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8.8: Concurrent Theory
// ═══════════════════════════════════════════════════════════════════════

/// A concurrent verification constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConcurrentConstraint {
    /// Mutex lock order: lock A before lock B.
    LockOrder(String, String),
    /// No data race: access to var protected by mutex.
    NoDataRace { variable: String, mutex: String },
    /// Happens-before relation: event A before event B.
    HappensBefore(String, String),
    /// Atomic operation: read-modify-write is atomic.
    AtomicRMW { variable: String, operation: String },
    /// Deadlock freedom: no circular lock dependency.
    DeadlockFree(Vec<String>),
    /// Channel send-receive ordering.
    ChannelOrder { channel: String, send_id: u64, recv_id: u64 },
}

impl ConcurrentConstraint {
    /// Returns a human-readable description.
    pub fn describe(&self) -> String {
        match self {
            Self::LockOrder(a, b) => format!("lock {a} before {b}"),
            Self::NoDataRace { variable, mutex } => {
                format!("no data race on {variable} (protected by {mutex})")
            }
            Self::HappensBefore(a, b) => format!("{a} happens-before {b}"),
            Self::AtomicRMW { variable, operation } => {
                format!("atomic {operation} on {variable}")
            }
            Self::DeadlockFree(locks) => {
                format!("no deadlock among: {}", locks.join(", "))
            }
            Self::ChannelOrder { channel, send_id, recv_id } => {
                format!("channel {channel}: send#{send_id} -> recv#{recv_id}")
            }
        }
    }
}

/// Detects potential deadlocks from a lock ordering graph.
/// Returns pairs of locks that form cycles.
pub fn detect_deadlock_cycles(
    lock_orders: &[(String, String)],
) -> Vec<(String, String)> {
    // Simple cycle detection via adjacency and DFS.
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for (a, b) in lock_orders {
        adj.entry(a.clone()).or_default().push(b.clone());
    }

    let mut cycles = Vec::new();
    for (a, b) in lock_orders {
        // Check if there's a path from b back to a.
        if has_path(&adj, b, a) {
            cycles.push((a.clone(), b.clone()));
        }
    }
    cycles
}

/// Simple DFS to check if there's a path from `start` to `target`.
fn has_path(adj: &HashMap<String, Vec<String>>, start: &str, target: &str) -> bool {
    let mut visited = HashMap::new();
    dfs(adj, start, target, &mut visited)
}

fn dfs(
    adj: &HashMap<String, Vec<String>>,
    current: &str,
    target: &str,
    visited: &mut HashMap<String, bool>,
) -> bool {
    if current == target {
        return true;
    }
    if visited.get(current).copied().unwrap_or(false) {
        return false;
    }
    visited.insert(current.to_string(), true);
    if let Some(neighbors) = adj.get(current) {
        for next in neighbors {
            if dfs(adj, next, target, visited) {
                return true;
            }
        }
    }
    false
}

// ═══════════════════════════════════════════════════════════════════════
// V8.9: Custom Theory Plugins
// ═══════════════════════════════════════════════════════════════════════

/// A custom theory plugin (user-defined axioms).
#[derive(Debug, Clone)]
pub struct CustomTheory {
    /// Theory name.
    pub name: String,
    /// SMT-LIB2 sort declarations.
    pub sorts: Vec<String>,
    /// SMT-LIB2 function declarations.
    pub functions: Vec<String>,
    /// Axioms (universally quantified assertions).
    pub axioms: Vec<String>,
    /// Description.
    pub description: String,
}

impl CustomTheory {
    /// Creates a new empty custom theory.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            sorts: Vec::new(),
            functions: Vec::new(),
            axioms: Vec::new(),
            description: String::new(),
        }
    }

    /// Adds a sort declaration.
    pub fn add_sort(&mut self, sort: &str) {
        self.sorts.push(sort.to_string());
    }

    /// Adds a function declaration.
    pub fn add_function(&mut self, func: &str) {
        self.functions.push(func.to_string());
    }

    /// Adds an axiom.
    pub fn add_axiom(&mut self, axiom: &str) {
        self.axioms.push(axiom.to_string());
    }

    /// Generates the complete SMT-LIB2 preamble for this theory.
    pub fn to_smtlib2_preamble(&self) -> String {
        let mut result = format!("; Custom theory: {}\n", self.name);
        for sort in &self.sorts {
            result.push_str(&format!("{sort}\n"));
        }
        for func in &self.functions {
            result.push_str(&format!("{func}\n"));
        }
        for axiom in &self.axioms {
            result.push_str(&format!("(assert {axiom})\n"));
        }
        result
    }
}

impl fmt::Display for CustomTheory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Theory({}, {} sorts, {} fns, {} axioms)",
            self.name,
            self.sorts.len(),
            self.functions.len(),
            self.axioms.len(),
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8.10: Theory Combination (Nelson-Oppen)
// ═══════════════════════════════════════════════════════════════════════

/// A shared variable between theories (for Nelson-Oppen combination).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedVariable {
    /// Variable name.
    pub name: String,
    /// Theories that use this variable.
    pub theories: Vec<TheoryKind>,
    /// Sort in SMT-LIB2.
    pub sort: String,
}

/// Finds shared variables between theories for Nelson-Oppen combination.
pub fn find_shared_variables(
    theory_vars: &HashMap<TheoryKind, Vec<(String, String)>>,
) -> Vec<SharedVariable> {
    let mut var_theories: HashMap<String, Vec<(TheoryKind, String)>> = HashMap::new();

    for (theory, vars) in theory_vars {
        for (name, sort) in vars {
            var_theories
                .entry(name.clone())
                .or_default()
                .push((*theory, sort.clone()));
        }
    }

    let mut shared = Vec::new();
    for (name, theories) in &var_theories {
        if theories.len() > 1 {
            shared.push(SharedVariable {
                name: name.clone(),
                theories: theories.iter().map(|(t, _)| *t).collect(),
                sort: theories[0].1.clone(),
            });
        }
    }

    shared.sort_by(|a, b| a.name.cmp(&b.name));
    shared
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v8_1_theory_kind_display() {
        assert_eq!(format!("{}", TheoryKind::Bitvector), "QF_BV");
        assert_eq!(format!("{}", TheoryKind::FloatingPoint), "QF_FP");
        assert_eq!(format!("{}", TheoryKind::SeparationLogic), "SL");
        assert_eq!(format!("{}", TheoryKind::LinearArithmetic), "QF_LIA");
    }

    #[test]
    fn v8_1_engine_new() {
        let engine = SmtTheoryEngine::new();
        assert_eq!(engine.active_theories, vec![TheoryKind::LinearArithmetic]);
        assert_eq!(engine.combination, CombinationStrategy::NelsonOppen);
    }

    #[test]
    fn v8_1_engine_select_theories() {
        let mut engine = SmtTheoryEngine::new();
        let features = CodeFeatures {
            has_bitvector_ops: true,
            has_array_access: true,
            has_float_ops: false,
            has_string_ops: false,
            has_nonlinear_ops: false,
            has_heap_ops: false,
            has_concurrency: false,
            has_function_calls: false,
        };
        engine.select_theories(&features);
        assert!(engine.has_theory(TheoryKind::LinearArithmetic));
        assert!(engine.has_theory(TheoryKind::Bitvector));
        assert!(engine.has_theory(TheoryKind::Array));
        assert!(!engine.has_theory(TheoryKind::FloatingPoint));
    }

    #[test]
    fn v8_1_engine_combined_logic() {
        let mut engine = SmtTheoryEngine::new();
        assert_eq!(engine.combined_logic(), "QF_LIA");

        let features = CodeFeatures {
            has_bitvector_ops: true,
            has_float_ops: true,
            ..Default::default()
        };
        engine.select_theories(&features);
        assert_eq!(engine.combined_logic(), "ALL");
    }

    #[test]
    fn v8_1_engine_options() {
        let mut engine = SmtTheoryEngine::new();
        engine.set_option("random_seed", "42");
        assert_eq!(engine.options.get("random_seed").map(|s| s.as_str()), Some("42"));
    }

    #[test]
    fn v8_2_bv_sort_from_type() {
        let i32_sort = BvSort::from_type_name("i32");
        assert!(i32_sort.is_some());
        let sort = i32_sort.expect("i32 sort");
        assert_eq!(sort.width, 32);
        assert!(sort.signed);
        assert_eq!(sort.to_smtlib2(), "(_ BitVec 32)");
        assert_eq!(format!("{sort}"), "i32");

        let u8_sort = BvSort::from_type_name("u8").expect("u8 sort");
        assert_eq!(u8_sort.width, 8);
        assert!(!u8_sort.signed);
    }

    #[test]
    fn v8_2_bv_sort_unknown_type() {
        assert!(BvSort::from_type_name("f64").is_none());
        assert!(BvSort::from_type_name("str").is_none());
    }

    #[test]
    fn v8_2_bv_overflow_check() {
        let sort = BvSort { width: 32, signed: true };
        let check = sort.add_overflow_check("a", "b");
        assert!(check.contains("bvadd"));
        assert!(check.contains("bvsgt"));

        let usort = BvSort { width: 8, signed: false };
        let ucheck = usort.add_overflow_check("x", "y");
        assert!(ucheck.contains("bvult"));
    }

    #[test]
    fn v8_3_array_sort() {
        let arr = ArraySort::int_array();
        assert_eq!(arr.to_smtlib2(), "(Array Int Int)");
        assert_eq!(format!("{arr}"), "(Array Int Int)");

        let bv_arr = ArraySort::bv_array(32, 8);
        assert!(bv_arr.to_smtlib2().contains("BitVec 32"));
    }

    #[test]
    fn v8_3_array_operations() {
        let arr = ArraySort::int_array();
        assert_eq!(arr.select("a", "i"), "(select a i)");
        assert_eq!(arr.store("a", "i", "42"), "(store a i 42)");
        assert_eq!(arr.bounds_check("a", "i", "n"), "(and (>= i 0) (< i n))");
    }

    #[test]
    fn v8_4_fp_sort_f32() {
        let f32_sort = FpSort::float32();
        assert_eq!(f32_sort.exponent_bits, 8);
        assert_eq!(f32_sort.significand_bits, 24);
        assert_eq!(f32_sort.total_bits(), 32);
        assert_eq!(f32_sort.to_smtlib2(), "(_ FloatingPoint 8 24)");
        assert_eq!(format!("{f32_sort}"), "f32");
    }

    #[test]
    fn v8_4_fp_sort_f64() {
        let f64_sort = FpSort::float64();
        assert_eq!(f64_sort.exponent_bits, 11);
        assert_eq!(f64_sort.significand_bits, 53);
        assert_eq!(f64_sort.total_bits(), 64);
        assert_eq!(format!("{f64_sort}"), "f64");
    }

    #[test]
    fn v8_4_fp_checks() {
        let fp = FpSort::float64();
        assert_eq!(fp.is_nan_check("x"), "(fp.isNaN x)");
        assert_eq!(fp.is_infinite_check("x"), "(fp.isInfinite x)");
        assert_eq!(fp.is_normal_check("x"), "(fp.isNormal x)");
    }

    #[test]
    fn v8_5_string_constraint() {
        let len = StringConstraint::Length("s".to_string(), StringCompareOp::Gt, 0);
        assert_eq!(len.to_smtlib2(), "(> (str.len s) 0)");

        let contains = StringConstraint::Contains("s".to_string(), "hello".to_string());
        assert!(contains.to_smtlib2().contains("str.contains"));
    }

    #[test]
    fn v8_5_string_compare_op_display() {
        assert_eq!(format!("{}", StringCompareOp::Eq), "=");
        assert_eq!(format!("{}", StringCompareOp::Lt), "<");
        assert_eq!(format!("{}", StringCompareOp::Ge), ">=");
    }

    #[test]
    fn v8_5_string_prefix_suffix() {
        let prefix = StringConstraint::Prefix("s".to_string(), "http".to_string());
        assert!(prefix.to_smtlib2().contains("str.prefixof"));

        let suffix = StringConstraint::Suffix("s".to_string(), ".fj".to_string());
        assert!(suffix.to_smtlib2().contains("str.suffixof"));
    }

    #[test]
    fn v8_6_polynomial_term() {
        let constant = PolynomialTerm::constant(42);
        assert_eq!(constant.to_smtlib2(), "42");

        let linear = PolynomialTerm::linear(3, "x");
        assert_eq!(linear.to_smtlib2(), "(* 3 x)");

        let quad = PolynomialTerm::quadratic(1, "x");
        // x^2 = (* x (* x x)) or similar
        let smt = quad.to_smtlib2();
        assert!(smt.contains("x"));
    }

    #[test]
    fn v8_7_sep_logic_assertions() {
        let emp = SepLogicAssertion::Emp;
        assert!(emp.is_emp());
        assert_eq!(emp.describe(), "empty heap");

        let points_to = SepLogicAssertion::PointsTo("p".to_string(), "42".to_string());
        assert!(!points_to.is_emp());
        assert_eq!(points_to.describe(), "p -> 42");
    }

    #[test]
    fn v8_7_sep_logic_compose() {
        let left = SepLogicAssertion::PointsTo("p".to_string(), "1".to_string());
        let right = SepLogicAssertion::PointsTo("q".to_string(), "2".to_string());
        let sep = SepLogicAssertion::SepConj(Box::new(left), Box::new(right));
        let desc = sep.describe();
        assert!(desc.contains("p -> 1"));
        assert!(desc.contains("q -> 2"));
        assert!(desc.contains("*"));
    }

    #[test]
    fn v8_7_sep_logic_list_tree() {
        let list = SepLogicAssertion::ListSegment("head".to_string(), "null".to_string());
        assert_eq!(list.describe(), "ls(head, null)");

        let tree = SepLogicAssertion::Tree("root".to_string());
        assert_eq!(tree.describe(), "tree(root)");
    }

    #[test]
    fn v8_8_concurrent_constraint_describe() {
        let lock = ConcurrentConstraint::LockOrder("A".to_string(), "B".to_string());
        assert_eq!(lock.describe(), "lock A before B");

        let race = ConcurrentConstraint::NoDataRace {
            variable: "counter".to_string(),
            mutex: "mtx".to_string(),
        };
        assert!(race.describe().contains("counter"));
        assert!(race.describe().contains("mtx"));

        let hb = ConcurrentConstraint::HappensBefore("write".to_string(), "read".to_string());
        assert_eq!(hb.describe(), "write happens-before read");
    }

    #[test]
    fn v8_8_deadlock_detection_no_cycle() {
        let orders = vec![
            ("A".to_string(), "B".to_string()),
            ("B".to_string(), "C".to_string()),
        ];
        let cycles = detect_deadlock_cycles(&orders);
        assert!(cycles.is_empty());
    }

    #[test]
    fn v8_8_deadlock_detection_cycle() {
        let orders = vec![
            ("A".to_string(), "B".to_string()),
            ("B".to_string(), "A".to_string()),
        ];
        let cycles = detect_deadlock_cycles(&orders);
        assert!(!cycles.is_empty());
    }

    #[test]
    fn v8_9_custom_theory() {
        let mut theory = CustomTheory::new("permissions");
        theory.description = "RBAC permission theory".to_string();
        theory.add_sort("(declare-sort Permission 0)");
        theory.add_function("(declare-fun has-perm (User Permission) Bool)");
        theory.add_axiom("(forall ((u User)) (has-perm u read))");

        assert_eq!(format!("{theory}"), "Theory(permissions, 1 sorts, 1 fns, 1 axioms)");

        let preamble = theory.to_smtlib2_preamble();
        assert!(preamble.contains("Custom theory: permissions"));
        assert!(preamble.contains("declare-sort Permission"));
        assert!(preamble.contains("declare-fun has-perm"));
        assert!(preamble.contains("assert"));
    }

    #[test]
    fn v8_9_custom_theory_engine_integration() {
        let mut engine = SmtTheoryEngine::new();
        let theory = CustomTheory::new("test_theory");
        engine.add_custom_theory(theory);
        assert_eq!(engine.custom_theories.len(), 1);
    }

    #[test]
    fn v8_10_find_shared_variables() {
        let mut theory_vars = HashMap::new();
        theory_vars.insert(
            TheoryKind::LinearArithmetic,
            vec![("x".to_string(), "Int".to_string()), ("y".to_string(), "Int".to_string())],
        );
        theory_vars.insert(
            TheoryKind::Array,
            vec![("x".to_string(), "Int".to_string()), ("arr".to_string(), "(Array Int Int)".to_string())],
        );

        let shared = find_shared_variables(&theory_vars);
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].name, "x");
        assert_eq!(shared[0].theories.len(), 2);
    }

    #[test]
    fn v8_10_no_shared_variables() {
        let mut theory_vars = HashMap::new();
        theory_vars.insert(
            TheoryKind::LinearArithmetic,
            vec![("x".to_string(), "Int".to_string())],
        );
        theory_vars.insert(
            TheoryKind::Bitvector,
            vec![("y".to_string(), "(_ BitVec 32)".to_string())],
        );

        let shared = find_shared_variables(&theory_vars);
        assert!(shared.is_empty());
    }

    #[test]
    fn v8_combination_strategy_display() {
        assert_eq!(format!("{}", CombinationStrategy::NelsonOppen), "Nelson-Oppen");
        assert_eq!(format!("{}", CombinationStrategy::Shostak), "Shostak");
        assert_eq!(format!("{}", CombinationStrategy::Delayed), "Delayed");
    }

    #[test]
    fn v8_engine_default_trait() {
        let engine = SmtTheoryEngine::default();
        assert!(engine.has_theory(TheoryKind::LinearArithmetic));
    }

    #[test]
    fn v8_8_concurrent_channel_order() {
        let ch = ConcurrentConstraint::ChannelOrder {
            channel: "ch1".to_string(),
            send_id: 1,
            recv_id: 2,
        };
        assert!(ch.describe().contains("ch1"));
        assert!(ch.describe().contains("send#1"));
    }
}
