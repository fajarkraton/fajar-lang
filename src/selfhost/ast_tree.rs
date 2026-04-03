//! Sprint S1: Tree-Based AST — Self-hosted AST for Fajar Lang.
//!
//! Provides the full AST representation used by the self-hosted compiler,
//! including Expr (25+ variants), Stmt, TypeExpr, Pattern, Item, and Program.
//! Includes span tracking, pretty printer, visitor pattern, and JSON serialization.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S1.1: Span Tracking
// ═══════════════════════════════════════════════════════════════════════

/// Source location span for error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AstSpan {
    /// Start offset in source (bytes).
    pub start: usize,
    /// End offset in source (bytes).
    pub end: usize,
    /// Line number (1-based).
    pub line: usize,
    /// Column number (1-based).
    pub col: usize,
}

impl AstSpan {
    /// Creates a new span.
    pub fn new(start: usize, end: usize, line: usize, col: usize) -> Self {
        Self {
            start,
            end,
            line,
            col,
        }
    }

    /// Creates a dummy span for testing.
    pub fn dummy() -> Self {
        Self {
            start: 0,
            end: 0,
            line: 0,
            col: 0,
        }
    }

    /// Merges two spans into one covering both.
    pub fn merge(a: &AstSpan, b: &AstSpan) -> AstSpan {
        AstSpan {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
            line: a.line.min(b.line),
            col: if a.line <= b.line { a.col } else { b.col },
        }
    }

    /// Returns the length of this span in bytes.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns whether this span is empty.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

impl fmt::Display for AstSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.2: Binary & Unary Operators
// ═══════════════════════════════════════════════════════════════════════

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
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
    Pipeline,
    MatMul,
    Range,
    RangeInclusive,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Pow => "**",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::Pipeline => "|>",
            BinOp::MatMul => "@",
            BinOp::Range => "..",
            BinOp::RangeInclusive => "..=",
        };
        write!(f, "{s}")
    }
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Ref,
    RefMut,
    Deref,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
            UnaryOp::BitNot => "~",
            UnaryOp::Ref => "&",
            UnaryOp::RefMut => "&mut",
            UnaryOp::Deref => "*",
        };
        write!(f, "{s}")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.3: Expression Enum (25+ variants)
// ═══════════════════════════════════════════════════════════════════════

/// Expression AST node — 28 variants covering the full Fajar Lang expression set.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Integer literal: `42`, `0xFF`
    IntLit { value: i64, span: AstSpan },
    /// Float literal: `3.14`
    FloatLit { value: f64, span: AstSpan },
    /// Boolean literal: `true`, `false`
    BoolLit { value: bool, span: AstSpan },
    /// String literal: `"hello"`
    StringLit { value: String, span: AstSpan },
    /// Char literal: `'a'`
    CharLit { value: char, span: AstSpan },
    /// Null literal: `null`
    NullLit { span: AstSpan },
    /// Identifier: `x`, `my_var`
    Ident { name: String, span: AstSpan },
    /// Binary operation: `a + b`, `x && y`
    BinOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: AstSpan,
    },
    /// Unary operation: `-x`, `!flag`, `&val`
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
        span: AstSpan,
    },
    /// Function call: `foo(a, b)`
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: AstSpan,
    },
    /// Method call: `obj.method(args)`
    MethodCall {
        object: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        span: AstSpan,
    },
    /// Field access: `obj.field`
    FieldAccess {
        object: Box<Expr>,
        field: String,
        span: AstSpan,
    },
    /// Index access: `arr[idx]`
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: AstSpan,
    },
    /// If expression: `if cond { then } else { otherwise }`
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
        span: AstSpan,
    },
    /// Match expression: `match val { pat => expr, ... }`
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        span: AstSpan,
    },
    /// Block expression: `{ stmts; expr }`
    Block {
        stmts: Vec<Stmt>,
        expr: Option<Box<Expr>>,
        span: AstSpan,
    },
    /// Lambda / closure: `|x, y| x + y`
    Lambda {
        params: Vec<LambdaParam>,
        body: Box<Expr>,
        span: AstSpan,
    },
    /// Array literal: `[1, 2, 3]`
    ArrayLit { elements: Vec<Expr>, span: AstSpan },
    /// Tuple literal: `(a, b, c)`
    TupleLit { elements: Vec<Expr>, span: AstSpan },
    /// Struct literal: `Point { x: 1.0, y: 2.0 }`
    StructLit {
        name: String,
        fields: Vec<(String, Expr)>,
        span: AstSpan,
    },
    /// Type cast: `expr as Type`
    Cast {
        expr: Box<Expr>,
        ty: TypeExpr,
        span: AstSpan,
    },
    /// Try operator: `expr?`
    Try { expr: Box<Expr>, span: AstSpan },
    /// Assignment: `x = 5`, `arr[0] = 1`
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
        span: AstSpan,
    },
    /// Compound assignment: `x += 1`
    CompoundAssign {
        op: BinOp,
        target: Box<Expr>,
        value: Box<Expr>,
        span: AstSpan,
    },
    /// Path expression: `std::io::println`
    Path {
        segments: Vec<String>,
        span: AstSpan,
    },
    /// Enum variant constructor: `Some(42)`, `None`
    EnumVariant {
        enum_name: Option<String>,
        variant: String,
        data: Option<Box<Expr>>,
        span: AstSpan,
    },
    /// F-string interpolation: `f"Hello {name}!"`
    FString {
        parts: Vec<FStringPart>,
        span: AstSpan,
    },
    /// Yield expression (generators): `yield value`
    Yield {
        value: Option<Box<Expr>>,
        span: AstSpan,
    },
    /// Macro metavariable reference: `$x`
    MacroVar { name: String, span: AstSpan },
}

impl Expr {
    /// Returns the span of this expression.
    pub fn span(&self) -> AstSpan {
        match self {
            Expr::IntLit { span, .. }
            | Expr::FloatLit { span, .. }
            | Expr::BoolLit { span, .. }
            | Expr::StringLit { span, .. }
            | Expr::CharLit { span, .. }
            | Expr::NullLit { span }
            | Expr::Ident { span, .. }
            | Expr::BinOp { span, .. }
            | Expr::UnaryOp { span, .. }
            | Expr::Call { span, .. }
            | Expr::MethodCall { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::Index { span, .. }
            | Expr::If { span, .. }
            | Expr::Match { span, .. }
            | Expr::Block { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::ArrayLit { span, .. }
            | Expr::TupleLit { span, .. }
            | Expr::StructLit { span, .. }
            | Expr::Cast { span, .. }
            | Expr::Try { span, .. }
            | Expr::Assign { span, .. }
            | Expr::CompoundAssign { span, .. }
            | Expr::Path { span, .. }
            | Expr::EnumVariant { span, .. }
            | Expr::FString { span, .. }
            | Expr::Yield { span, .. }
            | Expr::MacroVar { span, .. } => *span,
        }
    }

    /// Returns a short description of this expression kind.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Expr::IntLit { .. } => "int_lit",
            Expr::FloatLit { .. } => "float_lit",
            Expr::BoolLit { .. } => "bool_lit",
            Expr::StringLit { .. } => "string_lit",
            Expr::CharLit { .. } => "char_lit",
            Expr::NullLit { .. } => "null_lit",
            Expr::Ident { .. } => "ident",
            Expr::BinOp { .. } => "bin_op",
            Expr::UnaryOp { .. } => "unary_op",
            Expr::Call { .. } => "call",
            Expr::MethodCall { .. } => "method_call",
            Expr::FieldAccess { .. } => "field_access",
            Expr::Index { .. } => "index",
            Expr::If { .. } => "if",
            Expr::Match { .. } => "match",
            Expr::Block { .. } => "block",
            Expr::Lambda { .. } => "lambda",
            Expr::ArrayLit { .. } => "array_lit",
            Expr::TupleLit { .. } => "tuple_lit",
            Expr::StructLit { .. } => "struct_lit",
            Expr::Cast { .. } => "cast",
            Expr::Try { .. } => "try",
            Expr::Assign { .. } => "assign",
            Expr::CompoundAssign { .. } => "compound_assign",
            Expr::Path { .. } => "path",
            Expr::EnumVariant { .. } => "enum_variant",
            Expr::FString { .. } => "f_string",
            Expr::Yield { .. } => "yield",
            Expr::MacroVar { .. } => "macro_var",
        }
    }
}

/// F-string part — literal text or interpolated expression.
#[derive(Debug, Clone, PartialEq)]
pub enum FStringPart {
    /// Literal text portion.
    Literal(String),
    /// Interpolated expression: `{expr}`.
    Expr(Box<Expr>),
}

/// A match arm: `pattern => body`.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// Pattern to match.
    pub pattern: Pattern,
    /// Optional guard: `if condition`.
    pub guard: Option<Box<Expr>>,
    /// Body expression.
    pub body: Box<Expr>,
    /// Source span.
    pub span: AstSpan,
}

/// A lambda parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct LambdaParam {
    /// Parameter name.
    pub name: String,
    /// Optional type annotation.
    pub ty: Option<TypeExpr>,
}

// ═══════════════════════════════════════════════════════════════════════
// S1.4: Statement Enum
// ═══════════════════════════════════════════════════════════════════════

/// Statement AST node — 12 variants.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Let binding: `let [mut] name [: Type] = expr`
    Let {
        name: String,
        mutable: bool,
        ty: Option<TypeExpr>,
        init: Option<Box<Expr>>,
        span: AstSpan,
    },
    /// Function definition.
    FnDef(FnDefNode),
    /// Struct definition.
    StructDef(StructDefNode),
    /// Enum definition.
    EnumDef(EnumDefNode),
    /// Impl block.
    ImplBlock(ImplBlockNode),
    /// Trait definition.
    TraitDef(TraitDefNode),
    /// While loop: `while cond { body }`
    While {
        condition: Box<Expr>,
        body: Box<Expr>,
        span: AstSpan,
    },
    /// For loop: `for name in iter { body }`
    For {
        name: String,
        iter: Box<Expr>,
        body: Box<Expr>,
        span: AstSpan,
    },
    /// Return statement: `return [expr]`
    Return {
        value: Option<Box<Expr>>,
        span: AstSpan,
    },
    /// Break statement.
    Break { span: AstSpan },
    /// Continue statement.
    Continue { span: AstSpan },
    /// Expression statement: `expr;` or trailing `expr`
    ExprStmt { expr: Box<Expr>, span: AstSpan },
}

impl Stmt {
    /// Returns the span of this statement.
    pub fn span(&self) -> AstSpan {
        match self {
            Stmt::Let { span, .. }
            | Stmt::While { span, .. }
            | Stmt::For { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Break { span }
            | Stmt::Continue { span }
            | Stmt::ExprStmt { span, .. } => *span,
            Stmt::FnDef(f) => f.span,
            Stmt::StructDef(s) => s.span,
            Stmt::EnumDef(e) => e.span,
            Stmt::ImplBlock(i) => i.span,
            Stmt::TraitDef(t) => t.span,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.5: Type Expression & Pattern Enums
// ═══════════════════════════════════════════════════════════════════════

/// Type expression — the syntax representation of a type annotation.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// Named type: `i32`, `String`, `Point`
    Name(String, AstSpan),
    /// Generic instantiation: `Vec<T>`, `Result<T, E>`
    Generic(String, Vec<TypeExpr>, AstSpan),
    /// Array type: `[T; N]` or `[T]`
    Array(Box<TypeExpr>, Option<usize>, AstSpan),
    /// Reference type: `&T` or `&mut T`
    Ref(Box<TypeExpr>, bool, AstSpan),
    /// Function type: `fn(A, B) -> C`
    Fn(Vec<TypeExpr>, Box<TypeExpr>, AstSpan),
    /// Tuple type: `(A, B, C)`
    Tuple(Vec<TypeExpr>, AstSpan),
    /// Pointer type: `*const T` or `*mut T`
    Ptr(Box<TypeExpr>, bool, AstSpan),
    /// Never type: `!` / `never`
    Never(AstSpan),
    /// Inferred: `_`
    Inferred(AstSpan),
}

impl TypeExpr {
    /// Returns the span of this type expression.
    pub fn span(&self) -> AstSpan {
        match self {
            TypeExpr::Name(_, span)
            | TypeExpr::Generic(_, _, span)
            | TypeExpr::Array(_, _, span)
            | TypeExpr::Ref(_, _, span)
            | TypeExpr::Fn(_, _, span)
            | TypeExpr::Tuple(_, span)
            | TypeExpr::Ptr(_, _, span)
            | TypeExpr::Never(span)
            | TypeExpr::Inferred(span) => *span,
        }
    }
}

impl fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeExpr::Name(n, _) => write!(f, "{n}"),
            TypeExpr::Generic(n, params, _) => {
                let ps: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                write!(f, "{n}<{}>", ps.join(", "))
            }
            TypeExpr::Array(inner, len, _) => {
                if let Some(n) = len {
                    write!(f, "[{inner}; {n}]")
                } else {
                    write!(f, "[{inner}]")
                }
            }
            TypeExpr::Ref(inner, mutable, _) => {
                if *mutable {
                    write!(f, "&mut {inner}")
                } else {
                    write!(f, "&{inner}")
                }
            }
            TypeExpr::Fn(params, ret, _) => {
                let ps: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                write!(f, "fn({}) -> {ret}", ps.join(", "))
            }
            TypeExpr::Tuple(elems, _) => {
                let es: Vec<String> = elems.iter().map(|e| e.to_string()).collect();
                write!(f, "({})", es.join(", "))
            }
            TypeExpr::Ptr(inner, mutable, _) => {
                if *mutable {
                    write!(f, "*mut {inner}")
                } else {
                    write!(f, "*const {inner}")
                }
            }
            TypeExpr::Never(_) => write!(f, "never"),
            TypeExpr::Inferred(_) => write!(f, "_"),
        }
    }
}

/// Pattern for match arms and let bindings.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// Identifier pattern: `x`
    Ident(String, AstSpan),
    /// Tuple pattern: `(a, b, c)`
    Tuple(Vec<Pattern>, AstSpan),
    /// Struct pattern: `Point { x, y }`
    Struct(String, Vec<(String, Option<Pattern>)>, AstSpan),
    /// Enum variant pattern: `Some(x)`, `None`
    Enum(String, Option<String>, Option<Box<Pattern>>, AstSpan),
    /// Wildcard pattern: `_`
    Wildcard(AstSpan),
    /// Literal pattern: `42`, `"hello"`, `true`
    Literal(Box<Expr>, AstSpan),
    /// Or pattern: `A | B`
    Or(Vec<Pattern>, AstSpan),
    /// Rest pattern: `..`
    Rest(AstSpan),
}

impl Pattern {
    /// Returns the span of this pattern.
    pub fn span(&self) -> AstSpan {
        match self {
            Pattern::Ident(_, span)
            | Pattern::Tuple(_, span)
            | Pattern::Struct(_, _, span)
            | Pattern::Enum(_, _, _, span)
            | Pattern::Wildcard(span)
            | Pattern::Literal(_, span)
            | Pattern::Or(_, span)
            | Pattern::Rest(span) => *span,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.6: Top-Level Item & Definition Nodes
// ═══════════════════════════════════════════════════════════════════════

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub ty: TypeExpr,
    /// Whether parameter is mutable.
    pub mutable: bool,
}

/// Function definition node.
#[derive(Debug, Clone, PartialEq)]
pub struct FnDefNode {
    /// Function name.
    pub name: String,
    /// Generic type parameters.
    pub type_params: Vec<String>,
    /// Parameters.
    pub params: Vec<Param>,
    /// Return type (None = void).
    pub ret_type: Option<TypeExpr>,
    /// Function body.
    pub body: Box<Expr>,
    /// Whether this function is public.
    pub is_pub: bool,
    /// Context annotation (@kernel, @device, etc.).
    pub context: Option<String>,
    /// Whether this is an async function.
    pub is_async: bool,
    /// Whether this is a generator function.
    pub is_gen: bool,
    /// Source span.
    pub span: AstSpan,
}

/// Struct definition node.
#[derive(Debug, Clone, PartialEq)]
pub struct StructDefNode {
    /// Struct name.
    pub name: String,
    /// Generic type parameters.
    pub type_params: Vec<String>,
    /// Fields: (name, type, is_pub).
    pub fields: Vec<(String, TypeExpr, bool)>,
    /// Whether this struct is public.
    pub is_pub: bool,
    /// Source span.
    pub span: AstSpan,
}

/// Enum definition node.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDefNode {
    /// Enum name.
    pub name: String,
    /// Generic type parameters.
    pub type_params: Vec<String>,
    /// Variants: (name, optional data types).
    pub variants: Vec<(String, Vec<TypeExpr>)>,
    /// Whether this enum is public.
    pub is_pub: bool,
    /// Source span.
    pub span: AstSpan,
}

/// Impl block node.
#[derive(Debug, Clone, PartialEq)]
pub struct ImplBlockNode {
    /// Type being implemented.
    pub target: String,
    /// Trait being implemented (None = inherent impl).
    pub trait_name: Option<String>,
    /// Methods.
    pub methods: Vec<FnDefNode>,
    /// Source span.
    pub span: AstSpan,
}

/// Trait definition node.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitDefNode {
    /// Trait name.
    pub name: String,
    /// Generic type parameters.
    pub type_params: Vec<String>,
    /// Method signatures (body may be empty for required methods).
    pub methods: Vec<FnDefNode>,
    /// Whether this trait is public.
    pub is_pub: bool,
    /// Source span.
    pub span: AstSpan,
}

/// Use/import declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct UseNode {
    /// Import path segments.
    pub path: Vec<String>,
    /// Optional alias.
    pub alias: Option<String>,
    /// Source span.
    pub span: AstSpan,
}

/// Module declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ModNode {
    /// Module name.
    pub name: String,
    /// Inline items (None = external file module).
    pub items: Option<Vec<Item>>,
    /// Source span.
    pub span: AstSpan,
}

/// Top-level item.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Function definition.
    FnDef(FnDefNode),
    /// Struct definition.
    StructDef(StructDefNode),
    /// Enum definition.
    EnumDef(EnumDefNode),
    /// Impl block.
    ImplBlock(ImplBlockNode),
    /// Trait definition.
    TraitDef(TraitDefNode),
    /// Use declaration.
    Use(UseNode),
    /// Module declaration.
    Mod(ModNode),
    /// Constant definition.
    Const {
        name: String,
        ty: Option<TypeExpr>,
        value: Box<Expr>,
        is_pub: bool,
        span: AstSpan,
    },
    /// Top-level statement.
    Stmt(Stmt),
}

// ═══════════════════════════════════════════════════════════════════════
// S1.7: Program (root)
// ═══════════════════════════════════════════════════════════════════════

/// The root AST node — a complete Fajar Lang program.
#[derive(Debug, Clone, PartialEq)]
pub struct AstProgram {
    /// Top-level items in source order.
    pub items: Vec<Item>,
    /// Source file name (for error messages).
    pub filename: String,
    /// Span covering the entire program.
    pub span: AstSpan,
}

impl AstProgram {
    /// Creates a new program.
    pub fn new(filename: &str, items: Vec<Item>) -> Self {
        let span = if items.is_empty() {
            AstSpan::dummy()
        } else {
            AstSpan::new(0, 0, 1, 1)
        };
        Self {
            items,
            filename: filename.into(),
            span,
        }
    }

    /// Returns the number of top-level items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns all function definitions.
    pub fn functions(&self) -> Vec<&FnDefNode> {
        self.items
            .iter()
            .filter_map(|item| match item {
                Item::FnDef(f) => Some(f),
                _ => None,
            })
            .collect()
    }

    /// Returns all struct definitions.
    pub fn structs(&self) -> Vec<&StructDefNode> {
        self.items
            .iter()
            .filter_map(|item| match item {
                Item::StructDef(s) => Some(s),
                _ => None,
            })
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.8: Pretty Printer
// ═══════════════════════════════════════════════════════════════════════

/// Pretty printer for AST nodes.
pub struct PrettyPrinter {
    /// Current indentation level.
    indent: usize,
    /// Output buffer.
    output: String,
}

impl PrettyPrinter {
    /// Creates a new pretty printer.
    pub fn new() -> Self {
        Self {
            indent: 0,
            output: String::new(),
        }
    }

    /// Pretty-prints an expression.
    pub fn print_expr(&mut self, expr: &Expr) -> String {
        self.output.clear();
        self.fmt_expr(expr);
        self.output.clone()
    }

    /// Pretty-prints a statement.
    pub fn print_stmt(&mut self, stmt: &Stmt) -> String {
        self.output.clear();
        self.fmt_stmt(stmt);
        self.output.clone()
    }

    /// Pretty-prints an entire program.
    pub fn print_program(&mut self, program: &AstProgram) -> String {
        self.output.clear();
        for (i, item) in program.items.iter().enumerate() {
            if i > 0 {
                self.output.push('\n');
            }
            self.fmt_item(item);
        }
        self.output.clone()
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    fn fmt_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntLit { value, .. } => self.output.push_str(&value.to_string()),
            Expr::FloatLit { value, .. } => self.output.push_str(&value.to_string()),
            Expr::BoolLit { value, .. } => self.output.push_str(&value.to_string()),
            Expr::StringLit { value, .. } => {
                self.output.push('"');
                self.output.push_str(value);
                self.output.push('"');
            }
            Expr::CharLit { value, .. } => {
                self.output.push('\'');
                self.output.push(*value);
                self.output.push('\'');
            }
            Expr::NullLit { .. } => self.output.push_str("null"),
            Expr::Ident { name, .. } => self.output.push_str(name),
            Expr::BinOp {
                op, left, right, ..
            } => {
                self.fmt_expr(left);
                self.output.push(' ');
                self.output.push_str(&op.to_string());
                self.output.push(' ');
                self.fmt_expr(right);
            }
            Expr::UnaryOp { op, operand, .. } => {
                self.output.push_str(&op.to_string());
                self.fmt_expr(operand);
            }
            Expr::Call { callee, args, .. } => {
                self.fmt_expr(callee);
                self.output.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.fmt_expr(arg);
                }
                self.output.push(')');
            }
            Expr::FieldAccess { object, field, .. } => {
                self.fmt_expr(object);
                self.output.push('.');
                self.output.push_str(field);
            }
            Expr::Index { object, index, .. } => {
                self.fmt_expr(object);
                self.output.push('[');
                self.fmt_expr(index);
                self.output.push(']');
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.output.push_str("if ");
                self.fmt_expr(condition);
                self.output.push_str(" { ");
                self.fmt_expr(then_branch);
                self.output.push_str(" }");
                if let Some(otherwise) = else_branch {
                    self.output.push_str(" else { ");
                    self.fmt_expr(otherwise);
                    self.output.push_str(" }");
                }
            }
            Expr::ArrayLit { elements, .. } => {
                self.output.push('[');
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.fmt_expr(elem);
                }
                self.output.push(']');
            }
            Expr::Path { segments, .. } => {
                self.output.push_str(&segments.join("::"));
            }
            _ => {
                self.output.push_str(&format!("<{}>", expr.kind_name()));
            }
        }
    }

    fn fmt_stmt(&mut self, stmt: &Stmt) {
        self.write_indent();
        match stmt {
            Stmt::Let {
                name,
                mutable,
                ty,
                init,
                ..
            } => {
                self.output.push_str("let ");
                if *mutable {
                    self.output.push_str("mut ");
                }
                self.output.push_str(name);
                if let Some(t) = ty {
                    self.output.push_str(": ");
                    self.output.push_str(&t.to_string());
                }
                if let Some(val) = init {
                    self.output.push_str(" = ");
                    self.fmt_expr(val);
                }
            }
            Stmt::Return { value, .. } => {
                self.output.push_str("return");
                if let Some(val) = value {
                    self.output.push(' ');
                    self.fmt_expr(val);
                }
            }
            Stmt::Break { .. } => self.output.push_str("break"),
            Stmt::Continue { .. } => self.output.push_str("continue"),
            Stmt::ExprStmt { expr, .. } => self.fmt_expr(expr),
            Stmt::FnDef(f) => {
                self.output.push_str("fn ");
                self.output.push_str(&f.name);
                self.output.push_str("(...)");
            }
            _ => {
                self.output.push_str("<stmt>");
            }
        }
    }

    fn fmt_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(f) => {
                self.write_indent();
                if f.is_pub {
                    self.output.push_str("pub ");
                }
                self.output.push_str("fn ");
                self.output.push_str(&f.name);
                self.output.push_str("(...)");
                if let Some(ret) = &f.ret_type {
                    self.output.push_str(" -> ");
                    self.output.push_str(&ret.to_string());
                }
                self.output.push_str(" { ... }\n");
            }
            Item::StructDef(s) => {
                self.write_indent();
                if s.is_pub {
                    self.output.push_str("pub ");
                }
                self.output.push_str("struct ");
                self.output.push_str(&s.name);
                self.output.push_str(" { ... }\n");
            }
            Item::Stmt(stmt) => {
                self.fmt_stmt(stmt);
                self.output.push('\n');
            }
            _ => {
                self.write_indent();
                self.output.push_str("<item>\n");
            }
        }
    }
}

impl Default for PrettyPrinter {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.9: Visitor Pattern
// ═══════════════════════════════════════════════════════════════════════

/// Visitor trait for walking the AST.
pub trait AstVisitor {
    /// Visit an expression. Return false to stop traversal.
    fn visit_expr(&mut self, expr: &Expr) -> bool {
        let _ = expr;
        true
    }

    /// Visit a statement.
    fn visit_stmt(&mut self, stmt: &Stmt) -> bool {
        let _ = stmt;
        true
    }

    /// Visit an item.
    fn visit_item(&mut self, item: &Item) -> bool {
        let _ = item;
        true
    }
}

/// Walks an expression tree, calling the visitor at each node.
pub fn walk_expr(visitor: &mut dyn AstVisitor, expr: &Expr) {
    if !visitor.visit_expr(expr) {
        return;
    }
    match expr {
        Expr::BinOp { left, right, .. } => {
            walk_expr(visitor, left);
            walk_expr(visitor, right);
        }
        Expr::UnaryOp { operand, .. } => {
            walk_expr(visitor, operand);
        }
        Expr::Call { callee, args, .. } => {
            walk_expr(visitor, callee);
            for arg in args {
                walk_expr(visitor, arg);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            walk_expr(visitor, object);
            for arg in args {
                walk_expr(visitor, arg);
            }
        }
        Expr::FieldAccess { object, .. } => walk_expr(visitor, object),
        Expr::Index { object, index, .. } => {
            walk_expr(visitor, object);
            walk_expr(visitor, index);
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            walk_expr(visitor, condition);
            walk_expr(visitor, then_branch);
            if let Some(e) = else_branch {
                walk_expr(visitor, e);
            }
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                walk_stmt(visitor, stmt);
            }
            if let Some(e) = expr {
                walk_expr(visitor, e);
            }
        }
        Expr::Lambda { body, .. } => walk_expr(visitor, body),
        Expr::ArrayLit { elements, .. } | Expr::TupleLit { elements, .. } => {
            for elem in elements {
                walk_expr(visitor, elem);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            walk_expr(visitor, scrutinee);
            for arm in arms {
                walk_expr(visitor, &arm.body);
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, expr) in fields {
                walk_expr(visitor, expr);
            }
        }
        Expr::Cast { expr, .. } | Expr::Try { expr, .. } => walk_expr(visitor, expr),
        Expr::Assign { target, value, .. } | Expr::CompoundAssign { target, value, .. } => {
            walk_expr(visitor, target);
            walk_expr(visitor, value);
        }
        Expr::Yield { value, .. } => {
            if let Some(v) = value {
                walk_expr(visitor, v);
            }
        }
        Expr::EnumVariant { data, .. } => {
            if let Some(d) = data {
                walk_expr(visitor, d);
            }
        }
        Expr::FString { parts, .. } => {
            for part in parts {
                if let FStringPart::Expr(e) = part {
                    walk_expr(visitor, e);
                }
            }
        }
        // Leaf nodes
        Expr::IntLit { .. }
        | Expr::FloatLit { .. }
        | Expr::BoolLit { .. }
        | Expr::StringLit { .. }
        | Expr::CharLit { .. }
        | Expr::NullLit { .. }
        | Expr::Ident { .. }
        | Expr::Path { .. }
        | Expr::MacroVar { .. } => {}
    }
}

/// Walks a statement, calling the visitor.
pub fn walk_stmt(visitor: &mut dyn AstVisitor, stmt: &Stmt) {
    if !visitor.visit_stmt(stmt) {
        return;
    }
    match stmt {
        Stmt::Let {
            init: Some(val), ..
        } => {
            walk_expr(visitor, val);
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr(visitor, condition);
            walk_expr(visitor, body);
        }
        Stmt::For { iter, body, .. } => {
            walk_expr(visitor, iter);
            walk_expr(visitor, body);
        }
        Stmt::Return {
            value: Some(val), ..
        } => {
            walk_expr(visitor, val);
        }
        Stmt::ExprStmt { expr, .. } => walk_expr(visitor, expr),
        Stmt::FnDef(f) => walk_expr(visitor, &f.body),
        _ => {}
    }
}

/// Walks a program, visiting all items.
pub fn walk_program(visitor: &mut dyn AstVisitor, program: &AstProgram) {
    for item in &program.items {
        if !visitor.visit_item(item) {
            continue;
        }
        match item {
            Item::FnDef(f) => walk_expr(visitor, &f.body),
            Item::Stmt(stmt) => walk_stmt(visitor, stmt),
            _ => {}
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.10: JSON Serialization
// ═══════════════════════════════════════════════════════════════════════

/// Serializes an expression to a JSON-like string (for debugging/testing).
pub fn expr_to_json(expr: &Expr) -> String {
    match expr {
        Expr::IntLit { value, .. } => format!(r#"{{"kind":"int","value":{value}}}"#),
        Expr::FloatLit { value, .. } => format!(r#"{{"kind":"float","value":{value}}}"#),
        Expr::BoolLit { value, .. } => format!(r#"{{"kind":"bool","value":{value}}}"#),
        Expr::StringLit { value, .. } => format!(r#"{{"kind":"string","value":"{value}"}}"#),
        Expr::CharLit { value, .. } => format!(r#"{{"kind":"char","value":"{value}"}}"#),
        Expr::NullLit { .. } => r#"{"kind":"null"}"#.to_string(),
        Expr::Ident { name, .. } => format!(r#"{{"kind":"ident","name":"{name}"}}"#),
        Expr::BinOp {
            op, left, right, ..
        } => {
            format!(
                r#"{{"kind":"bin_op","op":"{}","left":{},"right":{}}}"#,
                op,
                expr_to_json(left),
                expr_to_json(right)
            )
        }
        Expr::UnaryOp { op, operand, .. } => {
            format!(
                r#"{{"kind":"unary_op","op":"{}","operand":{}}}"#,
                op,
                expr_to_json(operand)
            )
        }
        Expr::Call { callee, args, .. } => {
            let args_json: Vec<String> = args.iter().map(expr_to_json).collect();
            format!(
                r#"{{"kind":"call","callee":{},"args":[{}]}}"#,
                expr_to_json(callee),
                args_json.join(",")
            )
        }
        Expr::ArrayLit { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(expr_to_json).collect();
            format!(r#"{{"kind":"array","elements":[{}]}}"#, elems.join(","))
        }
        _ => format!(r#"{{"kind":"{}"}}"#, expr.kind_name()),
    }
}

/// Collects all identifiers referenced in an expression tree.
pub fn collect_idents(expr: &Expr) -> Vec<String> {
    struct IdentCollector {
        idents: Vec<String>,
    }
    impl AstVisitor for IdentCollector {
        fn visit_expr(&mut self, expr: &Expr) -> bool {
            if let Expr::Ident { name, .. } = expr {
                self.idents.push(name.clone());
            }
            true
        }
    }
    let mut collector = IdentCollector { idents: Vec::new() };
    walk_expr(&mut collector, expr);
    collector.idents
}

/// Counts the total number of AST nodes in an expression tree.
pub fn count_nodes(expr: &Expr) -> usize {
    struct NodeCounter {
        count: usize,
    }
    impl AstVisitor for NodeCounter {
        fn visit_expr(&mut self, _expr: &Expr) -> bool {
            self.count += 1;
            true
        }
    }
    let mut counter = NodeCounter { count: 0 };
    walk_expr(&mut counter, expr);
    counter.count
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> AstSpan {
        AstSpan::dummy()
    }

    fn int_expr(v: i64) -> Expr {
        Expr::IntLit {
            value: v,
            span: span(),
        }
    }

    fn ident_expr(name: &str) -> Expr {
        Expr::Ident {
            name: name.into(),
            span: span(),
        }
    }

    // S1.1 — Span tracking
    #[test]
    fn s1_1_span_creation() {
        let s = AstSpan::new(0, 10, 1, 1);
        assert_eq!(s.len(), 10);
        assert!(!s.is_empty());
        assert_eq!(s.to_string(), "1:1");
    }

    #[test]
    fn s1_1_span_merge() {
        let a = AstSpan::new(0, 5, 1, 1);
        let b = AstSpan::new(10, 20, 3, 5);
        let merged = AstSpan::merge(&a, &b);
        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 20);
    }

    #[test]
    fn s1_1_span_dummy() {
        let s = AstSpan::dummy();
        assert!(s.is_empty());
    }

    // S1.2 — Operators
    #[test]
    fn s1_2_binop_display() {
        assert_eq!(BinOp::Add.to_string(), "+");
        assert_eq!(BinOp::Pipeline.to_string(), "|>");
        assert_eq!(BinOp::Pow.to_string(), "**");
        assert_eq!(BinOp::MatMul.to_string(), "@");
    }

    #[test]
    fn s1_2_unaryop_display() {
        assert_eq!(UnaryOp::Neg.to_string(), "-");
        assert_eq!(UnaryOp::Not.to_string(), "!");
        assert_eq!(UnaryOp::RefMut.to_string(), "&mut");
    }

    // S1.3 — Expression variants
    #[test]
    fn s1_3_expr_int_lit() {
        let e = int_expr(42);
        assert_eq!(e.kind_name(), "int_lit");
        assert_eq!(e.span(), span());
    }

    #[test]
    fn s1_3_expr_binop() {
        let e = Expr::BinOp {
            op: BinOp::Add,
            left: Box::new(int_expr(1)),
            right: Box::new(int_expr(2)),
            span: span(),
        };
        assert_eq!(e.kind_name(), "bin_op");
    }

    #[test]
    fn s1_3_expr_call() {
        let e = Expr::Call {
            callee: Box::new(ident_expr("foo")),
            args: vec![int_expr(1), int_expr(2)],
            span: span(),
        };
        assert_eq!(e.kind_name(), "call");
    }

    #[test]
    fn s1_3_expr_if() {
        let e = Expr::If {
            condition: Box::new(Expr::BoolLit {
                value: true,
                span: span(),
            }),
            then_branch: Box::new(int_expr(1)),
            else_branch: Some(Box::new(int_expr(2))),
            span: span(),
        };
        assert_eq!(e.kind_name(), "if");
    }

    #[test]
    fn s1_3_expr_all_28_variants_have_kind_name() {
        // Verify that all 28 variants produce non-empty kind names.
        let variants: Vec<Expr> = vec![
            int_expr(0),
            Expr::FloatLit {
                value: 1.0,
                span: span(),
            },
            Expr::BoolLit {
                value: true,
                span: span(),
            },
            Expr::StringLit {
                value: "hi".into(),
                span: span(),
            },
            Expr::CharLit {
                value: 'a',
                span: span(),
            },
            Expr::NullLit { span: span() },
            ident_expr("x"),
            Expr::BinOp {
                op: BinOp::Add,
                left: Box::new(int_expr(0)),
                right: Box::new(int_expr(0)),
                span: span(),
            },
            Expr::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(int_expr(0)),
                span: span(),
            },
            Expr::Call {
                callee: Box::new(ident_expr("f")),
                args: vec![],
                span: span(),
            },
            Expr::MethodCall {
                object: Box::new(ident_expr("x")),
                method: "m".into(),
                args: vec![],
                span: span(),
            },
            Expr::FieldAccess {
                object: Box::new(ident_expr("x")),
                field: "f".into(),
                span: span(),
            },
            Expr::Index {
                object: Box::new(ident_expr("a")),
                index: Box::new(int_expr(0)),
                span: span(),
            },
            Expr::If {
                condition: Box::new(Expr::BoolLit {
                    value: true,
                    span: span(),
                }),
                then_branch: Box::new(int_expr(0)),
                else_branch: None,
                span: span(),
            },
            Expr::Match {
                scrutinee: Box::new(int_expr(0)),
                arms: vec![],
                span: span(),
            },
            Expr::Block {
                stmts: vec![],
                expr: None,
                span: span(),
            },
            Expr::Lambda {
                params: vec![],
                body: Box::new(int_expr(0)),
                span: span(),
            },
            Expr::ArrayLit {
                elements: vec![],
                span: span(),
            },
            Expr::TupleLit {
                elements: vec![],
                span: span(),
            },
            Expr::StructLit {
                name: "S".into(),
                fields: vec![],
                span: span(),
            },
            Expr::Cast {
                expr: Box::new(int_expr(0)),
                ty: TypeExpr::Name("i64".into(), span()),
                span: span(),
            },
            Expr::Try {
                expr: Box::new(int_expr(0)),
                span: span(),
            },
            Expr::Assign {
                target: Box::new(ident_expr("x")),
                value: Box::new(int_expr(0)),
                span: span(),
            },
            Expr::CompoundAssign {
                op: BinOp::Add,
                target: Box::new(ident_expr("x")),
                value: Box::new(int_expr(1)),
                span: span(),
            },
            Expr::Path {
                segments: vec!["std".into(), "io".into()],
                span: span(),
            },
            Expr::EnumVariant {
                enum_name: None,
                variant: "None".into(),
                data: None,
                span: span(),
            },
            Expr::FString {
                parts: vec![],
                span: span(),
            },
            Expr::Yield {
                value: None,
                span: span(),
            },
        ];
        assert_eq!(variants.len(), 28);
        for v in &variants {
            assert!(!v.kind_name().is_empty());
        }
    }

    // S1.4 — Statement enum
    #[test]
    fn s1_4_stmt_let() {
        let s = Stmt::Let {
            name: "x".into(),
            mutable: false,
            ty: Some(TypeExpr::Name("i32".into(), span())),
            init: Some(Box::new(int_expr(42))),
            span: span(),
        };
        assert_eq!(s.span(), span());
    }

    #[test]
    fn s1_4_stmt_return_break_continue() {
        let r = Stmt::Return {
            value: None,
            span: span(),
        };
        let b = Stmt::Break { span: span() };
        let c = Stmt::Continue { span: span() };
        assert_eq!(r.span(), span());
        assert_eq!(b.span(), span());
        assert_eq!(c.span(), span());
    }

    // S1.5 — TypeExpr & Pattern
    #[test]
    fn s1_5_type_expr_display() {
        assert_eq!(TypeExpr::Name("i32".into(), span()).to_string(), "i32");
        assert_eq!(
            TypeExpr::Generic(
                "Vec".into(),
                vec![TypeExpr::Name("i32".into(), span())],
                span()
            )
            .to_string(),
            "Vec<i32>"
        );
        assert_eq!(
            TypeExpr::Array(
                Box::new(TypeExpr::Name("u8".into(), span())),
                Some(10),
                span()
            )
            .to_string(),
            "[u8; 10]"
        );
        assert_eq!(
            TypeExpr::Ref(Box::new(TypeExpr::Name("T".into(), span())), true, span()).to_string(),
            "&mut T"
        );
        assert_eq!(TypeExpr::Never(span()).to_string(), "never");
        assert_eq!(TypeExpr::Inferred(span()).to_string(), "_");
    }

    #[test]
    fn s1_5_pattern_span() {
        let p = Pattern::Wildcard(AstSpan::new(5, 6, 1, 5));
        assert_eq!(p.span().start, 5);
    }

    #[test]
    fn s1_5_pattern_variants() {
        let _ = Pattern::Ident("x".into(), span());
        let _ = Pattern::Tuple(vec![Pattern::Wildcard(span())], span());
        let _ = Pattern::Struct("Point".into(), vec![("x".into(), None)], span());
        let _ = Pattern::Enum("Option".into(), Some("Some".into()), None, span());
        let _ = Pattern::Or(vec![Pattern::Wildcard(span())], span());
        let _ = Pattern::Rest(span());
    }

    // S1.6 — Top-level items
    #[test]
    fn s1_6_fn_def_node() {
        let f = FnDefNode {
            name: "add".into(),
            type_params: vec![],
            params: vec![
                Param {
                    name: "a".into(),
                    ty: TypeExpr::Name("i32".into(), span()),
                    mutable: false,
                },
                Param {
                    name: "b".into(),
                    ty: TypeExpr::Name("i32".into(), span()),
                    mutable: false,
                },
            ],
            ret_type: Some(TypeExpr::Name("i32".into(), span())),
            body: Box::new(Expr::BinOp {
                op: BinOp::Add,
                left: Box::new(ident_expr("a")),
                right: Box::new(ident_expr("b")),
                span: span(),
            }),
            is_pub: true,
            context: None,
            is_async: false,
            is_gen: false,
            span: span(),
        };
        assert_eq!(f.name, "add");
        assert_eq!(f.params.len(), 2);
    }

    // S1.7 — Program
    #[test]
    fn s1_7_program_creation() {
        let prog = AstProgram::new(
            "test.fj",
            vec![Item::Stmt(Stmt::ExprStmt {
                expr: Box::new(int_expr(42)),
                span: span(),
            })],
        );
        assert_eq!(prog.item_count(), 1);
        assert_eq!(prog.filename, "test.fj");
    }

    #[test]
    fn s1_7_program_functions() {
        let prog = AstProgram::new(
            "test.fj",
            vec![Item::FnDef(FnDefNode {
                name: "main".into(),
                type_params: vec![],
                params: vec![],
                ret_type: None,
                body: Box::new(Expr::Block {
                    stmts: vec![],
                    expr: None,
                    span: span(),
                }),
                is_pub: false,
                context: None,
                is_async: false,
                is_gen: false,
                span: span(),
            })],
        );
        assert_eq!(prog.functions().len(), 1);
        assert_eq!(prog.structs().len(), 0);
    }

    // S1.8 — Pretty printer
    #[test]
    fn s1_8_pretty_print_int() {
        let mut pp = PrettyPrinter::new();
        let result = pp.print_expr(&int_expr(42));
        assert_eq!(result, "42");
    }

    #[test]
    fn s1_8_pretty_print_binop() {
        let mut pp = PrettyPrinter::new();
        let e = Expr::BinOp {
            op: BinOp::Add,
            left: Box::new(int_expr(1)),
            right: Box::new(int_expr(2)),
            span: span(),
        };
        let result = pp.print_expr(&e);
        assert_eq!(result, "1 + 2");
    }

    #[test]
    fn s1_8_pretty_print_let() {
        let mut pp = PrettyPrinter::new();
        let s = Stmt::Let {
            name: "x".into(),
            mutable: true,
            ty: Some(TypeExpr::Name("i32".into(), span())),
            init: Some(Box::new(int_expr(42))),
            span: span(),
        };
        let result = pp.print_stmt(&s);
        assert!(result.contains("let mut x: i32 = 42"));
    }

    // S1.9 — Visitor pattern
    #[test]
    fn s1_9_collect_idents() {
        let e = Expr::BinOp {
            op: BinOp::Add,
            left: Box::new(ident_expr("x")),
            right: Box::new(ident_expr("y")),
            span: span(),
        };
        let idents = collect_idents(&e);
        assert_eq!(idents, vec!["x".to_string(), "y".to_string()]);
    }

    #[test]
    fn s1_9_count_nodes() {
        let e = Expr::BinOp {
            op: BinOp::Add,
            left: Box::new(int_expr(1)),
            right: Box::new(int_expr(2)),
            span: span(),
        };
        assert_eq!(count_nodes(&e), 3); // binop + 2 ints
    }

    #[test]
    fn s1_9_walk_program() {
        struct ItemCounter {
            count: usize,
        }
        impl AstVisitor for ItemCounter {
            fn visit_item(&mut self, _item: &Item) -> bool {
                self.count += 1;
                true
            }
        }
        let prog = AstProgram::new(
            "test.fj",
            vec![
                Item::Stmt(Stmt::ExprStmt {
                    expr: Box::new(int_expr(1)),
                    span: span(),
                }),
                Item::Stmt(Stmt::ExprStmt {
                    expr: Box::new(int_expr(2)),
                    span: span(),
                }),
            ],
        );
        let mut counter = ItemCounter { count: 0 };
        walk_program(&mut counter, &prog);
        assert_eq!(counter.count, 2);
    }

    // S1.10 — JSON serialization
    #[test]
    fn s1_10_json_int() {
        let json = expr_to_json(&int_expr(42));
        assert!(json.contains(r#""kind":"int""#));
        assert!(json.contains(r#""value":42"#));
    }

    #[test]
    fn s1_10_json_binop() {
        let e = Expr::BinOp {
            op: BinOp::Add,
            left: Box::new(int_expr(1)),
            right: Box::new(int_expr(2)),
            span: span(),
        };
        let json = expr_to_json(&e);
        assert!(json.contains(r#""kind":"bin_op""#));
        assert!(json.contains(r#""op":"+""#));
    }

    #[test]
    fn s1_10_json_ident() {
        let json = expr_to_json(&ident_expr("foo"));
        assert!(json.contains(r#""kind":"ident""#));
        assert!(json.contains(r#""name":"foo""#));
    }

    #[test]
    fn s1_10_json_call() {
        let e = Expr::Call {
            callee: Box::new(ident_expr("add")),
            args: vec![int_expr(1), int_expr(2)],
            span: span(),
        };
        let json = expr_to_json(&e);
        assert!(json.contains(r#""kind":"call""#));
        assert!(json.contains(r#""args":[{"kind":"int","value":1},{"kind":"int","value":2}]"#));
    }
}
