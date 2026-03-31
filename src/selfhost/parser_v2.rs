//! Sprint S2: Parser Upgrade — 19-level Pratt expression parser and full
//! statement/type/pattern parsing for the self-hosted compiler.
//!
//! Provides a complete recursive-descent + Pratt parser that produces the
//! self-hosted AST (`ast_tree` module). Includes multi-error collection,
//! error recovery, and module/import parsing.

use std::fmt;

use super::ast_tree::{
    AstProgram, AstSpan, BinOp, EnumDefNode, Expr, FnDefNode, ImplBlockNode, Item, MatchArm,
    ModNode, Param, Pattern, Stmt, StructDefNode, TraitDefNode, TypeExpr, UnaryOp, UseNode,
};

// ═══════════════════════════════════════════════════════════════════════
// S2.1: Token representation (simplified for self-hosted parser)
// ═══════════════════════════════════════════════════════════════════════

/// Simplified token for the self-hosted parser.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// Token kind.
    pub kind: TokenKind,
    /// Lexeme text.
    pub text: String,
    /// Source span.
    pub span: AstSpan,
}

/// Token kinds for the self-hosted parser.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // Literals
    IntLit,
    FloatLit,
    StringLit,
    CharLit,
    BoolLit,

    // Identifiers & Keywords
    Ident,
    Let,
    Mut,
    Fn,
    Struct,
    Enum,
    Impl,
    Trait,
    If,
    Else,
    Match,
    While,
    For,
    In,
    Return,
    Break,
    Continue,
    Loop,
    Pub,
    Use,
    Mod,
    Const,
    Type,
    As,
    True,
    False,
    Null,
    Async,
    Await,
    Gen,
    Yield,
    Extern,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    DoubleStar,
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
    Not,
    Tilde,
    Pipeline,
    At,
    DotDot,
    DotDotEq,
    Question,

    // Assignment
    Assign,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    BitAndEq,
    BitOrEq,
    BitXorEq,
    ShlEq,
    ShrEq,

    // Delimiters
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    // Punctuation
    Comma,
    Colon,
    ColonColon,
    Semicolon,
    Dot,
    Arrow,
    FatArrow,

    // Special
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TokenKind::IntLit => "int literal",
            TokenKind::FloatLit => "float literal",
            TokenKind::StringLit => "string literal",
            TokenKind::CharLit => "char literal",
            TokenKind::BoolLit => "bool literal",
            TokenKind::Ident => "identifier",
            TokenKind::Let => "let",
            TokenKind::Fn => "fn",
            TokenKind::If => "if",
            TokenKind::Else => "else",
            TokenKind::While => "while",
            TokenKind::For => "for",
            TokenKind::Return => "return",
            TokenKind::Match => "match",
            TokenKind::Struct => "struct",
            TokenKind::Enum => "enum",
            TokenKind::Trait => "trait",
            TokenKind::Impl => "impl",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Assign => "=",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::Comma => ",",
            TokenKind::Semicolon => ";",
            TokenKind::Eof => "EOF",
            _ => "<token>",
        };
        write!(f, "{s}")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S2.2: Parse Error
// ═══════════════════════════════════════════════════════════════════════

/// A parse error with source span.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// Error code (e.g., "PE001").
    pub code: String,
    /// Error message.
    pub message: String,
    /// Source span.
    pub span: AstSpan,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] at {}: {}", self.code, self.span, self.message)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S2.3: Operator Precedence Table (19 levels)
// ═══════════════════════════════════════════════════════════════════════

/// Operator precedence levels (1 = lowest, 19 = highest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Precedence(pub u8);

impl Precedence {
    pub const NONE: Self = Precedence(0);
    pub const ASSIGNMENT: Self = Precedence(1);
    pub const PIPELINE: Self = Precedence(2);
    pub const LOGICAL_OR: Self = Precedence(3);
    pub const LOGICAL_AND: Self = Precedence(4);
    pub const BITWISE_OR: Self = Precedence(5);
    pub const BITWISE_XOR: Self = Precedence(6);
    pub const BITWISE_AND: Self = Precedence(7);
    pub const EQUALITY: Self = Precedence(8);
    pub const COMPARISON: Self = Precedence(9);
    pub const RANGE: Self = Precedence(10);
    pub const SHIFT: Self = Precedence(11);
    pub const ADDITION: Self = Precedence(12);
    pub const MULTIPLY: Self = Precedence(13);
    pub const POWER: Self = Precedence(14);
    pub const CAST: Self = Precedence(15);
    pub const UNARY: Self = Precedence(16);
    pub const TRY: Self = Precedence(17);
    pub const POSTFIX: Self = Precedence(18);
    pub const PRIMARY: Self = Precedence(19);
}

/// Returns the precedence and associativity of a binary operator token.
pub fn infix_precedence(kind: &TokenKind) -> Option<(Precedence, bool)> {
    match kind {
        // (precedence, right_associative)
        TokenKind::Assign
        | TokenKind::PlusEq
        | TokenKind::MinusEq
        | TokenKind::StarEq
        | TokenKind::SlashEq
        | TokenKind::PercentEq
        | TokenKind::BitAndEq
        | TokenKind::BitOrEq
        | TokenKind::BitXorEq
        | TokenKind::ShlEq
        | TokenKind::ShrEq => Some((Precedence::ASSIGNMENT, true)),
        TokenKind::Pipeline => Some((Precedence::PIPELINE, false)),
        TokenKind::Or => Some((Precedence::LOGICAL_OR, false)),
        TokenKind::And => Some((Precedence::LOGICAL_AND, false)),
        TokenKind::BitOr => Some((Precedence::BITWISE_OR, false)),
        TokenKind::BitXor => Some((Precedence::BITWISE_XOR, false)),
        TokenKind::BitAnd => Some((Precedence::BITWISE_AND, false)),
        TokenKind::Eq | TokenKind::Ne => Some((Precedence::EQUALITY, false)),
        TokenKind::Lt | TokenKind::Le | TokenKind::Gt | TokenKind::Ge => {
            Some((Precedence::COMPARISON, false))
        }
        TokenKind::DotDot | TokenKind::DotDotEq => Some((Precedence::RANGE, false)),
        TokenKind::Shl | TokenKind::Shr => Some((Precedence::SHIFT, false)),
        TokenKind::Plus | TokenKind::Minus => Some((Precedence::ADDITION, false)),
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent | TokenKind::At => {
            Some((Precedence::MULTIPLY, false))
        }
        TokenKind::DoubleStar => Some((Precedence::POWER, true)),
        TokenKind::As => Some((Precedence::CAST, false)),
        _ => None,
    }
}

/// Maps a token kind to a BinOp.
pub fn token_to_binop(kind: &TokenKind) -> Option<BinOp> {
    match kind {
        TokenKind::Plus => Some(BinOp::Add),
        TokenKind::Minus => Some(BinOp::Sub),
        TokenKind::Star => Some(BinOp::Mul),
        TokenKind::Slash => Some(BinOp::Div),
        TokenKind::Percent => Some(BinOp::Mod),
        TokenKind::DoubleStar => Some(BinOp::Pow),
        TokenKind::Eq => Some(BinOp::Eq),
        TokenKind::Ne => Some(BinOp::Ne),
        TokenKind::Lt => Some(BinOp::Lt),
        TokenKind::Le => Some(BinOp::Le),
        TokenKind::Gt => Some(BinOp::Gt),
        TokenKind::Ge => Some(BinOp::Ge),
        TokenKind::And => Some(BinOp::And),
        TokenKind::Or => Some(BinOp::Or),
        TokenKind::BitAnd => Some(BinOp::BitAnd),
        TokenKind::BitOr => Some(BinOp::BitOr),
        TokenKind::BitXor => Some(BinOp::BitXor),
        TokenKind::Shl => Some(BinOp::Shl),
        TokenKind::Shr => Some(BinOp::Shr),
        TokenKind::Pipeline => Some(BinOp::Pipeline),
        TokenKind::At => Some(BinOp::MatMul),
        TokenKind::DotDot => Some(BinOp::Range),
        TokenKind::DotDotEq => Some(BinOp::RangeInclusive),
        _ => None,
    }
}

/// Maps a compound assignment token to the underlying BinOp.
pub fn compound_assign_op(kind: &TokenKind) -> Option<BinOp> {
    match kind {
        TokenKind::PlusEq => Some(BinOp::Add),
        TokenKind::MinusEq => Some(BinOp::Sub),
        TokenKind::StarEq => Some(BinOp::Mul),
        TokenKind::SlashEq => Some(BinOp::Div),
        TokenKind::PercentEq => Some(BinOp::Mod),
        TokenKind::BitAndEq => Some(BinOp::BitAnd),
        TokenKind::BitOrEq => Some(BinOp::BitOr),
        TokenKind::BitXorEq => Some(BinOp::BitXor),
        TokenKind::ShlEq => Some(BinOp::Shl),
        TokenKind::ShrEq => Some(BinOp::Shr),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S2.4: Parser Struct
// ═══════════════════════════════════════════════════════════════════════

/// The self-hosted Pratt parser.
pub struct ParserV2 {
    /// Token stream.
    tokens: Vec<Token>,
    /// Current position.
    pos: usize,
    /// Collected errors.
    errors: Vec<ParseError>,
    /// Sentinel EOF token (avoids returning references to temporaries).
    eof_sentinel: Token,
}

impl ParserV2 {
    /// Creates a new parser from a token stream.
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
            eof_sentinel: Token {
                kind: TokenKind::Eof,
                text: String::new(),
                span: AstSpan::dummy(),
            },
        }
    }

    /// Returns collected parse errors.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// Returns the current token.
    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&self.eof_sentinel)
    }

    /// Returns the current token kind.
    fn peek_kind(&self) -> TokenKind {
        self.peek().kind.clone()
    }

    /// Advances to the next token and returns the consumed one.
    fn advance(&mut self) -> Token {
        let tok = self.peek().clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    /// Checks if the current token matches the given kind.
    fn check(&self, kind: &TokenKind) -> bool {
        &self.peek_kind() == kind
    }

    /// Consumes a token of the given kind, or records an error.
    fn expect(&mut self, kind: &TokenKind) -> Option<Token> {
        if self.check(kind) {
            Some(self.advance())
        } else {
            self.errors.push(ParseError {
                code: "PE001".into(),
                message: format!("expected {kind}, found {}", self.peek_kind()),
                span: self.peek().span,
            });
            None
        }
    }

    /// Tries to match and consume a token.
    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Gets the current span.
    fn current_span(&self) -> AstSpan {
        self.peek().span
    }

    // ═══════════════════════════════════════════════════════════════════
    // S2.5: Program & Item Parsing
    // ═══════════════════════════════════════════════════════════════════

    /// Parses a complete program.
    pub fn parse_program(&mut self, filename: &str) -> AstProgram {
        let mut items = Vec::new();
        while !self.check(&TokenKind::Eof) {
            if let Some(item) = self.parse_item() {
                items.push(item);
            } else {
                // Error recovery: skip to next statement boundary.
                self.advance();
            }
        }
        AstProgram::new(filename, items)
    }

    /// Parses a top-level item.
    fn parse_item(&mut self) -> Option<Item> {
        let is_pub = self.eat(&TokenKind::Pub);
        match self.peek_kind() {
            TokenKind::Fn | TokenKind::Async | TokenKind::Gen => {
                self.parse_fn_def(is_pub).map(Item::FnDef)
            }
            TokenKind::Struct => self.parse_struct_def(is_pub).map(Item::StructDef),
            TokenKind::Enum => self.parse_enum_def(is_pub).map(Item::EnumDef),
            TokenKind::Impl => self.parse_impl_block().map(Item::ImplBlock),
            TokenKind::Trait => self.parse_trait_def(is_pub).map(Item::TraitDef),
            TokenKind::Use => self.parse_use_decl().map(Item::Use),
            TokenKind::Mod => self.parse_mod_decl().map(Item::Mod),
            TokenKind::Const => {
                let span = self.current_span();
                self.advance(); // consume 'const'
                let name_tok = self.advance();
                let ty = if self.eat(&TokenKind::Colon) {
                    Some(self.parse_type_expr())
                } else {
                    None
                };
                self.expect(&TokenKind::Assign);
                let value = self.parse_expr(Precedence::NONE);
                self.eat(&TokenKind::Semicolon);
                Some(Item::Const {
                    name: name_tok.text,
                    ty,
                    value: Box::new(value),
                    is_pub,
                    span,
                })
            }
            _ => {
                let stmt = self.parse_stmt()?;
                Some(Item::Stmt(stmt))
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // S2.6: Statement Parsing
    // ═══════════════════════════════════════════════════════════════════

    /// Parses a statement.
    fn parse_stmt(&mut self) -> Option<Stmt> {
        match self.peek_kind() {
            TokenKind::Let => self.parse_let_stmt(),
            TokenKind::Return => {
                let span = self.current_span();
                self.advance();
                let value = if !self.check(&TokenKind::Semicolon) && !self.check(&TokenKind::RBrace)
                {
                    Some(Box::new(self.parse_expr(Precedence::NONE)))
                } else {
                    None
                };
                self.eat(&TokenKind::Semicolon);
                Some(Stmt::Return { value, span })
            }
            TokenKind::Break => {
                let span = self.current_span();
                self.advance();
                self.eat(&TokenKind::Semicolon);
                Some(Stmt::Break { span })
            }
            TokenKind::Continue => {
                let span = self.current_span();
                self.advance();
                self.eat(&TokenKind::Semicolon);
                Some(Stmt::Continue { span })
            }
            TokenKind::While => self.parse_while_stmt(),
            TokenKind::For => self.parse_for_stmt(),
            _ => {
                let span = self.current_span();
                let expr = self.parse_expr(Precedence::NONE);
                self.eat(&TokenKind::Semicolon);
                Some(Stmt::ExprStmt {
                    expr: Box::new(expr),
                    span,
                })
            }
        }
    }

    /// Parses a let statement.
    fn parse_let_stmt(&mut self) -> Option<Stmt> {
        let span = self.current_span();
        self.expect(&TokenKind::Let)?;
        let mutable = self.eat(&TokenKind::Mut);
        let name_tok = self.advance();
        let ty = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type_expr())
        } else {
            None
        };
        let init = if self.eat(&TokenKind::Assign) {
            Some(Box::new(self.parse_expr(Precedence::NONE)))
        } else {
            None
        };
        self.eat(&TokenKind::Semicolon);
        Some(Stmt::Let {
            name: name_tok.text,
            mutable,
            ty,
            init,
            span,
        })
    }

    /// Parses a while statement.
    fn parse_while_stmt(&mut self) -> Option<Stmt> {
        let span = self.current_span();
        self.expect(&TokenKind::While)?;
        let condition = self.parse_expr(Precedence::NONE);
        let body = self.parse_block_expr();
        Some(Stmt::While {
            condition: Box::new(condition),
            body: Box::new(body),
            span,
        })
    }

    /// Parses a for statement.
    fn parse_for_stmt(&mut self) -> Option<Stmt> {
        let span = self.current_span();
        self.expect(&TokenKind::For)?;
        let name_tok = self.advance();
        self.expect(&TokenKind::In);
        let iter = self.parse_expr(Precedence::NONE);
        let body = self.parse_block_expr();
        Some(Stmt::For {
            name: name_tok.text,
            iter: Box::new(iter),
            body: Box::new(body),
            span,
        })
    }

    // ═══════════════════════════════════════════════════════════════════
    // S2.7: Expression Parsing (Pratt Algorithm)
    // ═══════════════════════════════════════════════════════════════════

    /// Pratt expression parser entry point.
    pub fn parse_expr(&mut self, min_prec: Precedence) -> Expr {
        let mut left = self.parse_prefix();

        loop {
            // Postfix: try, method call, field access, index
            match self.peek_kind() {
                TokenKind::Question => {
                    let span = self.current_span();
                    self.advance();
                    left = Expr::Try {
                        expr: Box::new(left),
                        span,
                    };
                    continue;
                }
                TokenKind::Dot => {
                    self.advance();
                    let field_tok = self.advance();
                    if self.check(&TokenKind::LParen) {
                        // Method call
                        let span = self.current_span();
                        self.advance(); // consume '('
                        let args = self.parse_args_list();
                        self.expect(&TokenKind::RParen);
                        left = Expr::MethodCall {
                            object: Box::new(left),
                            method: field_tok.text,
                            args,
                            span,
                        };
                    } else {
                        let span = field_tok.span;
                        left = Expr::FieldAccess {
                            object: Box::new(left),
                            field: field_tok.text,
                            span,
                        };
                    }
                    continue;
                }
                TokenKind::LBracket => {
                    let span = self.current_span();
                    self.advance(); // consume '['
                    let index = self.parse_expr(Precedence::NONE);
                    self.expect(&TokenKind::RBracket);
                    left = Expr::Index {
                        object: Box::new(left),
                        index: Box::new(index),
                        span,
                    };
                    continue;
                }
                TokenKind::LParen if min_prec.0 <= Precedence::POSTFIX.0 => {
                    // Function call (only if callee position)
                    let span = self.current_span();
                    self.advance(); // consume '('
                    let args = self.parse_args_list();
                    self.expect(&TokenKind::RParen);
                    left = Expr::Call {
                        callee: Box::new(left),
                        args,
                        span,
                    };
                    continue;
                }
                _ => {}
            }

            // Infix operators
            let Some((prec, right_assoc)) = infix_precedence(&self.peek_kind()) else {
                break;
            };
            if prec < min_prec {
                break;
            }

            let op_tok = self.advance();
            let next_prec = if right_assoc {
                Precedence(prec.0)
            } else {
                Precedence(prec.0 + 1)
            };

            // Handle assignment and compound assignment
            if op_tok.kind == TokenKind::Assign {
                let value = self.parse_expr(next_prec);
                let span = AstSpan::merge(&left.span(), &value.span());
                left = Expr::Assign {
                    target: Box::new(left),
                    value: Box::new(value),
                    span,
                };
                continue;
            }

            if let Some(compound_op) = compound_assign_op(&op_tok.kind) {
                let value = self.parse_expr(next_prec);
                let span = AstSpan::merge(&left.span(), &value.span());
                left = Expr::CompoundAssign {
                    op: compound_op,
                    target: Box::new(left),
                    value: Box::new(value),
                    span,
                };
                continue;
            }

            // Cast: `expr as Type`
            if op_tok.kind == TokenKind::As {
                let ty = self.parse_type_expr();
                let span = AstSpan::merge(&left.span(), &ty.span());
                left = Expr::Cast {
                    expr: Box::new(left),
                    ty,
                    span,
                };
                continue;
            }

            // Regular binary op
            if let Some(op) = token_to_binop(&op_tok.kind) {
                let right = self.parse_expr(next_prec);
                let span = AstSpan::merge(&left.span(), &right.span());
                left = Expr::BinOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                    span,
                };
            }
        }

        left
    }

    /// Parses a prefix expression (unary, literals, grouping).
    fn parse_prefix(&mut self) -> Expr {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::IntLit => {
                self.advance();
                Expr::IntLit {
                    value: tok.text.parse().unwrap_or(0),
                    span: tok.span,
                }
            }
            TokenKind::FloatLit => {
                self.advance();
                Expr::FloatLit {
                    value: tok.text.parse().unwrap_or(0.0),
                    span: tok.span,
                }
            }
            TokenKind::StringLit => {
                self.advance();
                Expr::StringLit {
                    value: tok.text.clone(),
                    span: tok.span,
                }
            }
            TokenKind::CharLit => {
                self.advance();
                Expr::CharLit {
                    value: tok.text.chars().next().unwrap_or('\0'),
                    span: tok.span,
                }
            }
            TokenKind::True | TokenKind::False | TokenKind::BoolLit => {
                self.advance();
                Expr::BoolLit {
                    value: tok.text == "true",
                    span: tok.span,
                }
            }
            TokenKind::Null => {
                self.advance();
                Expr::NullLit { span: tok.span }
            }
            TokenKind::Ident => {
                self.advance();
                // Check for path expression (a::b::c)
                if self.check(&TokenKind::ColonColon) {
                    let mut segments = vec![tok.text.clone()];
                    while self.eat(&TokenKind::ColonColon) {
                        let seg = self.advance();
                        segments.push(seg.text);
                    }
                    Expr::Path {
                        segments,
                        span: tok.span,
                    }
                } else {
                    Expr::Ident {
                        name: tok.text,
                        span: tok.span,
                    }
                }
            }
            TokenKind::Minus => {
                self.advance();
                let operand = self.parse_expr(Precedence::UNARY);
                let span = AstSpan::merge(&tok.span, &operand.span());
                Expr::UnaryOp {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                    span,
                }
            }
            TokenKind::Not => {
                self.advance();
                let operand = self.parse_expr(Precedence::UNARY);
                let span = AstSpan::merge(&tok.span, &operand.span());
                Expr::UnaryOp {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                    span,
                }
            }
            TokenKind::BitAnd => {
                self.advance();
                let mutable = self.eat(&TokenKind::Mut);
                let operand = self.parse_expr(Precedence::UNARY);
                let span = AstSpan::merge(&tok.span, &operand.span());
                Expr::UnaryOp {
                    op: if mutable {
                        UnaryOp::RefMut
                    } else {
                        UnaryOp::Ref
                    },
                    operand: Box::new(operand),
                    span,
                }
            }
            TokenKind::LParen => {
                self.advance();
                if self.check(&TokenKind::RParen) {
                    self.advance();
                    return Expr::TupleLit {
                        elements: vec![],
                        span: tok.span,
                    };
                }
                let expr = self.parse_expr(Precedence::NONE);
                if self.check(&TokenKind::Comma) {
                    // Tuple literal
                    let mut elements = vec![expr];
                    while self.eat(&TokenKind::Comma) {
                        if self.check(&TokenKind::RParen) {
                            break;
                        }
                        elements.push(self.parse_expr(Precedence::NONE));
                    }
                    self.expect(&TokenKind::RParen);
                    Expr::TupleLit {
                        elements,
                        span: tok.span,
                    }
                } else {
                    self.expect(&TokenKind::RParen);
                    expr // Grouping
                }
            }
            TokenKind::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                while !self.check(&TokenKind::RBracket) && !self.check(&TokenKind::Eof) {
                    elements.push(self.parse_expr(Precedence::NONE));
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RBracket);
                Expr::ArrayLit {
                    elements,
                    span: tok.span,
                }
            }
            TokenKind::LBrace => self.parse_block_expr(),
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::Yield => {
                self.advance();
                let value = if !self.check(&TokenKind::Semicolon) && !self.check(&TokenKind::RBrace)
                {
                    Some(Box::new(self.parse_expr(Precedence::NONE)))
                } else {
                    None
                };
                Expr::Yield {
                    value,
                    span: tok.span,
                }
            }
            _ => {
                self.errors.push(ParseError {
                    code: "PE002".into(),
                    message: format!("unexpected token: {}", tok.kind),
                    span: tok.span,
                });
                self.advance();
                Expr::NullLit { span: tok.span }
            }
        }
    }

    /// Parses a block expression.
    fn parse_block_expr(&mut self) -> Expr {
        let span = self.current_span();
        self.expect(&TokenKind::LBrace);
        let mut stmts = Vec::new();
        let mut trailing_expr = None;

        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            if let Some(stmt) = self.parse_stmt() {
                stmts.push(stmt);
            } else {
                break;
            }
        }

        // Check if last statement is an expression without semicolon
        if let Some(Stmt::ExprStmt { expr, .. }) = stmts.last() {
            let expr_clone = expr.clone();
            stmts.pop();
            trailing_expr = Some(expr_clone);
        }

        self.expect(&TokenKind::RBrace);
        Expr::Block {
            stmts,
            expr: trailing_expr,
            span,
        }
    }

    /// Parses an if expression.
    fn parse_if_expr(&mut self) -> Expr {
        let span = self.current_span();
        self.expect(&TokenKind::If);
        let condition = self.parse_expr(Precedence::NONE);
        let then_branch = self.parse_block_expr();
        let else_branch = if self.eat(&TokenKind::Else) {
            if self.check(&TokenKind::If) {
                Some(Box::new(self.parse_if_expr()))
            } else {
                Some(Box::new(self.parse_block_expr()))
            }
        } else {
            None
        };
        Expr::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch,
            span,
        }
    }

    /// Parses a match expression.
    fn parse_match_expr(&mut self) -> Expr {
        let span = self.current_span();
        self.expect(&TokenKind::Match);
        let scrutinee = self.parse_expr(Precedence::NONE);
        self.expect(&TokenKind::LBrace);
        let mut arms = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let arm_span = self.current_span();
            let pattern = self.parse_pattern();
            self.expect(&TokenKind::FatArrow);
            let body = self.parse_expr(Precedence::NONE);
            self.eat(&TokenKind::Comma);
            arms.push(MatchArm {
                pattern,
                guard: None,
                body: Box::new(body),
                span: arm_span,
            });
        }
        self.expect(&TokenKind::RBrace);
        Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
            span,
        }
    }

    /// Parses a comma-separated argument list.
    fn parse_args_list(&mut self) -> Vec<Expr> {
        let mut args = Vec::new();
        while !self.check(&TokenKind::RParen) && !self.check(&TokenKind::Eof) {
            args.push(self.parse_expr(Precedence::NONE));
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        args
    }

    // ═══════════════════════════════════════════════════════════════════
    // S2.8: Type Expression Parsing
    // ═══════════════════════════════════════════════════════════════════

    /// Parses a type expression.
    pub fn parse_type_expr(&mut self) -> TypeExpr {
        let span = self.current_span();
        match self.peek_kind() {
            TokenKind::BitAnd => {
                self.advance();
                let mutable = self.eat(&TokenKind::Mut);
                let inner = self.parse_type_expr();
                TypeExpr::Ref(Box::new(inner), mutable, span)
            }
            TokenKind::LBracket => {
                self.advance();
                let inner = self.parse_type_expr();
                let len = if self.eat(&TokenKind::Semicolon) {
                    let len_tok = self.advance();
                    Some(len_tok.text.parse().unwrap_or(0))
                } else {
                    None
                };
                self.expect(&TokenKind::RBracket);
                TypeExpr::Array(Box::new(inner), len, span)
            }
            TokenKind::LParen => {
                self.advance();
                let mut elems = Vec::new();
                while !self.check(&TokenKind::RParen) && !self.check(&TokenKind::Eof) {
                    elems.push(self.parse_type_expr());
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen);
                TypeExpr::Tuple(elems, span)
            }
            TokenKind::Fn => {
                self.advance();
                self.expect(&TokenKind::LParen);
                let mut params = Vec::new();
                while !self.check(&TokenKind::RParen) && !self.check(&TokenKind::Eof) {
                    params.push(self.parse_type_expr());
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen);
                self.expect(&TokenKind::Arrow);
                let ret = self.parse_type_expr();
                TypeExpr::Fn(params, Box::new(ret), span)
            }
            _ => {
                let name_tok = self.advance();
                if name_tok.text == "never" || name_tok.text == "!" {
                    return TypeExpr::Never(span);
                }
                if name_tok.text == "_" {
                    return TypeExpr::Inferred(span);
                }
                if self.check(&TokenKind::Lt) {
                    self.advance(); // consume '<'
                    let mut params = Vec::new();
                    while !self.check(&TokenKind::Gt) && !self.check(&TokenKind::Eof) {
                        params.push(self.parse_type_expr());
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::Gt);
                    TypeExpr::Generic(name_tok.text, params, span)
                } else {
                    TypeExpr::Name(name_tok.text, span)
                }
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // S2.9: Pattern Parsing
    // ═══════════════════════════════════════════════════════════════════

    /// Parses a pattern.
    pub fn parse_pattern(&mut self) -> Pattern {
        let span = self.current_span();
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Ident => {
                self.advance();
                if tok.text == "_" {
                    Pattern::Wildcard(span)
                } else if self.check(&TokenKind::LParen) {
                    // Enum variant: Some(x)
                    self.advance();
                    let inner = if !self.check(&TokenKind::RParen) {
                        Some(Box::new(self.parse_pattern()))
                    } else {
                        None
                    };
                    self.expect(&TokenKind::RParen);
                    Pattern::Enum(tok.text, None, inner, span)
                } else if self.check(&TokenKind::LBrace) {
                    // Struct pattern: Point { x, y }
                    self.advance();
                    let mut fields = Vec::new();
                    while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
                        let field_tok = self.advance();
                        let pat = if self.eat(&TokenKind::Colon) {
                            Some(self.parse_pattern())
                        } else {
                            None
                        };
                        fields.push((field_tok.text, pat));
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RBrace);
                    Pattern::Struct(tok.text, fields, span)
                } else {
                    Pattern::Ident(tok.text, span)
                }
            }
            TokenKind::LParen => {
                self.advance();
                let mut pats = Vec::new();
                while !self.check(&TokenKind::RParen) && !self.check(&TokenKind::Eof) {
                    pats.push(self.parse_pattern());
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen);
                Pattern::Tuple(pats, span)
            }
            TokenKind::DotDot => {
                self.advance();
                Pattern::Rest(span)
            }
            TokenKind::IntLit | TokenKind::StringLit | TokenKind::True | TokenKind::False => {
                let expr = self.parse_prefix();
                Pattern::Literal(Box::new(expr), span)
            }
            _ => {
                self.errors.push(ParseError {
                    code: "PE003".into(),
                    message: format!("unexpected token in pattern: {}", tok.kind),
                    span,
                });
                self.advance();
                Pattern::Wildcard(span)
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // S2.10: Definition Parsing (fn, struct, enum, impl, trait, use, mod)
    // ═══════════════════════════════════════════════════════════════════

    /// Parses a function definition.
    fn parse_fn_def(&mut self, is_pub: bool) -> Option<FnDefNode> {
        let span = self.current_span();
        let is_async = self.eat(&TokenKind::Async);
        let is_gen = self.eat(&TokenKind::Gen);
        self.expect(&TokenKind::Fn)?;
        let name_tok = self.advance();

        // Generic parameters
        let type_params = if self.eat(&TokenKind::Lt) {
            let mut params = Vec::new();
            while !self.check(&TokenKind::Gt) && !self.check(&TokenKind::Eof) {
                let p = self.advance();
                params.push(p.text);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Gt);
            params
        } else {
            vec![]
        };

        self.expect(&TokenKind::LParen);
        let mut params = Vec::new();
        while !self.check(&TokenKind::RParen) && !self.check(&TokenKind::Eof) {
            let mutable = self.eat(&TokenKind::Mut);
            let pname_tok = self.advance();
            self.expect(&TokenKind::Colon);
            let ty = self.parse_type_expr();
            params.push(Param {
                name: pname_tok.text,
                ty,
                mutable,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen);

        let ret_type = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type_expr())
        } else {
            None
        };

        let body = self.parse_block_expr();

        Some(FnDefNode {
            name: name_tok.text,
            type_params,
            params,
            ret_type,
            body: Box::new(body),
            is_pub,
            context: None,
            is_async,
            is_gen,
            span,
        })
    }

    /// Parses a struct definition.
    fn parse_struct_def(&mut self, is_pub: bool) -> Option<StructDefNode> {
        let span = self.current_span();
        self.expect(&TokenKind::Struct)?;
        let name_tok = self.advance();

        let type_params = if self.eat(&TokenKind::Lt) {
            let mut params = Vec::new();
            while !self.check(&TokenKind::Gt) && !self.check(&TokenKind::Eof) {
                let p = self.advance();
                params.push(p.text);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Gt);
            params
        } else {
            vec![]
        };

        self.expect(&TokenKind::LBrace);
        let mut fields = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let field_pub = self.eat(&TokenKind::Pub);
            let fname_tok = self.advance();
            self.expect(&TokenKind::Colon);
            let ty = self.parse_type_expr();
            fields.push((fname_tok.text, ty, field_pub));
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace);

        Some(StructDefNode {
            name: name_tok.text,
            type_params,
            fields,
            is_pub,
            span,
        })
    }

    /// Parses an enum definition.
    fn parse_enum_def(&mut self, is_pub: bool) -> Option<EnumDefNode> {
        let span = self.current_span();
        self.expect(&TokenKind::Enum)?;
        let name_tok = self.advance();

        let type_params = if self.eat(&TokenKind::Lt) {
            let mut params = Vec::new();
            while !self.check(&TokenKind::Gt) && !self.check(&TokenKind::Eof) {
                let p = self.advance();
                params.push(p.text);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Gt);
            params
        } else {
            vec![]
        };

        self.expect(&TokenKind::LBrace);
        let mut variants = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let variant_tok = self.advance();
            let data = if self.eat(&TokenKind::LParen) {
                let mut types = Vec::new();
                while !self.check(&TokenKind::RParen) && !self.check(&TokenKind::Eof) {
                    types.push(self.parse_type_expr());
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen);
                types
            } else {
                vec![]
            };
            variants.push((variant_tok.text, data));
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace);

        Some(EnumDefNode {
            name: name_tok.text,
            type_params,
            variants,
            is_pub,
            span,
        })
    }

    /// Parses an impl block.
    fn parse_impl_block(&mut self) -> Option<ImplBlockNode> {
        let span = self.current_span();
        self.expect(&TokenKind::Impl)?;
        let first_tok = self.advance();

        let (target, trait_name) = if self.eat(&TokenKind::For) {
            let target_tok = self.advance();
            (target_tok.text, Some(first_tok.text))
        } else {
            (first_tok.text, None)
        };

        self.expect(&TokenKind::LBrace);
        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let is_pub = self.eat(&TokenKind::Pub);
            if let Some(f) = self.parse_fn_def(is_pub) {
                methods.push(f);
            }
        }
        self.expect(&TokenKind::RBrace);

        Some(ImplBlockNode {
            target,
            trait_name,
            methods,
            span,
        })
    }

    /// Parses a trait definition.
    fn parse_trait_def(&mut self, is_pub: bool) -> Option<TraitDefNode> {
        let span = self.current_span();
        self.expect(&TokenKind::Trait)?;
        let name_tok = self.advance();

        let type_params = if self.eat(&TokenKind::Lt) {
            let mut params = Vec::new();
            while !self.check(&TokenKind::Gt) && !self.check(&TokenKind::Eof) {
                let p = self.advance();
                params.push(p.text);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Gt);
            params
        } else {
            vec![]
        };

        self.expect(&TokenKind::LBrace);
        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let m_pub = self.eat(&TokenKind::Pub);
            if let Some(f) = self.parse_fn_def(m_pub) {
                methods.push(f);
            }
        }
        self.expect(&TokenKind::RBrace);

        Some(TraitDefNode {
            name: name_tok.text,
            type_params,
            methods,
            is_pub,
            span,
        })
    }

    /// Parses a use declaration.
    fn parse_use_decl(&mut self) -> Option<UseNode> {
        let span = self.current_span();
        self.expect(&TokenKind::Use)?;
        let mut path = Vec::new();
        let first = self.advance();
        path.push(first.text);
        while self.eat(&TokenKind::ColonColon) {
            let seg = self.advance();
            path.push(seg.text);
        }
        let alias = if self.eat(&TokenKind::As) {
            let alias_tok = self.advance();
            Some(alias_tok.text)
        } else {
            None
        };
        self.eat(&TokenKind::Semicolon);
        Some(UseNode { path, alias, span })
    }

    /// Parses a module declaration.
    fn parse_mod_decl(&mut self) -> Option<ModNode> {
        let span = self.current_span();
        self.expect(&TokenKind::Mod)?;
        let name_tok = self.advance();

        let items = if self.check(&TokenKind::LBrace) {
            self.advance();
            let mut mod_items = Vec::new();
            while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
                if let Some(item) = self.parse_item() {
                    mod_items.push(item);
                } else {
                    self.advance();
                }
            }
            self.expect(&TokenKind::RBrace);
            Some(mod_items)
        } else {
            self.eat(&TokenKind::Semicolon);
            None
        };

        Some(ModNode {
            name: name_tok.text,
            items,
            span,
        })
    }
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

    fn tok(kind: TokenKind, text: &str) -> Token {
        Token {
            kind,
            text: text.into(),
            span: span(),
        }
    }

    fn eof() -> Token {
        tok(TokenKind::Eof, "")
    }

    // S2.1 — Token kinds
    #[test]
    fn s2_1_token_kind_display() {
        assert_eq!(TokenKind::Plus.to_string(), "+");
        assert_eq!(TokenKind::Fn.to_string(), "fn");
        assert_eq!(TokenKind::Eof.to_string(), "EOF");
    }

    // S2.2 — Parse error
    #[test]
    fn s2_2_parse_error_display() {
        let err = ParseError {
            code: "PE001".into(),
            message: "expected }".into(),
            span: AstSpan::new(10, 11, 2, 5),
        };
        assert!(err.to_string().contains("PE001"));
        assert!(err.to_string().contains("2:5"));
    }

    // S2.3 — Precedence table
    #[test]
    fn s2_3_precedence_ordering() {
        assert!(Precedence::PRIMARY > Precedence::ADDITION);
        assert!(Precedence::MULTIPLY > Precedence::ADDITION);
        assert!(Precedence::POWER > Precedence::MULTIPLY);
        assert!(Precedence::UNARY > Precedence::POWER);
    }

    #[test]
    fn s2_3_infix_precedence_lookup() {
        assert_eq!(
            infix_precedence(&TokenKind::Plus),
            Some((Precedence::ADDITION, false))
        );
        assert_eq!(
            infix_precedence(&TokenKind::Star),
            Some((Precedence::MULTIPLY, false))
        );
        assert_eq!(
            infix_precedence(&TokenKind::DoubleStar),
            Some((Precedence::POWER, true)) // right assoc
        );
        assert!(infix_precedence(&TokenKind::LParen).is_none());
    }

    #[test]
    fn s2_3_token_to_binop_mapping() {
        assert_eq!(token_to_binop(&TokenKind::Plus), Some(BinOp::Add));
        assert_eq!(token_to_binop(&TokenKind::Star), Some(BinOp::Mul));
        assert_eq!(token_to_binop(&TokenKind::Pipeline), Some(BinOp::Pipeline));
        assert!(token_to_binop(&TokenKind::LParen).is_none());
    }

    // S2.4 — Parser construction
    #[test]
    fn s2_4_parser_new() {
        let parser = ParserV2::new(vec![eof()]);
        assert!(parser.errors().is_empty());
    }

    // S2.5 — Program parsing
    #[test]
    fn s2_5_parse_empty_program() {
        let mut parser = ParserV2::new(vec![eof()]);
        let prog = parser.parse_program("test.fj");
        assert_eq!(prog.item_count(), 0);
        assert_eq!(prog.filename, "test.fj");
    }

    // S2.6 — Statement parsing
    #[test]
    fn s2_6_parse_let_stmt() {
        let tokens = vec![
            tok(TokenKind::Let, "let"),
            tok(TokenKind::Mut, "mut"),
            tok(TokenKind::Ident, "x"),
            tok(TokenKind::Colon, ":"),
            tok(TokenKind::Ident, "i32"),
            tok(TokenKind::Assign, "="),
            tok(TokenKind::IntLit, "42"),
            tok(TokenKind::Semicolon, ";"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let stmt = parser.parse_stmt().expect("should parse let");
        match stmt {
            Stmt::Let {
                name, mutable, ty, ..
            } => {
                assert_eq!(name, "x");
                assert!(mutable);
                assert!(ty.is_some());
            }
            _ => panic!("expected Let statement"),
        }
    }

    #[test]
    fn s2_6_parse_return_stmt() {
        let tokens = vec![
            tok(TokenKind::Return, "return"),
            tok(TokenKind::IntLit, "42"),
            tok(TokenKind::Semicolon, ";"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let stmt = parser.parse_stmt().expect("should parse return");
        match stmt {
            Stmt::Return { value, .. } => assert!(value.is_some()),
            _ => panic!("expected Return"),
        }
    }

    // S2.7 — Expression parsing (Pratt)
    #[test]
    fn s2_7_parse_int_literal() {
        let tokens = vec![tok(TokenKind::IntLit, "42"), eof()];
        let mut parser = ParserV2::new(tokens);
        let expr = parser.parse_expr(Precedence::NONE);
        match expr {
            Expr::IntLit { value, .. } => assert_eq!(value, 42),
            _ => panic!("expected IntLit"),
        }
    }

    #[test]
    fn s2_7_parse_binop_add() {
        let tokens = vec![
            tok(TokenKind::IntLit, "1"),
            tok(TokenKind::Plus, "+"),
            tok(TokenKind::IntLit, "2"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let expr = parser.parse_expr(Precedence::NONE);
        match expr {
            Expr::BinOp { op, .. } => assert_eq!(op, BinOp::Add),
            _ => panic!("expected BinOp"),
        }
    }

    #[test]
    fn s2_7_parse_precedence_mul_over_add() {
        // 1 + 2 * 3 should parse as 1 + (2 * 3)
        let tokens = vec![
            tok(TokenKind::IntLit, "1"),
            tok(TokenKind::Plus, "+"),
            tok(TokenKind::IntLit, "2"),
            tok(TokenKind::Star, "*"),
            tok(TokenKind::IntLit, "3"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let expr = parser.parse_expr(Precedence::NONE);
        match expr {
            Expr::BinOp {
                op: BinOp::Add,
                right,
                ..
            } => match *right {
                Expr::BinOp { op: BinOp::Mul, .. } => {}
                _ => panic!("expected inner Mul"),
            },
            _ => panic!("expected outer Add"),
        }
    }

    #[test]
    fn s2_7_parse_unary_neg() {
        let tokens = vec![
            tok(TokenKind::Minus, "-"),
            tok(TokenKind::IntLit, "5"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let expr = parser.parse_expr(Precedence::NONE);
        match expr {
            Expr::UnaryOp {
                op: UnaryOp::Neg, ..
            } => {}
            _ => panic!("expected UnaryOp Neg"),
        }
    }

    #[test]
    fn s2_7_parse_call() {
        let tokens = vec![
            tok(TokenKind::Ident, "foo"),
            tok(TokenKind::LParen, "("),
            tok(TokenKind::IntLit, "1"),
            tok(TokenKind::Comma, ","),
            tok(TokenKind::IntLit, "2"),
            tok(TokenKind::RParen, ")"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let expr = parser.parse_expr(Precedence::NONE);
        match expr {
            Expr::Call { args, .. } => assert_eq!(args.len(), 2),
            _ => panic!("expected Call"),
        }
    }

    // S2.8 — Type expression parsing
    #[test]
    fn s2_8_parse_simple_type() {
        let tokens = vec![tok(TokenKind::Ident, "i32"), eof()];
        let mut parser = ParserV2::new(tokens);
        let ty = parser.parse_type_expr();
        match ty {
            TypeExpr::Name(name, _) => assert_eq!(name, "i32"),
            _ => panic!("expected Name type"),
        }
    }

    #[test]
    fn s2_8_parse_generic_type() {
        let tokens = vec![
            tok(TokenKind::Ident, "Vec"),
            tok(TokenKind::Lt, "<"),
            tok(TokenKind::Ident, "i32"),
            tok(TokenKind::Gt, ">"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let ty = parser.parse_type_expr();
        match ty {
            TypeExpr::Generic(name, params, _) => {
                assert_eq!(name, "Vec");
                assert_eq!(params.len(), 1);
            }
            _ => panic!("expected Generic type"),
        }
    }

    #[test]
    fn s2_8_parse_ref_type() {
        let tokens = vec![
            tok(TokenKind::BitAnd, "&"),
            tok(TokenKind::Mut, "mut"),
            tok(TokenKind::Ident, "T"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let ty = parser.parse_type_expr();
        match ty {
            TypeExpr::Ref(_, mutable, _) => assert!(mutable),
            _ => panic!("expected Ref type"),
        }
    }

    // S2.9 — Pattern parsing
    #[test]
    fn s2_9_parse_wildcard_pattern() {
        let tokens = vec![tok(TokenKind::Ident, "_"), eof()];
        let mut parser = ParserV2::new(tokens);
        let pat = parser.parse_pattern();
        assert!(matches!(pat, Pattern::Wildcard(_)));
    }

    #[test]
    fn s2_9_parse_ident_pattern() {
        let tokens = vec![tok(TokenKind::Ident, "x"), eof()];
        let mut parser = ParserV2::new(tokens);
        let pat = parser.parse_pattern();
        match pat {
            Pattern::Ident(name, _) => assert_eq!(name, "x"),
            _ => panic!("expected Ident pattern"),
        }
    }

    #[test]
    fn s2_9_parse_enum_pattern() {
        let tokens = vec![
            tok(TokenKind::Ident, "Some"),
            tok(TokenKind::LParen, "("),
            tok(TokenKind::Ident, "x"),
            tok(TokenKind::RParen, ")"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let pat = parser.parse_pattern();
        match pat {
            Pattern::Enum(name, _, inner, _) => {
                assert_eq!(name, "Some");
                assert!(inner.is_some());
            }
            _ => panic!("expected Enum pattern"),
        }
    }

    // S2.10 — Definition parsing
    #[test]
    fn s2_10_parse_fn_def() {
        let tokens = vec![
            tok(TokenKind::Fn, "fn"),
            tok(TokenKind::Ident, "add"),
            tok(TokenKind::LParen, "("),
            tok(TokenKind::Ident, "a"),
            tok(TokenKind::Colon, ":"),
            tok(TokenKind::Ident, "i32"),
            tok(TokenKind::Comma, ","),
            tok(TokenKind::Ident, "b"),
            tok(TokenKind::Colon, ":"),
            tok(TokenKind::Ident, "i32"),
            tok(TokenKind::RParen, ")"),
            tok(TokenKind::Arrow, "->"),
            tok(TokenKind::Ident, "i32"),
            tok(TokenKind::LBrace, "{"),
            tok(TokenKind::Ident, "a"),
            tok(TokenKind::Plus, "+"),
            tok(TokenKind::Ident, "b"),
            tok(TokenKind::RBrace, "}"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let f = parser.parse_fn_def(false).expect("should parse fn");
        assert_eq!(f.name, "add");
        assert_eq!(f.params.len(), 2);
        assert!(f.ret_type.is_some());
    }

    #[test]
    fn s2_10_parse_struct_def() {
        let tokens = vec![
            tok(TokenKind::Struct, "struct"),
            tok(TokenKind::Ident, "Point"),
            tok(TokenKind::LBrace, "{"),
            tok(TokenKind::Ident, "x"),
            tok(TokenKind::Colon, ":"),
            tok(TokenKind::Ident, "f64"),
            tok(TokenKind::Comma, ","),
            tok(TokenKind::Ident, "y"),
            tok(TokenKind::Colon, ":"),
            tok(TokenKind::Ident, "f64"),
            tok(TokenKind::RBrace, "}"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let s = parser.parse_struct_def(false).expect("should parse struct");
        assert_eq!(s.name, "Point");
        assert_eq!(s.fields.len(), 2);
    }

    #[test]
    fn s2_10_parse_use_decl() {
        let tokens = vec![
            tok(TokenKind::Use, "use"),
            tok(TokenKind::Ident, "std"),
            tok(TokenKind::ColonColon, "::"),
            tok(TokenKind::Ident, "io"),
            tok(TokenKind::ColonColon, "::"),
            tok(TokenKind::Ident, "println"),
            tok(TokenKind::Semicolon, ";"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let u = parser.parse_use_decl().expect("should parse use");
        assert_eq!(u.path, vec!["std", "io", "println"]);
    }

    #[test]
    fn s2_10_error_recovery_collects_errors() {
        // A block missing its closing brace triggers an expect error.
        let tokens = vec![
            tok(TokenKind::LBrace, "{"),
            tok(TokenKind::IntLit, "42"),
            // Missing RBrace — parser should record PE001 error
            tok(TokenKind::Eof, ""),
        ];
        let mut parser = ParserV2::new(tokens);
        let _expr = parser.parse_expr(Precedence::NONE);
        assert!(!parser.errors().is_empty());
    }

    #[test]
    fn s2_10_parse_array_literal() {
        let tokens = vec![
            tok(TokenKind::LBracket, "["),
            tok(TokenKind::IntLit, "1"),
            tok(TokenKind::Comma, ","),
            tok(TokenKind::IntLit, "2"),
            tok(TokenKind::Comma, ","),
            tok(TokenKind::IntLit, "3"),
            tok(TokenKind::RBracket, "]"),
            eof(),
        ];
        let mut parser = ParserV2::new(tokens);
        let expr = parser.parse_expr(Precedence::NONE);
        match expr {
            Expr::ArrayLit { elements, .. } => assert_eq!(elements.len(), 3),
            _ => panic!("expected ArrayLit"),
        }
    }
}
