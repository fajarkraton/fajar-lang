//! Symbolic Execution Engine — Sprint V1: 10 tasks.
//!
//! Provides symbolic values, constraint tracking, path conditions, symbolic memory,
//! loop unrolling, function summaries, path explosion mitigation, concolic execution,
//! and counterexample generation. All simulated (no real Z3 dependency).

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V1.1: Symbolic Values
// ═══════════════════════════════════════════════════════════════════════

/// A symbolic value: can be concrete, symbolic, or constrained.
#[derive(Debug, Clone, PartialEq)]
pub enum SymValue {
    /// A known concrete value.
    Concrete(ConcreteVal),
    /// A purely symbolic variable (no known value).
    Symbolic(String),
    /// A symbolic expression tree.
    Expr(SymExpr),
    /// A constrained symbolic value (variable + constraints that apply).
    Constrained {
        /// Variable name.
        name: String,
        /// Constraints on this value.
        constraints: Vec<String>,
    },
}

impl fmt::Display for SymValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Concrete(v) => write!(f, "{v}"),
            Self::Symbolic(name) => write!(f, "sym({name})"),
            Self::Expr(expr) => write!(f, "{expr}"),
            Self::Constrained { name, constraints } => {
                write!(f, "{name} where [{}]", constraints.join(", "))
            }
        }
    }
}

/// Concrete value types.
#[derive(Debug, Clone, PartialEq)]
pub enum ConcreteVal {
    /// Integer value.
    Int(i64),
    /// Floating-point value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// String value.
    Str(String),
    /// Null/unit.
    Null,
}

impl fmt::Display for ConcreteVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::Null => write!(f, "null"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V1.2: Symbolic Expression Tree
// ═══════════════════════════════════════════════════════════════════════

/// A symbolic expression tree for tracking computations.
#[derive(Debug, Clone, PartialEq)]
pub enum SymExpr {
    /// Concrete literal.
    Lit(ConcreteVal),
    /// Named variable.
    Var(String),
    /// Binary operation.
    BinOp(Box<SymExpr>, SymBinOp, Box<SymExpr>),
    /// Unary operation.
    UnaryOp(SymUnaryOp, Box<SymExpr>),
    /// Conditional (ite: if-then-else).
    Ite(Box<SymExpr>, Box<SymExpr>, Box<SymExpr>),
    /// Array select (read).
    Select(Box<SymExpr>, Box<SymExpr>),
    /// Array store (write).
    Store(Box<SymExpr>, Box<SymExpr>, Box<SymExpr>),
    /// Function application (for summaries).
    Apply(String, Vec<SymExpr>),
}

/// Binary operators for symbolic expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymBinOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
    /// Modulo.
    Mod,
    /// Equality.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Less or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater or equal.
    Ge,
    /// Logical AND.
    And,
    /// Logical OR.
    Or,
    /// Bitwise AND.
    BitAnd,
    /// Bitwise OR.
    BitOr,
    /// Bitwise XOR.
    BitXor,
    /// Left shift.
    Shl,
    /// Right shift.
    Shr,
}

/// Unary operators for symbolic expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymUnaryOp {
    /// Logical NOT.
    Not,
    /// Arithmetic negation.
    Neg,
    /// Bitwise complement.
    BitNot,
}

impl fmt::Display for SymExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lit(v) => write!(f, "{v}"),
            Self::Var(name) => write!(f, "{name}"),
            Self::BinOp(lhs, op, rhs) => write!(f, "({lhs} {op} {rhs})"),
            Self::UnaryOp(op, inner) => write!(f, "({op} {inner})"),
            Self::Ite(cond, then_v, else_v) => {
                write!(f, "(if {cond} then {then_v} else {else_v})")
            }
            Self::Select(arr, idx) => write!(f, "{arr}[{idx}]"),
            Self::Store(arr, idx, val) => write!(f, "store({arr}, {idx}, {val})"),
            Self::Apply(name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                write!(f, "{name}({})", args_str.join(", "))
            }
        }
    }
}

impl fmt::Display for SymBinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::And => "&&",
            Self::Or => "||",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::Shl => "<<",
            Self::Shr => ">>",
        };
        write!(f, "{s}")
    }
}

impl fmt::Display for SymUnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Not => "!",
            Self::Neg => "-",
            Self::BitNot => "~",
        };
        write!(f, "{s}")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V1.3: Path Condition Tracking
// ═══════════════════════════════════════════════════════════════════════

/// A path condition: conjunction of constraints accumulated along an execution path.
#[derive(Debug, Clone, Default)]
pub struct PathCondition {
    /// Ordered constraints (all must hold for this path).
    pub constraints: Vec<String>,
    /// Whether this path is feasible (not yet contradicted).
    pub feasible: bool,
    /// Depth of the path (number of branch decisions).
    pub depth: u32,
}

impl PathCondition {
    /// Creates a new empty (feasible) path condition.
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
            feasible: true,
            depth: 0,
        }
    }

    /// Adds a constraint for a taken branch.
    pub fn add_constraint(&mut self, constraint: String) {
        self.constraints.push(constraint);
        self.depth += 1;
    }

    /// Forks the path condition for a branch, returning the true and false branches.
    pub fn fork(&self, condition: &str) -> (PathCondition, PathCondition) {
        let mut true_branch = self.clone();
        true_branch.add_constraint(condition.to_string());

        let mut false_branch = self.clone();
        false_branch.add_constraint(format!("!({condition})"));

        (true_branch, false_branch)
    }

    /// Checks feasibility using simple constraint analysis (simulated).
    /// Returns true if no obvious contradictions are found.
    pub fn check_feasibility(&mut self) -> bool {
        // Simple contradiction detection: if both `x` and `!(x)` exist
        for i in 0..self.constraints.len() {
            for j in (i + 1)..self.constraints.len() {
                let neg = format!("!({})", self.constraints[i]);
                if self.constraints[j] == neg {
                    self.feasible = false;
                    return false;
                }
                let neg_reverse = format!("!({})", self.constraints[j]);
                if self.constraints[i] == neg_reverse {
                    self.feasible = false;
                    return false;
                }
            }
        }
        self.feasible = true;
        true
    }

    /// Exports the path condition as an SMT-LIB2 assertion string.
    pub fn to_smtlib2(&self) -> String {
        if self.constraints.is_empty() {
            return "true".to_string();
        }
        if self.constraints.len() == 1 {
            return self.constraints[0].clone();
        }
        let conjoined: Vec<String> = self.constraints.clone();
        format!("(and {})", conjoined.join(" "))
    }

    /// Returns the number of constraints.
    pub fn len(&self) -> usize {
        self.constraints.len()
    }

    /// Returns true if there are no constraints.
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }
}

impl fmt::Display for PathCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.constraints.is_empty() {
            write!(f, "true")
        } else {
            write!(f, "{}", self.constraints.join(" && "))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V1.4: Symbolic Memory Model
// ═══════════════════════════════════════════════════════════════════════

/// Symbolic memory: models heap and stack as symbolic arrays.
#[derive(Debug, Clone, Default)]
pub struct SymbolicMemory {
    /// Stack variables: name -> symbolic value.
    stack: HashMap<String, SymValue>,
    /// Heap regions: address label -> symbolic value.
    heap: HashMap<String, SymValue>,
    /// Memory version counter (for SSA-like tracking).
    version: u64,
}

impl SymbolicMemory {
    /// Creates an empty symbolic memory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads a stack variable.
    pub fn read_stack(&self, name: &str) -> Option<&SymValue> {
        self.stack.get(name)
    }

    /// Writes a stack variable, returning the previous value if any.
    pub fn write_stack(&mut self, name: String, value: SymValue) -> Option<SymValue> {
        self.version += 1;
        self.stack.insert(name, value)
    }

    /// Reads a heap location.
    pub fn read_heap(&self, addr: &str) -> Option<&SymValue> {
        self.heap.get(addr)
    }

    /// Writes a heap location.
    pub fn write_heap(&mut self, addr: String, value: SymValue) -> Option<SymValue> {
        self.version += 1;
        self.heap.insert(addr, value)
    }

    /// Returns the current memory version.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Returns the number of stack entries.
    pub fn stack_size(&self) -> usize {
        self.stack.len()
    }

    /// Returns the number of heap entries.
    pub fn heap_size(&self) -> usize {
        self.heap.len()
    }

    /// Clears all memory (for reset between paths).
    pub fn clear(&mut self) {
        self.stack.clear();
        self.heap.clear();
        self.version = 0;
    }

    /// Creates a snapshot of the current memory state.
    pub fn snapshot(&self) -> SymbolicMemory {
        self.clone()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V1.5: Loop Unrolling (Bounded)
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for bounded loop unrolling during symbolic execution.
#[derive(Debug, Clone)]
pub struct LoopUnrollConfig {
    /// Maximum number of iterations to unroll.
    pub max_iterations: u32,
    /// Whether to add a "remaining iterations" summary after unrolling.
    pub add_summary: bool,
    /// Whether to widen symbolic values after the bound.
    pub widen_after_bound: bool,
}

impl Default for LoopUnrollConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            add_summary: true,
            widen_after_bound: false,
        }
    }
}

/// Result of loop unrolling.
#[derive(Debug, Clone)]
pub struct LoopUnrollResult {
    /// The path conditions for each unrolled iteration.
    pub iteration_paths: Vec<PathCondition>,
    /// Whether the bound was reached (loop may have more iterations).
    pub bound_reached: bool,
    /// Number of iterations actually unrolled.
    pub iterations_unrolled: u32,
    /// Summary constraint added after bound (if configured).
    pub summary_constraint: Option<String>,
}

/// Unrolls a loop symbolically with a bounded number of iterations.
///
/// The `loop_condition` is a string representation of the loop guard.
/// The `body_effect` closure is called for each iteration to update the path condition.
pub fn unroll_loop(
    config: &LoopUnrollConfig,
    loop_condition: &str,
    body_effects: &[String],
) -> LoopUnrollResult {
    let mut paths = Vec::new();
    let mut current = PathCondition::new();
    let mut iterations = 0u32;

    while iterations < config.max_iterations {
        // Add loop condition to path
        let mut iter_path = current.clone();
        iter_path.add_constraint(loop_condition.to_string());

        // Apply body effects
        for effect in body_effects {
            iter_path.add_constraint(effect.clone());
        }

        paths.push(iter_path.clone());
        current = iter_path;
        iterations += 1;
    }

    let bound_reached = iterations >= config.max_iterations;
    let summary = if bound_reached && config.add_summary {
        Some(format!(
            "loop_may_continue({loop_condition}, unrolled={iterations})"
        ))
    } else {
        None
    };

    // Add the exit path (negation of loop condition)
    let mut exit_path = current;
    exit_path.add_constraint(format!("!({loop_condition})"));
    paths.push(exit_path);

    LoopUnrollResult {
        iteration_paths: paths,
        bound_reached,
        iterations_unrolled: iterations,
        summary_constraint: summary,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V1.6: Function Summaries Cache
// ═══════════════════════════════════════════════════════════════════════

/// A function summary: captures the effect of a function symbolically.
#[derive(Debug, Clone)]
pub struct FunctionSummary {
    /// Function name.
    pub name: String,
    /// Parameter names.
    pub params: Vec<String>,
    /// Preconditions (constraints on inputs).
    pub preconditions: Vec<String>,
    /// Postconditions (constraints on outputs, may reference params).
    pub postconditions: Vec<String>,
    /// Modified global state (variable names).
    pub modifies: Vec<String>,
    /// Whether the function may diverge (non-terminating).
    pub may_diverge: bool,
    /// Whether the function is pure (no side effects).
    pub is_pure: bool,
}

/// Cache of function summaries to avoid re-analyzing called functions.
#[derive(Debug, Clone, Default)]
pub struct SummaryCache {
    /// Function name -> summary.
    summaries: HashMap<String, FunctionSummary>,
    /// Cache hits.
    pub hits: u64,
    /// Cache misses.
    pub misses: u64,
}

impl SummaryCache {
    /// Creates an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks up a function summary.
    pub fn get(&mut self, name: &str) -> Option<&FunctionSummary> {
        if self.summaries.contains_key(name) {
            self.hits += 1;
            self.summaries.get(name)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Inserts a function summary.
    pub fn insert(&mut self, summary: FunctionSummary) {
        self.summaries.insert(summary.name.clone(), summary);
    }

    /// Returns the number of cached summaries.
    pub fn size(&self) -> usize {
        self.summaries.len()
    }

    /// Returns the cache hit rate.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V1.7: Path Explosion Mitigation
// ═══════════════════════════════════════════════════════════════════════

/// Strategy for mitigating path explosion in symbolic execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// No merging (explore all paths independently).
    None,
    /// Merge paths at join points (if-then-else confluences).
    JoinPoint,
    /// Merge paths with similar constraint prefixes.
    ConstraintPrefix,
    /// Subsumption: drop paths subsumed by others.
    Subsumption,
}

/// Configuration for path explosion mitigation.
#[derive(Debug, Clone)]
pub struct ExplosionMitigationConfig {
    /// Maximum number of active paths.
    pub max_paths: usize,
    /// Merge strategy.
    pub strategy: MergeStrategy,
    /// Maximum path depth before pruning.
    pub max_depth: u32,
    /// Whether to use function summaries to reduce paths.
    pub use_summaries: bool,
}

impl Default for ExplosionMitigationConfig {
    fn default() -> Self {
        Self {
            max_paths: 1000,
            strategy: MergeStrategy::JoinPoint,
            max_depth: 50,
            use_summaries: true,
        }
    }
}

/// Merges two path conditions at a join point, producing a merged condition.
///
/// The merged condition uses an ITE (if-then-else) abstraction for differing
/// constraints. This is a simulated merge (no real solver).
pub fn merge_paths(path_a: &PathCondition, path_b: &PathCondition) -> PathCondition {
    let mut merged = PathCondition::new();

    // Shared prefix: constraints common to both paths
    let shared_len = path_a
        .constraints
        .iter()
        .zip(path_b.constraints.iter())
        .take_while(|(a, b)| a == b)
        .count();

    for constraint in path_a.constraints.iter().take(shared_len) {
        merged.add_constraint(constraint.clone());
    }

    // Divergent suffix: create an ITE abstraction
    let a_suffix: Vec<&str> = path_a.constraints[shared_len..]
        .iter()
        .map(|s| s.as_str())
        .collect();
    let b_suffix: Vec<&str> = path_b.constraints[shared_len..]
        .iter()
        .map(|s| s.as_str())
        .collect();

    if !a_suffix.is_empty() || !b_suffix.is_empty() {
        let ite = format!(
            "ite(branch, [{}], [{}])",
            a_suffix.join(", "),
            b_suffix.join(", ")
        );
        merged.add_constraint(ite);
    }

    merged.feasible = path_a.feasible || path_b.feasible;
    merged
}

/// Prunes paths that exceed the depth limit or are infeasible.
pub fn prune_paths(
    paths: &[PathCondition],
    config: &ExplosionMitigationConfig,
) -> Vec<PathCondition> {
    paths
        .iter()
        .filter(|p| p.feasible && p.depth <= config.max_depth)
        .take(config.max_paths)
        .cloned()
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// V1.8: Concolic Execution
// ═══════════════════════════════════════════════════════════════════════

/// Concolic execution state: runs concrete + symbolic simultaneously.
#[derive(Debug, Clone)]
pub struct ConcolicState {
    /// Concrete values for each variable.
    pub concrete: HashMap<String, ConcreteVal>,
    /// Symbolic expressions for each variable.
    pub symbolic: HashMap<String, SymExpr>,
    /// Path condition accumulated from branch decisions.
    pub path_condition: PathCondition,
    /// Negated constraints for path exploration (to generate new inputs).
    pub negated_constraints: Vec<String>,
}

impl ConcolicState {
    /// Creates a new concolic state with initial concrete inputs.
    pub fn new(inputs: HashMap<String, ConcreteVal>) -> Self {
        let mut symbolic = HashMap::new();
        for name in inputs.keys() {
            symbolic.insert(name.clone(), SymExpr::Var(name.clone()));
        }
        Self {
            concrete: inputs,
            symbolic,
            path_condition: PathCondition::new(),
            negated_constraints: Vec::new(),
        }
    }

    /// Records a branch decision (concrete value taken, symbolic condition tracked).
    pub fn branch(&mut self, condition: &str, taken: bool) {
        if taken {
            self.path_condition.add_constraint(condition.to_string());
        } else {
            self.path_condition
                .add_constraint(format!("!({condition})"));
        }
    }

    /// Generates alternative inputs by negating the last branch constraint.
    ///
    /// Returns the negated constraint that should be solved to find new inputs.
    pub fn generate_alternative(&mut self) -> Option<String> {
        if self.path_condition.constraints.is_empty() {
            return None;
        }
        let last_idx = self.path_condition.constraints.len() - 1;
        let last = &self.path_condition.constraints[last_idx];
        let negated = if let Some(stripped) = last.strip_prefix("!(") {
            stripped.trim_end_matches(')').to_string()
        } else {
            format!("!({last})")
        };
        self.negated_constraints.push(negated.clone());
        Some(negated)
    }

    /// Returns the number of explored paths so far.
    pub fn paths_explored(&self) -> usize {
        self.negated_constraints.len() + 1
    }

    /// Updates a variable with both concrete and symbolic values.
    pub fn update_var(&mut self, name: &str, concrete: ConcreteVal, symbolic: SymExpr) {
        self.concrete.insert(name.to_string(), concrete);
        self.symbolic.insert(name.to_string(), symbolic);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V1.9: Counterexample Generation
// ═══════════════════════════════════════════════════════════════════════

/// A counterexample from symbolic execution.
#[derive(Debug, Clone)]
pub struct SymCounterexample {
    /// Variable assignments that violate the property.
    pub assignments: HashMap<String, ConcreteVal>,
    /// The path condition that led to the violation.
    pub path_condition: PathCondition,
    /// The violated property description.
    pub violated_property: String,
    /// Source file.
    pub file: String,
    /// Source line.
    pub line: u32,
}

impl fmt::Display for SymCounterexample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Counterexample found at {}:{}", self.file, self.line)?;
        writeln!(f, "  Violated: {}", self.violated_property)?;
        writeln!(f, "  Path: {}", self.path_condition)?;
        let mut sorted: Vec<_> = self.assignments.iter().collect();
        sorted.sort_by_key(|(k, _)| (*k).clone());
        for (name, val) in &sorted {
            writeln!(f, "  {name} = {val}")?;
        }
        Ok(())
    }
}

/// Attempts to generate a counterexample for a property violation.
///
/// Given a path condition and a property, checks if the negation of the
/// property is satisfiable under the path condition (simulated).
pub fn generate_counterexample(
    path: &PathCondition,
    property: &str,
    file: &str,
    line: u32,
    known_values: &HashMap<String, ConcreteVal>,
) -> Option<SymCounterexample> {
    // Simulated: if the property contains a variable that has a known concrete
    // value violating it, produce a counterexample.
    // In a real implementation, this would call an SMT solver.

    let negated = format!("!({property})");
    let mut combined = path.clone();
    combined.add_constraint(negated);

    // Simple heuristic: check feasibility
    if !combined.check_feasibility() {
        return None; // Path is infeasible, property holds
    }

    // If we have known values, use them as the counterexample
    if !known_values.is_empty() {
        return Some(SymCounterexample {
            assignments: known_values.clone(),
            path_condition: path.clone(),
            violated_property: property.to_string(),
            file: file.to_string(),
            line,
        });
    }

    // If path is feasible and we have no contradiction, assume counterexample exists
    Some(SymCounterexample {
        assignments: HashMap::new(),
        path_condition: path.clone(),
        violated_property: property.to_string(),
        file: file.to_string(),
        line,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// V1.10: Symbolic Execution Engine (orchestrator)
// ═══════════════════════════════════════════════════════════════════════

/// The main symbolic execution engine, orchestrating all components.
#[derive(Debug)]
pub struct SymbolicEngine {
    /// Symbolic memory.
    pub memory: SymbolicMemory,
    /// Active paths being explored.
    pub active_paths: Vec<PathCondition>,
    /// Completed paths (reached end of function).
    pub completed_paths: Vec<PathCondition>,
    /// Function summary cache.
    pub summary_cache: SummaryCache,
    /// Loop unrolling configuration.
    pub loop_config: LoopUnrollConfig,
    /// Path explosion mitigation configuration.
    pub explosion_config: ExplosionMitigationConfig,
    /// Counterexamples found.
    pub counterexamples: Vec<SymCounterexample>,
    /// Statistics.
    pub stats: EngineStats,
}

/// Statistics for the symbolic execution engine.
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    /// Total paths explored.
    pub paths_explored: u64,
    /// Total paths pruned.
    pub paths_pruned: u64,
    /// Total paths merged.
    pub paths_merged: u64,
    /// Total loop unrollings.
    pub loops_unrolled: u64,
    /// Total function summary lookups.
    pub summary_lookups: u64,
    /// Total counterexamples found.
    pub counterexamples_found: u64,
}

impl SymbolicEngine {
    /// Creates a new symbolic execution engine with default settings.
    pub fn new() -> Self {
        Self {
            memory: SymbolicMemory::new(),
            active_paths: vec![PathCondition::new()],
            completed_paths: Vec::new(),
            summary_cache: SummaryCache::new(),
            loop_config: LoopUnrollConfig::default(),
            explosion_config: ExplosionMitigationConfig::default(),
            counterexamples: Vec::new(),
            stats: EngineStats::default(),
        }
    }

    /// Initializes a symbolic variable in memory.
    pub fn init_symbolic_var(&mut self, name: &str) {
        self.memory
            .write_stack(name.to_string(), SymValue::Symbolic(name.to_string()));
    }

    /// Initializes a concrete variable in memory.
    pub fn init_concrete_var(&mut self, name: &str, val: ConcreteVal) {
        self.memory
            .write_stack(name.to_string(), SymValue::Concrete(val));
    }

    /// Processes a branch point, forking all active paths.
    pub fn fork_on_branch(&mut self, condition: &str) {
        let mut new_paths = Vec::new();
        for path in &self.active_paths {
            let (true_path, false_path) = path.fork(condition);
            new_paths.push(true_path);
            new_paths.push(false_path);
        }
        self.stats.paths_explored += new_paths.len() as u64;
        self.active_paths = prune_paths(&new_paths, &self.explosion_config);
        self.stats.paths_pruned += new_paths.len() as u64 - self.active_paths.len() as u64;
    }

    /// Checks a property against all active paths.
    pub fn check_property(
        &mut self,
        property: &str,
        file: &str,
        line: u32,
    ) -> Vec<SymCounterexample> {
        let mut violations = Vec::new();
        for path in &self.active_paths {
            if let Some(ce) = generate_counterexample(path, property, file, line, &HashMap::new()) {
                violations.push(ce);
            }
        }
        self.stats.counterexamples_found += violations.len() as u64;
        self.counterexamples.extend(violations.clone());
        violations
    }

    /// Completes the current active paths (marks them as finished).
    pub fn complete_paths(&mut self) {
        self.completed_paths.append(&mut self.active_paths);
    }

    /// Returns total number of paths (active + completed).
    pub fn total_paths(&self) -> usize {
        self.active_paths.len() + self.completed_paths.len()
    }

    /// Resets the engine for a new analysis.
    pub fn reset(&mut self) {
        self.memory.clear();
        self.active_paths = vec![PathCondition::new()];
        self.completed_paths.clear();
        self.counterexamples.clear();
        self.stats = EngineStats::default();
    }
}

impl Default for SymbolicEngine {
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

    // --- V1.1: SymValue ---

    #[test]
    fn v1_1_sym_value_concrete() {
        let v = SymValue::Concrete(ConcreteVal::Int(42));
        assert_eq!(format!("{v}"), "42");
    }

    #[test]
    fn v1_1_sym_value_symbolic() {
        let v = SymValue::Symbolic("x".to_string());
        assert_eq!(format!("{v}"), "sym(x)");
    }

    #[test]
    fn v1_1_sym_value_constrained() {
        let v = SymValue::Constrained {
            name: "x".to_string(),
            constraints: vec!["x > 0".to_string(), "x < 100".to_string()],
        };
        let s = format!("{v}");
        assert!(s.contains("x where"));
        assert!(s.contains("x > 0"));
    }

    // --- V1.2: SymExpr ---

    #[test]
    fn v1_2_sym_expr_display() {
        let expr = SymExpr::BinOp(
            Box::new(SymExpr::Var("x".to_string())),
            SymBinOp::Add,
            Box::new(SymExpr::Lit(ConcreteVal::Int(1))),
        );
        assert_eq!(format!("{expr}"), "(x + 1)");
    }

    #[test]
    fn v1_2_sym_expr_ite() {
        let expr = SymExpr::Ite(
            Box::new(SymExpr::Var("cond".to_string())),
            Box::new(SymExpr::Lit(ConcreteVal::Int(1))),
            Box::new(SymExpr::Lit(ConcreteVal::Int(0))),
        );
        let s = format!("{expr}");
        assert!(s.contains("if cond then 1 else 0"));
    }

    #[test]
    fn v1_2_sym_expr_select_store() {
        let select = SymExpr::Select(
            Box::new(SymExpr::Var("arr".to_string())),
            Box::new(SymExpr::Lit(ConcreteVal::Int(3))),
        );
        assert_eq!(format!("{select}"), "arr[3]");

        let store = SymExpr::Store(
            Box::new(SymExpr::Var("arr".to_string())),
            Box::new(SymExpr::Lit(ConcreteVal::Int(0))),
            Box::new(SymExpr::Lit(ConcreteVal::Int(99))),
        );
        assert!(format!("{store}").contains("store(arr, 0, 99)"));
    }

    // --- V1.3: PathCondition ---

    #[test]
    fn v1_3_path_condition_empty() {
        let pc = PathCondition::new();
        assert!(pc.is_empty());
        assert_eq!(pc.len(), 0);
        assert!(pc.feasible);
        assert_eq!(format!("{pc}"), "true");
    }

    #[test]
    fn v1_3_path_condition_add_constraint() {
        let mut pc = PathCondition::new();
        pc.add_constraint("x > 0".to_string());
        pc.add_constraint("y < 10".to_string());
        assert_eq!(pc.len(), 2);
        assert_eq!(pc.depth, 2);
        assert_eq!(format!("{pc}"), "x > 0 && y < 10");
    }

    #[test]
    fn v1_3_path_condition_fork() {
        let mut pc = PathCondition::new();
        pc.add_constraint("x > 0".to_string());
        let (true_branch, false_branch) = pc.fork("y == 5");
        assert_eq!(true_branch.len(), 2);
        assert!(true_branch.constraints.contains(&"y == 5".to_string()));
        assert_eq!(false_branch.len(), 2);
        assert!(false_branch.constraints.contains(&"!(y == 5)".to_string()));
    }

    #[test]
    fn v1_3_path_condition_feasibility() {
        let mut pc = PathCondition::new();
        pc.add_constraint("x > 0".to_string());
        pc.add_constraint("!(x > 0)".to_string());
        assert!(!pc.check_feasibility());
        assert!(!pc.feasible);
    }

    #[test]
    fn v1_3_path_condition_smtlib2() {
        let mut pc = PathCondition::new();
        pc.add_constraint("(> x 0)".to_string());
        pc.add_constraint("(< y 10)".to_string());
        let smt = pc.to_smtlib2();
        assert!(smt.contains("(and"));
    }

    // --- V1.4: SymbolicMemory ---

    #[test]
    fn v1_4_symbolic_memory_stack() {
        let mut mem = SymbolicMemory::new();
        mem.write_stack("x".to_string(), SymValue::Concrete(ConcreteVal::Int(42)));
        assert_eq!(mem.stack_size(), 1);
        let val = mem.read_stack("x");
        assert!(val.is_some());
        assert_eq!(val.cloned(), Some(SymValue::Concrete(ConcreteVal::Int(42))));
    }

    #[test]
    fn v1_4_symbolic_memory_heap() {
        let mut mem = SymbolicMemory::new();
        mem.write_heap("0x1000".to_string(), SymValue::Symbolic("data".to_string()));
        assert_eq!(mem.heap_size(), 1);
        assert!(mem.read_heap("0x1000").is_some());
        assert!(mem.read_heap("0x2000").is_none());
    }

    #[test]
    fn v1_4_symbolic_memory_versioning() {
        let mut mem = SymbolicMemory::new();
        assert_eq!(mem.version(), 0);
        mem.write_stack("x".to_string(), SymValue::Concrete(ConcreteVal::Int(1)));
        assert_eq!(mem.version(), 1);
        mem.write_stack("x".to_string(), SymValue::Concrete(ConcreteVal::Int(2)));
        assert_eq!(mem.version(), 2);
    }

    #[test]
    fn v1_4_symbolic_memory_snapshot_and_clear() {
        let mut mem = SymbolicMemory::new();
        mem.write_stack("a".to_string(), SymValue::Concrete(ConcreteVal::Int(10)));
        let snap = mem.snapshot();
        assert_eq!(snap.stack_size(), 1);
        mem.clear();
        assert_eq!(mem.stack_size(), 0);
        assert_eq!(mem.version(), 0);
        // Snapshot still valid
        assert_eq!(snap.stack_size(), 1);
    }

    // --- V1.5: Loop Unrolling ---

    #[test]
    fn v1_5_loop_unroll_basic() {
        let config = LoopUnrollConfig {
            max_iterations: 3,
            add_summary: true,
            widen_after_bound: false,
        };
        let result = unroll_loop(&config, "i < 10", &["i = i + 1".to_string()]);
        assert_eq!(result.iterations_unrolled, 3);
        assert!(result.bound_reached);
        assert!(result.summary_constraint.is_some());
        // 3 iteration paths + 1 exit path
        assert_eq!(result.iteration_paths.len(), 4);
    }

    #[test]
    fn v1_5_loop_unroll_no_summary() {
        let config = LoopUnrollConfig {
            max_iterations: 2,
            add_summary: false,
            widen_after_bound: false,
        };
        let result = unroll_loop(&config, "x != 0", &[]);
        assert!(result.bound_reached);
        assert!(result.summary_constraint.is_none());
    }

    // --- V1.6: Function Summaries ---

    #[test]
    fn v1_6_summary_cache() {
        let mut cache = SummaryCache::new();
        assert_eq!(cache.size(), 0);

        cache.insert(FunctionSummary {
            name: "abs".to_string(),
            params: vec!["x".to_string()],
            preconditions: vec![],
            postconditions: vec!["result >= 0".to_string()],
            modifies: vec![],
            may_diverge: false,
            is_pure: true,
        });
        assert_eq!(cache.size(), 1);

        let summary = cache.get("abs");
        assert!(summary.is_some());
        assert!(summary.is_some_and(|s| s.is_pure));
        assert_eq!(cache.hits, 1);

        let miss = cache.get("unknown_fn");
        assert!(miss.is_none());
        assert_eq!(cache.misses, 1);
        assert!((cache.hit_rate() - 0.5).abs() < 0.001);
    }

    // --- V1.7: Path Explosion Mitigation ---

    #[test]
    fn v1_7_merge_paths_shared_prefix() {
        let mut a = PathCondition::new();
        a.add_constraint("x > 0".to_string());
        a.add_constraint("y == 1".to_string());

        let mut b = PathCondition::new();
        b.add_constraint("x > 0".to_string());
        b.add_constraint("y == 2".to_string());

        let merged = merge_paths(&a, &b);
        // Shared prefix "x > 0" + ITE for divergent suffix
        assert_eq!(merged.len(), 2);
        assert!(merged.constraints[0] == "x > 0");
        assert!(merged.constraints[1].contains("ite"));
    }

    #[test]
    fn v1_7_prune_paths() {
        let config = ExplosionMitigationConfig {
            max_paths: 2,
            max_depth: 5,
            strategy: MergeStrategy::None,
            use_summaries: false,
        };
        let mut deep_path = PathCondition::new();
        for i in 0..10 {
            deep_path.add_constraint(format!("c{i}"));
        }
        let shallow_path = PathCondition::new();

        let paths = vec![shallow_path.clone(), deep_path, shallow_path];
        let pruned = prune_paths(&paths, &config);
        // deep_path exceeds max_depth=5, and max_paths=2
        assert!(pruned.len() <= 2);
    }

    // --- V1.8: Concolic Execution ---

    #[test]
    fn v1_8_concolic_basic() {
        let inputs = HashMap::from([
            ("x".to_string(), ConcreteVal::Int(5)),
            ("y".to_string(), ConcreteVal::Int(3)),
        ]);
        let mut state = ConcolicState::new(inputs);
        assert_eq!(state.paths_explored(), 1);

        state.branch("x > 0", true);
        assert_eq!(state.path_condition.len(), 1);
        assert!(
            state
                .path_condition
                .constraints
                .contains(&"x > 0".to_string())
        );

        let alt = state.generate_alternative();
        assert!(alt.is_some());
        assert_eq!(alt.as_deref(), Some("!(x > 0)"));
        assert_eq!(state.paths_explored(), 2);
    }

    #[test]
    fn v1_8_concolic_update_var() {
        let inputs = HashMap::from([("x".to_string(), ConcreteVal::Int(1))]);
        let mut state = ConcolicState::new(inputs);
        state.update_var(
            "x",
            ConcreteVal::Int(2),
            SymExpr::BinOp(
                Box::new(SymExpr::Var("x".to_string())),
                SymBinOp::Add,
                Box::new(SymExpr::Lit(ConcreteVal::Int(1))),
            ),
        );
        assert_eq!(state.concrete.get("x"), Some(&ConcreteVal::Int(2)));
        assert!(state.symbolic.contains_key("x"));
    }

    // --- V1.9: Counterexample Generation ---

    #[test]
    fn v1_9_counterexample_display() {
        let ce = SymCounterexample {
            assignments: HashMap::from([
                ("x".to_string(), ConcreteVal::Int(-1)),
                ("y".to_string(), ConcreteVal::Int(10)),
            ]),
            path_condition: PathCondition::new(),
            violated_property: "x >= 0".to_string(),
            file: "main.fj".to_string(),
            line: 42,
        };
        let s = format!("{ce}");
        assert!(s.contains("main.fj:42"));
        assert!(s.contains("x >= 0"));
        assert!(s.contains("x = -1"));
    }

    #[test]
    fn v1_9_generate_counterexample_with_values() {
        let path = PathCondition::new();
        let values = HashMap::from([("idx".to_string(), ConcreteVal::Int(10))]);
        let ce = generate_counterexample(&path, "idx < len", "test.fj", 5, &values);
        assert!(ce.is_some());
        let ce = ce.expect("counterexample should exist in test");
        assert_eq!(ce.violated_property, "idx < len");
        assert_eq!(ce.line, 5);
    }

    #[test]
    fn v1_9_generate_counterexample_infeasible() {
        let mut path = PathCondition::new();
        path.add_constraint("x > 0".to_string());
        path.add_constraint("!(x > 0)".to_string());
        let ce = generate_counterexample(&path, "anything", "test.fj", 1, &HashMap::new());
        assert!(ce.is_none()); // Infeasible path, no counterexample
    }

    // --- V1.10: SymbolicEngine ---

    #[test]
    fn v1_10_engine_init() {
        let engine = SymbolicEngine::new();
        assert_eq!(engine.active_paths.len(), 1);
        assert_eq!(engine.completed_paths.len(), 0);
        assert_eq!(engine.total_paths(), 1);
    }

    #[test]
    fn v1_10_engine_symbolic_var() {
        let mut engine = SymbolicEngine::new();
        engine.init_symbolic_var("x");
        let val = engine.memory.read_stack("x");
        assert!(val.is_some());
        assert!(matches!(val, Some(SymValue::Symbolic(_))));
    }

    #[test]
    fn v1_10_engine_fork_and_check() {
        let mut engine = SymbolicEngine::new();
        engine.init_symbolic_var("x");
        engine.fork_on_branch("x > 0");
        // Should have 2 active paths (true and false branches)
        assert_eq!(engine.active_paths.len(), 2);
        assert!(engine.stats.paths_explored >= 2);
    }

    #[test]
    fn v1_10_engine_complete_and_reset() {
        let mut engine = SymbolicEngine::new();
        engine.fork_on_branch("x > 0");
        engine.complete_paths();
        assert_eq!(engine.active_paths.len(), 0);
        assert_eq!(engine.completed_paths.len(), 2);

        engine.reset();
        assert_eq!(engine.active_paths.len(), 1);
        assert_eq!(engine.completed_paths.len(), 0);
        assert_eq!(engine.memory.stack_size(), 0);
    }

    #[test]
    fn v1_10_engine_check_property() {
        let mut engine = SymbolicEngine::new();
        let violations = engine.check_property("x >= 0", "test.fj", 10);
        // With an unconstrained path, a counterexample is possible
        assert!(!violations.is_empty());
        assert_eq!(engine.stats.counterexamples_found, violations.len() as u64);
    }
}
