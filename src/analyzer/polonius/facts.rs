//! Polonius fact types and fact generation from AST.
//!
//! Defines the core Polonius data types (`Origin`, `Loan`, `Point`, `Place`)
//! and the `FactGenerator` that walks the AST to populate `PoloniusFacts`.
//!
//! # Polonius Model
//!
//! - **Origin**: A unique ID representing a region/lifetime where a reference is valid.
//! - **Loan**: A unique ID representing a specific borrow (`&x` or `&mut x`).
//! - **Point**: A unique program location (basic block + statement index).
//! - **Place**: A path to a memory location (variable + optional field projections).

use std::collections::HashMap;

use crate::lexer::token::Span;
use crate::parser::ast::*;

// ═══════════════════════════════════════════════════════════════════════
// Core types
// ═══════════════════════════════════════════════════════════════════════

/// A unique origin (region/lifetime) identifier.
///
/// Each borrow expression creates a new origin that tracks where the
/// resulting reference is valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Origin(pub u32);

/// A unique loan identifier.
///
/// Each borrow expression (`&x`, `&mut x`) creates a new loan that
/// tracks which place was borrowed and the mutability of the borrow.
#[derive(Debug, Clone)]
pub struct Loan {
    /// Unique loan ID.
    pub id: u32,
    /// The place being borrowed.
    pub place: Place,
    /// Whether the borrow is shared or mutable.
    pub mutability: Mutability,
    /// Source span of the borrow expression.
    pub span: Span,
}

impl PartialEq for Loan {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Loan {}

impl std::hash::Hash for Loan {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// A unique program point (location in the control-flow graph).
///
/// Points represent positions before or in the middle of statements.
/// Each basic block + statement index pair has both a `Start` and `Mid` point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    /// Basic block index.
    pub block: u32,
    /// Statement index within the block.
    pub statement: u32,
    /// Whether this is the start or mid-point of the statement.
    pub kind: PointKind,
}

/// Distinguishes the start and mid-point of a statement.
///
/// - `Start`: before the statement executes
/// - `Mid`: after sub-expressions but before side effects complete
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointKind {
    /// Before the statement executes.
    Start,
    /// After sub-expressions evaluate but before completion.
    Mid,
}

/// A place (memory path) that can be borrowed.
///
/// Represents a variable optionally followed by field/index projections.
/// Examples: `x`, `x.field`, `x[0].field`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Place {
    /// Root variable name.
    pub base: String,
    /// Optional field/index projections from the base.
    pub projections: Vec<PlaceProjection>,
}

/// A single projection step within a place path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlaceProjection {
    /// Field access: `.field_name`.
    Field(String),
    /// Index access: `[index]`.
    Index(u64),
    /// Dereference: `*place`.
    Deref,
}

/// Borrow mutability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mutability {
    /// Shared/immutable borrow (`&x`).
    Shared,
    /// Mutable borrow (`&mut x`).
    Mutable,
}

impl Place {
    /// Creates a simple place from a variable name.
    pub fn from_var(name: &str) -> Self {
        Self {
            base: name.to_string(),
            projections: Vec::new(),
        }
    }

    /// Creates a place with a field projection.
    pub fn with_field(mut self, field: &str) -> Self {
        self.projections
            .push(PlaceProjection::Field(field.to_string()));
        self
    }

    /// Creates a place with an index projection.
    pub fn with_index(mut self, idx: u64) -> Self {
        self.projections.push(PlaceProjection::Index(idx));
        self
    }

    /// Creates a place with a deref projection.
    pub fn with_deref(mut self) -> Self {
        self.projections.push(PlaceProjection::Deref);
        self
    }

    /// Returns the root variable name.
    pub fn root(&self) -> &str {
        &self.base
    }
}

impl std::fmt::Display for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.base)?;
        for proj in &self.projections {
            match proj {
                PlaceProjection::Field(name) => write!(f, ".{name}")?,
                PlaceProjection::Index(idx) => write!(f, "[{idx}]")?,
                PlaceProjection::Deref => write!(f, ".*")?,
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for Mutability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mutability::Shared => write!(f, "shared"),
            Mutability::Mutable => write!(f, "mutable"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PoloniusFacts — all fact tables
// ═══════════════════════════════════════════════════════════════════════

/// All Polonius input facts generated from the AST.
///
/// These tables are consumed by `PoloniusSolver` to compute derived
/// relations and detect borrow errors.
#[derive(Debug, Clone, Default)]
pub struct PoloniusFacts {
    /// `loan_issued_at(origin, loan, point)` — a loan is created at a point
    /// and associated with an origin.
    pub loan_issued_at: Vec<(Origin, Loan, Point)>,

    /// `origin_contains_loan_on_entry(origin, loan, point)` — the origin
    /// contains the loan at the entry of the given point. Seeded from
    /// `loan_issued_at` and propagated along CFG edges.
    pub origin_contains_loan_on_entry: Vec<(Origin, Loan, Point)>,

    /// `loan_invalidated_at(loan, point)` — the loan is invalidated
    /// (e.g., the borrowed place is mutated or moved).
    pub loan_invalidated_at: Vec<(Loan, Point)>,

    /// `origin_live_on_entry(origin, point)` — the origin (reference) is
    /// live (used later) at the entry of the given point.
    pub origin_live_on_entry: Vec<(Origin, Point)>,

    /// `cfg_edge(from, to)` — a control-flow edge between two points.
    pub cfg_edge: Vec<(Point, Point)>,

    /// `loan_killed_at(loan, point)` — the loan is killed (storage
    /// goes out of scope or is reassigned) at the given point.
    pub loan_killed_at: Vec<(Loan, Point)>,

    /// `subset(origin1, origin2, point)` — origin1 is a subset of origin2
    /// at the given point. Used for reborrow propagation.
    pub subset: Vec<(Origin, Origin, Point)>,
}

impl PoloniusFacts {
    /// Creates a new empty facts collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the total number of facts across all tables.
    pub fn total_facts(&self) -> usize {
        self.loan_issued_at.len()
            + self.origin_contains_loan_on_entry.len()
            + self.loan_invalidated_at.len()
            + self.origin_live_on_entry.len()
            + self.cfg_edge.len()
            + self.loan_killed_at.len()
            + self.subset.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FactGenerator — walks AST and populates PoloniusFacts
// ═══════════════════════════════════════════════════════════════════════

/// Generates Polonius facts by walking the AST.
///
/// The generator assigns unique IDs to origins, loans, and points,
/// then walks statements and expressions to populate the fact tables.
pub struct FactGenerator {
    /// Accumulated facts.
    facts: PoloniusFacts,
    /// Next origin ID to assign.
    next_origin: u32,
    /// Next loan ID to assign.
    next_loan: u32,
    /// Current basic block index.
    current_block: u32,
    /// Current statement index within the block.
    current_stmt: u32,
    /// Next block ID to assign.
    next_block: u32,
    /// Maps variable names to their declaration origins.
    var_origins: HashMap<String, Origin>,
    /// Maps variable names to active loans on them.
    var_loans: HashMap<String, Vec<Loan>>,
    /// Tracks which variables are declared mutable.
    mutable_vars: HashMap<String, bool>,
}

impl FactGenerator {
    /// Creates a new fact generator.
    pub fn new() -> Self {
        Self {
            facts: PoloniusFacts::new(),
            next_origin: 0,
            next_loan: 0,
            current_block: 0,
            current_stmt: 0,
            next_block: 1,
            var_origins: HashMap::new(),
            var_loans: HashMap::new(),
            mutable_vars: HashMap::new(),
        }
    }

    /// Generates facts from a program's top-level items.
    pub fn generate(mut self, program: &Program) -> PoloniusFacts {
        for item in &program.items {
            self.visit_item(item);
        }
        self.facts
    }

    /// Generates facts from a single expression (e.g., function body).
    pub fn generate_from_expr(mut self, expr: &Expr) -> PoloniusFacts {
        self.visit_expr(expr);
        self.facts
    }

    /// Generates facts from a sequence of statements.
    pub fn generate_from_stmts(mut self, stmts: &[Stmt]) -> PoloniusFacts {
        for stmt in stmts {
            self.visit_stmt(stmt);
        }
        self.facts
    }

    /// Returns the current program point.
    fn current_point(&self) -> Point {
        Point {
            block: self.current_block,
            statement: self.current_stmt,
            kind: PointKind::Start,
        }
    }

    /// Returns the mid-point of the current statement.
    fn current_mid_point(&self) -> Point {
        Point {
            block: self.current_block,
            statement: self.current_stmt,
            kind: PointKind::Mid,
        }
    }

    /// Advances to the next statement, emitting a CFG edge.
    fn advance_stmt(&mut self) {
        let from = self.current_mid_point();
        self.current_stmt += 1;
        let to = self.current_point();
        self.facts.cfg_edge.push((from, to));
    }

    /// Allocates a fresh origin ID.
    fn fresh_origin(&mut self) -> Origin {
        let o = Origin(self.next_origin);
        self.next_origin += 1;
        o
    }

    /// Allocates a fresh loan.
    fn fresh_loan(&mut self, place: Place, mutability: Mutability, span: Span) -> Loan {
        let loan = Loan {
            id: self.next_loan,
            place,
            mutability,
            span,
        };
        self.next_loan += 1;
        loan
    }

    /// Starts a new basic block, returning its ID.
    fn new_block(&mut self) -> u32 {
        let id = self.next_block;
        self.next_block += 1;
        id
    }

    /// Emits a CFG edge from the current mid-point to a target.
    fn emit_edge_to(&mut self, target: Point) {
        let from = self.current_mid_point();
        self.facts.cfg_edge.push((from, target));
    }

    /// Records that an origin is live at the current point.
    fn mark_origin_live(&mut self, origin: Origin) {
        let point = self.current_point();
        self.facts.origin_live_on_entry.push((origin, point));
    }
}

impl Default for FactGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ── AST visitor methods ────────────────────────────────────────────────

impl FactGenerator {
    fn visit_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(fn_def) => self.visit_fn_def(fn_def),
            Item::Stmt(stmt) => self.visit_stmt(stmt),
            Item::ImplBlock(impl_block) => {
                for method in &impl_block.methods {
                    self.visit_fn_def(method);
                }
            }
            _ => {}
        }
    }

    fn visit_fn_def(&mut self, fn_def: &FnDef) {
        // Save state for function boundary.
        let saved_block = self.current_block;
        let saved_stmt = self.current_stmt;

        let fn_block = self.new_block();
        self.current_block = fn_block;
        self.current_stmt = 0;

        // Register parameters as having origins.
        for param in &fn_def.params {
            let origin = self.fresh_origin();
            self.var_origins.insert(param.name.clone(), origin);
        }

        self.visit_expr(&fn_def.body);

        // Restore state.
        self.current_block = saved_block;
        self.current_stmt = saved_stmt;
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        // Emit start-to-mid edge for the current statement.
        let start = self.current_point();
        let mid = self.current_mid_point();
        self.facts.cfg_edge.push((start, mid));

        match stmt {
            Stmt::Let {
                name,
                mutable,
                value,
                ..
            } => {
                self.visit_let(name, *mutable, value);
            }
            Stmt::Expr { expr, .. } => {
                self.visit_expr(expr);
            }
            Stmt::Return { value, .. } => {
                if let Some(val) = value {
                    self.visit_expr(val);
                }
            }
            Stmt::Break { value, .. } => {
                if let Some(val) = value {
                    self.visit_expr(val);
                }
            }
            Stmt::Continue { .. } => {}
            Stmt::Const { value, .. } => {
                self.visit_expr(value);
            }
            Stmt::Item(item) => {
                self.visit_item(item);
            }
        }

        self.advance_stmt();
    }

    fn visit_let(&mut self, name: &str, mutable: bool, value: &Expr) {
        let origin = self.fresh_origin();
        self.var_origins.insert(name.to_string(), origin);
        self.mutable_vars.insert(name.to_string(), mutable);

        // If the value is a borrow expression, create a loan.
        if let Some((place, mutability, span)) = self.extract_borrow(value) {
            let loan = self.fresh_loan(place.clone(), mutability, span);
            let point = self.current_point();

            self.facts
                .loan_issued_at
                .push((origin, loan.clone(), point));
            self.facts
                .origin_contains_loan_on_entry
                .push((origin, loan.clone(), point));

            // Track the loan on the borrowed variable.
            self.var_loans
                .entry(place.root().to_string())
                .or_default()
                .push(loan);
        }

        self.visit_expr(value);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident { name, .. } => {
                // Mark the variable's origin as live.
                if let Some(&origin) = self.var_origins.get(name) {
                    self.mark_origin_live(origin);
                }
            }
            Expr::Unary { op, operand, span } => {
                self.visit_borrow_expr(op, operand, *span);
            }
            Expr::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Assign {
                target,
                value,
                span,
                ..
            } => {
                self.visit_assign(target, value, *span);
            }
            Expr::Block { stmts, expr, .. } => {
                self.visit_block(stmts, expr.as_deref());
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.visit_if(condition, then_branch, else_branch.as_deref());
            }
            Expr::While {
                label: _,
                condition,
                body,
                ..
            } => {
                self.visit_while(condition, body);
            }
            Expr::For {
                label: _,
                variable,
                iterable,
                body,
                ..
            } => {
                self.visit_for(variable, iterable, body);
            }
            Expr::Loop { label: _, body, .. } => {
                self.visit_loop(body);
            }
            Expr::Call { callee, args, .. } => {
                self.visit_expr(callee);
                for arg in args {
                    self.visit_expr(&arg.value);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.visit_expr(receiver);
                for arg in args {
                    self.visit_expr(&arg.value);
                }
            }
            Expr::Field { object, field, .. } => {
                self.visit_expr(object);
                // Track field access for place projections.
                if let Expr::Ident { name, .. } = object.as_ref() {
                    if let Some(&origin) = self.var_origins.get(name) {
                        let point = self.current_point();
                        self.facts.origin_live_on_entry.push((origin, point));
                    }
                    let _ = field; // used for place projection in borrow extraction
                }
            }
            Expr::Index { object, index, .. } => {
                self.visit_expr(object);
                self.visit_expr(index);
            }
            Expr::Match { subject, arms, .. } => {
                self.visit_expr(subject);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_expr(&arm.body);
                }
            }
            Expr::Array { elements, .. } | Expr::Tuple { elements, .. } => {
                for elem in elements {
                    self.visit_expr(elem);
                }
            }
            Expr::ArrayRepeat { value, count, .. } => {
                self.visit_expr(value);
                self.visit_expr(count);
            }
            Expr::Closure { body, .. } => {
                self.visit_expr(body);
            }
            Expr::StructInit { fields, .. } => {
                for f in fields {
                    self.visit_expr(&f.value);
                }
            }
            Expr::Cast { expr, .. }
            | Expr::Try { expr, .. }
            | Expr::Grouped { expr, .. }
            | Expr::Await { expr, .. } => {
                self.visit_expr(expr);
            }
            Expr::Pipe { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.visit_expr(s);
                }
                if let Some(e) = end {
                    self.visit_expr(e);
                }
            }
            Expr::AsyncBlock { body, .. } => {
                self.visit_expr(body);
            }
            Expr::FString { parts, .. } => {
                for part in parts {
                    if let FStringExprPart::Expr(e) = part {
                        self.visit_expr(e);
                    }
                }
            }
            Expr::InlineAsm { operands, .. } => {
                for op in operands {
                    match op {
                        AsmOperand::In { expr, .. }
                        | AsmOperand::Out { expr, .. }
                        | AsmOperand::InOut { expr, .. }
                        | AsmOperand::LateOut { expr, .. }
                        | AsmOperand::Const { expr } => {
                            self.visit_expr(expr);
                        }
                        AsmOperand::Sym { .. } => {}
                    }
                }
            }
            Expr::HandleEffect { body, handlers, .. } => {
                self.visit_expr(body);
                for handler in handlers {
                    self.visit_expr(&handler.body);
                }
            }
            Expr::ResumeExpr { value, .. } => {
                self.visit_expr(value);
            }
            Expr::Comptime { body, .. } => {
                self.visit_expr(body);
            }
            Expr::MacroInvocation { .. } => {}
            Expr::Literal { .. } | Expr::Path { .. } => {}
            Expr::Yield { .. } => {}
            Expr::MacroVar { .. } => {}
        }
    }

    fn visit_borrow_expr(&mut self, op: &UnaryOp, operand: &Expr, span: Span) {
        let mutability = match op {
            UnaryOp::Ref => Some(Mutability::Shared),
            UnaryOp::RefMut => Some(Mutability::Mutable),
            _ => None,
        };

        if let Some(mut_kind) = mutability {
            if let Some(place) = self.extract_place(operand) {
                let origin = self.fresh_origin();
                let loan = self.fresh_loan(place.clone(), mut_kind, span);
                let point = self.current_point();

                self.facts
                    .loan_issued_at
                    .push((origin, loan.clone(), point));
                self.facts
                    .origin_contains_loan_on_entry
                    .push((origin, loan.clone(), point));

                self.var_loans
                    .entry(place.root().to_string())
                    .or_default()
                    .push(loan);
            }
        }

        self.visit_expr(operand);
    }

    fn visit_assign(&mut self, target: &Expr, value: &Expr, _span: Span) {
        // Assignment invalidates active loans on the target.
        if let Some(place) = self.extract_place(target) {
            let var_name = place.root().to_string();
            if let Some(loans) = self.var_loans.get(&var_name) {
                let point = self.current_point();
                for loan in loans.clone() {
                    self.facts.loan_invalidated_at.push((loan, point));
                }
            }
        }

        self.visit_expr(target);
        self.visit_expr(value);
    }

    fn visit_block(&mut self, stmts: &[Stmt], tail: Option<&Expr>) {
        for stmt in stmts {
            self.visit_stmt(stmt);
        }
        if let Some(expr) = tail {
            self.visit_expr(expr);
        }
    }

    fn visit_if(&mut self, condition: &Expr, then_branch: &Expr, else_branch: Option<&Expr>) {
        self.visit_expr(condition);

        let pre_block = self.current_block;
        let pre_stmt = self.current_stmt;

        // Then branch.
        let then_block = self.new_block();
        self.current_block = then_block;
        self.current_stmt = 0;
        let pre_point = Point {
            block: pre_block,
            statement: pre_stmt,
            kind: PointKind::Mid,
        };
        let then_entry = self.current_point();
        self.facts.cfg_edge.push((pre_point, then_entry));
        self.visit_expr(then_branch);
        let then_exit = self.current_mid_point();

        // Else branch.
        let else_exit = if let Some(else_br) = else_branch {
            let else_block = self.new_block();
            self.current_block = else_block;
            self.current_stmt = 0;
            let else_entry = self.current_point();
            self.facts.cfg_edge.push((pre_point, else_entry));
            self.visit_expr(else_br);
            Some(self.current_mid_point())
        } else {
            None
        };

        // Merge block.
        let merge_block = self.new_block();
        self.current_block = merge_block;
        self.current_stmt = 0;
        let merge_entry = self.current_point();
        self.facts.cfg_edge.push((then_exit, merge_entry));
        if let Some(exit) = else_exit {
            self.facts.cfg_edge.push((exit, merge_entry));
        } else {
            self.facts.cfg_edge.push((pre_point, merge_entry));
        }
    }

    fn visit_while(&mut self, condition: &Expr, body: &Expr) {
        let loop_block = self.new_block();
        self.emit_edge_to(Point {
            block: loop_block,
            statement: 0,
            kind: PointKind::Start,
        });

        self.current_block = loop_block;
        self.current_stmt = 0;
        self.visit_expr(condition);

        let body_block = self.new_block();
        self.emit_edge_to(Point {
            block: body_block,
            statement: 0,
            kind: PointKind::Start,
        });

        self.current_block = body_block;
        self.current_stmt = 0;
        self.visit_expr(body);

        // Back-edge to loop header.
        self.emit_edge_to(Point {
            block: loop_block,
            statement: 0,
            kind: PointKind::Start,
        });

        // Exit edge.
        let exit_block = self.new_block();
        self.current_block = exit_block;
        self.current_stmt = 0;
    }

    fn visit_for(&mut self, variable: &str, iterable: &Expr, body: &Expr) {
        self.visit_expr(iterable);

        let origin = self.fresh_origin();
        self.var_origins.insert(variable.to_string(), origin);

        let loop_block = self.new_block();
        self.emit_edge_to(Point {
            block: loop_block,
            statement: 0,
            kind: PointKind::Start,
        });

        self.current_block = loop_block;
        self.current_stmt = 0;
        self.visit_expr(body);

        // Back-edge.
        self.emit_edge_to(Point {
            block: loop_block,
            statement: 0,
            kind: PointKind::Start,
        });

        let exit_block = self.new_block();
        self.current_block = exit_block;
        self.current_stmt = 0;
    }

    fn visit_loop(&mut self, body: &Expr) {
        let loop_block = self.new_block();
        self.emit_edge_to(Point {
            block: loop_block,
            statement: 0,
            kind: PointKind::Start,
        });

        self.current_block = loop_block;
        self.current_stmt = 0;
        self.visit_expr(body);

        // Back-edge.
        self.emit_edge_to(Point {
            block: loop_block,
            statement: 0,
            kind: PointKind::Start,
        });

        let exit_block = self.new_block();
        self.current_block = exit_block;
        self.current_stmt = 0;
    }
}

// ── Place extraction helpers ───────────────────────────────────────────

impl FactGenerator {
    /// Extracts a `Place` from an expression, if it represents a borrowable place.
    fn extract_place(&self, expr: &Expr) -> Option<Place> {
        match expr {
            Expr::Ident { name, .. } => Some(Place::from_var(name)),
            Expr::Field { object, field, .. } => {
                let base = self.extract_place(object)?;
                Some(base.with_field(field))
            }
            Expr::Index { object, index, .. } => {
                let base = self.extract_place(object)?;
                if let Expr::Literal {
                    kind: LiteralKind::Int(idx),
                    ..
                } = index.as_ref()
                {
                    Some(base.with_index(*idx as u64))
                } else {
                    // Dynamic index — still track the base.
                    Some(base.with_index(0))
                }
            }
            Expr::Unary {
                op: UnaryOp::Deref,
                operand,
                ..
            } => {
                let base = self.extract_place(operand)?;
                Some(base.with_deref())
            }
            _ => None,
        }
    }

    /// Extracts borrow information from a value expression.
    ///
    /// Returns `(place, mutability, span)` if the expression is a borrow.
    fn extract_borrow(&self, expr: &Expr) -> Option<(Place, Mutability, Span)> {
        match expr {
            Expr::Unary {
                op: UnaryOp::Ref,
                operand,
                span,
            } => {
                let place = self.extract_place(operand)?;
                Some((place, Mutability::Shared, *span))
            }
            Expr::Unary {
                op: UnaryOp::RefMut,
                operand,
                span,
            } => {
                let place = self.extract_place(operand)?;
                Some((place, Mutability::Mutable, *span))
            }
            _ => None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ────────────────────────────────────────────────────────

    fn span(start: usize, end: usize) -> Span {
        Span::new(start, end)
    }

    fn ident_expr(name: &str, s: usize, e: usize) -> Expr {
        Expr::Ident {
            name: name.to_string(),
            span: span(s, e),
        }
    }

    fn int_lit(val: i64, s: usize, e: usize) -> Expr {
        Expr::Literal {
            kind: LiteralKind::Int(val),
            span: span(s, e),
        }
    }

    fn borrow_expr(operand: Expr, s: usize, e: usize) -> Expr {
        Expr::Unary {
            op: UnaryOp::Ref,
            operand: Box::new(operand),
            span: span(s, e),
        }
    }

    fn borrow_mut_expr(operand: Expr, s: usize, e: usize) -> Expr {
        Expr::Unary {
            op: UnaryOp::RefMut,
            operand: Box::new(operand),
            span: span(s, e),
        }
    }

    fn let_stmt(name: &str, mutable: bool, value: Expr, s: usize, e: usize) -> Stmt {
        Stmt::Let {
            mutable,
            linear: false,
            name: name.to_string(),
            ty: None,
            value: Box::new(value),
            span: span(s, e),
        }
    }

    fn expr_stmt(expr: Expr, s: usize, e: usize) -> Stmt {
        Stmt::Expr {
            expr: Box::new(expr),
            span: span(s, e),
        }
    }

    fn block_expr(stmts: Vec<Stmt>, tail: Option<Expr>, s: usize, e: usize) -> Expr {
        Expr::Block {
            stmts,
            expr: tail.map(Box::new),
            span: span(s, e),
        }
    }

    fn field_expr(object: Expr, field: &str, s: usize, e: usize) -> Expr {
        Expr::Field {
            object: Box::new(object),
            field: field.to_string(),
            span: span(s, e),
        }
    }

    // ── S9.1: Simple borrow fact generation ───────────────────────────

    #[test]
    fn s9_1_simple_borrow_generates_loan_issued() {
        // let x = 42
        // let r = &x
        let body = block_expr(
            vec![
                let_stmt("x", false, int_lit(42, 5, 7), 0, 10),
                let_stmt(
                    "r",
                    false,
                    borrow_expr(ident_expr("x", 18, 19), 17, 19),
                    12,
                    20,
                ),
            ],
            None,
            0,
            22,
        );

        let fact_gen = FactGenerator::new();
        let facts = fact_gen.generate_from_expr(&body);

        // Should have at least one loan_issued_at for the borrow of x.
        assert!(
            !facts.loan_issued_at.is_empty(),
            "expected loan_issued_at facts for &x"
        );

        // The loan should borrow place "x".
        let (_, loan, _) = &facts.loan_issued_at[0];
        assert_eq!(loan.place.base, "x");
        assert_eq!(loan.mutability, Mutability::Shared);
    }

    // ── S9.2: Mutable borrow fact generation ──────────────────────────

    #[test]
    fn s9_2_mutable_borrow_generates_mutable_loan() {
        // let mut x = 42
        // let r = &mut x
        let body = block_expr(
            vec![
                let_stmt("x", true, int_lit(42, 5, 7), 0, 10),
                let_stmt(
                    "r",
                    false,
                    borrow_mut_expr(ident_expr("x", 22, 23), 18, 23),
                    12,
                    25,
                ),
            ],
            None,
            0,
            27,
        );

        let fact_gen = FactGenerator::new();
        let facts = fact_gen.generate_from_expr(&body);

        assert!(!facts.loan_issued_at.is_empty());
        let (_, loan, _) = &facts.loan_issued_at[0];
        assert_eq!(loan.place.base, "x");
        assert_eq!(loan.mutability, Mutability::Mutable);
    }

    // ── S9.3: Reborrow chain facts ────────────────────────────────────

    #[test]
    fn s9_3_reborrow_chain_generates_multiple_loans() {
        // let x = 42
        // let r1 = &x
        // let r2 = &x
        let body = block_expr(
            vec![
                let_stmt("x", false, int_lit(42, 5, 7), 0, 10),
                let_stmt(
                    "r1",
                    false,
                    borrow_expr(ident_expr("x", 18, 19), 17, 19),
                    12,
                    20,
                ),
                let_stmt(
                    "r2",
                    false,
                    borrow_expr(ident_expr("x", 28, 29), 27, 29),
                    22,
                    30,
                ),
            ],
            None,
            0,
            32,
        );

        let fact_gen = FactGenerator::new();
        let facts = fact_gen.generate_from_expr(&body);

        // Two borrow expressions should generate two distinct loans.
        assert!(
            facts.loan_issued_at.len() >= 2,
            "expected at least 2 loans for two borrows, got {}",
            facts.loan_issued_at.len()
        );

        let ids: Vec<u32> = facts.loan_issued_at.iter().map(|(_, l, _)| l.id).collect();
        assert_ne!(ids[0], ids[1], "loan IDs should be distinct");
    }

    // ── S9.4: Function return borrow facts ────────────────────────────

    #[test]
    fn s9_4_function_with_borrow_return() {
        // fn get_ref(x: i32) -> &i32 { &x }
        let fn_def = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_gen: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            no_inline: false,
            doc_comment: None,
            annotation: None,
            name: "get_ref".to_string(),
            lifetime_params: Vec::new(),
            generic_params: Vec::new(),
            params: vec![Param {
                name: "x".to_string(),
                ty: TypeExpr::Simple {
                    name: "i32".to_string(),
                    span: span(18, 21),
                },
                span: span(12, 21),
            }],
            return_type: Some(TypeExpr::Reference {
                lifetime: None,
                mutable: false,
                inner: Box::new(TypeExpr::Simple {
                    name: "i32".to_string(),
                    span: span(30, 33),
                }),
                span: span(28, 33),
            }),
            where_clauses: Vec::new(),
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            effect_row_var: None,
            body: Box::new(block_expr(
                vec![],
                Some(borrow_expr(ident_expr("x", 38, 39), 37, 39)),
                35,
                41,
            )),
            span: span(0, 41),
        };

        let program = Program {
            items: vec![Item::FnDef(fn_def)],
            span: span(0, 41),
        };

        let fact_gen = FactGenerator::new();
        let facts = fact_gen.generate(&program);

        // The function body should produce a loan for &x.
        assert!(
            !facts.loan_issued_at.is_empty(),
            "expected loan for borrow in function body"
        );
        assert_eq!(facts.loan_issued_at[0].1.place.base, "x");
    }

    // ── S9.5: Struct field borrow facts ───────────────────────────────

    #[test]
    fn s9_5_struct_field_borrow() {
        // let p = Point { x: 1, y: 2 }
        // let r = &p.x
        let body = block_expr(
            vec![
                let_stmt(
                    "p",
                    false,
                    Expr::StructInit {
                        name: "Point".to_string(),
                        fields: vec![
                            FieldInit {
                                name: "x".to_string(),
                                value: int_lit(1, 20, 21),
                                span: span(17, 21),
                            },
                            FieldInit {
                                name: "y".to_string(),
                                value: int_lit(2, 26, 27),
                                span: span(23, 27),
                            },
                        ],
                        span: span(8, 29),
                    },
                    0,
                    30,
                ),
                let_stmt(
                    "r",
                    false,
                    borrow_expr(field_expr(ident_expr("p", 40, 41), "x", 40, 43), 39, 43),
                    35,
                    44,
                ),
            ],
            None,
            0,
            46,
        );

        let fact_gen = FactGenerator::new();
        let facts = fact_gen.generate_from_expr(&body);

        assert!(!facts.loan_issued_at.is_empty());
        let (_, loan, _) = &facts.loan_issued_at[0];
        assert_eq!(loan.place.base, "p");
        assert_eq!(loan.place.projections.len(), 1);
        assert_eq!(
            loan.place.projections[0],
            PlaceProjection::Field("x".to_string())
        );
    }

    // ── S9.6: CFG edge generation ─────────────────────────────────────

    #[test]
    fn s9_6_cfg_edges_for_sequential_stmts() {
        // let x = 1
        // let y = 2
        // let z = 3
        let body = block_expr(
            vec![
                let_stmt("x", false, int_lit(1, 5, 6), 0, 10),
                let_stmt("y", false, int_lit(2, 15, 16), 12, 20),
                let_stmt("z", false, int_lit(3, 25, 26), 22, 30),
            ],
            None,
            0,
            32,
        );

        let fact_gen = FactGenerator::new();
        let facts = fact_gen.generate_from_expr(&body);

        // Sequential statements should produce CFG edges connecting them.
        assert!(
            facts.cfg_edge.len() >= 3,
            "expected at least 3 CFG edges for 3 sequential stmts, got {}",
            facts.cfg_edge.len()
        );
    }

    // ── S9.7: If branch CFG edges ─────────────────────────────────────

    #[test]
    fn s9_7_if_branch_generates_forked_cfg() {
        // let x = 1
        // if x > 0 { let a = &x } else { let b = &x }
        let body = block_expr(
            vec![
                let_stmt("x", false, int_lit(1, 5, 6), 0, 10),
                expr_stmt(
                    Expr::If {
                        condition: Box::new(Expr::Binary {
                            left: Box::new(ident_expr("x", 15, 16)),
                            op: BinOp::Gt,
                            right: Box::new(int_lit(0, 19, 20)),
                            span: span(15, 20),
                        }),
                        then_branch: Box::new(block_expr(
                            vec![let_stmt(
                                "a",
                                false,
                                borrow_expr(ident_expr("x", 30, 31), 29, 31),
                                24,
                                32,
                            )],
                            None,
                            22,
                            34,
                        )),
                        else_branch: Some(Box::new(block_expr(
                            vec![let_stmt(
                                "b",
                                false,
                                borrow_expr(ident_expr("x", 45, 46), 44, 46),
                                40,
                                47,
                            )],
                            None,
                            38,
                            49,
                        ))),
                        span: span(12, 50),
                    },
                    12,
                    50,
                ),
            ],
            None,
            0,
            52,
        );

        let fact_gen = FactGenerator::new();
        let facts = fact_gen.generate_from_expr(&body);

        // Should have CFG edges for: sequential stmts + if branching.
        assert!(
            facts.cfg_edge.len() >= 5,
            "expected at least 5 CFG edges for if/else branch, got {}",
            facts.cfg_edge.len()
        );
    }

    // ── S9.8: Loan invalidation on assignment ─────────────────────────

    #[test]
    fn s9_8_assignment_invalidates_loan() {
        // let mut x = 42
        // let r = &x
        // x = 100       <-- invalidates the loan on x
        let body = block_expr(
            vec![
                let_stmt("x", true, int_lit(42, 5, 7), 0, 10),
                let_stmt(
                    "r",
                    false,
                    borrow_expr(ident_expr("x", 18, 19), 17, 19),
                    12,
                    20,
                ),
                expr_stmt(
                    Expr::Assign {
                        target: Box::new(ident_expr("x", 22, 23)),
                        op: AssignOp::Assign,
                        value: Box::new(int_lit(100, 26, 29)),
                        span: span(22, 29),
                    },
                    22,
                    30,
                ),
            ],
            None,
            0,
            32,
        );

        let fact_gen = FactGenerator::new();
        let facts = fact_gen.generate_from_expr(&body);

        assert!(
            !facts.loan_invalidated_at.is_empty(),
            "expected loan_invalidated_at for assignment to x"
        );
        assert_eq!(facts.loan_invalidated_at[0].0.place.base, "x");
    }

    // ── S9.9: Origin liveness tracking ────────────────────────────────

    #[test]
    fn s9_9_origin_live_on_entry_for_used_ref() {
        // let x = 42
        // let r = &x
        // use r (ident "r")
        let body = block_expr(
            vec![
                let_stmt("x", false, int_lit(42, 5, 7), 0, 10),
                let_stmt(
                    "r",
                    false,
                    borrow_expr(ident_expr("x", 18, 19), 17, 19),
                    12,
                    20,
                ),
                expr_stmt(ident_expr("r", 22, 23), 22, 24),
            ],
            None,
            0,
            26,
        );

        let fact_gen = FactGenerator::new();
        let facts = fact_gen.generate_from_expr(&body);

        // The origin for "r" should be live at the use point.
        assert!(
            !facts.origin_live_on_entry.is_empty(),
            "expected origin_live_on_entry for variable use"
        );
    }

    // ── S9.10: While loop CFG edges (back-edges) ─────────────────────

    #[test]
    fn s9_10_while_loop_generates_back_edge() {
        // while cond { let r = &x }
        let body = block_expr(
            vec![
                let_stmt("x", false, int_lit(1, 5, 6), 0, 10),
                expr_stmt(
                    Expr::While {
                        label: None,
                        condition: Box::new(Expr::Literal {
                            kind: LiteralKind::Bool(true),
                            span: span(18, 22),
                        }),
                        body: Box::new(block_expr(
                            vec![let_stmt(
                                "r",
                                false,
                                borrow_expr(ident_expr("x", 32, 33), 31, 33),
                                26,
                                34,
                            )],
                            None,
                            24,
                            36,
                        )),
                        span: span(12, 38),
                    },
                    12,
                    38,
                ),
            ],
            None,
            0,
            40,
        );

        let fact_gen = FactGenerator::new();
        let facts = fact_gen.generate_from_expr(&body);

        // A while loop should produce a back-edge from the loop body to the header.
        // Check that there's at least one back-edge (edge to a lower block).
        let has_back_edge = facts.cfg_edge.iter().any(|(from, to)| {
            to.block < from.block || (to.block == from.block && to.statement < from.statement)
        });
        assert!(has_back_edge, "expected a back-edge in CFG for while loop");
    }
}
