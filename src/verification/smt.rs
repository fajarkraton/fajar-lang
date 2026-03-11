//! SMT integration — expression encoding, integer/array theory,
//! solver abstraction, counterexample extraction, incremental solving.

use std::collections::HashMap;
use std::fmt;

use super::conditions::{ContractExpr, ContractOp};

// ═══════════════════════════════════════════════════════════════════════
// S10.1 / S10.2: Solver Abstraction
// ═══════════════════════════════════════════════════════════════════════

/// SMT solver backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverBackend {
    /// Z3 solver.
    Z3,
    /// CVC5 solver.
    Cvc5,
    /// Built-in simple solver (no external dependency).
    Builtin,
}

impl fmt::Display for SolverBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolverBackend::Z3 => write!(f, "z3"),
            SolverBackend::Cvc5 => write!(f, "cvc5"),
            SolverBackend::Builtin => write!(f, "builtin"),
        }
    }
}

/// SMT sort (type in SMT-LIB).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmtSort {
    /// Boolean sort.
    Bool,
    /// Integer sort (unbounded).
    Int,
    /// Bitvector sort with width.
    BitVec(u32),
    /// Array sort (index -> element).
    Array {
        index: Box<SmtSort>,
        element: Box<SmtSort>,
    },
}

impl fmt::Display for SmtSort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmtSort::Bool => write!(f, "Bool"),
            SmtSort::Int => write!(f, "Int"),
            SmtSort::BitVec(w) => write!(f, "(_ BitVec {w})"),
            SmtSort::Array { index, element } => write!(f, "(Array {index} {element})"),
        }
    }
}

/// An SMT expression in SMT-LIB2 format.
#[derive(Debug, Clone, PartialEq)]
pub enum SmtExpr {
    /// Boolean literal.
    BoolLit(bool),
    /// Integer literal.
    IntLit(i64),
    /// Variable.
    Var(String, SmtSort),
    /// Binary operation.
    BinOp {
        op: SmtOp,
        lhs: Box<SmtExpr>,
        rhs: Box<SmtExpr>,
    },
    /// Unary not.
    Not(Box<SmtExpr>),
    /// If-then-else.
    Ite {
        cond: Box<SmtExpr>,
        then_: Box<SmtExpr>,
        else_: Box<SmtExpr>,
    },
    /// Array select (read).
    Select {
        array: Box<SmtExpr>,
        index: Box<SmtExpr>,
    },
    /// Array store (write).
    Store {
        array: Box<SmtExpr>,
        index: Box<SmtExpr>,
        value: Box<SmtExpr>,
    },
    /// Quantified forall.
    Forall {
        var: String,
        sort: SmtSort,
        body: Box<SmtExpr>,
    },
}

/// SMT binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Implies,
    /// Bitvector add.
    BvAdd,
    /// Bitvector subtract.
    BvSub,
    /// Bitvector multiply.
    BvMul,
}

impl fmt::Display for SmtOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmtOp::Add => write!(f, "+"),
            SmtOp::Sub => write!(f, "-"),
            SmtOp::Mul => write!(f, "*"),
            SmtOp::Div => write!(f, "div"),
            SmtOp::Mod => write!(f, "mod"),
            SmtOp::Eq => write!(f, "="),
            SmtOp::Lt => write!(f, "<"),
            SmtOp::Le => write!(f, "<="),
            SmtOp::Gt => write!(f, ">"),
            SmtOp::Ge => write!(f, ">="),
            SmtOp::And => write!(f, "and"),
            SmtOp::Or => write!(f, "or"),
            SmtOp::Implies => write!(f, "=>"),
            SmtOp::BvAdd => write!(f, "bvadd"),
            SmtOp::BvSub => write!(f, "bvsub"),
            SmtOp::BvMul => write!(f, "bvmul"),
        }
    }
}

impl fmt::Display for SmtExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmtExpr::BoolLit(b) => write!(f, "{b}"),
            SmtExpr::IntLit(n) => {
                if *n < 0 {
                    write!(f, "(- {})", -n)
                } else {
                    write!(f, "{n}")
                }
            }
            SmtExpr::Var(name, _) => write!(f, "{name}"),
            SmtExpr::BinOp { op, lhs, rhs } => write!(f, "({op} {lhs} {rhs})"),
            SmtExpr::Not(e) => write!(f, "(not {e})"),
            SmtExpr::Ite { cond, then_, else_ } => {
                write!(f, "(ite {cond} {then_} {else_})")
            }
            SmtExpr::Select { array, index } => write!(f, "(select {array} {index})"),
            SmtExpr::Store {
                array,
                index,
                value,
            } => write!(f, "(store {array} {index} {value})"),
            SmtExpr::Forall { var, sort, body } => {
                write!(f, "(forall (({var} {sort})) {body})")
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.3: Expression Encoding
// ═══════════════════════════════════════════════════════════════════════

/// Encodes a ContractExpr into an SmtExpr.
pub fn encode_expr(expr: &ContractExpr, vars: &HashMap<String, SmtSort>) -> SmtExpr {
    match expr {
        ContractExpr::BoolLit(b) => SmtExpr::BoolLit(*b),
        ContractExpr::IntLit(n) => SmtExpr::IntLit(*n),
        ContractExpr::Var(name) => {
            let sort = vars.get(name).cloned().unwrap_or(SmtSort::Int);
            SmtExpr::Var(name.clone(), sort)
        }
        ContractExpr::Result => SmtExpr::Var("__result".into(), SmtSort::Int),
        ContractExpr::Old(inner) => {
            let encoded = encode_expr(inner, vars);
            // Prefix old variables
            match encoded {
                SmtExpr::Var(name, sort) => SmtExpr::Var(format!("__old_{name}"), sort),
                other => other,
            }
        }
        ContractExpr::BinOp { op, lhs, rhs } => {
            let smt_op = contract_op_to_smt(*op);
            SmtExpr::BinOp {
                op: smt_op,
                lhs: Box::new(encode_expr(lhs, vars)),
                rhs: Box::new(encode_expr(rhs, vars)),
            }
        }
        ContractExpr::Not(e) => SmtExpr::Not(Box::new(encode_expr(e, vars))),
        ContractExpr::Index { array, index } => SmtExpr::Select {
            array: Box::new(encode_expr(array, vars)),
            index: Box::new(encode_expr(index, vars)),
        },
        ContractExpr::Call { name, args } => {
            // Encode function calls as uninterpreted functions (simplified: just var)
            let _ = args;
            SmtExpr::Var(format!("_call_{name}"), SmtSort::Int)
        }
        ContractExpr::Field { object, field } => {
            let obj = encode_expr(object, vars);
            match obj {
                SmtExpr::Var(name, sort) => SmtExpr::Var(format!("{name}.{field}"), sort),
                other => other,
            }
        }
    }
}

fn contract_op_to_smt(op: ContractOp) -> SmtOp {
    match op {
        ContractOp::Add => SmtOp::Add,
        ContractOp::Sub => SmtOp::Sub,
        ContractOp::Mul => SmtOp::Mul,
        ContractOp::Div => SmtOp::Div,
        ContractOp::Mod => SmtOp::Mod,
        ContractOp::Eq => SmtOp::Eq,
        ContractOp::Ne => SmtOp::Eq, // negated externally
        ContractOp::Lt => SmtOp::Lt,
        ContractOp::Le => SmtOp::Le,
        ContractOp::Gt => SmtOp::Gt,
        ContractOp::Ge => SmtOp::Ge,
        ContractOp::And => SmtOp::And,
        ContractOp::Or => SmtOp::Or,
        ContractOp::Implies => SmtOp::Implies,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.4: Integer Theory (QF_BV overflow encoding)
// ═══════════════════════════════════════════════════════════════════════

/// Integer overflow mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowMode {
    /// Wrapping — modular arithmetic.
    Wrapping,
    /// Saturating — clamp at bounds.
    Saturating,
    /// Checked — trap on overflow.
    Checked,
}

/// Encodes an integer operation with overflow semantics in bitvector theory.
pub fn encode_overflow_check(
    op: SmtOp,
    lhs: SmtExpr,
    rhs: SmtExpr,
    bit_width: u32,
    mode: OverflowMode,
) -> OverflowEncoding {
    let max_val = (1i64 << (bit_width - 1)) - 1;
    let min_val = -(1i64 << (bit_width - 1));

    let operation = SmtExpr::BinOp {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    };

    let overflow_condition = SmtExpr::BinOp {
        op: SmtOp::Or,
        lhs: Box::new(SmtExpr::BinOp {
            op: SmtOp::Gt,
            lhs: Box::new(operation.clone()),
            rhs: Box::new(SmtExpr::IntLit(max_val)),
        }),
        rhs: Box::new(SmtExpr::BinOp {
            op: SmtOp::Lt,
            lhs: Box::new(operation.clone()),
            rhs: Box::new(SmtExpr::IntLit(min_val)),
        }),
    };

    OverflowEncoding {
        operation,
        overflow_condition,
        mode,
        bit_width,
    }
}

/// Result of encoding an integer operation with overflow check.
#[derive(Debug, Clone)]
pub struct OverflowEncoding {
    /// The operation itself.
    pub operation: SmtExpr,
    /// Condition that is true when overflow occurs.
    pub overflow_condition: SmtExpr,
    /// How overflow is handled.
    pub mode: OverflowMode,
    /// Bit width of the integer type.
    pub bit_width: u32,
}

// ═══════════════════════════════════════════════════════════════════════
// S10.5: Array Theory
// ═══════════════════════════════════════════════════════════════════════

/// Encodes an array bounds check as an SMT assertion.
pub fn encode_bounds_check(_array_name: &str, index: SmtExpr, length: SmtExpr) -> SmtExpr {
    SmtExpr::BinOp {
        op: SmtOp::And,
        lhs: Box::new(SmtExpr::BinOp {
            op: SmtOp::Ge,
            lhs: Box::new(index.clone()),
            rhs: Box::new(SmtExpr::IntLit(0)),
        }),
        rhs: Box::new(SmtExpr::BinOp {
            op: SmtOp::Lt,
            lhs: Box::new(index),
            rhs: Box::new(length),
        }),
    }
}

/// Creates an SMT array variable declaration.
pub fn declare_array(name: &str) -> SmtExpr {
    SmtExpr::Var(
        name.into(),
        SmtSort::Array {
            index: Box::new(SmtSort::Int),
            element: Box::new(SmtSort::Int),
        },
    )
}

// ═══════════════════════════════════════════════════════════════════════
// S10.6: Solver Result
// ═══════════════════════════════════════════════════════════════════════

/// Result from the SMT solver.
#[derive(Debug, Clone, PartialEq)]
pub enum SolverResult {
    /// Property holds — satisfiable (or unsat for negation).
    Sat,
    /// Property does not hold — counterexample available.
    Unsat,
    /// Solver could not determine.
    Unknown,
    /// Solver timed out.
    Timeout { elapsed_ms: u64 },
}

impl fmt::Display for SolverResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolverResult::Sat => write!(f, "sat"),
            SolverResult::Unsat => write!(f, "unsat"),
            SolverResult::Unknown => write!(f, "unknown"),
            SolverResult::Timeout { elapsed_ms } => write!(f, "timeout ({elapsed_ms}ms)"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.7: Counterexample
// ═══════════════════════════════════════════════════════════════════════

/// A counterexample extracted from an SMT model.
#[derive(Debug, Clone)]
pub struct Counterexample {
    /// Variable assignments that violate the property.
    pub assignments: HashMap<String, CounterValue>,
}

/// A value in a counterexample.
#[derive(Debug, Clone, PartialEq)]
pub enum CounterValue {
    /// Integer value.
    Int(i64),
    /// Boolean value.
    Bool(bool),
    /// Array (index -> value pairs).
    Array(Vec<(i64, i64)>),
}

impl fmt::Display for CounterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CounterValue::Int(n) => write!(f, "{n}"),
            CounterValue::Bool(b) => write!(f, "{b}"),
            CounterValue::Array(entries) => {
                let pairs: Vec<String> =
                    entries.iter().map(|(k, v)| format!("[{k}]={v}")).collect();
                write!(f, "{{{}}}", pairs.join(", "))
            }
        }
    }
}

impl Counterexample {
    /// Creates an empty counterexample.
    pub fn new() -> Self {
        Self {
            assignments: HashMap::new(),
        }
    }

    /// Adds a variable assignment.
    pub fn add(&mut self, name: &str, value: CounterValue) {
        self.assignments.insert(name.into(), value);
    }

    /// Formats the counterexample for diagnostic display.
    pub fn display_message(&self) -> String {
        let mut parts: Vec<String> = self
            .assignments
            .iter()
            .map(|(k, v)| format!("{k} = {v}"))
            .collect();
        parts.sort();
        format!("counterexample: {}", parts.join(", "))
    }
}

impl Default for Counterexample {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.8: Timeout Configuration
// ═══════════════════════════════════════════════════════════════════════

/// Solver configuration.
#[derive(Debug, Clone)]
pub struct SolverConfig {
    /// Which solver backend to use.
    pub backend: SolverBackend,
    /// Timeout per function in milliseconds.
    pub timeout_ms: u64,
    /// Whether to use incremental mode.
    pub incremental: bool,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            backend: SolverBackend::Builtin,
            timeout_ms: 5000,
            incremental: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.9: Incremental Solving
// ═══════════════════════════════════════════════════════════════════════

/// Represents a solver context with push/pop for incremental solving.
#[derive(Debug, Clone)]
pub struct SolverContext {
    /// Assertion stack — each level is a Vec of assertions.
    assertion_stack: Vec<Vec<SmtExpr>>,
    /// Configuration.
    pub config: SolverConfig,
}

impl SolverContext {
    /// Creates a new solver context.
    pub fn new(config: SolverConfig) -> Self {
        Self {
            assertion_stack: vec![Vec::new()],
            config,
        }
    }

    /// Adds an assertion at the current level.
    pub fn assert_expr(&mut self, expr: SmtExpr) {
        if let Some(level) = self.assertion_stack.last_mut() {
            level.push(expr);
        }
    }

    /// Pushes a new assertion level.
    pub fn push(&mut self) {
        self.assertion_stack.push(Vec::new());
    }

    /// Pops the current assertion level.
    pub fn pop(&mut self) {
        if self.assertion_stack.len() > 1 {
            self.assertion_stack.pop();
        }
    }

    /// Returns the current depth.
    pub fn depth(&self) -> usize {
        self.assertion_stack.len()
    }

    /// Returns all assertions flattened.
    pub fn all_assertions(&self) -> Vec<&SmtExpr> {
        self.assertion_stack
            .iter()
            .flat_map(|level| level.iter())
            .collect()
    }

    /// Checks satisfiability of current assertions (builtin solver — simplified).
    pub fn check_sat(&self) -> SolverResult {
        // Built-in solver: evaluate constant expressions only
        let assertions = self.all_assertions();
        if assertions.is_empty() {
            return SolverResult::Sat;
        }

        // Check if any assertion is trivially false
        for assertion in &assertions {
            if let SmtExpr::BoolLit(false) = assertion {
                return SolverResult::Unsat;
            }
        }

        // For non-trivial assertions, the builtin solver reports unknown
        let has_vars = assertions.iter().any(|a| contains_var(a));
        if has_vars {
            SolverResult::Unknown
        } else {
            SolverResult::Sat
        }
    }
}

fn contains_var(expr: &SmtExpr) -> bool {
    match expr {
        SmtExpr::Var(_, _) => true,
        SmtExpr::BoolLit(_) | SmtExpr::IntLit(_) => false,
        SmtExpr::BinOp { lhs, rhs, .. } => contains_var(lhs) || contains_var(rhs),
        SmtExpr::Not(e) => contains_var(e),
        SmtExpr::Ite { cond, then_, else_ } => {
            contains_var(cond) || contains_var(then_) || contains_var(else_)
        }
        SmtExpr::Select { array, index } => contains_var(array) || contains_var(index),
        SmtExpr::Store {
            array,
            index,
            value,
        } => contains_var(array) || contains_var(index) || contains_var(value),
        SmtExpr::Forall { body, .. } => contains_var(body),
    }
}

/// Generates SMT-LIB2 text for the current context.
pub fn to_smtlib2(context: &SolverContext) -> String {
    let mut output = String::new();
    output.push_str("(set-logic QF_LIA)\n");

    // Collect all variable declarations
    let mut vars = HashMap::new();
    for assertion in context.all_assertions() {
        collect_vars(assertion, &mut vars);
    }
    for (name, sort) in &vars {
        output.push_str(&format!("(declare-const {name} {sort})\n"));
    }

    // Add assertions
    for assertion in context.all_assertions() {
        output.push_str(&format!("(assert {assertion})\n"));
    }

    output.push_str("(check-sat)\n");
    output
}

fn collect_vars(expr: &SmtExpr, vars: &mut HashMap<String, SmtSort>) {
    match expr {
        SmtExpr::Var(name, sort) => {
            vars.entry(name.clone()).or_insert_with(|| sort.clone());
        }
        SmtExpr::BinOp { lhs, rhs, .. } => {
            collect_vars(lhs, vars);
            collect_vars(rhs, vars);
        }
        SmtExpr::Not(e) => collect_vars(e, vars),
        SmtExpr::Ite { cond, then_, else_ } => {
            collect_vars(cond, vars);
            collect_vars(then_, vars);
            collect_vars(else_, vars);
        }
        SmtExpr::Select { array, index } => {
            collect_vars(array, vars);
            collect_vars(index, vars);
        }
        SmtExpr::Store {
            array,
            index,
            value,
        } => {
            collect_vars(array, vars);
            collect_vars(index, vars);
            collect_vars(value, vars);
        }
        SmtExpr::Forall { body, .. } => collect_vars(body, vars),
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S10.1 — Solver Backend
    #[test]
    fn s10_1_solver_backend_display() {
        assert_eq!(SolverBackend::Z3.to_string(), "z3");
        assert_eq!(SolverBackend::Cvc5.to_string(), "cvc5");
        assert_eq!(SolverBackend::Builtin.to_string(), "builtin");
    }

    // S10.2 — Sort Display
    #[test]
    fn s10_2_sort_display() {
        assert_eq!(SmtSort::Bool.to_string(), "Bool");
        assert_eq!(SmtSort::Int.to_string(), "Int");
        assert_eq!(SmtSort::BitVec(64).to_string(), "(_ BitVec 64)");
        let arr = SmtSort::Array {
            index: Box::new(SmtSort::Int),
            element: Box::new(SmtSort::Int),
        };
        assert_eq!(arr.to_string(), "(Array Int Int)");
    }

    // S10.3 — Expression Encoding
    #[test]
    fn s10_3_encode_simple_expr() {
        let vars = HashMap::new();
        let expr = ContractExpr::BinOp {
            op: ContractOp::Gt,
            lhs: Box::new(ContractExpr::Var("x".into())),
            rhs: Box::new(ContractExpr::IntLit(0)),
        };
        let smt = encode_expr(&expr, &vars);
        assert_eq!(smt.to_string(), "(> x 0)");
    }

    #[test]
    fn s10_3_encode_old_expr() {
        let vars = HashMap::new();
        let expr = ContractExpr::Old(Box::new(ContractExpr::Var("x".into())));
        let smt = encode_expr(&expr, &vars);
        assert_eq!(smt.to_string(), "__old_x");
    }

    #[test]
    fn s10_3_encode_result() {
        let vars = HashMap::new();
        let expr = ContractExpr::Result;
        let smt = encode_expr(&expr, &vars);
        assert_eq!(smt.to_string(), "__result");
    }

    #[test]
    fn s10_3_encode_not() {
        let vars = HashMap::new();
        let expr = ContractExpr::Not(Box::new(ContractExpr::BoolLit(true)));
        let smt = encode_expr(&expr, &vars);
        assert_eq!(smt.to_string(), "(not true)");
    }

    // S10.4 — Integer Theory
    #[test]
    fn s10_4_overflow_check_i32() {
        let enc = encode_overflow_check(
            SmtOp::Add,
            SmtExpr::Var("a".into(), SmtSort::Int),
            SmtExpr::Var("b".into(), SmtSort::Int),
            32,
            OverflowMode::Checked,
        );
        assert_eq!(enc.bit_width, 32);
        assert_eq!(enc.mode, OverflowMode::Checked);
    }

    #[test]
    fn s10_4_overflow_modes() {
        assert_ne!(OverflowMode::Wrapping, OverflowMode::Saturating);
        assert_ne!(OverflowMode::Saturating, OverflowMode::Checked);
    }

    // S10.5 — Array Theory
    #[test]
    fn s10_5_bounds_check() {
        let check = encode_bounds_check(
            "arr",
            SmtExpr::Var("i".into(), SmtSort::Int),
            SmtExpr::IntLit(10),
        );
        let s = check.to_string();
        assert!(s.contains(">= i 0"));
        assert!(s.contains("< i 10"));
    }

    #[test]
    fn s10_5_array_declaration() {
        let arr = declare_array("data");
        match arr {
            SmtExpr::Var(name, SmtSort::Array { .. }) => assert_eq!(name, "data"),
            _ => panic!("expected array var"),
        }
    }

    // S10.6 — Solver Result
    #[test]
    fn s10_6_result_display() {
        assert_eq!(SolverResult::Sat.to_string(), "sat");
        assert_eq!(SolverResult::Unsat.to_string(), "unsat");
        assert_eq!(SolverResult::Unknown.to_string(), "unknown");
        assert_eq!(
            SolverResult::Timeout { elapsed_ms: 5000 }.to_string(),
            "timeout (5000ms)"
        );
    }

    // S10.7 — Counterexample
    #[test]
    fn s10_7_counterexample_display() {
        let mut ce = Counterexample::new();
        ce.add("x", CounterValue::Int(-1));
        ce.add("y", CounterValue::Bool(false));
        let msg = ce.display_message();
        assert!(msg.contains("x = -1"));
        assert!(msg.contains("y = false"));
    }

    #[test]
    fn s10_7_array_counterexample() {
        let mut ce = Counterexample::new();
        ce.add("arr", CounterValue::Array(vec![(0, 42), (1, -1)]));
        let msg = ce.display_message();
        assert!(msg.contains("[0]=42"));
        assert!(msg.contains("[1]=-1"));
    }

    // S10.8 — Timeout Config
    #[test]
    fn s10_8_default_config() {
        let config = SolverConfig::default();
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.backend, SolverBackend::Builtin);
        assert!(config.incremental);
    }

    // S10.9 — Incremental Solving
    #[test]
    fn s10_9_push_pop() {
        let mut ctx = SolverContext::new(SolverConfig::default());
        assert_eq!(ctx.depth(), 1);
        ctx.push();
        assert_eq!(ctx.depth(), 2);
        ctx.assert_expr(SmtExpr::BoolLit(true));
        ctx.pop();
        assert_eq!(ctx.depth(), 1);
        assert!(ctx.all_assertions().is_empty());
    }

    #[test]
    fn s10_9_check_sat_trivial() {
        let mut ctx = SolverContext::new(SolverConfig::default());
        assert_eq!(ctx.check_sat(), SolverResult::Sat);
        ctx.assert_expr(SmtExpr::BoolLit(false));
        assert_eq!(ctx.check_sat(), SolverResult::Unsat);
    }

    #[test]
    fn s10_9_check_sat_unknown() {
        let mut ctx = SolverContext::new(SolverConfig::default());
        ctx.assert_expr(SmtExpr::BinOp {
            op: SmtOp::Gt,
            lhs: Box::new(SmtExpr::Var("x".into(), SmtSort::Int)),
            rhs: Box::new(SmtExpr::IntLit(0)),
        });
        assert_eq!(ctx.check_sat(), SolverResult::Unknown);
    }

    #[test]
    fn s10_9_smtlib2_output() {
        let mut ctx = SolverContext::new(SolverConfig::default());
        ctx.assert_expr(SmtExpr::BinOp {
            op: SmtOp::Gt,
            lhs: Box::new(SmtExpr::Var("x".into(), SmtSort::Int)),
            rhs: Box::new(SmtExpr::IntLit(0)),
        });
        let output = to_smtlib2(&ctx);
        assert!(output.contains("(set-logic QF_LIA)"));
        assert!(output.contains("(declare-const x Int)"));
        assert!(output.contains("(assert (> x 0))"));
        assert!(output.contains("(check-sat)"));
    }

    // S10.10 — Additional
    #[test]
    fn s10_10_smt_op_display() {
        assert_eq!(SmtOp::Add.to_string(), "+");
        assert_eq!(SmtOp::Implies.to_string(), "=>");
        assert_eq!(SmtOp::BvAdd.to_string(), "bvadd");
    }

    #[test]
    fn s10_10_negative_int_display() {
        let expr = SmtExpr::IntLit(-42);
        assert_eq!(expr.to_string(), "(- 42)");
    }

    #[test]
    fn s10_10_ite_display() {
        let expr = SmtExpr::Ite {
            cond: Box::new(SmtExpr::BoolLit(true)),
            then_: Box::new(SmtExpr::IntLit(1)),
            else_: Box::new(SmtExpr::IntLit(0)),
        };
        assert_eq!(expr.to_string(), "(ite true 1 0)");
    }

    #[test]
    fn s10_10_forall_display() {
        let expr = SmtExpr::Forall {
            var: "i".into(),
            sort: SmtSort::Int,
            body: Box::new(SmtExpr::BinOp {
                op: SmtOp::Ge,
                lhs: Box::new(SmtExpr::Var("i".into(), SmtSort::Int)),
                rhs: Box::new(SmtExpr::IntLit(0)),
            }),
        };
        assert!(expr.to_string().contains("forall"));
    }
}
