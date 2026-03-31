//! Property Specification Language — Sprint V2: 10 tasks.
//!
//! Defines annotation types for formal verification: @requires, @ensures,
//! @invariant, @assert, @forall, @eventually, @typestate, @no_leak,
//! and custom @property macros. Includes a PropertyChecker that validates
//! each annotation type against simulated constraints (no real Z3).

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V2.1: Core Annotation Types
// ═══════════════════════════════════════════════════════════════════════

/// A property annotation attached to a function, loop, or block.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyAnnotation {
    /// Precondition: must hold on function entry.
    /// `@requires(x > 0 && y != null)`
    Requires(PropertyExpr),
    /// Postcondition: must hold on function exit.
    /// `@ensures(result >= 0)`
    Ensures(PropertyExpr),
    /// Loop invariant: holds at entry and is preserved by the body.
    /// `@invariant(0 <= i && i <= n)`
    Invariant(PropertyExpr),
    /// Inline assertion: must hold at this program point.
    /// `@assert(len(arr) > 0)`
    Assert(PropertyExpr),
    /// Quantified property: holds for all values in a range.
    /// `@forall(i, 0, n, arr[i] >= 0)`
    Forall(QuantifiedProp),
    /// Temporal property: something must eventually hold.
    /// `@eventually(state == Done)`
    Eventually(PropertyExpr),
    /// Type state property: variable transitions through valid states.
    /// `@typestate(file: Closed -> Open -> Closed)`
    TypeState(TypeStateProp),
    /// Data flow: no data leak from a scope.
    /// `@no_leak(secret)`
    NoLeak(String),
    /// Custom property macro (user-defined).
    /// `@property(my_custom_check, arg1, arg2)`
    Custom(CustomPropertyMacro),
}

impl fmt::Display for PropertyAnnotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Requires(e) => write!(f, "@requires({e})"),
            Self::Ensures(e) => write!(f, "@ensures({e})"),
            Self::Invariant(e) => write!(f, "@invariant({e})"),
            Self::Assert(e) => write!(f, "@assert({e})"),
            Self::Forall(q) => write!(f, "@forall({q})"),
            Self::Eventually(e) => write!(f, "@eventually({e})"),
            Self::TypeState(ts) => write!(f, "@typestate({ts})"),
            Self::NoLeak(var) => write!(f, "@no_leak({var})"),
            Self::Custom(c) => write!(f, "@property({c})"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V2.2: Property Expressions
// ═══════════════════════════════════════════════════════════════════════

/// A property expression (logical formula used in annotations).
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyExpr {
    /// Boolean literal.
    BoolLit(bool),
    /// Integer literal.
    IntLit(i64),
    /// Float literal.
    FloatLit(f64),
    /// Variable reference.
    Var(String),
    /// `result` keyword (return value in @ensures).
    Result,
    /// `old(expr)` (value at function entry).
    Old(Box<PropertyExpr>),
    /// Binary operation.
    BinOp(Box<PropertyExpr>, PropBinOp, Box<PropertyExpr>),
    /// Unary operation.
    UnaryOp(PropUnaryOp, Box<PropertyExpr>),
    /// Implication: `a ==> b`.
    Implies(Box<PropertyExpr>, Box<PropertyExpr>),
    /// Array/collection indexing.
    Index(Box<PropertyExpr>, Box<PropertyExpr>),
    /// `len(expr)`.
    Len(Box<PropertyExpr>),
    /// Pure function call in spec context.
    Call(String, Vec<PropertyExpr>),
}

impl fmt::Display for PropertyExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoolLit(b) => write!(f, "{b}"),
            Self::IntLit(i) => write!(f, "{i}"),
            Self::FloatLit(v) => write!(f, "{v}"),
            Self::Var(name) => write!(f, "{name}"),
            Self::Result => write!(f, "result"),
            Self::Old(inner) => write!(f, "old({inner})"),
            Self::BinOp(lhs, op, rhs) => write!(f, "({lhs} {op} {rhs})"),
            Self::UnaryOp(op, inner) => write!(f, "({op}{inner})"),
            Self::Implies(lhs, rhs) => write!(f, "({lhs} ==> {rhs})"),
            Self::Index(arr, idx) => write!(f, "{arr}[{idx}]"),
            Self::Len(inner) => write!(f, "len({inner})"),
            Self::Call(name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                write!(f, "{name}({})", args_str.join(", "))
            }
        }
    }
}

/// Binary operators for property expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropBinOp {
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
}

impl fmt::Display for PropBinOp {
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
        };
        write!(f, "{s}")
    }
}

/// Unary operators for property expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropUnaryOp {
    /// Logical NOT.
    Not,
    /// Arithmetic negation.
    Neg,
}

impl fmt::Display for PropUnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Not => "!",
            Self::Neg => "-",
        };
        write!(f, "{s}")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V2.3: Quantified Properties
// ═══════════════════════════════════════════════════════════════════════

/// A universally quantified property: `@forall(var, lo, hi, body)`.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantifiedProp {
    /// Bound variable name.
    pub var: String,
    /// Lower bound (inclusive).
    pub lo: PropertyExpr,
    /// Upper bound (exclusive).
    pub hi: PropertyExpr,
    /// Body expression (must hold for all var in [lo, hi)).
    pub body: PropertyExpr,
}

impl fmt::Display for QuantifiedProp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}, {}, {}", self.var, self.lo, self.hi, self.body)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V2.4: Temporal Properties
// ═══════════════════════════════════════════════════════════════════════

/// Temporal property kind (for liveness/safety over execution traces).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemporalKind {
    /// Something must eventually hold.
    Eventually,
    /// Something must always hold.
    Always,
    /// Something holds until another condition.
    Until,
}

impl fmt::Display for TemporalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Eventually => "eventually",
            Self::Always => "always",
            Self::Until => "until",
        };
        write!(f, "{s}")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V2.5: Type State Properties
// ═══════════════════════════════════════════════════════════════════════

/// A type state property: `@typestate(var: S1 -> S2 -> S3)`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeStateProp {
    /// Variable being tracked.
    pub variable: String,
    /// Ordered valid state transitions.
    pub transitions: Vec<String>,
}

impl TypeStateProp {
    /// Checks if a transition from `from` to `to` is valid.
    pub fn is_valid_transition(&self, from: &str, to: &str) -> bool {
        for i in 0..self.transitions.len().saturating_sub(1) {
            if self.transitions[i] == from && self.transitions[i + 1] == to {
                return true;
            }
        }
        false
    }

    /// Returns the initial state.
    pub fn initial_state(&self) -> Option<&str> {
        self.transitions.first().map(|s| s.as_str())
    }

    /// Returns the final state.
    pub fn final_state(&self) -> Option<&str> {
        self.transitions.last().map(|s| s.as_str())
    }
}

impl fmt::Display for TypeStateProp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.variable, self.transitions.join(" -> "))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V2.6: Data Flow Properties (@no_leak)
// ═══════════════════════════════════════════════════════════════════════

/// A data flow property check result.
#[derive(Debug, Clone, PartialEq)]
pub struct DataFlowResult {
    /// Variable being tracked.
    pub variable: String,
    /// Whether a leak was detected.
    pub leaked: bool,
    /// If leaked, where the data escaped.
    pub leak_site: Option<String>,
    /// Flow path (how the data propagated).
    pub flow_path: Vec<String>,
}

impl fmt::Display for DataFlowResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.leaked {
            write!(
                f,
                "LEAK: {} escaped via {}",
                self.variable,
                self.leak_site.as_deref().unwrap_or("unknown")
            )?;
            if !self.flow_path.is_empty() {
                write!(f, " (path: {})", self.flow_path.join(" -> "))?;
            }
        } else {
            write!(f, "OK: {} is contained", self.variable)?;
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V2.7: Custom Property Macros
// ═══════════════════════════════════════════════════════════════════════

/// A custom property macro: `@property(name, arg1, arg2, ...)`.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomPropertyMacro {
    /// Macro name.
    pub name: String,
    /// Arguments.
    pub args: Vec<PropertyExpr>,
    /// Expansion (the property expression this macro expands to).
    pub expansion: Option<PropertyExpr>,
}

impl fmt::Display for CustomPropertyMacro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let args_str: Vec<String> = self.args.iter().map(|a| format!("{a}")).collect();
        write!(f, "{}, {}", self.name, args_str.join(", "))
    }
}

/// Registry of custom property macros.
#[derive(Debug, Clone, Default)]
pub struct PropertyMacroRegistry {
    /// Macro name -> definition.
    macros: HashMap<String, CustomPropertyMacro>,
}

impl PropertyMacroRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a custom property macro.
    pub fn register(&mut self, name: String, macro_def: CustomPropertyMacro) {
        self.macros.insert(name, macro_def);
    }

    /// Looks up a macro by name.
    pub fn get(&self, name: &str) -> Option<&CustomPropertyMacro> {
        self.macros.get(name)
    }

    /// Returns the number of registered macros.
    pub fn len(&self) -> usize {
        self.macros.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.macros.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V2.8: Function Property Specification
// ═══════════════════════════════════════════════════════════════════════

/// A complete property specification for a function.
#[derive(Debug, Clone, Default)]
pub struct FunctionPropertySpec {
    /// Function name.
    pub name: String,
    /// Parameter names and types.
    pub params: Vec<(String, String)>,
    /// Return type.
    pub return_type: String,
    /// Preconditions (@requires).
    pub requires: Vec<PropertyExpr>,
    /// Postconditions (@ensures).
    pub ensures: Vec<PropertyExpr>,
    /// Data flow properties (@no_leak).
    pub no_leak: Vec<String>,
    /// Type state properties.
    pub type_states: Vec<TypeStateProp>,
    /// Custom properties.
    pub custom: Vec<CustomPropertyMacro>,
}

// ═══════════════════════════════════════════════════════════════════════
// V2.9-V2.10: Property Checker
// ═══════════════════════════════════════════════════════════════════════

/// Result of checking a single property.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyCheckResult {
    /// Property holds.
    Verified,
    /// Property may be violated (with description).
    Violated(String),
    /// Could not determine (constraints too complex for simulated checker).
    Unknown(String),
    /// Checking timed out.
    Timeout,
}

impl fmt::Display for PropertyCheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verified => write!(f, "VERIFIED"),
            Self::Violated(msg) => write!(f, "VIOLATED: {msg}"),
            Self::Unknown(msg) => write!(f, "UNKNOWN: {msg}"),
            Self::Timeout => write!(f, "TIMEOUT"),
        }
    }
}

/// A single check report entry.
#[derive(Debug, Clone)]
pub struct PropertyCheckEntry {
    /// The annotation that was checked.
    pub annotation: String,
    /// Source file.
    pub file: String,
    /// Source line.
    pub line: u32,
    /// Check result.
    pub result: PropertyCheckResult,
}

/// Property checker: validates annotations against simulated constraints.
#[derive(Debug)]
pub struct PropertyChecker {
    /// Known variable types for checking.
    known_types: HashMap<String, String>,
    /// Known variable value ranges (name -> (min, max)).
    known_ranges: HashMap<String, (i64, i64)>,
    /// Known state transitions (variable -> current state).
    state_tracker: HashMap<String, String>,
    /// Data flow tracker (variable -> set of scopes it has been passed to).
    flow_tracker: HashMap<String, Vec<String>>,
    /// Check results.
    pub results: Vec<PropertyCheckEntry>,
    /// Custom macro registry.
    pub macro_registry: PropertyMacroRegistry,
}

impl PropertyChecker {
    /// Creates a new property checker.
    pub fn new() -> Self {
        Self {
            known_types: HashMap::new(),
            known_ranges: HashMap::new(),
            state_tracker: HashMap::new(),
            flow_tracker: HashMap::new(),
            results: Vec::new(),
            macro_registry: PropertyMacroRegistry::new(),
        }
    }

    /// Registers a variable with its type.
    pub fn register_var(&mut self, name: &str, var_type: &str) {
        self.known_types
            .insert(name.to_string(), var_type.to_string());
    }

    /// Registers a variable's known value range.
    pub fn register_range(&mut self, name: &str, min: i64, max: i64) {
        self.known_ranges.insert(name.to_string(), (min, max));
    }

    /// Sets the current type state of a variable.
    pub fn set_state(&mut self, name: &str, state: &str) {
        self.state_tracker
            .insert(name.to_string(), state.to_string());
    }

    /// Records a data flow event (variable passed to a scope).
    pub fn record_flow(&mut self, name: &str, scope: &str) {
        self.flow_tracker
            .entry(name.to_string())
            .or_default()
            .push(scope.to_string());
    }

    /// Checks a @requires annotation.
    pub fn check_requires(
        &mut self,
        expr: &PropertyExpr,
        file: &str,
        line: u32,
    ) -> PropertyCheckResult {
        let result = self.evaluate_expr(expr);
        let entry = PropertyCheckEntry {
            annotation: format!("@requires({expr})"),
            file: file.to_string(),
            line,
            result: result.clone(),
        };
        self.results.push(entry);
        result
    }

    /// Checks an @ensures annotation.
    pub fn check_ensures(
        &mut self,
        expr: &PropertyExpr,
        file: &str,
        line: u32,
    ) -> PropertyCheckResult {
        let result = self.evaluate_expr(expr);
        let entry = PropertyCheckEntry {
            annotation: format!("@ensures({expr})"),
            file: file.to_string(),
            line,
            result: result.clone(),
        };
        self.results.push(entry);
        result
    }

    /// Checks an @invariant annotation.
    pub fn check_invariant(
        &mut self,
        expr: &PropertyExpr,
        file: &str,
        line: u32,
    ) -> PropertyCheckResult {
        let result = self.evaluate_expr(expr);
        let entry = PropertyCheckEntry {
            annotation: format!("@invariant({expr})"),
            file: file.to_string(),
            line,
            result: result.clone(),
        };
        self.results.push(entry);
        result
    }

    /// Checks an @assert annotation.
    pub fn check_assert(
        &mut self,
        expr: &PropertyExpr,
        file: &str,
        line: u32,
    ) -> PropertyCheckResult {
        let result = self.evaluate_expr(expr);
        let entry = PropertyCheckEntry {
            annotation: format!("@assert({expr})"),
            file: file.to_string(),
            line,
            result: result.clone(),
        };
        self.results.push(entry);
        result
    }

    /// Checks a @forall annotation.
    pub fn check_forall(
        &mut self,
        prop: &QuantifiedProp,
        file: &str,
        line: u32,
    ) -> PropertyCheckResult {
        // Simulated: check if we can evaluate bounds concretely
        let lo_val = self.try_eval_int(&prop.lo);
        let hi_val = self.try_eval_int(&prop.hi);

        let result = match (lo_val, hi_val) {
            (Some(lo), Some(hi)) => {
                if lo >= hi {
                    PropertyCheckResult::Verified // vacuously true (empty range)
                } else if hi - lo > 1000 {
                    PropertyCheckResult::Unknown("range too large for enumeration".to_string())
                } else {
                    // Enumerate and check each value
                    let mut all_ok = true;
                    for i in lo..hi {
                        self.register_range(&prop.var, i, i);
                        let check = self.evaluate_expr(&prop.body);
                        if matches!(check, PropertyCheckResult::Violated(_)) {
                            all_ok = false;
                            break;
                        }
                    }
                    if all_ok {
                        PropertyCheckResult::Verified
                    } else {
                        PropertyCheckResult::Violated(format!(
                            "counterexample found in range [{lo}, {hi})"
                        ))
                    }
                }
            }
            _ => PropertyCheckResult::Unknown("cannot evaluate bounds concretely".to_string()),
        };

        let entry = PropertyCheckEntry {
            annotation: format!("@forall({prop})"),
            file: file.to_string(),
            line,
            result: result.clone(),
        };
        self.results.push(entry);
        result
    }

    /// Checks a @typestate annotation.
    pub fn check_typestate(
        &mut self,
        ts: &TypeStateProp,
        file: &str,
        line: u32,
    ) -> PropertyCheckResult {
        let current = self.state_tracker.get(&ts.variable).cloned();
        let result = match current {
            Some(ref state) => {
                if ts.transitions.contains(state) {
                    PropertyCheckResult::Verified
                } else {
                    PropertyCheckResult::Violated(format!(
                        "variable '{}' in invalid state '{state}', expected one of: {}",
                        ts.variable,
                        ts.transitions.join(", ")
                    ))
                }
            }
            None => {
                // No state tracked yet, check if initial state is set
                if let Some(initial) = ts.initial_state() {
                    self.state_tracker
                        .insert(ts.variable.clone(), initial.to_string());
                    PropertyCheckResult::Verified
                } else {
                    PropertyCheckResult::Unknown("no states defined".to_string())
                }
            }
        };

        let entry = PropertyCheckEntry {
            annotation: format!("@typestate({ts})"),
            file: file.to_string(),
            line,
            result: result.clone(),
        };
        self.results.push(entry);
        result
    }

    /// Checks a @no_leak annotation.
    pub fn check_no_leak(
        &mut self,
        variable: &str,
        allowed_scopes: &[&str],
        file: &str,
        line: u32,
    ) -> PropertyCheckResult {
        let flows = self.flow_tracker.get(variable).cloned().unwrap_or_default();
        let leaked: Vec<&String> = flows
            .iter()
            .filter(|scope| !allowed_scopes.contains(&scope.as_str()))
            .collect();

        let result = if leaked.is_empty() {
            PropertyCheckResult::Verified
        } else {
            PropertyCheckResult::Violated(format!(
                "variable '{variable}' leaked to: {}",
                leaked
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
            ))
        };

        let entry = PropertyCheckEntry {
            annotation: format!("@no_leak({variable})"),
            file: file.to_string(),
            line,
            result: result.clone(),
        };
        self.results.push(entry);
        result
    }

    /// Checks a @eventually annotation (simulated over a trace).
    pub fn check_eventually(
        &mut self,
        expr: &PropertyExpr,
        trace: &[HashMap<String, i64>],
        file: &str,
        line: u32,
    ) -> PropertyCheckResult {
        let mut found = false;
        for step in trace {
            // Temporarily register ranges from this trace step
            for (name, val) in step {
                self.register_range(name, *val, *val);
            }
            let check = self.evaluate_expr(expr);
            if matches!(check, PropertyCheckResult::Verified) {
                found = true;
                break;
            }
        }

        let result = if found {
            PropertyCheckResult::Verified
        } else if trace.is_empty() {
            PropertyCheckResult::Unknown("empty trace".to_string())
        } else {
            PropertyCheckResult::Violated(format!(
                "property {expr} never held in {}-step trace",
                trace.len()
            ))
        };

        let entry = PropertyCheckEntry {
            annotation: format!("@eventually({expr})"),
            file: file.to_string(),
            line,
            result: result.clone(),
        };
        self.results.push(entry);
        result
    }

    /// Returns the number of verified properties.
    pub fn verified_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.result, PropertyCheckResult::Verified))
            .count()
    }

    /// Returns the number of violated properties.
    pub fn violated_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.result, PropertyCheckResult::Violated(_)))
            .count()
    }

    /// Returns the total number of checks performed.
    pub fn total_checks(&self) -> usize {
        self.results.len()
    }

    // --- Internal helpers ---

    /// Evaluates a property expression using known ranges (simulated).
    fn evaluate_expr(&self, expr: &PropertyExpr) -> PropertyCheckResult {
        match expr {
            PropertyExpr::BoolLit(true) => PropertyCheckResult::Verified,
            PropertyExpr::BoolLit(false) => {
                PropertyCheckResult::Violated("literal false".to_string())
            }
            PropertyExpr::BinOp(lhs, op, rhs) => {
                let lv = self.try_eval_int(lhs);
                let rv = self.try_eval_int(rhs);
                match (lv, rv) {
                    (Some(l), Some(r)) => {
                        let holds = match op {
                            PropBinOp::Eq => l == r,
                            PropBinOp::Ne => l != r,
                            PropBinOp::Lt => l < r,
                            PropBinOp::Le => l <= r,
                            PropBinOp::Gt => l > r,
                            PropBinOp::Ge => l >= r,
                            _ => {
                                return PropertyCheckResult::Unknown(
                                    "non-relational binop".to_string(),
                                );
                            }
                        };
                        if holds {
                            PropertyCheckResult::Verified
                        } else {
                            PropertyCheckResult::Violated(format!("{l} {op} {r} is false"))
                        }
                    }
                    _ => PropertyCheckResult::Unknown("cannot evaluate operands".to_string()),
                }
            }
            PropertyExpr::UnaryOp(PropUnaryOp::Not, inner) => match self.evaluate_expr(inner) {
                PropertyCheckResult::Verified => {
                    PropertyCheckResult::Violated("negation of verified".to_string())
                }
                PropertyCheckResult::Violated(_) => PropertyCheckResult::Verified,
                other => other,
            },
            _ => PropertyCheckResult::Unknown("expression type not handled".to_string()),
        }
    }

    /// Tries to evaluate an expression to a concrete integer.
    fn try_eval_int(&self, expr: &PropertyExpr) -> Option<i64> {
        match expr {
            PropertyExpr::IntLit(i) => Some(*i),
            PropertyExpr::Var(name) => self.known_ranges.get(name).map(|(min, _max)| *min),
            PropertyExpr::BinOp(lhs, op, rhs) => {
                let l = self.try_eval_int(lhs)?;
                let r = self.try_eval_int(rhs)?;
                match op {
                    PropBinOp::Add => Some(l + r),
                    PropBinOp::Sub => Some(l - r),
                    PropBinOp::Mul => Some(l * r),
                    PropBinOp::Div => {
                        if r != 0 {
                            Some(l / r)
                        } else {
                            None
                        }
                    }
                    PropBinOp::Mod => {
                        if r != 0 {
                            Some(l % r)
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            PropertyExpr::UnaryOp(PropUnaryOp::Neg, inner) => self.try_eval_int(inner).map(|v| -v),
            _ => None,
        }
    }
}

impl Default for PropertyChecker {
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

    // --- V2.1: Annotation Display ---

    #[test]
    fn v2_1_requires_display() {
        let ann = PropertyAnnotation::Requires(PropertyExpr::BinOp(
            Box::new(PropertyExpr::Var("x".to_string())),
            PropBinOp::Gt,
            Box::new(PropertyExpr::IntLit(0)),
        ));
        let s = format!("{ann}");
        assert!(s.contains("@requires"));
        assert!(s.contains("x"));
    }

    #[test]
    fn v2_1_ensures_display() {
        let ann = PropertyAnnotation::Ensures(PropertyExpr::BinOp(
            Box::new(PropertyExpr::Result),
            PropBinOp::Ge,
            Box::new(PropertyExpr::IntLit(0)),
        ));
        let s = format!("{ann}");
        assert!(s.contains("@ensures"));
        assert!(s.contains("result"));
    }

    #[test]
    fn v2_1_no_leak_display() {
        let ann = PropertyAnnotation::NoLeak("secret".to_string());
        assert_eq!(format!("{ann}"), "@no_leak(secret)");
    }

    // --- V2.2: PropertyExpr ---

    #[test]
    fn v2_2_expr_display() {
        let expr = PropertyExpr::BinOp(
            Box::new(PropertyExpr::Var("x".to_string())),
            PropBinOp::Add,
            Box::new(PropertyExpr::IntLit(1)),
        );
        assert_eq!(format!("{expr}"), "(x + 1)");
    }

    #[test]
    fn v2_2_old_result_display() {
        let expr = PropertyExpr::BinOp(
            Box::new(PropertyExpr::Result),
            PropBinOp::Ge,
            Box::new(PropertyExpr::Old(Box::new(PropertyExpr::Var(
                "x".to_string(),
            )))),
        );
        let s = format!("{expr}");
        assert!(s.contains("result"));
        assert!(s.contains("old(x)"));
    }

    #[test]
    fn v2_2_implies_display() {
        let expr = PropertyExpr::Implies(
            Box::new(PropertyExpr::Var("a".to_string())),
            Box::new(PropertyExpr::Var("b".to_string())),
        );
        assert_eq!(format!("{expr}"), "(a ==> b)");
    }

    // --- V2.3: Quantified ---

    #[test]
    fn v2_3_forall_display() {
        let ann = PropertyAnnotation::Forall(QuantifiedProp {
            var: "i".to_string(),
            lo: PropertyExpr::IntLit(0),
            hi: PropertyExpr::Var("n".to_string()),
            body: PropertyExpr::BinOp(
                Box::new(PropertyExpr::Index(
                    Box::new(PropertyExpr::Var("arr".to_string())),
                    Box::new(PropertyExpr::Var("i".to_string())),
                )),
                PropBinOp::Ge,
                Box::new(PropertyExpr::IntLit(0)),
            ),
        });
        let s = format!("{ann}");
        assert!(s.contains("@forall"));
        assert!(s.contains("arr"));
    }

    // --- V2.5: TypeState ---

    #[test]
    fn v2_5_typestate_valid_transition() {
        let ts = TypeStateProp {
            variable: "file".to_string(),
            transitions: vec![
                "Closed".to_string(),
                "Open".to_string(),
                "Closed".to_string(),
            ],
        };
        assert!(ts.is_valid_transition("Closed", "Open"));
        assert!(ts.is_valid_transition("Open", "Closed"));
        assert!(!ts.is_valid_transition("Open", "Open"));
        assert_eq!(ts.initial_state(), Some("Closed"));
        assert_eq!(ts.final_state(), Some("Closed"));
    }

    #[test]
    fn v2_5_typestate_display() {
        let ts = TypeStateProp {
            variable: "conn".to_string(),
            transitions: vec![
                "Init".to_string(),
                "Connected".to_string(),
                "Closed".to_string(),
            ],
        };
        let s = format!("{ts}");
        assert!(s.contains("conn: Init -> Connected -> Closed"));
    }

    // --- V2.6: Data Flow ---

    #[test]
    fn v2_6_data_flow_no_leak() {
        let r = DataFlowResult {
            variable: "key".to_string(),
            leaked: false,
            leak_site: None,
            flow_path: vec!["encrypt".to_string()],
        };
        let s = format!("{r}");
        assert!(s.contains("OK"));
        assert!(s.contains("contained"));
    }

    #[test]
    fn v2_6_data_flow_leak() {
        let r = DataFlowResult {
            variable: "secret".to_string(),
            leaked: true,
            leak_site: Some("log()".to_string()),
            flow_path: vec!["process".to_string(), "log()".to_string()],
        };
        let s = format!("{r}");
        assert!(s.contains("LEAK"));
        assert!(s.contains("log()"));
    }

    // --- V2.7: Custom Macros ---

    #[test]
    fn v2_7_macro_registry() {
        let mut reg = PropertyMacroRegistry::new();
        assert!(reg.is_empty());

        reg.register(
            "bounded".to_string(),
            CustomPropertyMacro {
                name: "bounded".to_string(),
                args: vec![PropertyExpr::Var("x".to_string())],
                expansion: Some(PropertyExpr::BinOp(
                    Box::new(PropertyExpr::Var("x".to_string())),
                    PropBinOp::Ge,
                    Box::new(PropertyExpr::IntLit(0)),
                )),
            },
        );
        assert_eq!(reg.len(), 1);
        assert!(reg.get("bounded").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    // --- V2.9: PropertyChecker requires/ensures ---

    #[test]
    fn v2_9_check_requires_verified() {
        let mut checker = PropertyChecker::new();
        checker.register_range("x", 5, 5);
        let expr = PropertyExpr::BinOp(
            Box::new(PropertyExpr::Var("x".to_string())),
            PropBinOp::Gt,
            Box::new(PropertyExpr::IntLit(0)),
        );
        let result = checker.check_requires(&expr, "main.fj", 10);
        assert_eq!(result, PropertyCheckResult::Verified);
        assert_eq!(checker.verified_count(), 1);
    }

    #[test]
    fn v2_9_check_requires_violated() {
        let mut checker = PropertyChecker::new();
        checker.register_range("x", -3, -3);
        let expr = PropertyExpr::BinOp(
            Box::new(PropertyExpr::Var("x".to_string())),
            PropBinOp::Gt,
            Box::new(PropertyExpr::IntLit(0)),
        );
        let result = checker.check_requires(&expr, "main.fj", 10);
        assert!(matches!(result, PropertyCheckResult::Violated(_)));
        assert_eq!(checker.violated_count(), 1);
    }

    #[test]
    fn v2_9_check_ensures() {
        let mut checker = PropertyChecker::new();
        let expr = PropertyExpr::BoolLit(true);
        let result = checker.check_ensures(&expr, "fn.fj", 20);
        assert_eq!(result, PropertyCheckResult::Verified);
    }

    #[test]
    fn v2_9_check_assert() {
        let mut checker = PropertyChecker::new();
        let expr = PropertyExpr::BoolLit(false);
        let result = checker.check_assert(&expr, "fn.fj", 30);
        assert!(matches!(result, PropertyCheckResult::Violated(_)));
    }

    // --- V2.10: Checker forall/typestate/no_leak/eventually ---

    #[test]
    fn v2_10_check_forall_small_range() {
        let mut checker = PropertyChecker::new();
        // forall(i, 0, 5, i >= 0) — all non-negative, should verify
        let prop = QuantifiedProp {
            var: "i".to_string(),
            lo: PropertyExpr::IntLit(0),
            hi: PropertyExpr::IntLit(5),
            body: PropertyExpr::BinOp(
                Box::new(PropertyExpr::Var("i".to_string())),
                PropBinOp::Ge,
                Box::new(PropertyExpr::IntLit(0)),
            ),
        };
        let result = checker.check_forall(&prop, "test.fj", 1);
        assert_eq!(result, PropertyCheckResult::Verified);
    }

    #[test]
    fn v2_10_check_forall_empty_range() {
        let mut checker = PropertyChecker::new();
        let prop = QuantifiedProp {
            var: "i".to_string(),
            lo: PropertyExpr::IntLit(5),
            hi: PropertyExpr::IntLit(0), // empty range
            body: PropertyExpr::BoolLit(false),
        };
        let result = checker.check_forall(&prop, "test.fj", 1);
        assert_eq!(result, PropertyCheckResult::Verified); // vacuously true
    }

    #[test]
    fn v2_10_check_typestate_valid() {
        let mut checker = PropertyChecker::new();
        checker.set_state("file", "Open");
        let ts = TypeStateProp {
            variable: "file".to_string(),
            transitions: vec![
                "Closed".to_string(),
                "Open".to_string(),
                "Closed".to_string(),
            ],
        };
        let result = checker.check_typestate(&ts, "io.fj", 15);
        assert_eq!(result, PropertyCheckResult::Verified);
    }

    #[test]
    fn v2_10_check_typestate_invalid() {
        let mut checker = PropertyChecker::new();
        checker.set_state("file", "Error");
        let ts = TypeStateProp {
            variable: "file".to_string(),
            transitions: vec!["Closed".to_string(), "Open".to_string()],
        };
        let result = checker.check_typestate(&ts, "io.fj", 20);
        assert!(matches!(result, PropertyCheckResult::Violated(_)));
    }

    #[test]
    fn v2_10_check_no_leak_ok() {
        let mut checker = PropertyChecker::new();
        checker.record_flow("key", "encrypt");
        checker.record_flow("key", "decrypt");
        let result = checker.check_no_leak("key", &["encrypt", "decrypt"], "crypto.fj", 5);
        assert_eq!(result, PropertyCheckResult::Verified);
    }

    #[test]
    fn v2_10_check_no_leak_violated() {
        let mut checker = PropertyChecker::new();
        checker.record_flow("secret", "process");
        checker.record_flow("secret", "log"); // leak!
        let result = checker.check_no_leak("secret", &["process"], "sec.fj", 10);
        assert!(matches!(result, PropertyCheckResult::Violated(_)));
    }

    #[test]
    fn v2_10_check_eventually() {
        let mut checker = PropertyChecker::new();
        let expr = PropertyExpr::BinOp(
            Box::new(PropertyExpr::Var("state".to_string())),
            PropBinOp::Eq,
            Box::new(PropertyExpr::IntLit(1)),
        );
        let trace = vec![
            HashMap::from([("state".to_string(), 0i64)]),
            HashMap::from([("state".to_string(), 0i64)]),
            HashMap::from([("state".to_string(), 1i64)]),
        ];
        let result = checker.check_eventually(&expr, &trace, "async.fj", 50);
        assert_eq!(result, PropertyCheckResult::Verified);
    }

    #[test]
    fn v2_10_check_eventually_never() {
        let mut checker = PropertyChecker::new();
        let expr = PropertyExpr::BinOp(
            Box::new(PropertyExpr::Var("done".to_string())),
            PropBinOp::Eq,
            Box::new(PropertyExpr::IntLit(1)),
        );
        let trace = vec![
            HashMap::from([("done".to_string(), 0i64)]),
            HashMap::from([("done".to_string(), 0i64)]),
        ];
        let result = checker.check_eventually(&expr, &trace, "stuck.fj", 60);
        assert!(matches!(result, PropertyCheckResult::Violated(_)));
    }

    #[test]
    fn v2_10_checker_totals() {
        let mut checker = PropertyChecker::new();
        checker.check_requires(&PropertyExpr::BoolLit(true), "a.fj", 1);
        checker.check_ensures(&PropertyExpr::BoolLit(false), "a.fj", 2);
        checker.check_assert(&PropertyExpr::BoolLit(true), "a.fj", 3);
        assert_eq!(checker.total_checks(), 3);
        assert_eq!(checker.verified_count(), 2);
        assert_eq!(checker.violated_count(), 1);
    }
}
