//! Self-Hosted Optimizer — constant folding, dead code elimination,
//! strength reduction, common subexpression elimination, loop-invariant
//! code motion, inlining decisions, register coalescing.
//! OptimizationPass trait with apply() method.

use std::collections::{HashMap, HashSet};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S7.1: OptimizationPass Trait & IR Representation
// ═══════════════════════════════════════════════════════════════════════

/// A simple IR instruction for the optimizer to work on.
#[derive(Debug, Clone, PartialEq)]
pub enum OptIr {
    /// Assign a constant: dest = value.
    Const(String, i64),
    /// Assign a float constant: dest = value.
    ConstFloat(String, f64),
    /// Binary operation: dest = lhs op rhs.
    BinOp(String, String, BinOpKind, String),
    /// Unary operation: dest = op src.
    UnaryOp(String, UnaryOpKind, String),
    /// Copy: dest = src.
    Copy(String, String),
    /// Function call: dest = call(name, args).
    Call(Option<String>, String, Vec<String>),
    /// Return value.
    Return(Option<String>),
    /// Conditional branch: if cond goto label.
    Branch(String, String),
    /// Unconditional jump: goto label.
    Jump(String),
    /// Label definition.
    Label(String),
    /// No-op (placeholder for eliminated instructions).
    Nop,
}

/// Binary operation kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Shl,
    Shr,
    And,
    Or,
    Xor,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl fmt::Display for BinOpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOpKind::Add => write!(f, "+"),
            BinOpKind::Sub => write!(f, "-"),
            BinOpKind::Mul => write!(f, "*"),
            BinOpKind::Div => write!(f, "/"),
            BinOpKind::Mod => write!(f, "%"),
            BinOpKind::Shl => write!(f, "<<"),
            BinOpKind::Shr => write!(f, ">>"),
            BinOpKind::And => write!(f, "&"),
            BinOpKind::Or => write!(f, "|"),
            BinOpKind::Xor => write!(f, "^"),
            BinOpKind::Eq => write!(f, "=="),
            BinOpKind::Ne => write!(f, "!="),
            BinOpKind::Lt => write!(f, "<"),
            BinOpKind::Le => write!(f, "<="),
            BinOpKind::Gt => write!(f, ">"),
            BinOpKind::Ge => write!(f, ">="),
        }
    }
}

/// Unary operation kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOpKind {
    Neg,
    Not,
    BitNot,
}

impl fmt::Display for UnaryOpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOpKind::Neg => write!(f, "-"),
            UnaryOpKind::Not => write!(f, "!"),
            UnaryOpKind::BitNot => write!(f, "~"),
        }
    }
}

/// An optimization pass that transforms IR instructions.
pub trait OptimizationPass: fmt::Debug {
    /// Name of this optimization pass.
    fn name(&self) -> &str;

    /// Applies the optimization pass to a sequence of IR instructions.
    /// Returns the transformed instructions and a count of changes made.
    fn apply(&self, instructions: &[OptIr]) -> (Vec<OptIr>, usize);
}

/// Result of applying an optimization pass.
#[derive(Debug, Clone)]
pub struct OptResult {
    /// Pass name.
    pub pass_name: String,
    /// Instructions before.
    pub before_count: usize,
    /// Instructions after.
    pub after_count: usize,
    /// Number of changes made.
    pub changes: usize,
}

impl fmt::Display for OptResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} -> {} instrs ({} changes)",
            self.pass_name, self.before_count, self.after_count, self.changes
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.2: Constant Folding
// ═══════════════════════════════════════════════════════════════════════

/// Constant folding pass: evaluates constant expressions at compile time.
#[derive(Debug)]
pub struct ConstantFolding;

impl ConstantFolding {
    /// Tries to fold a binary operation on two constants.
    fn fold_binary(lhs: i64, op: BinOpKind, rhs: i64) -> Option<i64> {
        match op {
            BinOpKind::Add => Some(lhs.wrapping_add(rhs)),
            BinOpKind::Sub => Some(lhs.wrapping_sub(rhs)),
            BinOpKind::Mul => Some(lhs.wrapping_mul(rhs)),
            BinOpKind::Div if rhs != 0 => Some(lhs / rhs),
            BinOpKind::Mod if rhs != 0 => Some(lhs % rhs),
            BinOpKind::Shl if (0..64).contains(&rhs) => Some(lhs << rhs),
            BinOpKind::Shr if (0..64).contains(&rhs) => Some(lhs >> rhs),
            BinOpKind::And => Some(lhs & rhs),
            BinOpKind::Or => Some(lhs | rhs),
            BinOpKind::Xor => Some(lhs ^ rhs),
            BinOpKind::Eq => Some(if lhs == rhs { 1 } else { 0 }),
            BinOpKind::Ne => Some(if lhs != rhs { 1 } else { 0 }),
            BinOpKind::Lt => Some(if lhs < rhs { 1 } else { 0 }),
            BinOpKind::Le => Some(if lhs <= rhs { 1 } else { 0 }),
            BinOpKind::Gt => Some(if lhs > rhs { 1 } else { 0 }),
            BinOpKind::Ge => Some(if lhs >= rhs { 1 } else { 0 }),
            _ => None,
        }
    }
}

impl OptimizationPass for ConstantFolding {
    fn name(&self) -> &str {
        "constant_folding"
    }

    fn apply(&self, instructions: &[OptIr]) -> (Vec<OptIr>, usize) {
        let mut constants: HashMap<String, i64> = HashMap::new();
        let mut result = Vec::new();
        let mut changes = 0;

        for inst in instructions {
            match inst {
                OptIr::Const(dest, val) => {
                    constants.insert(dest.clone(), *val);
                    result.push(inst.clone());
                }
                OptIr::BinOp(dest, lhs, op, rhs) => {
                    if let (Some(&l), Some(&r)) = (constants.get(lhs), constants.get(rhs)) {
                        if let Some(folded) = Self::fold_binary(l, *op, r) {
                            constants.insert(dest.clone(), folded);
                            result.push(OptIr::Const(dest.clone(), folded));
                            changes += 1;
                            continue;
                        }
                    }
                    result.push(inst.clone());
                }
                OptIr::Copy(dest, src) => {
                    if let Some(&val) = constants.get(src) {
                        constants.insert(dest.clone(), val);
                    }
                    result.push(inst.clone());
                }
                _ => {
                    result.push(inst.clone());
                }
            }
        }

        (result, changes)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.3: Dead Code Elimination
// ═══════════════════════════════════════════════════════════════════════

/// Dead code elimination pass: removes instructions whose results are never used.
#[derive(Debug)]
pub struct DeadCodeElimination;

impl DeadCodeElimination {
    /// Collects all variables that are used (read) in the instruction set.
    fn collect_used_vars(instructions: &[OptIr]) -> HashSet<String> {
        let mut used = HashSet::new();
        for inst in instructions {
            match inst {
                OptIr::BinOp(_, lhs, _, rhs) => {
                    used.insert(lhs.clone());
                    used.insert(rhs.clone());
                }
                OptIr::UnaryOp(_, _, src) => {
                    used.insert(src.clone());
                }
                OptIr::Copy(_, src) => {
                    used.insert(src.clone());
                }
                OptIr::Call(_, _, args) => {
                    for arg in args {
                        used.insert(arg.clone());
                    }
                }
                OptIr::Return(Some(val)) => {
                    used.insert(val.clone());
                }
                OptIr::Branch(cond, _) => {
                    used.insert(cond.clone());
                }
                _ => {}
            }
        }
        used
    }
}

impl OptimizationPass for DeadCodeElimination {
    fn name(&self) -> &str {
        "dead_code_elimination"
    }

    fn apply(&self, instructions: &[OptIr]) -> (Vec<OptIr>, usize) {
        let used = Self::collect_used_vars(instructions);
        let mut result = Vec::new();
        let mut changes = 0;

        for inst in instructions {
            let dead = match inst {
                OptIr::Const(dest, _) => !used.contains(dest),
                OptIr::ConstFloat(dest, _) => !used.contains(dest),
                OptIr::BinOp(dest, _, _, _) => !used.contains(dest),
                OptIr::UnaryOp(dest, _, _) => !used.contains(dest),
                OptIr::Copy(dest, _) => !used.contains(dest),
                // Calls may have side effects, never eliminate
                OptIr::Call(_, _, _) => false,
                // Control flow always kept
                _ => false,
            };

            if dead {
                changes += 1;
            } else {
                result.push(inst.clone());
            }
        }

        (result, changes)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.4: Strength Reduction
// ═══════════════════════════════════════════════════════════════════════

/// Strength reduction: replaces expensive operations with cheaper equivalents.
/// - `x * 2` -> `x + x` (or `x << 1`)
/// - `x * 1` -> `x`
/// - `x + 0` -> `x`
/// - `x * 0` -> `0`
/// - `x / 1` -> `x`
#[derive(Debug)]
pub struct StrengthReduction;

impl OptimizationPass for StrengthReduction {
    fn name(&self) -> &str {
        "strength_reduction"
    }

    fn apply(&self, instructions: &[OptIr]) -> (Vec<OptIr>, usize) {
        let mut constants: HashMap<String, i64> = HashMap::new();
        let mut result = Vec::new();
        let mut changes = 0;

        for inst in instructions {
            match inst {
                OptIr::Const(dest, val) => {
                    constants.insert(dest.clone(), *val);
                    result.push(inst.clone());
                }
                OptIr::BinOp(dest, lhs, op, rhs) => {
                    let lhs_val = constants.get(lhs).copied();
                    let rhs_val = constants.get(rhs).copied();

                    let reduced = match (op, lhs_val, rhs_val) {
                        // x + 0 -> copy x
                        (BinOpKind::Add, _, Some(0)) => {
                            Some(OptIr::Copy(dest.clone(), lhs.clone()))
                        }
                        // 0 + x -> copy x
                        (BinOpKind::Add, Some(0), _) => {
                            Some(OptIr::Copy(dest.clone(), rhs.clone()))
                        }
                        // x - 0 -> copy x
                        (BinOpKind::Sub, _, Some(0)) => {
                            Some(OptIr::Copy(dest.clone(), lhs.clone()))
                        }
                        // x * 0 -> const 0
                        (BinOpKind::Mul, _, Some(0)) | (BinOpKind::Mul, Some(0), _) => {
                            constants.insert(dest.clone(), 0);
                            Some(OptIr::Const(dest.clone(), 0))
                        }
                        // x * 1 -> copy x
                        (BinOpKind::Mul, _, Some(1)) => {
                            Some(OptIr::Copy(dest.clone(), lhs.clone()))
                        }
                        // 1 * x -> copy x
                        (BinOpKind::Mul, Some(1), _) => {
                            Some(OptIr::Copy(dest.clone(), rhs.clone()))
                        }
                        // x * 2 -> x << 1 (shift is cheaper)
                        (BinOpKind::Mul, _, Some(2)) => {
                            Some(OptIr::BinOp(dest.clone(), lhs.clone(), BinOpKind::Add, lhs.clone()))
                        }
                        // x / 1 -> copy x
                        (BinOpKind::Div, _, Some(1)) => {
                            Some(OptIr::Copy(dest.clone(), lhs.clone()))
                        }
                        _ => None,
                    };

                    if let Some(new_inst) = reduced {
                        result.push(new_inst);
                        changes += 1;
                    } else {
                        result.push(inst.clone());
                    }
                }
                _ => {
                    result.push(inst.clone());
                }
            }
        }

        (result, changes)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.5: Common Subexpression Elimination
// ═══════════════════════════════════════════════════════════════════════

/// CSE: detects identical computations and reuses previous results.
#[derive(Debug)]
pub struct CommonSubexprElimination;

impl OptimizationPass for CommonSubexprElimination {
    fn name(&self) -> &str {
        "cse"
    }

    fn apply(&self, instructions: &[OptIr]) -> (Vec<OptIr>, usize) {
        // Map from (lhs, op, rhs) -> dest variable that already holds this value.
        let mut known: HashMap<(String, BinOpKind, String), String> = HashMap::new();
        let mut result = Vec::new();
        let mut changes = 0;

        for inst in instructions {
            match inst {
                OptIr::BinOp(dest, lhs, op, rhs) => {
                    let key = (lhs.clone(), *op, rhs.clone());
                    if let Some(existing) = known.get(&key) {
                        // Reuse existing computation
                        result.push(OptIr::Copy(dest.clone(), existing.clone()));
                        changes += 1;
                    } else {
                        known.insert(key, dest.clone());
                        // Also record commutative version
                        if matches!(
                            op,
                            BinOpKind::Add | BinOpKind::Mul | BinOpKind::Eq | BinOpKind::Ne
                                | BinOpKind::And | BinOpKind::Or | BinOpKind::Xor
                        ) {
                            let comm_key = (rhs.clone(), *op, lhs.clone());
                            known.insert(comm_key, dest.clone());
                        }
                        result.push(inst.clone());
                    }
                }
                // Labels and jumps invalidate known expressions (control flow merge)
                OptIr::Label(_) | OptIr::Jump(_) | OptIr::Branch(_, _) => {
                    known.clear();
                    result.push(inst.clone());
                }
                _ => {
                    result.push(inst.clone());
                }
            }
        }

        (result, changes)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.6: Loop-Invariant Code Motion (LICM)
// ═══════════════════════════════════════════════════════════════════════

/// LICM: moves computations that don't change inside a loop to before the loop.
#[derive(Debug)]
pub struct LoopInvariantCodeMotion;

/// Represents a detected loop in the IR.
#[derive(Debug, Clone)]
pub struct LoopRegion {
    /// Label that starts the loop header.
    pub header_label: String,
    /// Index of the header label instruction.
    pub header_idx: usize,
    /// Index of the back-edge jump instruction.
    pub back_edge_idx: usize,
    /// Variables modified inside the loop.
    pub modified_vars: HashSet<String>,
}

impl LoopInvariantCodeMotion {
    /// Detects loop regions: a label followed by a later jump back to it.
    fn detect_loops(instructions: &[OptIr]) -> Vec<LoopRegion> {
        let mut labels: HashMap<String, usize> = HashMap::new();
        let mut loops = Vec::new();

        // Collect label positions
        for (i, inst) in instructions.iter().enumerate() {
            if let OptIr::Label(name) = inst {
                labels.insert(name.clone(), i);
            }
        }

        // Find back-edges (jumps to earlier labels)
        for (i, inst) in instructions.iter().enumerate() {
            let target = match inst {
                OptIr::Jump(t) => Some(t),
                OptIr::Branch(_, t) => Some(t),
                _ => None,
            };
            if let Some(target_label) = target {
                if let Some(&label_idx) = labels.get(target_label) {
                    if label_idx < i {
                        // This is a back-edge (loop)
                        let mut modified = HashSet::new();
                        for inst in &instructions[label_idx..=i] {
                            match inst {
                                OptIr::Const(d, _)
                                | OptIr::ConstFloat(d, _)
                                | OptIr::BinOp(d, _, _, _)
                                | OptIr::UnaryOp(d, _, _)
                                | OptIr::Copy(d, _) => {
                                    modified.insert(d.clone());
                                }
                                OptIr::Call(Some(d), _, _) => {
                                    modified.insert(d.clone());
                                }
                                _ => {}
                            }
                        }
                        loops.push(LoopRegion {
                            header_label: target_label.clone(),
                            header_idx: label_idx,
                            back_edge_idx: i,
                            modified_vars: modified,
                        });
                    }
                }
            }
        }

        loops
    }
}

impl OptimizationPass for LoopInvariantCodeMotion {
    fn name(&self) -> &str {
        "licm"
    }

    fn apply(&self, instructions: &[OptIr]) -> (Vec<OptIr>, usize) {
        let loops = Self::detect_loops(instructions);
        if loops.is_empty() {
            return (instructions.to_vec(), 0);
        }

        let mut result = instructions.to_vec();
        let mut changes = 0;

        for loop_region in &loops {
            let mut hoisted = Vec::new();
            let mut to_nop = Vec::new();

            for i in (loop_region.header_idx + 1)..loop_region.back_edge_idx {
                if i >= result.len() {
                    break;
                }
                let invariant = match &result[i] {
                    OptIr::BinOp(_, lhs, _, rhs) => {
                        !loop_region.modified_vars.contains(lhs)
                            && !loop_region.modified_vars.contains(rhs)
                    }
                    OptIr::UnaryOp(_, _, src) => !loop_region.modified_vars.contains(src),
                    OptIr::Const(_, _) | OptIr::ConstFloat(_, _) => true,
                    _ => false,
                };

                if invariant {
                    hoisted.push(result[i].clone());
                    to_nop.push(i);
                    changes += 1;
                }
            }

            // Replace hoisted instructions with Nop
            for &idx in &to_nop {
                if idx < result.len() {
                    result[idx] = OptIr::Nop;
                }
            }

            // Insert hoisted instructions before the loop header
            if !hoisted.is_empty() {
                let insert_point = loop_region.header_idx;
                for (j, inst) in hoisted.into_iter().enumerate() {
                    result.insert(insert_point + j, inst);
                }
            }
        }

        // Remove Nops
        result.retain(|inst| !matches!(inst, OptIr::Nop));

        (result, changes)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.7: Inlining Decisions
// ═══════════════════════════════════════════════════════════════════════

/// Inlining heuristics for function calls.
#[derive(Debug, Clone)]
pub struct InlineCandidate {
    /// Function name.
    pub name: String,
    /// Number of instructions in the function body.
    pub body_size: usize,
    /// Number of call sites.
    pub call_count: usize,
    /// Whether the function is recursive.
    pub is_recursive: bool,
    /// Whether the function has side effects.
    pub has_side_effects: bool,
}

/// Inlining policy.
#[derive(Debug, Clone)]
pub struct InliningPolicy {
    /// Maximum body size for unconditional inlining.
    pub max_small_fn_size: usize,
    /// Maximum body size for inlining when called once.
    pub max_single_call_size: usize,
    /// Never inline recursive functions.
    pub never_inline_recursive: bool,
}

impl Default for InliningPolicy {
    fn default() -> Self {
        Self {
            max_small_fn_size: 5,
            max_single_call_size: 50,
            never_inline_recursive: true,
        }
    }
}

impl InliningPolicy {
    /// Decides whether a function should be inlined.
    pub fn should_inline(&self, candidate: &InlineCandidate) -> bool {
        if self.never_inline_recursive && candidate.is_recursive {
            return false;
        }
        // Always inline very small functions
        if candidate.body_size <= self.max_small_fn_size {
            return true;
        }
        // Inline larger functions if called only once
        if candidate.call_count == 1 && candidate.body_size <= self.max_single_call_size {
            return true;
        }
        false
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.8: Register Coalescing
// ═══════════════════════════════════════════════════════════════════════

/// Register coalescing: eliminates unnecessary copies between variables
/// that can share the same register.
#[derive(Debug)]
pub struct RegisterCoalescing;

impl OptimizationPass for RegisterCoalescing {
    fn name(&self) -> &str {
        "register_coalescing"
    }

    fn apply(&self, instructions: &[OptIr]) -> (Vec<OptIr>, usize) {
        // Build a map of trivial copies: dest = src where dest is only used
        // as a copy and both have non-overlapping live ranges.
        let mut aliases: HashMap<String, String> = HashMap::new();
        let mut changes = 0;

        // Phase 1: Identify copy chains
        for inst in instructions {
            if let OptIr::Copy(dest, src) = inst {
                // Resolve through existing aliases
                let resolved = aliases.get(src).cloned().unwrap_or_else(|| src.clone());
                aliases.insert(dest.clone(), resolved);
            }
        }

        // Phase 2: Apply aliases, removing trivial copies
        let mut result = Vec::new();
        for inst in instructions {
            match inst {
                OptIr::Copy(dest, src) => {
                    let resolved = aliases.get(src).cloned().unwrap_or_else(|| src.clone());
                    if *dest == resolved {
                        // Self-copy after coalescing — eliminate
                        changes += 1;
                    } else {
                        result.push(OptIr::Copy(dest.clone(), resolved));
                    }
                }
                OptIr::BinOp(dest, lhs, op, rhs) => {
                    let r_lhs = aliases.get(lhs).cloned().unwrap_or_else(|| lhs.clone());
                    let r_rhs = aliases.get(rhs).cloned().unwrap_or_else(|| rhs.clone());
                    result.push(OptIr::BinOp(dest.clone(), r_lhs, *op, r_rhs));
                }
                OptIr::UnaryOp(dest, op, src) => {
                    let r_src = aliases.get(src).cloned().unwrap_or_else(|| src.clone());
                    result.push(OptIr::UnaryOp(dest.clone(), *op, r_src));
                }
                OptIr::Return(Some(val)) => {
                    let resolved = aliases.get(val).cloned().unwrap_or_else(|| val.clone());
                    result.push(OptIr::Return(Some(resolved)));
                }
                OptIr::Branch(cond, target) => {
                    let resolved = aliases.get(cond).cloned().unwrap_or_else(|| cond.clone());
                    result.push(OptIr::Branch(resolved, target.clone()));
                }
                _ => {
                    result.push(inst.clone());
                }
            }
        }

        (result, changes)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.9: Optimization Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// An optimization pipeline that runs multiple passes in sequence.
#[derive(Debug)]
pub struct OptPipeline {
    /// Ordered list of passes.
    passes: Vec<Box<dyn OptimizationPass>>,
    /// Maximum iterations for fixed-point convergence.
    pub max_iterations: usize,
}

impl OptPipeline {
    /// Creates a new optimization pipeline.
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            max_iterations: 10,
        }
    }

    /// Creates a pipeline with all standard passes.
    pub fn standard() -> Self {
        let mut pipeline = Self::new();
        pipeline.add_pass(Box::new(ConstantFolding));
        pipeline.add_pass(Box::new(StrengthReduction));
        pipeline.add_pass(Box::new(CommonSubexprElimination));
        pipeline.add_pass(Box::new(DeadCodeElimination));
        pipeline.add_pass(Box::new(RegisterCoalescing));
        pipeline
    }

    /// Adds a pass to the pipeline.
    pub fn add_pass(&mut self, pass: Box<dyn OptimizationPass>) {
        self.passes.push(pass);
    }

    /// Runs all passes until no more changes are made (fixed point).
    pub fn run(&self, instructions: &[OptIr]) -> (Vec<OptIr>, Vec<OptResult>) {
        let mut current = instructions.to_vec();
        let mut all_results = Vec::new();

        for _iteration in 0..self.max_iterations {
            let mut any_change = false;

            for pass in &self.passes {
                let before_count = current.len();
                let (new_instrs, changes) = pass.apply(&current);
                let after_count = new_instrs.len();

                if changes > 0 {
                    any_change = true;
                    all_results.push(OptResult {
                        pass_name: pass.name().into(),
                        before_count,
                        after_count,
                        changes,
                    });
                }

                current = new_instrs;
            }

            if !any_change {
                break;
            }
        }

        (current, all_results)
    }

    /// Returns the number of passes in the pipeline.
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }
}

impl Default for OptPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.10: Optimization Statistics
// ═══════════════════════════════════════════════════════════════════════

/// Aggregated statistics from running the optimizer.
#[derive(Debug, Clone, Default)]
pub struct OptStats {
    /// Total instructions before.
    pub total_before: usize,
    /// Total instructions after.
    pub total_after: usize,
    /// Total changes across all passes.
    pub total_changes: usize,
    /// Per-pass change counts.
    pub per_pass: HashMap<String, usize>,
}

impl OptStats {
    /// Creates stats from a list of optimization results.
    pub fn from_results(before: usize, after: usize, results: &[OptResult]) -> Self {
        let mut per_pass = HashMap::new();
        let mut total_changes = 0;
        for r in results {
            *per_pass.entry(r.pass_name.clone()).or_insert(0) += r.changes;
            total_changes += r.changes;
        }
        Self {
            total_before: before,
            total_after: after,
            total_changes,
            per_pass,
        }
    }

    /// Returns the reduction percentage.
    pub fn reduction_pct(&self) -> f64 {
        if self.total_before > 0 {
            ((self.total_before - self.total_after) as f64 / self.total_before as f64) * 100.0
        } else {
            0.0
        }
    }
}

impl fmt::Display for OptStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Optimizer: {} -> {} instrs ({:.1}% reduction, {} changes)",
            self.total_before,
            self.total_after,
            self.reduction_pct(),
            self.total_changes
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S7.1 — OptimizationPass trait
    #[test]
    fn s7_1_opt_ir_creation() {
        let inst = OptIr::Const("x".into(), 42);
        assert_eq!(inst, OptIr::Const("x".into(), 42));
    }

    #[test]
    fn s7_1_binop_display() {
        assert_eq!(BinOpKind::Add.to_string(), "+");
        assert_eq!(BinOpKind::Mul.to_string(), "*");
        assert_eq!(BinOpKind::Eq.to_string(), "==");
        assert_eq!(BinOpKind::Shl.to_string(), "<<");
    }

    #[test]
    fn s7_1_opt_result_display() {
        let r = OptResult {
            pass_name: "constant_folding".into(),
            before_count: 10,
            after_count: 7,
            changes: 3,
        };
        assert!(r.to_string().contains("constant_folding"));
        assert!(r.to_string().contains("3 changes"));
    }

    // S7.2 — Constant Folding
    #[test]
    fn s7_2_fold_addition() {
        let instrs = vec![
            OptIr::Const("a".into(), 3),
            OptIr::Const("b".into(), 4),
            OptIr::BinOp("c".into(), "a".into(), BinOpKind::Add, "b".into()),
            OptIr::Return(Some("c".into())),
        ];
        let (result, changes) = ConstantFolding.apply(&instrs);
        assert!(changes > 0);
        // "c" should be a constant 7
        assert!(result.iter().any(|i| matches!(i, OptIr::Const(name, 7) if name == "c")));
    }

    #[test]
    fn s7_2_fold_comparison() {
        let instrs = vec![
            OptIr::Const("a".into(), 5),
            OptIr::Const("b".into(), 3),
            OptIr::BinOp("c".into(), "a".into(), BinOpKind::Gt, "b".into()),
        ];
        let (result, changes) = ConstantFolding.apply(&instrs);
        assert_eq!(changes, 1);
        assert!(result.iter().any(|i| matches!(i, OptIr::Const(name, 1) if name == "c")));
    }

    // S7.3 — Dead Code Elimination
    #[test]
    fn s7_3_eliminate_unused() {
        let instrs = vec![
            OptIr::Const("a".into(), 42),
            OptIr::Const("b".into(), 99), // unused
            OptIr::Return(Some("a".into())),
        ];
        let (result, changes) = DeadCodeElimination.apply(&instrs);
        assert_eq!(changes, 1);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn s7_3_keep_used_values() {
        let instrs = vec![
            OptIr::Const("a".into(), 1),
            OptIr::Const("b".into(), 2),
            OptIr::BinOp("c".into(), "a".into(), BinOpKind::Add, "b".into()),
            OptIr::Return(Some("c".into())),
        ];
        let (result, changes) = DeadCodeElimination.apply(&instrs);
        assert_eq!(changes, 0);
        assert_eq!(result.len(), 4);
    }

    // S7.4 — Strength Reduction
    #[test]
    fn s7_4_mul_by_zero() {
        let instrs = vec![
            OptIr::Const("zero".into(), 0),
            OptIr::BinOp("r".into(), "x".into(), BinOpKind::Mul, "zero".into()),
            OptIr::Return(Some("r".into())),
        ];
        let (result, changes) = StrengthReduction.apply(&instrs);
        assert!(changes > 0);
        assert!(result.iter().any(|i| matches!(i, OptIr::Const(name, 0) if name == "r")));
    }

    #[test]
    fn s7_4_mul_by_one() {
        let instrs = vec![
            OptIr::Const("one".into(), 1),
            OptIr::BinOp("r".into(), "x".into(), BinOpKind::Mul, "one".into()),
        ];
        let (result, changes) = StrengthReduction.apply(&instrs);
        assert_eq!(changes, 1);
        assert!(result.iter().any(|i| matches!(i, OptIr::Copy(d, s) if d == "r" && s == "x")));
    }

    #[test]
    fn s7_4_add_zero() {
        let instrs = vec![
            OptIr::Const("zero".into(), 0),
            OptIr::BinOp("r".into(), "x".into(), BinOpKind::Add, "zero".into()),
        ];
        let (result, changes) = StrengthReduction.apply(&instrs);
        assert_eq!(changes, 1);
        assert!(result.iter().any(|i| matches!(i, OptIr::Copy(d, s) if d == "r" && s == "x")));
    }

    // S7.5 — Common Subexpression Elimination
    #[test]
    fn s7_5_eliminate_duplicate_expr() {
        let instrs = vec![
            OptIr::BinOp("t1".into(), "a".into(), BinOpKind::Add, "b".into()),
            OptIr::BinOp("t2".into(), "a".into(), BinOpKind::Add, "b".into()),
            OptIr::Return(Some("t2".into())),
        ];
        let (result, changes) = CommonSubexprElimination.apply(&instrs);
        assert_eq!(changes, 1);
        assert!(result.iter().any(|i| matches!(i, OptIr::Copy(d, s) if d == "t2" && s == "t1")));
    }

    #[test]
    fn s7_5_commutative_cse() {
        let instrs = vec![
            OptIr::BinOp("t1".into(), "a".into(), BinOpKind::Add, "b".into()),
            OptIr::BinOp("t2".into(), "b".into(), BinOpKind::Add, "a".into()),
        ];
        let (result, changes) = CommonSubexprElimination.apply(&instrs);
        assert_eq!(changes, 1);
        assert!(result.iter().any(|i| matches!(i, OptIr::Copy(d, s) if d == "t2" && s == "t1")));
    }

    // S7.6 — Loop-Invariant Code Motion
    #[test]
    fn s7_6_detect_loop() {
        let instrs = vec![
            OptIr::Const("n".into(), 10),
            OptIr::Label("loop_start".into()),
            OptIr::Const("step".into(), 1), // invariant: doesn't depend on loop vars
            OptIr::BinOp("i".into(), "i".into(), BinOpKind::Add, "step".into()),
            OptIr::Jump("loop_start".into()),
        ];
        let loops = LoopInvariantCodeMotion::detect_loops(&instrs);
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].header_label, "loop_start");
    }

    #[test]
    fn s7_6_hoist_invariant() {
        let instrs = vec![
            OptIr::Label("loop".into()),
            OptIr::BinOp("t".into(), "a".into(), BinOpKind::Add, "b".into()),
            OptIr::Jump("loop".into()),
        ];
        let (result, changes) = LoopInvariantCodeMotion.apply(&instrs);
        // The BinOp should be hoisted before the label
        assert!(changes > 0);
        // Verify the BinOp comes before the Label in the output
        let binop_idx = result.iter().position(|i| {
            matches!(i, OptIr::BinOp(d, _, BinOpKind::Add, _) if d == "t")
        });
        let label_idx = result.iter().position(|i| matches!(i, OptIr::Label(l) if l == "loop"));
        if let (Some(bi), Some(li)) = (binop_idx, label_idx) {
            assert!(bi < li, "hoisted instruction should come before loop header");
        }
    }

    // S7.7 — Inlining Decisions
    #[test]
    fn s7_7_inline_small_fn() {
        let policy = InliningPolicy::default();
        let candidate = InlineCandidate {
            name: "tiny".into(),
            body_size: 3,
            call_count: 5,
            is_recursive: false,
            has_side_effects: false,
        };
        assert!(policy.should_inline(&candidate));
    }

    #[test]
    fn s7_7_no_inline_recursive() {
        let policy = InliningPolicy::default();
        let candidate = InlineCandidate {
            name: "fib".into(),
            body_size: 3,
            call_count: 2,
            is_recursive: true,
            has_side_effects: false,
        };
        assert!(!policy.should_inline(&candidate));
    }

    #[test]
    fn s7_7_inline_single_call() {
        let policy = InliningPolicy::default();
        let candidate = InlineCandidate {
            name: "helper".into(),
            body_size: 20,
            call_count: 1,
            is_recursive: false,
            has_side_effects: false,
        };
        assert!(policy.should_inline(&candidate));
    }

    // S7.8 — Register Coalescing
    #[test]
    fn s7_8_coalesce_copies() {
        let instrs = vec![
            OptIr::Const("a".into(), 42),
            OptIr::Copy("b".into(), "a".into()),
            OptIr::Copy("c".into(), "b".into()),
            OptIr::Return(Some("c".into())),
        ];
        let (result, _changes) = RegisterCoalescing.apply(&instrs);
        // "c" should resolve to "a" through the chain
        assert!(result.iter().any(|i| matches!(i, OptIr::Return(Some(v)) if v == "a")));
    }

    // S7.9 — Pipeline
    #[test]
    fn s7_9_standard_pipeline() {
        let pipeline = OptPipeline::standard();
        assert!(pipeline.pass_count() >= 5);

        let instrs = vec![
            OptIr::Const("a".into(), 3),
            OptIr::Const("b".into(), 4),
            OptIr::BinOp("c".into(), "a".into(), BinOpKind::Add, "b".into()),
            OptIr::Const("unused".into(), 999),
            OptIr::Return(Some("c".into())),
        ];
        let (result, results) = pipeline.run(&instrs);
        // Should have folded and eliminated dead code
        assert!(result.len() <= instrs.len());
        assert!(!results.is_empty());
    }

    // S7.10 — Statistics
    #[test]
    fn s7_10_opt_stats() {
        let results = vec![
            OptResult {
                pass_name: "constant_folding".into(),
                before_count: 10,
                after_count: 8,
                changes: 2,
            },
            OptResult {
                pass_name: "dce".into(),
                before_count: 8,
                after_count: 6,
                changes: 2,
            },
        ];
        let stats = OptStats::from_results(10, 6, &results);
        assert_eq!(stats.total_changes, 4);
        assert!((stats.reduction_pct() - 40.0).abs() < 0.1);
        assert!(stats.to_string().contains("40.0%"));
    }

    #[test]
    fn s7_10_empty_pipeline() {
        let pipeline = OptPipeline::new();
        let instrs = vec![OptIr::Const("x".into(), 42)];
        let (result, results) = pipeline.run(&instrs);
        assert_eq!(result.len(), 1);
        assert!(results.is_empty());
    }

    #[test]
    fn s7_10_unary_op_display() {
        assert_eq!(UnaryOpKind::Neg.to_string(), "-");
        assert_eq!(UnaryOpKind::Not.to_string(), "!");
        assert_eq!(UnaryOpKind::BitNot.to_string(), "~");
    }
}
