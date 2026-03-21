//! Abstract Syntax Tree node definitions for Fajar Lang.
//!
//! Defines [`Expr`], [`Stmt`], [`Item`], [`TypeExpr`], [`Pattern`], and [`Program`].
//! Every AST node carries a [`Span`] for error reporting.
//!
//! # Architecture
//!
//! ```text
//! Program
//!   └── Vec<Item>          (top-level declarations)
//!         ├── FnDef        (function definitions)
//!         ├── StructDef    (struct definitions)
//!         ├── EnumDef      (enum definitions)
//!         ├── ImplBlock    (impl blocks)
//!         ├── TraitDef     (trait definitions)
//!         ├── ConstDef     (const definitions)
//!         ├── UseDecl      (use imports)
//!         ├── ModDecl      (module declarations)
//!         └── ExternFn     (foreign function declarations)
//! ```

use crate::lexer::token::Span;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Program (root node)
// ═══════════════════════════════════════════════════════════════════════

/// The root AST node representing a complete Fajar Lang program.
///
/// A program is a sequence of top-level items (functions, structs, enums, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    /// Top-level items in source order.
    pub items: Vec<Item>,
    /// Span covering the entire program.
    pub span: Span,
}

// ═══════════════════════════════════════════════════════════════════════
// Items (top-level declarations)
// ═══════════════════════════════════════════════════════════════════════

/// A top-level declaration in a Fajar Lang program.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Function definition: `fn name(params) -> RetType { body }`
    FnDef(FnDef),
    /// Struct definition: `struct Name { fields }`
    StructDef(StructDef),
    /// Enum definition: `enum Name { variants }`
    EnumDef(EnumDef),
    /// Union definition: `union Name { fields }`
    UnionDef(UnionDef),
    /// Impl block: `impl [Trait for] Type { methods }`
    ImplBlock(ImplBlock),
    /// Trait definition: `trait Name { methods }`
    TraitDef(TraitDef),
    /// Constant definition: `const NAME: Type = value`
    ConstDef(ConstDef),
    /// Static variable definition: `static mut NAME: Type = value`
    StaticDef(StaticDef),
    /// Service definition: `service name { handler functions }`
    ServiceDef(ServiceDef),
    /// Use declaration: `use path::to::item`
    UseDecl(UseDecl),
    /// Module declaration: `mod name { items }`
    ModDecl(ModDecl),
    /// Extern function declaration: `@ffi("C") extern fn name(params) -> ret`
    ExternFn(ExternFn),
    /// Type alias: `type Name = TypeExpr`
    TypeAlias(TypeAlias),
    /// Global assembly: `global_asm!(".section .text\n...")`
    GlobalAsm(GlobalAsm),
    /// A statement at the top level (for REPL / scripts).
    Stmt(Stmt),
}

/// Global assembly block.
///
/// ```text
/// global_asm!(".section .text\njmp _start")
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalAsm {
    /// Assembly template string.
    pub template: String,
    /// Source span.
    pub span: Span,
}

/// Function definition.
///
/// ```text
/// @annotation fn name<T>(param: Type) -> RetType { body }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FnDef {
    /// Whether the function is declared `pub`.
    pub is_pub: bool,
    /// Whether the function is declared `const`.
    pub is_const: bool,
    /// Whether the function is declared `async`.
    pub is_async: bool,
    /// Whether this function is a test (`@test`).
    pub is_test: bool,
    /// Whether this test should expect a panic (`@should_panic`).
    pub should_panic: bool,
    /// Whether this test is ignored by default (`@ignore`).
    pub is_ignored: bool,
    /// Doc comment lines (from `///` comments preceding the function).
    pub doc_comment: Option<String>,
    /// Optional annotation (e.g., `@kernel`, `@device`).
    pub annotation: Option<Annotation>,
    /// Function name.
    pub name: String,
    /// Lifetime parameters (e.g., `'a`, `'b`).
    pub lifetime_params: Vec<LifetimeParam>,
    /// Generic type parameters.
    pub generic_params: Vec<GenericParam>,
    /// Function parameters.
    pub params: Vec<Param>,
    /// Return type (None = void).
    pub return_type: Option<TypeExpr>,
    /// Where clause bounds: `where T: Display, U: Ord`.
    pub where_clauses: Vec<WhereClause>,
    /// Function body.
    pub body: Box<Expr>,
    /// Source span.
    pub span: Span,
}

/// A single where clause: `T: Bound1 + Bound2`.
#[derive(Debug, Clone, PartialEq)]
pub struct WhereClause {
    /// Type parameter name.
    pub name: String,
    /// Trait bounds.
    pub bounds: Vec<TraitBound>,
    /// Source span.
    pub span: Span,
}

/// Extern function declaration (no body).
///
/// ```text
/// @ffi("C") extern fn name(params) -> RetType
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExternFn {
    /// Optional annotation (typically `@ffi`).
    pub annotation: Option<Annotation>,
    /// ABI string (e.g., `"C"`). None means default C ABI.
    pub abi: Option<String>,
    /// Function name.
    pub name: String,
    /// Function parameters.
    pub params: Vec<Param>,
    /// Return type (None = void).
    pub return_type: Option<TypeExpr>,
    /// Source span.
    pub span: Span,
}

/// Type alias declaration.
///
/// ```text
/// type Meters = f64
/// type Matrix = Tensor<f64>
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAlias {
    /// Whether the type alias is declared `pub`.
    pub is_pub: bool,
    /// Alias name.
    pub name: String,
    /// The target type expression.
    pub ty: TypeExpr,
    /// Source span.
    pub span: Span,
}

/// A function parameter: `name: Type`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub ty: TypeExpr,
    /// Source span.
    pub span: Span,
}

/// A generic type parameter: `T: Bound1 + Bound2`.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericParam {
    /// Type parameter name.
    pub name: String,
    /// Trait bounds.
    pub bounds: Vec<TraitBound>,
    /// Source span.
    pub span: Span,
}

/// A lifetime parameter in a generic list: `'a`, `'static`.
#[derive(Debug, Clone, PartialEq)]
pub struct LifetimeParam {
    /// Lifetime name (without the leading `'`).
    pub name: String,
    /// Source span.
    pub span: Span,
}

/// A trait bound: `TraitName<TypeArgs>`.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitBound {
    /// Trait name.
    pub name: String,
    /// Type arguments.
    pub type_args: Vec<TypeExpr>,
    /// Source span.
    pub span: Span,
}

/// Struct definition.
///
/// ```text
/// struct Point<T> { x: T, y: T }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    /// Whether the struct is declared `pub`.
    pub is_pub: bool,
    /// Doc comment lines (from `///` comments preceding the struct).
    pub doc_comment: Option<String>,
    /// Optional annotation.
    pub annotation: Option<Annotation>,
    /// Struct name.
    pub name: String,
    /// Lifetime parameters (e.g., `'a`, `'b`).
    pub lifetime_params: Vec<LifetimeParam>,
    /// Generic type parameters.
    pub generic_params: Vec<GenericParam>,
    /// Struct fields.
    pub fields: Vec<Field>,
    /// Source span.
    pub span: Span,
}

/// A struct field: `name: Type`.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Field name.
    pub name: String,
    /// Field type.
    pub ty: TypeExpr,
    /// Source span.
    pub span: Span,
}

/// Enum definition.
///
/// ```text
/// enum Shape { Circle(f64), Rect(f64, f64) }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    /// Whether the enum is declared `pub`.
    pub is_pub: bool,
    /// Doc comment lines (from `///` comments preceding the enum).
    pub doc_comment: Option<String>,
    /// Optional annotation.
    pub annotation: Option<Annotation>,
    /// Enum name.
    pub name: String,
    /// Lifetime parameters (e.g., `'a`, `'b`).
    pub lifetime_params: Vec<LifetimeParam>,
    /// Generic type parameters.
    pub generic_params: Vec<GenericParam>,
    /// Enum variants.
    pub variants: Vec<Variant>,
    /// Source span.
    pub span: Span,
}

/// Union definition — all fields share the same memory location.
///
/// ```text
/// union Register { as_u32: u32, as_bytes: [u8; 4] }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct UnionDef {
    /// Whether the union is declared `pub`.
    pub is_pub: bool,
    /// Doc comment lines (from `///` comments preceding the union).
    pub doc_comment: Option<String>,
    /// Optional annotation (e.g., `#[repr(C)]`).
    pub annotation: Option<Annotation>,
    /// Union name.
    pub name: String,
    /// Union fields (all overlap at offset 0).
    pub fields: Vec<Field>,
    /// Source span.
    pub span: Span,
}

/// An enum variant: `Name(Type1, Type2)` or `Name`.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    /// Variant name.
    pub name: String,
    /// Variant data types (empty for unit variants).
    pub fields: Vec<TypeExpr>,
    /// Source span.
    pub span: Span,
}

/// Impl block.
///
/// ```text
/// impl Trait for Type { fn method() { ... } }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ImplBlock {
    /// Doc comment lines (from `///` comments preceding the impl block).
    pub doc_comment: Option<String>,
    /// Lifetime parameters (e.g., `'a`, `'b`).
    pub lifetime_params: Vec<LifetimeParam>,
    /// Generic type parameters.
    pub generic_params: Vec<GenericParam>,
    /// Trait being implemented (None for inherent impls).
    pub trait_name: Option<String>,
    /// Type being implemented.
    pub target_type: String,
    /// Methods defined in this block.
    pub methods: Vec<FnDef>,
    /// Source span.
    pub span: Span,
}

/// Trait definition.
///
/// ```text
/// trait Summary { fn summarize(&self) -> str; }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TraitDef {
    /// Whether the trait is declared `pub`.
    pub is_pub: bool,
    /// Doc comment lines (from `///` comments preceding the trait).
    pub doc_comment: Option<String>,
    /// Trait name.
    pub name: String,
    /// Lifetime parameters (e.g., `'a`, `'b`).
    pub lifetime_params: Vec<LifetimeParam>,
    /// Generic type parameters.
    pub generic_params: Vec<GenericParam>,
    /// Trait methods (body is optional for default impls).
    pub methods: Vec<FnDef>,
    /// Source span.
    pub span: Span,
}

/// Constant definition: `const NAME: Type = value`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstDef {
    /// Whether the const is declared `pub`.
    pub is_pub: bool,
    /// Doc comment lines (from `///` comments preceding the const).
    pub doc_comment: Option<String>,
    /// Optional annotation.
    pub annotation: Option<Annotation>,
    /// Constant name.
    pub name: String,
    /// Constant type.
    pub ty: TypeExpr,
    /// Constant value.
    pub value: Box<Expr>,
    /// Source span.
    pub span: Span,
}

/// A static mutable variable definition: `static mut NAME: TYPE = VALUE`.
///
/// Static variables are global mutable state, accessible from any function.
/// In bare-metal mode, they are placed in the `.data` or `.bss` section.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticDef {
    /// Whether the static is declared `pub`.
    pub is_pub: bool,
    /// Whether the static is mutable (`static mut`).
    pub is_mut: bool,
    /// Doc comment lines.
    pub doc_comment: Option<String>,
    /// Optional annotation.
    pub annotation: Option<Annotation>,
    /// Variable name.
    pub name: String,
    /// Variable type.
    pub ty: TypeExpr,
    /// Initial value.
    pub value: Box<Expr>,
    /// Source span.
    pub span: Span,
}

/// Service definition: `service name { fn handlers... }`
///
/// Syntactic sugar for a process with an IPC message loop.
/// The compiler generates a `main()` that loops on `ipc_recv` and
/// dispatches to the handler functions.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceDef {
    /// Service name (e.g., "vfs", "net", "shell").
    pub name: String,
    /// Optional annotation (e.g., @safe, @device("net")).
    pub annotation: Option<Annotation>,
    /// Handler functions defined inside the service block.
    pub handlers: Vec<FnDef>,
    /// Source span.
    pub span: Span,
}

/// Use declaration: `use std::io::println`.
#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    /// Path segments (e.g., `["std", "io", "println"]`).
    pub path: Vec<String>,
    /// Import kind (specific, glob, or group).
    pub kind: UseKind,
    /// Source span.
    pub span: Span,
}

/// The kind of use import.
#[derive(Debug, Clone, PartialEq)]
pub enum UseKind {
    /// Import a specific item: `use std::io::println`
    Simple,
    /// Glob import: `use std::io::*`
    Glob,
    /// Group import: `use std::io::{println, read_line}`
    Group(Vec<String>),
}

/// Module declaration: `mod name { items }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModDecl {
    /// Module name.
    pub name: String,
    /// Module body (None for external module file references).
    pub body: Option<Vec<Item>>,
    /// Source span.
    pub span: Span,
}

/// A context annotation: `@kernel`, `@device`, `@safe`, `@unsafe`, `@ffi`.
///
/// Also used for `#[repr(C)]`, `#[repr(packed)]` attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    /// Annotation name (without `@` or `#[`).
    pub name: String,
    /// Optional parameter (e.g., `"C"` in `#[repr(C)]`).
    pub param: Option<String>,
    /// Source span.
    pub span: Span,
}

// ═══════════════════════════════════════════════════════════════════════
// Statements
// ═══════════════════════════════════════════════════════════════════════

/// A statement in Fajar Lang.
///
/// Statements do not produce values (expressions with `;` become statements).
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Let binding: `let [mut] name [: Type] = value`
    Let {
        /// Whether the binding is mutable.
        mutable: bool,
        /// Variable name.
        name: String,
        /// Optional type annotation.
        ty: Option<TypeExpr>,
        /// Initializer expression.
        value: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Const binding at statement level: `const NAME: Type = value`
    Const {
        /// Constant name.
        name: String,
        /// Constant type.
        ty: TypeExpr,
        /// Initializer expression.
        value: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Expression statement: `expr;` (value discarded).
    Expr {
        /// The expression.
        expr: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Return statement: `return [expr]`.
    Return {
        /// Optional return value.
        value: Option<Box<Expr>>,
        /// Source span.
        span: Span,
    },

    /// Break statement: `break ['label] [expr]`.
    Break {
        /// Optional label for multi-level break (e.g., `break 'outer`).
        label: Option<String>,
        /// Optional break value (for loop expressions).
        value: Option<Box<Expr>>,
        /// Source span.
        span: Span,
    },

    /// Continue statement: `continue ['label]`.
    Continue {
        /// Optional label for multi-level continue (e.g., `continue 'outer`).
        label: Option<String>,
        /// Source span.
        span: Span,
    },

    /// A top-level item used as a statement (e.g., function def inside a block).
    Item(Box<Item>),
}

// ═══════════════════════════════════════════════════════════════════════
// Expressions
// ═══════════════════════════════════════════════════════════════════════

/// An expression in Fajar Lang.
///
/// Everything is an expression in Fajar Lang — `if`, `match`, `while`, and blocks
/// all produce values.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal value: `42`, `3.14`, `"hello"`, `true`, `null`.
    Literal {
        /// The kind of literal.
        kind: LiteralKind,
        /// Source span.
        span: Span,
    },

    /// An identifier: `x`, `my_var`, `MyStruct`.
    Ident {
        /// The identifier name.
        name: String,
        /// Source span.
        span: Span,
    },

    /// Binary operation: `a + b`, `x == y`, `a && b`.
    Binary {
        /// Left operand.
        left: Box<Expr>,
        /// Operator.
        op: BinOp,
        /// Right operand.
        right: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Unary operation: `!x`, `-y`, `~bits`, `&val`, `&mut val`.
    Unary {
        /// Operator.
        op: UnaryOp,
        /// Operand.
        operand: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Function call: `f(a, b)`.
    Call {
        /// The function being called.
        callee: Box<Expr>,
        /// Arguments.
        args: Vec<CallArg>,
        /// Source span.
        span: Span,
    },

    /// Method call: `obj.method(a, b)`.
    MethodCall {
        /// The receiver object.
        receiver: Box<Expr>,
        /// Method name.
        method: String,
        /// Arguments.
        args: Vec<CallArg>,
        /// Source span.
        span: Span,
    },

    /// Await expression: `expr.await`.
    Await {
        /// The future expression.
        expr: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Async block expression: `async { body }`.
    AsyncBlock {
        /// The block body.
        body: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Field access: `obj.field`.
    Field {
        /// The object.
        object: Box<Expr>,
        /// Field name.
        field: String,
        /// Source span.
        span: Span,
    },

    /// Index access: `arr[i]`.
    Index {
        /// The object being indexed.
        object: Box<Expr>,
        /// Index expression.
        index: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Block expression: `{ stmt; stmt; expr }`.
    Block {
        /// Statements in the block.
        stmts: Vec<Stmt>,
        /// Final expression (block's value, if any).
        expr: Option<Box<Expr>>,
        /// Source span.
        span: Span,
    },

    /// If expression: `if cond { then } else { else_ }`.
    If {
        /// Condition.
        condition: Box<Expr>,
        /// Then branch (block expression).
        then_branch: Box<Expr>,
        /// Optional else branch (block or another if).
        else_branch: Option<Box<Expr>>,
        /// Source span.
        span: Span,
    },

    /// Match expression: `match subject { arms }`.
    Match {
        /// The value being matched.
        subject: Box<Expr>,
        /// Match arms.
        arms: Vec<MatchArm>,
        /// Source span.
        span: Span,
    },

    /// While loop: `while cond { body }`.
    /// While loop: `['label:] while condition { body }`.
    While {
        /// Optional label for break/continue (e.g., `'outer: while ...`).
        label: Option<String>,
        /// Loop condition.
        condition: Box<Expr>,
        /// Loop body.
        body: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// For loop: `['label:] for var in iter { body }`.
    For {
        /// Optional label.
        label: Option<String>,
        /// Loop variable name.
        variable: String,
        /// Iterator expression.
        iterable: Box<Expr>,
        /// Loop body.
        body: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Infinite loop: `['label:] loop { body }`.
    Loop {
        /// Optional label.
        label: Option<String>,
        /// Loop body.
        body: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Assignment: `target = value`, `target += value`, etc.
    Assign {
        /// Assignment target.
        target: Box<Expr>,
        /// Assignment operator.
        op: AssignOp,
        /// Value being assigned.
        value: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Pipeline: `x |> f` (desugars to `f(x)`).
    Pipe {
        /// Left operand (argument).
        left: Box<Expr>,
        /// Right operand (function).
        right: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Array literal: `[1, 2, 3]`.
    Array {
        /// Array elements.
        elements: Vec<Expr>,
        /// Source span.
        span: Span,
    },

    /// Array repeat: `[expr; count]`.
    ArrayRepeat {
        /// The value to repeat.
        value: Box<Expr>,
        /// Number of repetitions.
        count: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Tuple literal: `(1, "hello", true)`.
    Tuple {
        /// Tuple elements.
        elements: Vec<Expr>,
        /// Source span.
        span: Span,
    },

    /// Range expression: `start..end` or `start..=end`.
    Range {
        /// Start of range.
        start: Option<Box<Expr>>,
        /// End of range.
        end: Option<Box<Expr>>,
        /// Whether the range is inclusive (`..=`).
        inclusive: bool,
        /// Source span.
        span: Span,
    },

    /// Type cast: `expr as Type`.
    Cast {
        /// Expression to cast.
        expr: Box<Expr>,
        /// Target type.
        ty: TypeExpr,
        /// Source span.
        span: Span,
    },

    /// Try / error propagation: `expr?`.
    Try {
        /// Expression that may fail.
        expr: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Closure expression: `|params| body`.
    Closure {
        /// Closure parameters.
        params: Vec<ClosureParam>,
        /// Optional return type.
        return_type: Option<Box<TypeExpr>>,
        /// Closure body.
        body: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Struct instantiation: `Point { x: 1, y: 2 }`.
    StructInit {
        /// Struct name.
        name: String,
        /// Field initializers.
        fields: Vec<FieldInit>,
        /// Source span.
        span: Span,
    },

    /// Grouped expression: `(expr)` — parenthesized for precedence.
    Grouped {
        /// Inner expression.
        expr: Box<Expr>,
        /// Source span.
        span: Span,
    },

    /// Path expression: `std::io::println`.
    Path {
        /// Path segments.
        segments: Vec<String>,
        /// Source span.
        span: Span,
    },

    /// Inline assembly: `asm!("nop")` or `asm!("mov {}, {}", out(reg) x, in(reg) y)`.
    InlineAsm {
        /// Assembly template string.
        template: String,
        /// Operands (direction, constraint, expr).
        operands: Vec<AsmOperand>,
        /// Assembly options (`options(nomem, nostack)`).
        options: Vec<AsmOption>,
        /// Clobber ABI specification (`clobber_abi("C")`).
        clobber_abi: Option<String>,
        /// Source span.
        span: Span,
    },

    /// An f-string expression: `f"Hello {name}, {x + 1}"`.
    FString {
        /// Parts: literal text or parsed expressions.
        parts: Vec<FStringExprPart>,
        /// Source span.
        span: Span,
    },
}

/// A part of an f-string expression (post-parsing).
#[derive(Debug, Clone, PartialEq)]
pub enum FStringExprPart {
    /// Literal text segment.
    Literal(String),
    /// A parsed expression.
    Expr(Box<Expr>),
}

/// Direction and constraint for an inline assembly operand.
#[derive(Debug, Clone, PartialEq)]
pub enum AsmOperand {
    /// `in(reg) expr` — input operand.
    In { constraint: String, expr: Box<Expr> },
    /// `out(reg) expr` — output operand.
    Out { constraint: String, expr: Box<Expr> },
    /// `inout(reg) expr` — input+output operand.
    InOut { constraint: String, expr: Box<Expr> },
    /// `lateout(reg) expr` — output clobbered after all inputs consumed.
    LateOut { constraint: String, expr: Box<Expr> },
    /// `const expr` — compile-time constant.
    Const { expr: Box<Expr> },
    /// `sym name` — symbol reference (function pointer address).
    Sym { name: String },
}

/// Inline assembly option flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmOption {
    /// `nomem` — no memory reads or writes.
    Nomem,
    /// `nostack` — does not use the stack.
    Nostack,
    /// `readonly` — only reads memory, does not write.
    Readonly,
    /// `preserves_flags` — does not modify condition flags.
    PreservesFlags,
    /// `pure` — no side effects (implies nomem + nostack).
    Pure,
    /// `att_syntax` — use AT&T syntax instead of Intel.
    AttSyntax,
}

impl Expr {
    /// Returns the span of this expression.
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal { span, .. }
            | Expr::Ident { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Call { span, .. }
            | Expr::MethodCall { span, .. }
            | Expr::Await { span, .. }
            | Expr::AsyncBlock { span, .. }
            | Expr::Field { span, .. }
            | Expr::Index { span, .. }
            | Expr::Block { span, .. }
            | Expr::If { span, .. }
            | Expr::Match { span, .. }
            | Expr::While { span, .. }
            | Expr::For { span, .. }
            | Expr::Loop { span, .. }
            | Expr::Assign { span, .. }
            | Expr::Pipe { span, .. }
            | Expr::Array { span, .. }
            | Expr::ArrayRepeat { span, .. }
            | Expr::Tuple { span, .. }
            | Expr::Range { span, .. }
            | Expr::Cast { span, .. }
            | Expr::Try { span, .. }
            | Expr::Closure { span, .. }
            | Expr::StructInit { span, .. }
            | Expr::Grouped { span, .. }
            | Expr::Path { span, .. }
            | Expr::InlineAsm { span, .. }
            | Expr::FString { span, .. } => *span,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Expression helpers
// ═══════════════════════════════════════════════════════════════════════

/// A literal value in Fajar Lang source code.
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralKind {
    /// Integer literal (e.g., `42`, `0xFF`).
    Int(i64),
    /// Float literal (e.g., `3.14`, `1.0e-4`).
    Float(f64),
    /// String literal (e.g., `"hello"`).
    String(String),
    /// Raw string literal (e.g., `r"raw \n"`).
    RawString(String),
    /// Character literal (e.g., `'a'`).
    Char(char),
    /// Boolean literal (`true` or `false`).
    Bool(bool),
    /// Null literal.
    Null,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    // Arithmetic
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
    /// `**`
    Pow,
    /// `@` (matrix multiply)
    MatMul,

    // Comparison
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Le,
    /// `>=`
    Ge,

    // Logical
    /// `&&`
    And,
    /// `||`
    Or,

    // Bitwise
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Rem => write!(f, "%"),
            BinOp::Pow => write!(f, "**"),
            BinOp::MatMul => write!(f, "@"),
            BinOp::Eq => write!(f, "=="),
            BinOp::Ne => write!(f, "!="),
            BinOp::Lt => write!(f, "<"),
            BinOp::Gt => write!(f, ">"),
            BinOp::Le => write!(f, "<="),
            BinOp::Ge => write!(f, ">="),
            BinOp::And => write!(f, "&&"),
            BinOp::Or => write!(f, "||"),
            BinOp::BitAnd => write!(f, "&"),
            BinOp::BitOr => write!(f, "|"),
            BinOp::BitXor => write!(f, "^"),
            BinOp::Shl => write!(f, "<<"),
            BinOp::Shr => write!(f, ">>"),
        }
    }
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-` (negation)
    Neg,
    /// `!` (logical not)
    Not,
    /// `~` (bitwise not)
    BitNot,
    /// `&` (immutable reference)
    Ref,
    /// `&mut` (mutable reference)
    RefMut,
    /// `*` (dereference)
    Deref,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "-"),
            UnaryOp::Not => write!(f, "!"),
            UnaryOp::BitNot => write!(f, "~"),
            UnaryOp::Ref => write!(f, "&"),
            UnaryOp::RefMut => write!(f, "&mut"),
            UnaryOp::Deref => write!(f, "*"),
        }
    }
}

/// Assignment operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    /// `=`
    Assign,
    /// `+=`
    AddAssign,
    /// `-=`
    SubAssign,
    /// `*=`
    MulAssign,
    /// `/=`
    DivAssign,
    /// `%=`
    RemAssign,
    /// `&=`
    BitAndAssign,
    /// `|=`
    BitOrAssign,
    /// `^=`
    BitXorAssign,
    /// `<<=`
    ShlAssign,
    /// `>>=`
    ShrAssign,
}

impl fmt::Display for AssignOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssignOp::Assign => write!(f, "="),
            AssignOp::AddAssign => write!(f, "+="),
            AssignOp::SubAssign => write!(f, "-="),
            AssignOp::MulAssign => write!(f, "*="),
            AssignOp::DivAssign => write!(f, "/="),
            AssignOp::RemAssign => write!(f, "%="),
            AssignOp::BitAndAssign => write!(f, "&="),
            AssignOp::BitOrAssign => write!(f, "|="),
            AssignOp::BitXorAssign => write!(f, "^="),
            AssignOp::ShlAssign => write!(f, "<<="),
            AssignOp::ShrAssign => write!(f, ">>="),
        }
    }
}

/// A function call argument, optionally named.
///
/// ```text
/// add(a: 1, b: 2)   // named
/// add(1, 2)          // positional
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CallArg {
    /// Optional argument name for named arguments.
    pub name: Option<String>,
    /// Argument value.
    pub value: Expr,
    /// Source span.
    pub span: Span,
}

/// A match arm: `pattern [if guard] => body`.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// The pattern to match against.
    pub pattern: Pattern,
    /// Optional guard condition.
    pub guard: Option<Box<Expr>>,
    /// Body expression.
    pub body: Box<Expr>,
    /// Source span.
    pub span: Span,
}

/// A closure parameter: `name [: Type]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureParam {
    /// Parameter name.
    pub name: String,
    /// Optional type annotation.
    pub ty: Option<TypeExpr>,
    /// Source span.
    pub span: Span,
}

/// A field initializer in a struct literal: `field: value`.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInit {
    /// Field name.
    pub name: String,
    /// Field value.
    pub value: Expr,
    /// Source span.
    pub span: Span,
}

// ═══════════════════════════════════════════════════════════════════════
// Type expressions
// ═══════════════════════════════════════════════════════════════════════

/// A type expression in Fajar Lang.
///
/// Used in variable declarations, function signatures, struct fields, etc.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// A simple named type: `i32`, `bool`, `String`, `MyStruct`.
    Simple {
        /// Type name.
        name: String,
        /// Source span.
        span: Span,
    },

    /// A generic type: `Vec<T>`, `HashMap<K, V>`.
    Generic {
        /// Base type name.
        name: String,
        /// Type arguments.
        args: Vec<TypeExpr>,
        /// Source span.
        span: Span,
    },

    /// A tensor type: `Tensor<f32>[3, 4]` or `Tensor<f64>[*, 10]`.
    Tensor {
        /// Element type.
        element_type: Box<TypeExpr>,
        /// Dimensions (None = dynamic `*`).
        dims: Vec<Option<u64>>,
        /// Source span.
        span: Span,
    },

    /// A pointer type: `*const T` or `*mut T`.
    Pointer {
        /// Whether the pointer is mutable.
        mutable: bool,
        /// Pointee type.
        inner: Box<TypeExpr>,
        /// Source span.
        span: Span,
    },

    /// A reference type: `&T`, `&mut T`, `&'a T`, or `&'a mut T`.
    Reference {
        /// Optional lifetime annotation (e.g., `"a"` for `'a`).
        lifetime: Option<String>,
        /// Whether the reference is mutable.
        mutable: bool,
        /// Referenced type.
        inner: Box<TypeExpr>,
        /// Source span.
        span: Span,
    },

    /// A tuple type: `(i32, String, bool)`.
    Tuple {
        /// Element types.
        elements: Vec<TypeExpr>,
        /// Source span.
        span: Span,
    },

    /// An array type: `[T; N]`.
    Array {
        /// Element type.
        element: Box<TypeExpr>,
        /// Array length.
        size: u64,
        /// Source span.
        span: Span,
    },

    /// A slice type: `[T]`.
    Slice {
        /// Element type.
        element: Box<TypeExpr>,
        /// Source span.
        span: Span,
    },

    /// A function type: `fn(i32, i32) -> i32`.
    Fn {
        /// Parameter types.
        params: Vec<TypeExpr>,
        /// Return type.
        return_type: Box<TypeExpr>,
        /// Source span.
        span: Span,
    },

    /// A path type: `std::io::Error`.
    Path {
        /// Path segments.
        segments: Vec<String>,
        /// Source span.
        span: Span,
    },

    /// A trait object type: `dyn Trait`.
    DynTrait {
        /// Trait name.
        trait_name: String,
        /// Source span.
        span: Span,
    },
}

impl TypeExpr {
    /// Returns the span of this type expression.
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Simple { span, .. }
            | TypeExpr::Generic { span, .. }
            | TypeExpr::Tensor { span, .. }
            | TypeExpr::Pointer { span, .. }
            | TypeExpr::Reference { span, .. }
            | TypeExpr::Tuple { span, .. }
            | TypeExpr::Array { span, .. }
            | TypeExpr::Slice { span, .. }
            | TypeExpr::Fn { span, .. }
            | TypeExpr::Path { span, .. }
            | TypeExpr::DynTrait { span, .. } => *span,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Patterns (for match expressions)
// ═══════════════════════════════════════════════════════════════════════

/// A pattern used in `match` arms and destructuring.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// Literal pattern: `42`, `"hello"`, `true`.
    Literal {
        /// The literal value.
        kind: LiteralKind,
        /// Source span.
        span: Span,
    },

    /// Identifier pattern (binds value to name): `x`, `value`.
    Ident {
        /// The variable name to bind.
        name: String,
        /// Source span.
        span: Span,
    },

    /// Wildcard pattern: `_`.
    Wildcard {
        /// Source span.
        span: Span,
    },

    /// Tuple pattern: `(a, b, c)`.
    Tuple {
        /// Sub-patterns.
        elements: Vec<Pattern>,
        /// Source span.
        span: Span,
    },

    /// Struct pattern: `Point { x, y: 0 }`.
    Struct {
        /// Struct name.
        name: String,
        /// Field patterns.
        fields: Vec<FieldPattern>,
        /// Source span.
        span: Span,
    },

    /// Enum variant pattern: `Shape::Circle(r)`.
    Enum {
        /// Enum name.
        enum_name: String,
        /// Variant name.
        variant: String,
        /// Inner patterns.
        fields: Vec<Pattern>,
        /// Source span.
        span: Span,
    },

    /// Range pattern: `1..=10`.
    Range {
        /// Start of range.
        start: Box<Expr>,
        /// End of range.
        end: Box<Expr>,
        /// Whether inclusive.
        inclusive: bool,
        /// Source span.
        span: Span,
    },

    /// Or pattern: `0 | 1 | 2`.
    Or {
        /// Alternative patterns.
        patterns: Vec<Pattern>,
        /// Source span.
        span: Span,
    },
}

impl Pattern {
    /// Returns the span of this pattern.
    pub fn span(&self) -> Span {
        match self {
            Pattern::Literal { span, .. }
            | Pattern::Ident { span, .. }
            | Pattern::Wildcard { span, .. }
            | Pattern::Tuple { span, .. }
            | Pattern::Struct { span, .. }
            | Pattern::Or { span, .. }
            | Pattern::Enum { span, .. }
            | Pattern::Range { span, .. } => *span,
        }
    }
}

/// A field pattern in struct destructuring: `field: pattern` or shorthand `field`.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldPattern {
    /// Field name.
    pub name: String,
    /// Pattern to match (None for shorthand `name` which binds to `name`).
    pub pattern: Option<Pattern>,
    /// Source span.
    pub span: Span,
}

// ═══════════════════════════════════════════════════════════════════════
// Display implementations
// ═══════════════════════════════════════════════════════════════════════

impl fmt::Display for LiteralKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiteralKind::Int(v) => write!(f, "{v}"),
            LiteralKind::Float(v) => write!(f, "{v}"),
            LiteralKind::String(s) => write!(f, "\"{s}\""),
            LiteralKind::RawString(s) => write!(f, "r\"{s}\""),
            LiteralKind::Char(c) => write!(f, "'{c}'"),
            LiteralKind::Bool(b) => write!(f, "{b}"),
            LiteralKind::Null => write!(f, "null"),
        }
    }
}

impl fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeExpr::Simple { name, .. } => write!(f, "{name}"),
            TypeExpr::Generic { name, args, .. } => {
                write!(f, "{name}<")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ">")
            }
            TypeExpr::Tensor {
                element_type, dims, ..
            } => {
                write!(f, "Tensor<{element_type}>[")?;
                for (i, dim) in dims.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    match dim {
                        Some(n) => write!(f, "{n}")?,
                        None => write!(f, "*")?,
                    }
                }
                write!(f, "]")
            }
            TypeExpr::Pointer { mutable, inner, .. } => {
                if *mutable {
                    write!(f, "*mut {inner}")
                } else {
                    write!(f, "*const {inner}")
                }
            }
            TypeExpr::Reference {
                lifetime,
                mutable,
                inner,
                ..
            } => {
                write!(f, "&")?;
                if let Some(lt) = lifetime {
                    write!(f, "'{lt} ")?;
                }
                if *mutable {
                    write!(f, "mut {inner}")
                } else {
                    write!(f, "{inner}")
                }
            }
            TypeExpr::Tuple { elements, .. } => {
                write!(f, "(")?;
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem}")?;
                }
                write!(f, ")")
            }
            TypeExpr::Array { element, size, .. } => write!(f, "[{element}; {size}]"),
            TypeExpr::Slice { element, .. } => write!(f, "[{element}]"),
            TypeExpr::Fn {
                params,
                return_type,
                ..
            } => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {return_type}")
            }
            TypeExpr::Path { segments, .. } => {
                write!(f, "{}", segments.join("::"))
            }
            TypeExpr::DynTrait { trait_name, .. } => {
                write!(f, "dyn {trait_name}")
            }
        }
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Pattern::Literal { kind, .. } => write!(f, "{kind}"),
            Pattern::Ident { name, .. } => write!(f, "{name}"),
            Pattern::Wildcard { .. } => write!(f, "_"),
            Pattern::Tuple { elements, .. } => {
                write!(f, "(")?;
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem}")?;
                }
                write!(f, ")")
            }
            Pattern::Struct { name, fields, .. } => {
                write!(f, "{name} {{ ")?;
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", field.name)?;
                    if let Some(pat) = &field.pattern {
                        write!(f, ": {pat}")?;
                    }
                }
                write!(f, " }}")
            }
            Pattern::Enum {
                enum_name,
                variant,
                fields,
                ..
            } => {
                write!(f, "{enum_name}::{variant}")?;
                if !fields.is_empty() {
                    write!(f, "(")?;
                    for (i, field) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{field}")?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Pattern::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                write!(
                    f,
                    "{}..{}{}",
                    ExprDisplay(start),
                    if *inclusive { "=" } else { "" },
                    ExprDisplay(end)
                )
            }
            Pattern::Or { patterns, .. } => {
                for (i, p) in patterns.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{p}")?;
                }
                Ok(())
            }
        }
    }
}

/// Helper for displaying expressions in patterns (simplified).
struct ExprDisplay<'a>(&'a Expr);

impl fmt::Display for ExprDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Expr::Literal { kind, .. } => write!(f, "{kind}"),
            Expr::Ident { name, .. } => write!(f, "{name}"),
            _ => write!(f, "<expr>"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_span() -> Span {
        Span::new(0, 0)
    }

    // ── Program ────────────────────────────────────────────────────────

    #[test]
    fn program_holds_items() {
        let program = Program {
            items: vec![Item::Stmt(Stmt::Expr {
                expr: Box::new(Expr::Literal {
                    kind: LiteralKind::Int(42),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })],
            span: dummy_span(),
        };
        assert_eq!(program.items.len(), 1);
    }

    // ── Expr span ──────────────────────────────────────────────────────

    #[test]
    fn expr_span_returns_correct_span() {
        let span = Span::new(5, 10);
        let expr = Expr::Literal {
            kind: LiteralKind::Int(42),
            span,
        };
        assert_eq!(expr.span(), span);
    }

    #[test]
    fn expr_span_works_for_all_variants() {
        let s = Span::new(0, 1);
        let lit = || {
            Box::new(Expr::Literal {
                kind: LiteralKind::Int(0),
                span: s,
            })
        };

        // Test a representative subset
        assert_eq!(
            Expr::Ident {
                name: "x".into(),
                span: s
            }
            .span(),
            s
        );
        assert_eq!(
            Expr::Binary {
                left: lit(),
                op: BinOp::Add,
                right: lit(),
                span: s,
            }
            .span(),
            s
        );
        assert_eq!(
            Expr::Unary {
                op: UnaryOp::Neg,
                operand: lit(),
                span: s,
            }
            .span(),
            s
        );
        assert_eq!(
            Expr::Block {
                stmts: vec![],
                expr: None,
                span: s,
            }
            .span(),
            s
        );
    }

    // ── LiteralKind Display ────────────────────────────────────────────

    #[test]
    fn literal_kind_display() {
        assert_eq!(format!("{}", LiteralKind::Int(42)), "42");
        assert_eq!(format!("{}", LiteralKind::Float(3.14)), "3.14");
        assert_eq!(format!("{}", LiteralKind::String("hi".into())), "\"hi\"");
        assert_eq!(
            format!("{}", LiteralKind::RawString("raw".into())),
            "r\"raw\""
        );
        assert_eq!(format!("{}", LiteralKind::Char('a')), "'a'");
        assert_eq!(format!("{}", LiteralKind::Bool(true)), "true");
        assert_eq!(format!("{}", LiteralKind::Null), "null");
    }

    // ── BinOp Display ──────────────────────────────────────────────────

    #[test]
    fn binop_display() {
        assert_eq!(format!("{}", BinOp::Add), "+");
        assert_eq!(format!("{}", BinOp::Sub), "-");
        assert_eq!(format!("{}", BinOp::Mul), "*");
        assert_eq!(format!("{}", BinOp::Div), "/");
        assert_eq!(format!("{}", BinOp::Rem), "%");
        assert_eq!(format!("{}", BinOp::Pow), "**");
        assert_eq!(format!("{}", BinOp::MatMul), "@");
        assert_eq!(format!("{}", BinOp::Eq), "==");
        assert_eq!(format!("{}", BinOp::Ne), "!=");
        assert_eq!(format!("{}", BinOp::And), "&&");
        assert_eq!(format!("{}", BinOp::Or), "||");
        assert_eq!(format!("{}", BinOp::Shl), "<<");
        assert_eq!(format!("{}", BinOp::Shr), ">>");
    }

    // ── UnaryOp Display ────────────────────────────────────────────────

    #[test]
    fn unaryop_display() {
        assert_eq!(format!("{}", UnaryOp::Neg), "-");
        assert_eq!(format!("{}", UnaryOp::Not), "!");
        assert_eq!(format!("{}", UnaryOp::BitNot), "~");
        assert_eq!(format!("{}", UnaryOp::Ref), "&");
        assert_eq!(format!("{}", UnaryOp::RefMut), "&mut");
        assert_eq!(format!("{}", UnaryOp::Deref), "*");
    }

    // ── AssignOp Display ───────────────────────────────────────────────

    #[test]
    fn assignop_display() {
        assert_eq!(format!("{}", AssignOp::Assign), "=");
        assert_eq!(format!("{}", AssignOp::AddAssign), "+=");
        assert_eq!(format!("{}", AssignOp::SubAssign), "-=");
        assert_eq!(format!("{}", AssignOp::MulAssign), "*=");
        assert_eq!(format!("{}", AssignOp::DivAssign), "/=");
        assert_eq!(format!("{}", AssignOp::RemAssign), "%=");
        assert_eq!(format!("{}", AssignOp::BitAndAssign), "&=");
        assert_eq!(format!("{}", AssignOp::BitOrAssign), "|=");
        assert_eq!(format!("{}", AssignOp::BitXorAssign), "^=");
        assert_eq!(format!("{}", AssignOp::ShlAssign), "<<=");
        assert_eq!(format!("{}", AssignOp::ShrAssign), ">>=");
    }

    // ── TypeExpr Display ───────────────────────────────────────────────

    #[test]
    fn type_expr_simple_display() {
        let ty = TypeExpr::Simple {
            name: "i32".into(),
            span: dummy_span(),
        };
        assert_eq!(format!("{ty}"), "i32");
    }

    #[test]
    fn type_expr_generic_display() {
        let ty = TypeExpr::Generic {
            name: "Vec".into(),
            args: vec![TypeExpr::Simple {
                name: "i32".into(),
                span: dummy_span(),
            }],
            span: dummy_span(),
        };
        assert_eq!(format!("{ty}"), "Vec<i32>");
    }

    #[test]
    fn type_expr_tensor_display() {
        let ty = TypeExpr::Tensor {
            element_type: Box::new(TypeExpr::Simple {
                name: "f32".into(),
                span: dummy_span(),
            }),
            dims: vec![Some(3), Some(4)],
            span: dummy_span(),
        };
        assert_eq!(format!("{ty}"), "Tensor<f32>[3, 4]");
    }

    #[test]
    fn type_expr_tensor_dynamic_dim_display() {
        let ty = TypeExpr::Tensor {
            element_type: Box::new(TypeExpr::Simple {
                name: "f64".into(),
                span: dummy_span(),
            }),
            dims: vec![None, Some(10)],
            span: dummy_span(),
        };
        assert_eq!(format!("{ty}"), "Tensor<f64>[*, 10]");
    }

    #[test]
    fn type_expr_pointer_display() {
        let ty = TypeExpr::Pointer {
            mutable: true,
            inner: Box::new(TypeExpr::Simple {
                name: "u8".into(),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        assert_eq!(format!("{ty}"), "*mut u8");
    }

    #[test]
    fn type_expr_reference_display() {
        let ty = TypeExpr::Reference {
            lifetime: None,
            mutable: false,
            inner: Box::new(TypeExpr::Simple {
                name: "str".into(),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        assert_eq!(format!("{ty}"), "&str");
    }

    #[test]
    fn type_expr_reference_with_lifetime_display() {
        let ty = TypeExpr::Reference {
            lifetime: Some("a".into()),
            mutable: false,
            inner: Box::new(TypeExpr::Simple {
                name: "str".into(),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        assert_eq!(format!("{ty}"), "&'a str");
    }

    #[test]
    fn type_expr_mut_reference_with_lifetime_display() {
        let ty = TypeExpr::Reference {
            lifetime: Some("b".into()),
            mutable: true,
            inner: Box::new(TypeExpr::Simple {
                name: "i32".into(),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        assert_eq!(format!("{ty}"), "&'b mut i32");
    }

    #[test]
    fn type_expr_array_display() {
        let ty = TypeExpr::Array {
            element: Box::new(TypeExpr::Simple {
                name: "f32".into(),
                span: dummy_span(),
            }),
            size: 4,
            span: dummy_span(),
        };
        assert_eq!(format!("{ty}"), "[f32; 4]");
    }

    #[test]
    fn type_expr_fn_display() {
        let ty = TypeExpr::Fn {
            params: vec![
                TypeExpr::Simple {
                    name: "i32".into(),
                    span: dummy_span(),
                },
                TypeExpr::Simple {
                    name: "i32".into(),
                    span: dummy_span(),
                },
            ],
            return_type: Box::new(TypeExpr::Simple {
                name: "bool".into(),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        assert_eq!(format!("{ty}"), "fn(i32, i32) -> bool");
    }

    #[test]
    fn type_expr_path_display() {
        let ty = TypeExpr::Path {
            segments: vec!["std".into(), "io".into(), "Error".into()],
            span: dummy_span(),
        };
        assert_eq!(format!("{ty}"), "std::io::Error");
    }

    #[test]
    fn type_expr_span_works() {
        let s = Span::new(5, 15);
        let ty = TypeExpr::Simple {
            name: "i32".into(),
            span: s,
        };
        assert_eq!(ty.span(), s);
    }

    // ── Pattern Display ────────────────────────────────────────────────

    #[test]
    fn pattern_wildcard_display() {
        let pat = Pattern::Wildcard { span: dummy_span() };
        assert_eq!(format!("{pat}"), "_");
    }

    #[test]
    fn pattern_ident_display() {
        let pat = Pattern::Ident {
            name: "x".into(),
            span: dummy_span(),
        };
        assert_eq!(format!("{pat}"), "x");
    }

    #[test]
    fn pattern_literal_display() {
        let pat = Pattern::Literal {
            kind: LiteralKind::Int(42),
            span: dummy_span(),
        };
        assert_eq!(format!("{pat}"), "42");
    }

    #[test]
    fn pattern_tuple_display() {
        let pat = Pattern::Tuple {
            elements: vec![
                Pattern::Ident {
                    name: "a".into(),
                    span: dummy_span(),
                },
                Pattern::Ident {
                    name: "b".into(),
                    span: dummy_span(),
                },
            ],
            span: dummy_span(),
        };
        assert_eq!(format!("{pat}"), "(a, b)");
    }

    #[test]
    fn pattern_enum_display() {
        let pat = Pattern::Enum {
            enum_name: "Shape".into(),
            variant: "Circle".into(),
            fields: vec![Pattern::Ident {
                name: "r".into(),
                span: dummy_span(),
            }],
            span: dummy_span(),
        };
        assert_eq!(format!("{pat}"), "Shape::Circle(r)");
    }

    #[test]
    fn pattern_span_works() {
        let s = Span::new(3, 7);
        let pat = Pattern::Wildcard { span: s };
        assert_eq!(pat.span(), s);
    }

    // ── Stmt construction ──────────────────────────────────────────────

    #[test]
    fn stmt_let_construction() {
        let stmt = Stmt::Let {
            mutable: true,
            name: "x".into(),
            ty: Some(TypeExpr::Simple {
                name: "i32".into(),
                span: dummy_span(),
            }),
            value: Box::new(Expr::Literal {
                kind: LiteralKind::Int(42),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        match stmt {
            Stmt::Let {
                mutable, name, ty, ..
            } => {
                assert!(mutable);
                assert_eq!(name, "x");
                assert!(ty.is_some());
            }
            _ => panic!("expected Let"),
        }
    }

    // ── FnDef construction ─────────────────────────────────────────────

    #[test]
    fn fndef_construction() {
        let fndef = FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            doc_comment: None,
            annotation: Some(Annotation {
                name: "kernel".into(),
                param: None,
                span: dummy_span(),
            }),
            name: "init".into(),
            lifetime_params: vec![],
            generic_params: vec![],
            params: vec![Param {
                name: "x".into(),
                ty: TypeExpr::Simple {
                    name: "i32".into(),
                    span: dummy_span(),
                },
                span: dummy_span(),
            }],
            return_type: Some(TypeExpr::Simple {
                name: "void".into(),
                span: dummy_span(),
            }),
            where_clauses: vec![],
            body: Box::new(Expr::Block {
                stmts: vec![],
                expr: None,
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        assert_eq!(fndef.name, "init");
        assert_eq!(fndef.params.len(), 1);
        assert!(fndef.annotation.is_some());
    }

    // ── StructDef & EnumDef construction ───────────────────────────────

    #[test]
    fn structdef_construction() {
        let sd = StructDef {
            is_pub: false,
            doc_comment: None,
            annotation: None,
            name: "Point".into(),
            lifetime_params: vec![],
            generic_params: vec![],
            fields: vec![
                Field {
                    name: "x".into(),
                    ty: TypeExpr::Simple {
                        name: "f64".into(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
                Field {
                    name: "y".into(),
                    ty: TypeExpr::Simple {
                        name: "f64".into(),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                },
            ],
            span: dummy_span(),
        };
        assert_eq!(sd.name, "Point");
        assert_eq!(sd.fields.len(), 2);
    }

    #[test]
    fn enumdef_construction() {
        let ed = EnumDef {
            is_pub: false,
            doc_comment: None,
            annotation: None,
            name: "Shape".into(),
            lifetime_params: vec![],
            generic_params: vec![],
            variants: vec![
                Variant {
                    name: "Circle".into(),
                    fields: vec![TypeExpr::Simple {
                        name: "f64".into(),
                        span: dummy_span(),
                    }],
                    span: dummy_span(),
                },
                Variant {
                    name: "Rect".into(),
                    fields: vec![
                        TypeExpr::Simple {
                            name: "f64".into(),
                            span: dummy_span(),
                        },
                        TypeExpr::Simple {
                            name: "f64".into(),
                            span: dummy_span(),
                        },
                    ],
                    span: dummy_span(),
                },
            ],
            span: dummy_span(),
        };
        assert_eq!(ed.name, "Shape");
        assert_eq!(ed.variants.len(), 2);
        assert_eq!(ed.variants[0].fields.len(), 1);
        assert_eq!(ed.variants[1].fields.len(), 2);
    }

    // ── UseDecl ────────────────────────────────────────────────────────

    #[test]
    fn use_decl_simple() {
        let ud = UseDecl {
            path: vec!["std".into(), "io".into(), "println".into()],
            kind: UseKind::Simple,
            span: dummy_span(),
        };
        assert_eq!(ud.path, vec!["std", "io", "println"]);
    }

    #[test]
    fn use_decl_glob() {
        let ud = UseDecl {
            path: vec!["std".into(), "io".into()],
            kind: UseKind::Glob,
            span: dummy_span(),
        };
        assert!(matches!(ud.kind, UseKind::Glob));
    }

    #[test]
    fn use_decl_group() {
        let ud = UseDecl {
            path: vec!["std".into(), "io".into()],
            kind: UseKind::Group(vec!["println".into(), "read_line".into()]),
            span: dummy_span(),
        };
        match ud.kind {
            UseKind::Group(names) => assert_eq!(names.len(), 2),
            _ => panic!("expected Group"),
        }
    }

    // ── MatchArm & CallArg ─────────────────────────────────────────────

    #[test]
    fn match_arm_with_guard() {
        let arm = MatchArm {
            pattern: Pattern::Ident {
                name: "x".into(),
                span: dummy_span(),
            },
            guard: Some(Box::new(Expr::Binary {
                left: Box::new(Expr::Ident {
                    name: "x".into(),
                    span: dummy_span(),
                }),
                op: BinOp::Gt,
                right: Box::new(Expr::Literal {
                    kind: LiteralKind::Int(0),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            })),
            body: Box::new(Expr::Ident {
                name: "x".into(),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        assert!(arm.guard.is_some());
    }

    #[test]
    fn call_arg_named_and_positional() {
        let named = CallArg {
            name: Some("x".into()),
            value: Expr::Literal {
                kind: LiteralKind::Int(1),
                span: dummy_span(),
            },
            span: dummy_span(),
        };
        let positional = CallArg {
            name: None,
            value: Expr::Literal {
                kind: LiteralKind::Int(2),
                span: dummy_span(),
            },
            span: dummy_span(),
        };
        assert!(named.name.is_some());
        assert!(positional.name.is_none());
    }

    // ── Closure ────────────────────────────────────────────────────────

    #[test]
    fn closure_construction() {
        let closure = Expr::Closure {
            params: vec![ClosureParam {
                name: "x".into(),
                ty: Some(TypeExpr::Simple {
                    name: "i32".into(),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            }],
            return_type: None,
            body: Box::new(Expr::Binary {
                left: Box::new(Expr::Ident {
                    name: "x".into(),
                    span: dummy_span(),
                }),
                op: BinOp::Mul,
                right: Box::new(Expr::Literal {
                    kind: LiteralKind::Int(2),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        match closure {
            Expr::Closure { params, .. } => assert_eq!(params.len(), 1),
            _ => panic!("expected Closure"),
        }
    }
}
