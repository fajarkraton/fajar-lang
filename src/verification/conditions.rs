//! Pre/post conditions — requires/ensures/invariant syntax, contract
//! representation, runtime fallback, contract inheritance, old() capture,
//! multiple contracts, VE error codes.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S9.1 / S9.2 / S9.3: Contract Syntax
// ═══════════════════════════════════════════════════════════════════════

/// A verification condition expression (simplified AST for contracts).
#[derive(Debug, Clone, PartialEq)]
pub enum ContractExpr {
    /// A boolean literal.
    BoolLit(bool),
    /// An integer literal.
    IntLit(i64),
    /// A variable reference.
    Var(String),
    /// Binary operation.
    BinOp {
        op: ContractOp,
        lhs: Box<ContractExpr>,
        rhs: Box<ContractExpr>,
    },
    /// Unary not.
    Not(Box<ContractExpr>),
    /// `old(expr)` — captures value at function entry.
    Old(Box<ContractExpr>),
    /// `result` — the return value (valid in ensures only).
    Result,
    /// Function call within a contract.
    Call {
        name: String,
        args: Vec<ContractExpr>,
    },
    /// Array index.
    Index {
        array: Box<ContractExpr>,
        index: Box<ContractExpr>,
    },
    /// Field access.
    Field {
        object: Box<ContractExpr>,
        field: String,
    },
}

/// Binary operators in contract expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractOp {
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
    Implies,
}

impl fmt::Display for ContractOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractOp::Add => write!(f, "+"),
            ContractOp::Sub => write!(f, "-"),
            ContractOp::Mul => write!(f, "*"),
            ContractOp::Div => write!(f, "/"),
            ContractOp::Mod => write!(f, "%"),
            ContractOp::Eq => write!(f, "=="),
            ContractOp::Ne => write!(f, "!="),
            ContractOp::Lt => write!(f, "<"),
            ContractOp::Le => write!(f, "<="),
            ContractOp::Gt => write!(f, ">"),
            ContractOp::Ge => write!(f, ">="),
            ContractOp::And => write!(f, "&&"),
            ContractOp::Or => write!(f, "||"),
            ContractOp::Implies => write!(f, "==>"),
        }
    }
}

impl fmt::Display for ContractExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractExpr::BoolLit(b) => write!(f, "{b}"),
            ContractExpr::IntLit(n) => write!(f, "{n}"),
            ContractExpr::Var(name) => write!(f, "{name}"),
            ContractExpr::BinOp { op, lhs, rhs } => write!(f, "({lhs} {op} {rhs})"),
            ContractExpr::Not(e) => write!(f, "!{e}"),
            ContractExpr::Old(e) => write!(f, "old({e})"),
            ContractExpr::Result => write!(f, "result"),
            ContractExpr::Call { name, args } => {
                let args_str: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                write!(f, "{name}({})", args_str.join(", "))
            }
            ContractExpr::Index { array, index } => write!(f, "{array}[{index}]"),
            ContractExpr::Field { object, field } => write!(f, "{object}.{field}"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.1 / S9.2 / S9.3: Contract Types
// ═══════════════════════════════════════════════════════════════════════

/// A precondition (`requires(expr)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Requires {
    /// The condition that must hold at function entry.
    pub condition: ContractExpr,
    /// Optional human-readable message.
    pub message: Option<String>,
}

/// A postcondition (`ensures(result, expr)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Ensures {
    /// The condition that must hold at function exit.
    pub condition: ContractExpr,
    /// Optional human-readable message.
    pub message: Option<String>,
}

/// A loop invariant (`invariant(expr)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Invariant {
    /// The condition that must hold at each loop iteration.
    pub condition: ContractExpr,
    /// Optional human-readable message.
    pub message: Option<String>,
}

/// A decreases clause for loop termination.
#[derive(Debug, Clone, PartialEq)]
pub struct Decreases {
    /// Expression that strictly decreases each iteration.
    pub expr: ContractExpr,
}

/// A complete function contract — all conditions attached to a single function.
#[derive(Debug, Clone, Default)]
pub struct FunctionContract {
    /// All preconditions (`requires`).
    pub requires: Vec<Requires>,
    /// All postconditions (`ensures`).
    pub ensures: Vec<Ensures>,
    /// Whether this function is `@verified`.
    pub is_verified: bool,
}

impl FunctionContract {
    /// Creates an empty contract.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a precondition.
    pub fn add_requires(&mut self, condition: ContractExpr, message: Option<String>) {
        self.requires.push(Requires { condition, message });
    }

    /// Adds a postcondition.
    pub fn add_ensures(&mut self, condition: ContractExpr, message: Option<String>) {
        self.ensures.push(Ensures { condition, message });
    }

    /// Returns `true` if this contract has any conditions.
    pub fn has_conditions(&self) -> bool {
        !self.requires.is_empty() || !self.ensures.is_empty()
    }

    /// Total number of conditions.
    pub fn condition_count(&self) -> usize {
        self.requires.len() + self.ensures.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.4: Assert vs Contract Distinction
// ═══════════════════════════════════════════════════════════════════════

/// Distinguishes between runtime assertions and compile-time contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    /// `assert(expr)` — checked at runtime.
    RuntimeAssert,
    /// `requires(expr)` — checked at compile-time (or lowered to runtime).
    Precondition,
    /// `ensures(expr)` — checked at compile-time (or lowered to runtime).
    Postcondition,
    /// `invariant(expr)` — checked at compile-time (or lowered to runtime).
    LoopInvariant,
}

impl fmt::Display for CheckKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckKind::RuntimeAssert => write!(f, "assert"),
            CheckKind::Precondition => write!(f, "requires"),
            CheckKind::Postcondition => write!(f, "ensures"),
            CheckKind::LoopInvariant => write!(f, "invariant"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.5: Runtime Fallback
// ═══════════════════════════════════════════════════════════════════════

/// Lowered contract — when `--no-verify`, contracts become runtime checks.
#[derive(Debug, Clone)]
pub struct RuntimeCheck {
    /// The kind of check.
    pub kind: CheckKind,
    /// The condition expression.
    pub condition: ContractExpr,
    /// Error message on failure.
    pub failure_message: String,
    /// Function name where this check lives.
    pub function_name: String,
}

/// Lowers a function contract to runtime assertions.
pub fn lower_to_runtime(fn_name: &str, contract: &FunctionContract) -> Vec<RuntimeCheck> {
    let mut checks = Vec::new();

    for req in &contract.requires {
        checks.push(RuntimeCheck {
            kind: CheckKind::Precondition,
            condition: req.condition.clone(),
            failure_message: req
                .message
                .clone()
                .unwrap_or_else(|| format!("precondition failed in `{fn_name}`")),
            function_name: fn_name.into(),
        });
    }

    for ens in &contract.ensures {
        checks.push(RuntimeCheck {
            kind: CheckKind::Postcondition,
            condition: ens.condition.clone(),
            failure_message: ens
                .message
                .clone()
                .unwrap_or_else(|| format!("postcondition failed in `{fn_name}`")),
            function_name: fn_name.into(),
        });
    }

    checks
}

// ═══════════════════════════════════════════════════════════════════════
// S9.6: Contract Inheritance
// ═══════════════════════════════════════════════════════════════════════

/// A trait method contract that must be satisfied by implementations.
#[derive(Debug, Clone)]
pub struct TraitMethodContract {
    /// Trait name.
    pub trait_name: String,
    /// Method name.
    pub method_name: String,
    /// The contract to inherit.
    pub contract: FunctionContract,
}

/// Checks whether an impl method satisfies the trait method's contract.
/// Impl preconditions must be WEAKER (accept more), postconditions STRONGER (guarantee more).
pub fn check_contract_inheritance(
    trait_contract: &FunctionContract,
    impl_contract: &FunctionContract,
) -> Result<(), ContractError> {
    // Behavioral subtyping (Liskov): impl must accept all inputs trait accepts
    // and produce outputs within trait's guarantees.
    // Simplified check: impl must have all trait postconditions (can add more).
    for trait_ens in &trait_contract.ensures {
        let found = impl_contract
            .ensures
            .iter()
            .any(|ie| ie.condition == trait_ens.condition);
        if !found {
            return Err(ContractError::MissingInheritedPostcondition {
                condition: trait_ens.condition.to_string(),
            });
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// S9.7: Old Value Capture
// ═══════════════════════════════════════════════════════════════════════

/// Collects all `old(expr)` references from a contract expression.
pub fn collect_old_captures(expr: &ContractExpr) -> Vec<ContractExpr> {
    let mut captures = Vec::new();
    collect_old_inner(expr, &mut captures);
    captures
}

fn collect_old_inner(expr: &ContractExpr, captures: &mut Vec<ContractExpr>) {
    match expr {
        ContractExpr::Old(inner) => {
            captures.push(*inner.clone());
        }
        ContractExpr::BinOp { lhs, rhs, .. } => {
            collect_old_inner(lhs, captures);
            collect_old_inner(rhs, captures);
        }
        ContractExpr::Not(e) => collect_old_inner(e, captures),
        ContractExpr::Call { args, .. } => {
            for a in args {
                collect_old_inner(a, captures);
            }
        }
        ContractExpr::Index { array, index } => {
            collect_old_inner(array, captures);
            collect_old_inner(index, captures);
        }
        ContractExpr::Field { object, .. } => collect_old_inner(object, captures),
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.9: Contract Error Codes (VE001-VE008)
// ═══════════════════════════════════════════════════════════════════════

/// Verification error types.
#[derive(Debug, Clone, PartialEq)]
pub enum ContractError {
    /// VE001: Precondition violation.
    PreconditionViolation { function: String, condition: String },
    /// VE002: Postcondition violation.
    PostconditionViolation { function: String, condition: String },
    /// VE003: Loop invariant violation.
    InvariantViolation { condition: String },
    /// VE004: Impl doesn't satisfy trait contract.
    MissingInheritedPostcondition { condition: String },
    /// VE005: Invalid old() usage (not in ensures).
    InvalidOldUsage { context: String },
    /// VE006: Decreases clause not strictly decreasing.
    NonDecreasingLoop { expr: String },
    /// VE007: Verification timeout.
    VerificationTimeout { function: String, timeout_ms: u64 },
    /// VE008: Verification unknown (solver inconclusive).
    VerificationUnknown { function: String },
}

impl ContractError {
    /// Returns the error code string.
    pub fn code(&self) -> &'static str {
        match self {
            ContractError::PreconditionViolation { .. } => "VE001",
            ContractError::PostconditionViolation { .. } => "VE002",
            ContractError::InvariantViolation { .. } => "VE003",
            ContractError::MissingInheritedPostcondition { .. } => "VE004",
            ContractError::InvalidOldUsage { .. } => "VE005",
            ContractError::NonDecreasingLoop { .. } => "VE006",
            ContractError::VerificationTimeout { .. } => "VE007",
            ContractError::VerificationUnknown { .. } => "VE008",
        }
    }
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractError::PreconditionViolation {
                function,
                condition,
            } => write!(
                f,
                "precondition `{condition}` violated in function `{function}`"
            ),
            ContractError::PostconditionViolation {
                function,
                condition,
            } => write!(
                f,
                "postcondition `{condition}` violated in function `{function}`"
            ),
            ContractError::InvariantViolation { condition } => {
                write!(f, "loop invariant `{condition}` violated")
            }
            ContractError::MissingInheritedPostcondition { condition } => write!(
                f,
                "impl missing inherited postcondition `{condition}` from trait"
            ),
            ContractError::InvalidOldUsage { context } => {
                write!(f, "`old()` used outside ensures context: {context}")
            }
            ContractError::NonDecreasingLoop { expr } => {
                write!(
                    f,
                    "decreases expression `{expr}` is not strictly decreasing"
                )
            }
            ContractError::VerificationTimeout {
                function,
                timeout_ms,
            } => write!(
                f,
                "verification of `{function}` timed out after {timeout_ms}ms"
            ),
            ContractError::VerificationUnknown { function } => {
                write!(f, "verification of `{function}` is inconclusive")
            }
        }
    }
}

/// Validates that `old()` is only used in ensures clauses.
pub fn validate_old_usage(contract: &FunctionContract) -> Vec<ContractError> {
    let mut errors = Vec::new();
    for req in &contract.requires {
        let captures = collect_old_captures(&req.condition);
        if !captures.is_empty() {
            errors.push(ContractError::InvalidOldUsage {
                context: "requires".into(),
            });
        }
    }
    errors
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S9.1 — Requires
    #[test]
    fn s9_1_requires_creation() {
        let mut contract = FunctionContract::new();
        contract.add_requires(
            ContractExpr::BinOp {
                op: ContractOp::Gt,
                lhs: Box::new(ContractExpr::Var("n".into())),
                rhs: Box::new(ContractExpr::IntLit(0)),
            },
            None,
        );
        assert_eq!(contract.requires.len(), 1);
        assert!(contract.has_conditions());
    }

    #[test]
    fn s9_1_requires_display() {
        let expr = ContractExpr::BinOp {
            op: ContractOp::Gt,
            lhs: Box::new(ContractExpr::Var("x".into())),
            rhs: Box::new(ContractExpr::IntLit(0)),
        };
        assert_eq!(expr.to_string(), "(x > 0)");
    }

    // S9.2 — Ensures
    #[test]
    fn s9_2_ensures_creation() {
        let mut contract = FunctionContract::new();
        contract.add_ensures(
            ContractExpr::BinOp {
                op: ContractOp::Ge,
                lhs: Box::new(ContractExpr::Result),
                rhs: Box::new(ContractExpr::IntLit(0)),
            },
            None,
        );
        assert_eq!(contract.ensures.len(), 1);
    }

    #[test]
    fn s9_2_result_display() {
        let expr = ContractExpr::BinOp {
            op: ContractOp::Ge,
            lhs: Box::new(ContractExpr::Result),
            rhs: Box::new(ContractExpr::IntLit(0)),
        };
        assert_eq!(expr.to_string(), "(result >= 0)");
    }

    // S9.3 — Invariant
    #[test]
    fn s9_3_invariant_creation() {
        let inv = Invariant {
            condition: ContractExpr::BinOp {
                op: ContractOp::Lt,
                lhs: Box::new(ContractExpr::Var("i".into())),
                rhs: Box::new(ContractExpr::Var("n".into())),
            },
            message: Some("loop bound".into()),
        };
        assert_eq!(inv.message.as_deref(), Some("loop bound"));
    }

    #[test]
    fn s9_3_decreases_clause() {
        let dec = Decreases {
            expr: ContractExpr::BinOp {
                op: ContractOp::Sub,
                lhs: Box::new(ContractExpr::Var("n".into())),
                rhs: Box::new(ContractExpr::Var("i".into())),
            },
        };
        assert_eq!(dec.expr.to_string(), "(n - i)");
    }

    // S9.4 — Assert vs Contract
    #[test]
    fn s9_4_check_kind_display() {
        assert_eq!(CheckKind::RuntimeAssert.to_string(), "assert");
        assert_eq!(CheckKind::Precondition.to_string(), "requires");
        assert_eq!(CheckKind::Postcondition.to_string(), "ensures");
        assert_eq!(CheckKind::LoopInvariant.to_string(), "invariant");
    }

    #[test]
    fn s9_4_check_kind_distinction() {
        assert_ne!(CheckKind::RuntimeAssert, CheckKind::Precondition);
        assert_ne!(CheckKind::Precondition, CheckKind::Postcondition);
    }

    // S9.5 — Runtime Fallback
    #[test]
    fn s9_5_lower_to_runtime() {
        let mut contract = FunctionContract::new();
        contract.add_requires(
            ContractExpr::BinOp {
                op: ContractOp::Gt,
                lhs: Box::new(ContractExpr::Var("x".into())),
                rhs: Box::new(ContractExpr::IntLit(0)),
            },
            None,
        );
        contract.add_ensures(
            ContractExpr::BinOp {
                op: ContractOp::Ge,
                lhs: Box::new(ContractExpr::Result),
                rhs: Box::new(ContractExpr::IntLit(0)),
            },
            Some("result non-negative".into()),
        );
        let checks = lower_to_runtime("sqrt", &contract);
        assert_eq!(checks.len(), 2);
        assert_eq!(checks[0].kind, CheckKind::Precondition);
        assert_eq!(checks[1].kind, CheckKind::Postcondition);
        assert!(checks[0].failure_message.contains("precondition"));
        assert_eq!(checks[1].failure_message, "result non-negative");
    }

    #[test]
    fn s9_5_empty_contract_no_runtime() {
        let contract = FunctionContract::new();
        let checks = lower_to_runtime("foo", &contract);
        assert!(checks.is_empty());
    }

    // S9.6 — Contract Inheritance
    #[test]
    fn s9_6_inheritance_satisfied() {
        let post = ContractExpr::BinOp {
            op: ContractOp::Ge,
            lhs: Box::new(ContractExpr::Result),
            rhs: Box::new(ContractExpr::IntLit(0)),
        };
        let mut trait_contract = FunctionContract::new();
        trait_contract.add_ensures(post.clone(), None);
        let mut impl_contract = FunctionContract::new();
        impl_contract.add_ensures(post, None);
        assert!(check_contract_inheritance(&trait_contract, &impl_contract).is_ok());
    }

    #[test]
    fn s9_6_inheritance_missing_postcondition() {
        let post = ContractExpr::BinOp {
            op: ContractOp::Ge,
            lhs: Box::new(ContractExpr::Result),
            rhs: Box::new(ContractExpr::IntLit(0)),
        };
        let mut trait_contract = FunctionContract::new();
        trait_contract.add_ensures(post, None);
        let impl_contract = FunctionContract::new();
        let err = check_contract_inheritance(&trait_contract, &impl_contract).unwrap_err();
        assert_eq!(err.code(), "VE004");
    }

    // S9.7 — Old Value Capture
    #[test]
    fn s9_7_old_capture_collection() {
        let expr = ContractExpr::BinOp {
            op: ContractOp::Gt,
            lhs: Box::new(ContractExpr::Result),
            rhs: Box::new(ContractExpr::Old(Box::new(ContractExpr::Var("x".into())))),
        };
        let captures = collect_old_captures(&expr);
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0], ContractExpr::Var("x".into()));
    }

    #[test]
    fn s9_7_no_old_captures() {
        let expr = ContractExpr::BinOp {
            op: ContractOp::Gt,
            lhs: Box::new(ContractExpr::Var("x".into())),
            rhs: Box::new(ContractExpr::IntLit(0)),
        };
        assert!(collect_old_captures(&expr).is_empty());
    }

    #[test]
    fn s9_7_old_in_requires_rejected() {
        let mut contract = FunctionContract::new();
        contract.add_requires(
            ContractExpr::Old(Box::new(ContractExpr::Var("x".into()))),
            None,
        );
        let errors = validate_old_usage(&contract);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), "VE005");
    }

    // S9.8 — Multiple Contracts
    #[test]
    fn s9_8_multiple_conditions() {
        let mut contract = FunctionContract::new();
        contract.add_requires(
            ContractExpr::BinOp {
                op: ContractOp::Gt,
                lhs: Box::new(ContractExpr::Var("x".into())),
                rhs: Box::new(ContractExpr::IntLit(0)),
            },
            None,
        );
        contract.add_requires(
            ContractExpr::BinOp {
                op: ContractOp::Lt,
                lhs: Box::new(ContractExpr::Var("x".into())),
                rhs: Box::new(ContractExpr::IntLit(100)),
            },
            None,
        );
        contract.add_ensures(
            ContractExpr::BinOp {
                op: ContractOp::Ge,
                lhs: Box::new(ContractExpr::Result),
                rhs: Box::new(ContractExpr::IntLit(0)),
            },
            None,
        );
        assert_eq!(contract.condition_count(), 3);
    }

    // S9.9 — Error Codes
    #[test]
    fn s9_9_error_codes() {
        assert_eq!(
            ContractError::PreconditionViolation {
                function: "f".into(),
                condition: "x > 0".into()
            }
            .code(),
            "VE001"
        );
        assert_eq!(
            ContractError::PostconditionViolation {
                function: "f".into(),
                condition: "r >= 0".into()
            }
            .code(),
            "VE002"
        );
        assert_eq!(
            ContractError::InvariantViolation {
                condition: "i < n".into()
            }
            .code(),
            "VE003"
        );
        assert_eq!(
            ContractError::VerificationTimeout {
                function: "f".into(),
                timeout_ms: 5000
            }
            .code(),
            "VE007"
        );
        assert_eq!(
            ContractError::VerificationUnknown {
                function: "f".into()
            }
            .code(),
            "VE008"
        );
    }

    #[test]
    fn s9_9_error_display() {
        let err = ContractError::PreconditionViolation {
            function: "sqrt".into(),
            condition: "x >= 0".into(),
        };
        assert!(err.to_string().contains("precondition"));
        assert!(err.to_string().contains("sqrt"));
    }

    // S9.10 — Additional
    #[test]
    fn s9_10_complex_contract_expr() {
        let expr = ContractExpr::BinOp {
            op: ContractOp::Implies,
            lhs: Box::new(ContractExpr::BinOp {
                op: ContractOp::Gt,
                lhs: Box::new(ContractExpr::Var("n".into())),
                rhs: Box::new(ContractExpr::IntLit(0)),
            }),
            rhs: Box::new(ContractExpr::BinOp {
                op: ContractOp::Gt,
                lhs: Box::new(ContractExpr::Result),
                rhs: Box::new(ContractExpr::IntLit(0)),
            }),
        };
        assert!(expr.to_string().contains("==>"));
    }

    #[test]
    fn s9_10_field_and_index_expr() {
        let expr = ContractExpr::Index {
            array: Box::new(ContractExpr::Field {
                object: Box::new(ContractExpr::Var("self".into())),
                field: "data".into(),
            }),
            index: Box::new(ContractExpr::Var("i".into())),
        };
        assert_eq!(expr.to_string(), "self.data[i]");
    }

    #[test]
    fn s9_10_call_expr() {
        let expr = ContractExpr::Call {
            name: "len".into(),
            args: vec![ContractExpr::Var("arr".into())],
        };
        assert_eq!(expr.to_string(), "len(arr)");
    }

    #[test]
    fn s9_10_not_expr() {
        let expr = ContractExpr::Not(Box::new(ContractExpr::BoolLit(false)));
        assert_eq!(expr.to_string(), "!false");
    }

    #[test]
    fn s9_10_contract_ops_display() {
        assert_eq!(ContractOp::Add.to_string(), "+");
        assert_eq!(ContractOp::Mod.to_string(), "%");
        assert_eq!(ContractOp::Implies.to_string(), "==>");
    }
}
