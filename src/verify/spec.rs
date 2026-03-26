//! Specification Language — annotations, verification conditions, WP calculus.
//!
//! Phase V1: 20 tasks covering @requires/@ensures/@invariant/@assert/@decreases,
//! old() expressions, quantifiers, ghost variables, weakest precondition,
//! SSA transform, and proof obligations for bounds/overflow/null/division.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V1.1.1-V1.1.5: Specification Annotations
// ═══════════════════════════════════════════════════════════════════════

/// A specification annotation on a function or loop.
#[derive(Debug, Clone)]
pub enum SpecAnnotation {
    /// Precondition: must hold on function entry.
    Requires(SpecExpr),
    /// Postcondition: must hold on function exit.
    Ensures(SpecExpr),
    /// Loop invariant: holds at start and is preserved by body.
    Invariant(SpecExpr),
    /// Compile-time assertion (proof obligation).
    Assert(SpecExpr),
    /// Termination measure: must decrease on each iteration.
    Decreases(SpecExpr),
}

impl fmt::Display for SpecAnnotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Requires(e) => write!(f, "@requires({e})"),
            Self::Ensures(e) => write!(f, "@ensures({e})"),
            Self::Invariant(e) => write!(f, "@invariant({e})"),
            Self::Assert(e) => write!(f, "@assert({e})"),
            Self::Decreases(e) => write!(f, "@decreases({e})"),
        }
    }
}

/// A specification expression (logical formula).
#[derive(Debug, Clone, PartialEq)]
pub enum SpecExpr {
    /// Boolean literal.
    BoolLit(bool),
    /// Integer literal.
    IntLit(i64),
    /// Variable reference.
    Var(String),
    /// `old(x)` — value at function entry.
    Old(Box<SpecExpr>),
    /// `result` — function return value.
    Result,
    /// Binary operation.
    BinOp(Box<SpecExpr>, SpecBinOp, Box<SpecExpr>),
    /// Unary operation.
    UnaryOp(SpecUnaryOp, Box<SpecExpr>),
    /// Universal quantifier: `forall(var, range, body)`.
    Forall(String, Box<SpecExpr>, Box<SpecExpr>, Box<SpecExpr>),
    /// Existential quantifier.
    Exists(String, Box<SpecExpr>, Box<SpecExpr>, Box<SpecExpr>),
    /// Array/tensor indexing.
    Index(Box<SpecExpr>, Box<SpecExpr>),
    /// Function call (pure spec functions).
    Call(String, Vec<SpecExpr>),
    /// `len(x)`.
    Len(Box<SpecExpr>),
    /// Ghost variable.
    Ghost(String),
    /// Implication: `a ==> b`.
    Implies(Box<SpecExpr>, Box<SpecExpr>),
}

/// Specification binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// Specification unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecUnaryOp {
    Not,
    Neg,
}

impl fmt::Display for SpecExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoolLit(b) => write!(f, "{b}"),
            Self::IntLit(i) => write!(f, "{i}"),
            Self::Var(name) => write!(f, "{name}"),
            Self::Old(inner) => write!(f, "old({inner})"),
            Self::Result => write!(f, "result"),
            Self::BinOp(lhs, op, rhs) => write!(f, "({lhs} {op:?} {rhs})"),
            Self::UnaryOp(op, inner) => write!(f, "({op:?} {inner})"),
            Self::Forall(v, lo, hi, body) => write!(f, "forall({v}, {lo}..{hi}, {body})"),
            Self::Exists(v, lo, hi, body) => write!(f, "exists({v}, {lo}..{hi}, {body})"),
            Self::Index(arr, idx) => write!(f, "{arr}[{idx}]"),
            Self::Call(name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                write!(f, "{name}({})", args_str.join(", "))
            }
            Self::Len(inner) => write!(f, "len({inner})"),
            Self::Ghost(name) => write!(f, "ghost:{name}"),
            Self::Implies(lhs, rhs) => write!(f, "({lhs} ==> {rhs})"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V1.1.9-V1.1.10: Ghost Variables + Spec Context
// ═══════════════════════════════════════════════════════════════════════

/// A function's complete specification.
#[derive(Debug, Clone, Default)]
pub struct FunctionSpec {
    /// Function name.
    pub name: String,
    /// Preconditions.
    pub requires: Vec<SpecExpr>,
    /// Postconditions.
    pub ensures: Vec<SpecExpr>,
    /// Ghost variables (specification-only state).
    pub ghost_vars: Vec<GhostVar>,
}

/// A ghost variable (exists only in specs, not in compiled code).
#[derive(Debug, Clone)]
pub struct GhostVar {
    /// Name.
    pub name: String,
    /// Type.
    pub var_type: String,
    /// Initial value expression.
    pub init: Option<SpecExpr>,
}

/// A loop's specification.
#[derive(Debug, Clone)]
pub struct LoopSpec {
    /// Invariant (preserved by each iteration).
    pub invariant: Option<SpecExpr>,
    /// Termination variant (decreases).
    pub variant: Option<SpecExpr>,
}

// ═══════════════════════════════════════════════════════════════════════
// V1.2.1: Weakest Precondition Calculus
// ═══════════════════════════════════════════════════════════════════════

/// A verification condition (logical formula to prove).
#[derive(Debug, Clone)]
pub struct VerificationCondition {
    /// Unique ID.
    pub id: u64,
    /// Description.
    pub description: String,
    /// The formula to prove.
    pub formula: SpecExpr,
    /// Source location.
    pub file: String,
    pub line: u32,
    /// VC kind.
    pub kind: VcKind,
    /// Proof status.
    pub status: ProofStatus,
}

/// Verification condition kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcKind {
    Precondition,
    Postcondition,
    LoopInvariantEntry,
    LoopInvariantPreserved,
    ArrayBoundsCheck,
    IntegerOverflow,
    DivisionByZero,
    NullSafety,
    Termination,
    UserAssert,
}

impl fmt::Display for VcKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Precondition => write!(f, "precondition"),
            Self::Postcondition => write!(f, "postcondition"),
            Self::LoopInvariantEntry => write!(f, "loop invariant (entry)"),
            Self::LoopInvariantPreserved => write!(f, "loop invariant (preserved)"),
            Self::ArrayBoundsCheck => write!(f, "array bounds"),
            Self::IntegerOverflow => write!(f, "integer overflow"),
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::NullSafety => write!(f, "null safety"),
            Self::Termination => write!(f, "termination"),
            Self::UserAssert => write!(f, "assertion"),
        }
    }
}

/// Proof status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofStatus {
    /// Not yet checked.
    Pending,
    /// Proven valid by SMT solver.
    Verified,
    /// Counterexample found.
    Failed(String),
    /// Solver timeout.
    Timeout,
    /// Unknown (solver could not decide).
    Unknown,
}

impl fmt::Display for ProofStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Verified => write!(f, "verified"),
            Self::Failed(msg) => write!(f, "FAILED: {msg}"),
            Self::Timeout => write!(f, "timeout"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Computes weakest precondition for assignment: wp(x := e, Q) = Q[x/e].
pub fn wp_assign(var: &str, expr: &SpecExpr, postcondition: &SpecExpr) -> SpecExpr {
    substitute(postcondition, var, expr)
}

/// Substitutes all occurrences of `var` with `replacement` in `expr`.
pub fn substitute(expr: &SpecExpr, var: &str, replacement: &SpecExpr) -> SpecExpr {
    match expr {
        SpecExpr::Var(name) if name == var => replacement.clone(),
        SpecExpr::BinOp(lhs, op, rhs) => SpecExpr::BinOp(
            Box::new(substitute(lhs, var, replacement)),
            *op,
            Box::new(substitute(rhs, var, replacement)),
        ),
        SpecExpr::UnaryOp(op, inner) => {
            SpecExpr::UnaryOp(*op, Box::new(substitute(inner, var, replacement)))
        }
        SpecExpr::Old(inner) => SpecExpr::Old(Box::new(substitute(inner, var, replacement))),
        SpecExpr::Index(arr, idx) => SpecExpr::Index(
            Box::new(substitute(arr, var, replacement)),
            Box::new(substitute(idx, var, replacement)),
        ),
        SpecExpr::Len(inner) => SpecExpr::Len(Box::new(substitute(inner, var, replacement))),
        SpecExpr::Implies(lhs, rhs) => SpecExpr::Implies(
            Box::new(substitute(lhs, var, replacement)),
            Box::new(substitute(rhs, var, replacement)),
        ),
        SpecExpr::Forall(v, lo, hi, body) if v != var => SpecExpr::Forall(
            v.clone(),
            Box::new(substitute(lo, var, replacement)),
            Box::new(substitute(hi, var, replacement)),
            Box::new(substitute(body, var, replacement)),
        ),
        _ => expr.clone(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V1.2.2: SSA Transformation
// ═══════════════════════════════════════════════════════════════════════

/// SSA variable (name + version).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SsaVar {
    pub name: String,
    pub version: u32,
}

impl fmt::Display for SsaVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_{}", self.name, self.version)
    }
}

/// SSA variable tracker.
#[derive(Debug, Clone, Default)]
pub struct SsaContext {
    /// Current version for each variable.
    pub versions: HashMap<String, u32>,
}

impl SsaContext {
    /// Gets the current SSA version of a variable.
    pub fn current(&self, name: &str) -> SsaVar {
        let version = self.versions.get(name).copied().unwrap_or(0);
        SsaVar {
            name: name.to_string(),
            version,
        }
    }

    /// Creates a new version (for assignment).
    pub fn next_version(&mut self, name: &str) -> SsaVar {
        let version = self.versions.entry(name.to_string()).or_insert(0);
        *version += 1;
        SsaVar {
            name: name.to_string(),
            version: *version,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V1.2.5-V1.2.8: Safety Proof Generators
// ═══════════════════════════════════════════════════════════════════════

/// Generates array bounds check VC: `0 <= idx && idx < len(arr)`.
pub fn array_bounds_vc(
    arr_name: &str,
    index_expr: &SpecExpr,
    file: &str,
    line: u32,
) -> VerificationCondition {
    let formula = SpecExpr::BinOp(
        Box::new(SpecExpr::BinOp(
            Box::new(SpecExpr::IntLit(0)),
            SpecBinOp::Le,
            Box::new(index_expr.clone()),
        )),
        SpecBinOp::And,
        Box::new(SpecExpr::BinOp(
            Box::new(index_expr.clone()),
            SpecBinOp::Lt,
            Box::new(SpecExpr::Len(Box::new(SpecExpr::Var(arr_name.to_string())))),
        )),
    );
    VerificationCondition {
        id: 0,
        description: format!("array bounds: {arr_name}[{index_expr}]"),
        formula,
        file: file.to_string(),
        line,
        kind: VcKind::ArrayBoundsCheck,
        status: ProofStatus::Pending,
    }
}

/// Generates integer overflow check VC.
pub fn overflow_vc(
    expr: &SpecExpr,
    min: i64,
    max: i64,
    file: &str,
    line: u32,
) -> VerificationCondition {
    let formula = SpecExpr::BinOp(
        Box::new(SpecExpr::BinOp(
            Box::new(SpecExpr::IntLit(min)),
            SpecBinOp::Le,
            Box::new(expr.clone()),
        )),
        SpecBinOp::And,
        Box::new(SpecExpr::BinOp(
            Box::new(expr.clone()),
            SpecBinOp::Le,
            Box::new(SpecExpr::IntLit(max)),
        )),
    );
    VerificationCondition {
        id: 0,
        description: format!("no overflow: {min} <= {expr} <= {max}"),
        formula,
        file: file.to_string(),
        line,
        kind: VcKind::IntegerOverflow,
        status: ProofStatus::Pending,
    }
}

/// Generates division-by-zero check VC: `divisor != 0`.
pub fn div_zero_vc(divisor: &SpecExpr, file: &str, line: u32) -> VerificationCondition {
    let formula = SpecExpr::BinOp(
        Box::new(divisor.clone()),
        SpecBinOp::Ne,
        Box::new(SpecExpr::IntLit(0)),
    );
    VerificationCondition {
        id: 0,
        description: format!("division by zero: {divisor} != 0"),
        formula,
        file: file.to_string(),
        line,
        kind: VcKind::DivisionByZero,
        status: ProofStatus::Pending,
    }
}

/// Generates null safety VC: `x != null` (Option is Some).
pub fn null_safety_vc(var_name: &str, file: &str, line: u32) -> VerificationCondition {
    let formula = SpecExpr::BinOp(
        Box::new(SpecExpr::Var(var_name.to_string())),
        SpecBinOp::Ne,
        Box::new(SpecExpr::Var("null".to_string())),
    );
    VerificationCondition {
        id: 0,
        description: format!("null safety: {var_name} is Some"),
        formula,
        file: file.to_string(),
        line,
        kind: VcKind::NullSafety,
        status: ProofStatus::Pending,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V1.2.10: SMT-LIB2 Export
// ═══════════════════════════════════════════════════════════════════════

/// Exports a SpecExpr to SMT-LIB2 format.
pub fn to_smtlib2(expr: &SpecExpr) -> String {
    match expr {
        SpecExpr::BoolLit(true) => "true".to_string(),
        SpecExpr::BoolLit(false) => "false".to_string(),
        SpecExpr::IntLit(i) => {
            if *i < 0 {
                format!("(- {})", -i)
            } else {
                i.to_string()
            }
        }
        SpecExpr::Var(name) | SpecExpr::Ghost(name) => name.clone(),
        SpecExpr::Result => "result".to_string(),
        SpecExpr::Old(inner) => format!("old_{}", to_smtlib2(inner)),
        SpecExpr::BinOp(lhs, op, rhs) => {
            let op_str = match op {
                SpecBinOp::Add => "+",
                SpecBinOp::Sub => "-",
                SpecBinOp::Mul => "*",
                SpecBinOp::Div => "div",
                SpecBinOp::Mod => "mod",
                SpecBinOp::Eq => "=",
                SpecBinOp::Ne => "distinct",
                SpecBinOp::Lt => "<",
                SpecBinOp::Le => "<=",
                SpecBinOp::Gt => ">",
                SpecBinOp::Ge => ">=",
                SpecBinOp::And => "and",
                SpecBinOp::Or => "or",
                _ => "unknown",
            };
            format!("({} {} {})", op_str, to_smtlib2(lhs), to_smtlib2(rhs))
        }
        SpecExpr::UnaryOp(SpecUnaryOp::Not, inner) => format!("(not {})", to_smtlib2(inner)),
        SpecExpr::UnaryOp(SpecUnaryOp::Neg, inner) => format!("(- {})", to_smtlib2(inner)),
        SpecExpr::Forall(v, lo, hi, body) => {
            format!(
                "(forall (({v} Int)) (=> (and (>= {v} {}) (<= {v} {})) {}))",
                to_smtlib2(lo),
                to_smtlib2(hi),
                to_smtlib2(body)
            )
        }
        SpecExpr::Exists(v, lo, hi, body) => {
            format!(
                "(exists (({v} Int)) (and (>= {v} {}) (<= {v} {}) {}))",
                to_smtlib2(lo),
                to_smtlib2(hi),
                to_smtlib2(body)
            )
        }
        SpecExpr::Implies(lhs, rhs) => format!("(=> {} {})", to_smtlib2(lhs), to_smtlib2(rhs)),
        SpecExpr::Len(inner) => format!("(len {})", to_smtlib2(inner)),
        SpecExpr::Index(arr, idx) => format!("(select {} {})", to_smtlib2(arr), to_smtlib2(idx)),
        SpecExpr::Call(name, args) => {
            let args_str: Vec<String> = args.iter().map(to_smtlib2).collect();
            format!("({} {})", name, args_str.join(" "))
        }
    }
}

/// Generates a complete SMT-LIB2 check for a verification condition.
pub fn vc_to_smtlib2(vc: &VerificationCondition) -> String {
    let mut smt = String::new();
    smt.push_str("; Verification condition\n");
    smt.push_str(&format!("; {}: {}\n", vc.kind, vc.description));
    smt.push_str(&format!("; {}:{}\n", vc.file, vc.line));
    smt.push_str("(set-logic ALL)\n");
    smt.push_str(&format!("(assert (not {}))\n", to_smtlib2(&vc.formula)));
    smt.push_str("(check-sat)\n");
    smt.push_str("(get-model)\n");
    smt
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_1_spec_annotation_display() {
        let req = SpecAnnotation::Requires(SpecExpr::BinOp(
            Box::new(SpecExpr::Var("x".to_string())),
            SpecBinOp::Gt,
            Box::new(SpecExpr::IntLit(0)),
        ));
        let s = format!("{req}");
        assert!(s.contains("@requires"));
    }

    #[test]
    fn v1_1_spec_expr_display() {
        let expr = SpecExpr::Forall(
            "i".to_string(),
            Box::new(SpecExpr::IntLit(0)),
            Box::new(SpecExpr::Var("n".to_string())),
            Box::new(SpecExpr::BinOp(
                Box::new(SpecExpr::Index(
                    Box::new(SpecExpr::Var("arr".to_string())),
                    Box::new(SpecExpr::Var("i".to_string())),
                )),
                SpecBinOp::Ge,
                Box::new(SpecExpr::IntLit(0)),
            )),
        );
        let s = format!("{expr}");
        assert!(s.contains("forall"));
        assert!(s.contains("arr"));
    }

    #[test]
    fn v1_1_old_expression() {
        let expr = SpecExpr::BinOp(
            Box::new(SpecExpr::Result),
            SpecBinOp::Ge,
            Box::new(SpecExpr::Old(Box::new(SpecExpr::Var("x".to_string())))),
        );
        let s = format!("{expr}");
        assert!(s.contains("result"));
        assert!(s.contains("old(x)"));
    }

    #[test]
    fn v1_2_wp_assign() {
        // wp(x := x + 1, x > 5) = (x + 1) > 5
        let post = SpecExpr::BinOp(
            Box::new(SpecExpr::Var("x".to_string())),
            SpecBinOp::Gt,
            Box::new(SpecExpr::IntLit(5)),
        );
        let assign_expr = SpecExpr::BinOp(
            Box::new(SpecExpr::Var("x".to_string())),
            SpecBinOp::Add,
            Box::new(SpecExpr::IntLit(1)),
        );
        let wp = wp_assign("x", &assign_expr, &post);
        // Result should have (x + 1) in place of x
        let s = format!("{wp}");
        assert!(s.contains("Add"));
        assert!(s.contains("Gt"));
    }

    #[test]
    fn v1_2_ssa_context() {
        let mut ctx = SsaContext::default();
        assert_eq!(ctx.current("x").version, 0);
        let x1 = ctx.next_version("x");
        assert_eq!(x1.version, 1);
        assert_eq!(format!("{x1}"), "x_1");
        let x2 = ctx.next_version("x");
        assert_eq!(x2.version, 2);
    }

    #[test]
    fn v1_2_array_bounds_vc() {
        let idx = SpecExpr::Var("i".to_string());
        let vc = array_bounds_vc("arr", &idx, "main.fj", 10);
        assert_eq!(vc.kind, VcKind::ArrayBoundsCheck);
        assert_eq!(vc.status, ProofStatus::Pending);
    }

    #[test]
    fn v1_2_overflow_vc() {
        let expr = SpecExpr::BinOp(
            Box::new(SpecExpr::Var("a".to_string())),
            SpecBinOp::Add,
            Box::new(SpecExpr::Var("b".to_string())),
        );
        let vc = overflow_vc(&expr, -2147483648, 2147483647, "math.fj", 5);
        assert_eq!(vc.kind, VcKind::IntegerOverflow);
    }

    #[test]
    fn v1_2_div_zero_vc() {
        let divisor = SpecExpr::Var("d".to_string());
        let vc = div_zero_vc(&divisor, "calc.fj", 12);
        assert_eq!(vc.kind, VcKind::DivisionByZero);
        let smt = to_smtlib2(&vc.formula);
        assert!(smt.contains("distinct"));
    }

    #[test]
    fn v1_2_null_safety_vc() {
        let vc = null_safety_vc("opt_val", "safe.fj", 20);
        assert_eq!(vc.kind, VcKind::NullSafety);
    }

    #[test]
    fn v1_2_smtlib2_export() {
        let expr = SpecExpr::BinOp(
            Box::new(SpecExpr::Var("x".to_string())),
            SpecBinOp::Gt,
            Box::new(SpecExpr::IntLit(0)),
        );
        let smt = to_smtlib2(&expr);
        assert_eq!(smt, "(> x 0)");
    }

    #[test]
    fn v1_2_smtlib2_forall() {
        let expr = SpecExpr::Forall(
            "i".to_string(),
            Box::new(SpecExpr::IntLit(0)),
            Box::new(SpecExpr::Var("n".to_string())),
            Box::new(SpecExpr::BinOp(
                Box::new(SpecExpr::Index(
                    Box::new(SpecExpr::Var("a".to_string())),
                    Box::new(SpecExpr::Var("i".to_string())),
                )),
                SpecBinOp::Ge,
                Box::new(SpecExpr::IntLit(0)),
            )),
        );
        let smt = to_smtlib2(&expr);
        assert!(smt.contains("forall"));
        assert!(smt.contains("(select a i)"));
    }

    #[test]
    fn v1_2_vc_to_smtlib2() {
        let vc = div_zero_vc(&SpecExpr::Var("d".to_string()), "a.fj", 5);
        let smt = vc_to_smtlib2(&vc);
        assert!(smt.contains("(set-logic ALL)"));
        assert!(smt.contains("(assert (not"));
        assert!(smt.contains("(check-sat)"));
    }

    #[test]
    fn v1_1_ghost_var() {
        let spec = FunctionSpec {
            name: "sort".to_string(),
            requires: vec![],
            ensures: vec![SpecExpr::Forall(
                "i".to_string(),
                Box::new(SpecExpr::IntLit(0)),
                Box::new(SpecExpr::BinOp(
                    Box::new(SpecExpr::Len(Box::new(SpecExpr::Var("arr".to_string())))),
                    SpecBinOp::Sub,
                    Box::new(SpecExpr::IntLit(1)),
                )),
                Box::new(SpecExpr::BinOp(
                    Box::new(SpecExpr::Index(
                        Box::new(SpecExpr::Var("arr".to_string())),
                        Box::new(SpecExpr::Var("i".to_string())),
                    )),
                    SpecBinOp::Le,
                    Box::new(SpecExpr::Index(
                        Box::new(SpecExpr::Var("arr".to_string())),
                        Box::new(SpecExpr::BinOp(
                            Box::new(SpecExpr::Var("i".to_string())),
                            SpecBinOp::Add,
                            Box::new(SpecExpr::IntLit(1)),
                        )),
                    )),
                )),
            )],
            ghost_vars: vec![GhostVar {
                name: "perm".to_string(),
                var_type: "bool".to_string(),
                init: None,
            }],
        };
        assert_eq!(spec.ghost_vars.len(), 1);
        assert_eq!(spec.ensures.len(), 1);
    }

    #[test]
    fn v1_2_proof_status_display() {
        assert_eq!(format!("{}", ProofStatus::Verified), "verified");
        assert_eq!(
            format!("{}", ProofStatus::Failed("x = -1".to_string())),
            "FAILED: x = -1"
        );
        assert_eq!(format!("{}", ProofStatus::Timeout), "timeout");
    }

    #[test]
    fn v1_2_vc_kind_display() {
        assert_eq!(format!("{}", VcKind::ArrayBoundsCheck), "array bounds");
        assert_eq!(format!("{}", VcKind::DivisionByZero), "division by zero");
        assert_eq!(format!("{}", VcKind::Termination), "termination");
    }
}
