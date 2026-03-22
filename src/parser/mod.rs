//! Fajar Lang parser — converts tokens to AST.
//!
//! Entry point: [`parse`] takes `Vec<Token>` and returns `Result<Program, Vec<ParseError>>`.
//! Uses recursive descent for declarations/statements and Pratt parsing for expressions.
//!
//! Split into submodules:
//! - `items.rs` — item and statement parsing (fn, struct, enum, trait, impl, let, return)
//! - `expr.rs` — expression parsing (Pratt parser, literals, calls, blocks, control flow)
//!
//! # Example
//!
//! ```
//! use fajar_lang::lexer::tokenize;
//! use fajar_lang::parser::parse;
//!
//! let tokens = tokenize("let x = 42").unwrap();
//! let program = parse(tokens).unwrap();
//! ```

pub mod ast;
mod expr;
mod items;
pub mod macros;
pub mod pratt;
pub mod recovery;

use ast::*;
use pratt::*;

use crate::lexer::token::{Span, Token, TokenKind};
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// ParseError
// ═══════════════════════════════════════════════════════════════════════

/// Errors produced during parsing.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum ParseError {
    /// Expected a specific token but found something else (PE001).
    #[error("[PE001] expected {expected}, found {found} at {line}:{col}")]
    UnexpectedToken {
        /// What was expected.
        expected: String,
        /// What was found.
        found: String,
        /// Line number.
        line: u32,
        /// Column number.
        col: u32,
        /// Source span.
        span: Span,
    },

    /// Expected an expression but found something else (PE002).
    #[error("[PE002] expected expression at {line}:{col}")]
    ExpectedExpression {
        /// Line number.
        line: u32,
        /// Column number.
        col: u32,
        /// Source span.
        span: Span,
    },

    /// Expected a type expression (PE003).
    #[error("[PE003] expected type at {line}:{col}")]
    ExpectedType {
        /// Line number.
        line: u32,
        /// Column number.
        col: u32,
        /// Source span.
        span: Span,
    },

    /// Expected a pattern (PE004).
    #[error("[PE004] expected pattern at {line}:{col}")]
    ExpectedPattern {
        /// Line number.
        line: u32,
        /// Column number.
        col: u32,
        /// Source span.
        span: Span,
    },

    /// Expected an identifier (PE005).
    #[error("[PE005] expected identifier at {line}:{col}, found {found}")]
    ExpectedIdentifier {
        /// What was found instead.
        found: String,
        /// Line number.
        line: u32,
        /// Column number.
        col: u32,
        /// Source span.
        span: Span,
    },

    /// Unexpected end of file (PE006).
    #[error("[PE006] unexpected end of file")]
    UnexpectedEof {
        /// Source span.
        span: Span,
    },

    /// Invalid pattern in match expression (PE007).
    #[error("[PE007] invalid pattern at {line}:{col}")]
    InvalidPattern {
        /// Line number.
        line: u32,
        /// Column number.
        col: u32,
        /// Source span.
        span: Span,
    },

    /// Duplicate field in struct initialization (PE008).
    #[error("[PE008] duplicate field '{field}' at {line}:{col}")]
    DuplicateField {
        /// The duplicated field name.
        field: String,
        /// Line number.
        line: u32,
        /// Column number.
        col: u32,
        /// Source span.
        span: Span,
    },

    /// Trailing separator (PE009) — warning-level.
    #[error("[PE009] trailing separator at {line}:{col}")]
    TrailingSeparator {
        /// Line number.
        line: u32,
        /// Column number.
        col: u32,
        /// Source span.
        span: Span,
    },

    /// Invalid annotation (PE010).
    #[error("[PE010] invalid annotation at {line}:{col}")]
    InvalidAnnotation {
        /// Line number.
        line: u32,
        /// Column number.
        col: u32,
        /// Source span.
        span: Span,
    },

    /// Module file not found (PE011).
    #[error("[PE011] module file not found: {path}")]
    ModuleFileNotFound {
        /// The file path that was searched for.
        path: String,
        /// Source span.
        span: Span,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════════

/// Parses a token stream into a [`Program`] AST.
///
/// # Arguments
///
/// * `tokens` - Token stream from the lexer (must end with `TokenKind::Eof`).
///
/// # Returns
///
/// * `Ok(Program)` - The complete AST.
/// * `Err(Vec<ParseError>)` - All parse errors encountered.
///
/// # Examples
///
/// ```
/// use fajar_lang::lexer::tokenize;
/// use fajar_lang::parser::parse;
///
/// let tokens = tokenize("let x: i64 = 42").unwrap();
/// let program = parse(tokens).unwrap();
/// assert_eq!(program.items.len(), 1);
/// ```
pub fn parse(tokens: Vec<Token>) -> Result<Program, Vec<ParseError>> {
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program();

    if parser.errors.is_empty() {
        Ok(program)
    } else {
        Err(parser.errors)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Parser struct
// ═══════════════════════════════════════════════════════════════════════

/// The parser state.
struct Parser {
    /// Token stream from lexer.
    tokens: Vec<Token>,
    /// Current position in the token stream.
    pos: usize,
    /// Collected parse errors.
    errors: Vec<ParseError>,
    /// Pending statements to inject after the current statement (for desugaring).
    pending_stmts: Vec<Stmt>,
}

impl Parser {
    /// Creates a new parser.
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
            pending_stmts: Vec::new(),
        }
    }

    // ── Token cursor ───────────────────────────────────────────────────

    /// Returns the current token without advancing.
    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("token stream should have at least EOF")
        })
    }

    /// Returns the kind of the current token.
    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    /// Returns the token at `pos + offset` without advancing.
    fn peek_at(&self, offset: usize) -> &Token {
        self.tokens.get(self.pos + offset).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("token stream should have at least EOF")
        })
    }

    /// Advances the parser by one token and returns the consumed token.
    fn advance(&mut self) -> &Token {
        let token = self.tokens.get(self.pos).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("token stream should have at least EOF")
        });
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        token
    }

    /// Returns `true` if the current token matches the given kind.
    fn at(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.peek_kind()) == std::mem::discriminant(kind)
    }

    /// Returns `true` if at end of file.
    fn at_eof(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    /// Consumes the current token if it matches `kind`, returns `true` if consumed.
    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Expects the current token to be `kind`, consumes it and returns it.
    /// Pushes an error if it doesn't match.
    fn expect(&mut self, kind: &TokenKind) -> Result<Token, ParseError> {
        if self.at(kind) {
            Ok(self.advance().clone())
        } else {
            let token = self.peek().clone();
            let err = ParseError::UnexpectedToken {
                expected: format!("{kind}"),
                found: format!("{}", token.kind),
                line: token.line,
                col: token.col,
                span: token.span,
            };
            Err(err)
        }
    }

    /// Expects and consumes an identifier, returning its name.
    fn expect_ident(&mut self) -> Result<(String, Span), ParseError> {
        let token = self.peek().clone();
        match &token.kind {
            TokenKind::Ident(name) => {
                let name = name.clone();
                let span = token.span;
                self.advance();
                Ok((name, span))
            }
            // Allow domain-specific keywords as identifiers (contextual keywords)
            kind if Self::is_contextual_keyword(kind) => {
                let name = format!("{kind}");
                let span = token.span;
                self.advance();
                Ok((name, span))
            }
            _ => Err(ParseError::ExpectedIdentifier {
                found: format!("{}", token.kind),
                line: token.line,
                col: token.col,
                span: token.span,
            }),
        }
    }

    /// Returns true if the keyword can be used as an identifier in parameter/variable contexts.
    fn is_contextual_keyword(kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Addr
                | TokenKind::Page
                | TokenKind::Region
                | TokenKind::Ptr
                | TokenKind::Irq
                | TokenKind::Syscall
                | TokenKind::Layer
                | TokenKind::Model
                | TokenKind::Grad
        )
    }

    /// Skips tokens until reaching a synchronization point (statement boundary).
    /// Always advances at least one token to avoid infinite loops.
    /// Synchronizes on: statement-starting keywords, `;`, and `}`.
    fn synchronize(&mut self) {
        // Always advance at least one token to make progress
        if !self.at_eof() {
            self.advance();
        }
        while !self.at_eof() {
            // If we just passed a semicolon or closing brace, stop here
            if self.pos > 0 {
                let prev = &self.tokens[self.pos - 1].kind;
                if matches!(prev, TokenKind::Semi | TokenKind::RBrace) {
                    return;
                }
            }
            match self.peek_kind() {
                TokenKind::Fn
                | TokenKind::Let
                | TokenKind::Const
                | TokenKind::Struct
                | TokenKind::Union
                | TokenKind::Enum
                | TokenKind::Impl
                | TokenKind::Trait
                | TokenKind::Use
                | TokenKind::Mod
                | TokenKind::If
                | TokenKind::While
                | TokenKind::For
                | TokenKind::Return => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Optional semicolon consumption.
    fn eat_semi(&mut self) {
        self.eat(&TokenKind::Semi);
    }

    /// Returns the span of the previous token (for building spans after advancing).
    fn prev_span(&self) -> Span {
        if self.pos > 0 {
            self.tokens[self.pos - 1].span
        } else {
            Span::new(0, 0)
        }
    }

    /// Returns the line number of the previous token.
    fn prev_line(&self) -> u32 {
        if self.pos > 0 {
            self.tokens[self.pos - 1].line
        } else {
            0
        }
    }

    // ── Program ────────────────────────────────────────────────────────

    /// Parses the entire program.
    fn parse_program(&mut self) -> Program {
        let start = self.peek().span.start;
        let mut items = Vec::new();

        while !self.at_eof() {
            match self.parse_item_or_stmt() {
                Ok(item) => items.push(item),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
        }

        let end = self.prev_span().end;
        Program {
            items,
            span: Span::new(start, end),
        }
    }

    // ── Items ──────────────────────────────────────────────────────────

    /// Collects consecutive `///` doc comment tokens into a single string.
    fn collect_doc_comments(&mut self) -> Option<String> {
        let mut lines: Vec<String> = Vec::new();
        while let TokenKind::DocComment(content) = self.peek_kind() {
            lines.push(content.clone());
            self.advance();
        }
        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    /// Parses an item or a statement at the top level.
    fn parse_item_or_stmt(&mut self) -> Result<Item, ParseError> {
        // Collect leading doc comments (consecutive `///` lines)
        let doc_comment = self.collect_doc_comments();

        // Check for `pub` modifier
        let is_pub = if matches!(self.peek_kind(), TokenKind::Pub) {
            self.advance();
            true
        } else {
            false
        };

        // Collect all annotations (supports multiple: @test @should_panic fn ...)
        let mut is_test = false;
        let mut should_panic = false;
        let mut is_ignored = false;
        let annotation;
        loop {
            match self.peek_kind() {
                TokenKind::AtTest => {
                    self.advance();
                    is_test = true;
                }
                TokenKind::AtShouldPanic => {
                    self.advance();
                    should_panic = true;
                }
                TokenKind::AtIgnore => {
                    self.advance();
                    is_ignored = true;
                }
                _ => {
                    // Try non-test annotation (only one allowed)
                    annotation = self.try_parse_annotation();
                    break;
                }
            }
        }

        match self.peek_kind() {
            TokenKind::Fn => {
                let mut fndef = self.parse_fn_def(is_pub, annotation)?;
                fndef.is_test = is_test;
                fndef.should_panic = should_panic;
                fndef.is_ignored = is_ignored;
                fndef.doc_comment = doc_comment;
                Ok(Item::FnDef(fndef))
            }
            TokenKind::Async => {
                // Peek ahead: `async fn` → function def, `async {` → expression statement
                if self.pos + 1 < self.tokens.len()
                    && self.tokens[self.pos + 1].kind == TokenKind::Fn
                {
                    let mut fndef = self.parse_fn_def(is_pub, annotation)?;
                    fndef.is_test = is_test;
                    fndef.should_panic = should_panic;
                    fndef.is_ignored = is_ignored;
                    fndef.doc_comment = doc_comment;
                    Ok(Item::FnDef(fndef))
                } else {
                    // Fall through to expression statement (async block)
                    let stmt = self.parse_stmt()?;
                    Ok(Item::Stmt(stmt))
                }
            }
            TokenKind::Struct => {
                let mut sd = self.parse_struct_def(is_pub, annotation)?;
                sd.doc_comment = doc_comment;
                Ok(Item::StructDef(sd))
            }
            TokenKind::Union => {
                let mut ud = self.parse_union_def(is_pub, annotation)?;
                ud.doc_comment = doc_comment;
                Ok(Item::UnionDef(ud))
            }
            TokenKind::Enum => {
                let mut ed = self.parse_enum_def(is_pub, annotation)?;
                ed.doc_comment = doc_comment;
                Ok(Item::EnumDef(ed))
            }
            TokenKind::Impl => {
                let mut ib = self.parse_impl_block()?;
                ib.doc_comment = doc_comment;
                Ok(Item::ImplBlock(ib))
            }
            TokenKind::Trait => {
                let mut td = self.parse_trait_def(is_pub)?;
                td.doc_comment = doc_comment;
                Ok(Item::TraitDef(td))
            }
            TokenKind::Const => {
                // Check if next is `fn` → const fn
                if matches!(self.peek_at(1).kind, TokenKind::Fn) {
                    self.advance(); // consume `const`
                    let mut fd = self.parse_fn_def(is_pub, annotation)?;
                    fd.is_const = true;
                    fd.doc_comment = doc_comment;
                    Ok(Item::FnDef(fd))
                } else {
                    let mut cd = self.parse_const_def(is_pub, annotation)?;
                    cd.doc_comment = doc_comment;
                    Ok(Item::ConstDef(cd))
                }
            }
            TokenKind::Protocol => {
                // protocol Name { fn method() -> Type ... }
                // Parsed same as trait but stored as TraitDef
                self.advance(); // consume `protocol`
                let (name, _) = self.expect_ident()?;
                self.expect(&TokenKind::LBrace)?;
                let mut methods = Vec::new();
                while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                    if matches!(self.peek_kind(), TokenKind::Fn) {
                        let fndef = self.parse_fn_def(false, None)?;
                        methods.push(fndef);
                    } else {
                        self.advance();
                    }
                }
                self.expect(&TokenKind::RBrace)?;
                let end = self.prev_span().end;
                Ok(Item::TraitDef(TraitDef {
                    is_pub,
                    doc_comment,
                    name,
                    lifetime_params: vec![],
                    generic_params: vec![],
                    methods,
                    span: Span::new(0, end),
                }))
            }
            TokenKind::Service => {
                // service name [implements Proto] { fn handler... }
                let start = self.peek().span.start;
                self.advance(); // consume `service`
                let (name, _name_span) = self.expect_ident()?;

                // Optional: implements ProtocolName
                let implements = if self.eat(&TokenKind::Implements) {
                    let (proto_name, _) = self.expect_ident()?;
                    Some(proto_name)
                } else {
                    None
                };

                self.expect(&TokenKind::LBrace)?;
                let mut handlers = Vec::new();
                while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                    if matches!(self.peek_kind(), TokenKind::Fn) {
                        let fndef = self.parse_fn_def(false, annotation.clone())?;
                        handlers.push(fndef);
                    } else {
                        self.advance();
                    }
                }
                self.expect(&TokenKind::RBrace)?;
                let end = self.prev_span().end;

                Ok(Item::ServiceDef(ServiceDef {
                    name,
                    annotation,
                    implements,
                    handlers,
                    span: Span::new(start, end),
                }))
            }
            TokenKind::Static => {
                // static [mut] NAME[: TYPE] = VALUE
                let start = self.peek().span.start;
                self.advance(); // consume `static`
                let is_mut = self.eat(&TokenKind::Mut);
                let (name, name_span) = self.expect_ident()?;
                let ty = if self.eat(&TokenKind::Colon) {
                    self.parse_type_expr()?
                } else {
                    TypeExpr::Simple {
                        name: "i64".to_string(),
                        span: name_span,
                    }
                };
                self.expect(&TokenKind::Eq)?;
                let value = Box::new(self.parse_expr(0)?);
                let end = value.span().end;
                self.eat_semi();
                let mut sd = StaticDef {
                    is_pub,
                    is_mut,
                    doc_comment: None,
                    annotation,
                    name,
                    ty,
                    value,
                    span: Span::new(start, end),
                };
                sd.doc_comment = doc_comment;
                Ok(Item::StaticDef(sd))
            }
            TokenKind::Use => Ok(Item::UseDecl(self.parse_use_decl()?)),
            TokenKind::Mod => Ok(Item::ModDecl(self.parse_mod_decl()?)),
            TokenKind::Extern => Ok(Item::ExternFn(self.parse_extern_fn(annotation)?)),
            TokenKind::Type => Ok(Item::TypeAlias(self.parse_type_alias(is_pub)?)),
            _ => {
                if annotation.is_some() || is_pub {
                    let token = self.peek().clone();
                    return Err(ParseError::UnexpectedToken {
                        expected: "fn, struct, enum, const, or extern after annotation/pub".into(),
                        found: format!("{}", token.kind),
                        line: token.line,
                        col: token.col,
                        span: token.span,
                    });
                }
                // Check for global_asm!("...") at top level
                if let TokenKind::Ident(name) = self.peek_kind() {
                    if name == "global_asm" {
                        return Ok(Item::GlobalAsm(self.parse_global_asm()?));
                    }
                }
                let stmt = self.parse_stmt()?;
                Ok(Item::Stmt(stmt))
            }
        }
    }

    /// Tries to parse an annotation (`@kernel`, etc.). Returns `None` if no annotation.
    fn try_parse_annotation(&mut self) -> Option<Annotation> {
        match self.peek_kind() {
            TokenKind::AtKernel
            | TokenKind::AtDevice
            | TokenKind::AtNpu
            | TokenKind::AtSafe
            | TokenKind::AtUnsafe
            | TokenKind::AtFfi
            | TokenKind::AtPanicHandler
            | TokenKind::AtNoStd
            | TokenKind::AtEntry
            | TokenKind::AtReprC
            | TokenKind::AtReprPacked
            | TokenKind::AtSimd
            | TokenKind::AtSection
            | TokenKind::AtInterrupt
            | TokenKind::AtMessage
            | TokenKind::AtTest
            | TokenKind::AtShouldPanic
            | TokenKind::AtIgnore => {
                let token = self.advance().clone();
                let (name, param) = match &token.kind {
                    TokenKind::AtKernel => ("kernel", None),
                    TokenKind::AtDevice => {
                        // Parse optional capability parameter: @device("net")
                        let cap_param = if matches!(self.peek_kind(), TokenKind::LParen) {
                            self.advance(); // consume '('
                            let s = if let TokenKind::StringLit(val) = self.peek_kind().clone() {
                                self.advance(); // consume string
                                val
                            } else {
                                String::new()
                            };
                            if matches!(self.peek_kind(), TokenKind::RParen) {
                                self.advance(); // consume ')'
                            }
                            Some(s)
                        } else {
                            None
                        };
                        ("device", cap_param)
                    }
                    TokenKind::AtNpu => ("npu", None),
                    TokenKind::AtSafe => ("safe", None),
                    TokenKind::AtUnsafe => ("unsafe", None),
                    TokenKind::AtFfi => ("ffi", None),
                    TokenKind::AtPanicHandler => ("panic_handler", None),
                    TokenKind::AtNoStd => ("no_std", None),
                    TokenKind::AtEntry => ("entry", None),
                    TokenKind::AtReprC => ("repr", Some("C".to_string())),
                    TokenKind::AtReprPacked => ("repr", Some("packed".to_string())),
                    TokenKind::AtSimd => ("simd", None),
                    TokenKind::AtTest => ("test", None),
                    TokenKind::AtShouldPanic => ("should_panic", None),
                    TokenKind::AtIgnore => ("ignore", None),
                    TokenKind::AtInterrupt => ("interrupt", None),
                    TokenKind::AtMessage => ("message", None),
                    TokenKind::AtSection => {
                        // Parse @section("section_name")
                        let section_name = if matches!(self.peek_kind(), TokenKind::LParen) {
                            self.advance(); // consume '('
                            let s = if let TokenKind::StringLit(val) = self.peek_kind().clone() {
                                self.advance(); // consume string
                                val
                            } else {
                                // Fallback: no valid string param
                                String::new()
                            };
                            if matches!(self.peek_kind(), TokenKind::RParen) {
                                self.advance(); // consume ')'
                            }
                            Some(s)
                        } else {
                            None
                        };
                        ("section", section_name)
                    }
                    _ => unreachable!(),
                };
                Some(Annotation {
                    name: name.to_string(),
                    param,
                    span: token.span,
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    /// Helper: tokenize + parse, return the Program.
    fn parse_ok(source: &str) -> Program {
        let tokens = tokenize(source).unwrap();
        parse(tokens).unwrap()
    }

    /// Helper: tokenize + parse, return errors.
    fn parse_err(source: &str) -> Vec<ParseError> {
        let tokens = tokenize(source).unwrap();
        parse(tokens).unwrap_err()
    }

    /// Helper: parse and get the first item.
    fn first_item(source: &str) -> Item {
        let prog = parse_ok(source);
        prog.items
            .into_iter()
            .next()
            .expect("expected at least one item")
    }

    /// Helper: parse as expression (wraps in a stmt).
    fn parse_expr_ok(source: &str) -> Expr {
        let item = first_item(source);
        match item {
            Item::Stmt(Stmt::Expr { expr, .. }) => *expr,
            _ => panic!("expected expression statement, got {:?}", item),
        }
    }

    // ── Literals ───────────────────────────────────────────────────────

    #[test]
    fn parse_integer_literal() {
        let expr = parse_expr_ok("42");
        assert!(matches!(
            expr,
            Expr::Literal {
                kind: LiteralKind::Int(42),
                ..
            }
        ));
    }

    #[test]
    fn parse_float_literal() {
        let expr = parse_expr_ok("3.14");
        assert!(
            matches!(expr, Expr::Literal { kind: LiteralKind::Float(v), .. } if (v - 3.14).abs() < f64::EPSILON)
        );
    }

    #[test]
    fn parse_string_literal() {
        let expr = parse_expr_ok(r#""hello""#);
        assert!(
            matches!(expr, Expr::Literal { kind: LiteralKind::String(ref s), .. } if s == "hello")
        );
    }

    #[test]
    fn parse_bool_literals() {
        let t = parse_expr_ok("true");
        let f = parse_expr_ok("false");
        assert!(matches!(
            t,
            Expr::Literal {
                kind: LiteralKind::Bool(true),
                ..
            }
        ));
        assert!(matches!(
            f,
            Expr::Literal {
                kind: LiteralKind::Bool(false),
                ..
            }
        ));
    }

    #[test]
    fn parse_null_literal() {
        let expr = parse_expr_ok("null");
        assert!(matches!(
            expr,
            Expr::Literal {
                kind: LiteralKind::Null,
                ..
            }
        ));
    }

    // ── Binary expressions ─────────────────────────────────────────────

    #[test]
    fn parse_addition() {
        let expr = parse_expr_ok("1 + 2");
        match expr {
            Expr::Binary { op: BinOp::Add, .. } => {}
            _ => panic!("expected addition, got {:?}", expr),
        }
    }

    #[test]
    fn parse_precedence_mul_over_add() {
        // 1 + 2 * 3 → 1 + (2 * 3)
        let expr = parse_expr_ok("1 + 2 * 3");
        match expr {
            Expr::Binary {
                op: BinOp::Add,
                right,
                ..
            } => {
                assert!(matches!(*right, Expr::Binary { op: BinOp::Mul, .. }));
            }
            _ => panic!("expected add at top level"),
        }
    }

    #[test]
    fn parse_left_associative_subtraction() {
        // 1 - 2 - 3 → (1 - 2) - 3
        let expr = parse_expr_ok("1 - 2 - 3");
        match expr {
            Expr::Binary {
                op: BinOp::Sub,
                left,
                ..
            } => {
                assert!(matches!(*left, Expr::Binary { op: BinOp::Sub, .. }));
            }
            _ => panic!("expected sub at top level"),
        }
    }

    #[test]
    fn parse_right_associative_power() {
        // 2 ** 3 ** 4 → 2 ** (3 ** 4)
        let expr = parse_expr_ok("2 ** 3 ** 4");
        match expr {
            Expr::Binary {
                op: BinOp::Pow,
                right,
                ..
            } => {
                assert!(matches!(*right, Expr::Binary { op: BinOp::Pow, .. }));
            }
            _ => panic!("expected pow at top level"),
        }
    }

    #[test]
    fn parse_comparison_operators() {
        let expr = parse_expr_ok("a < b");
        assert!(matches!(expr, Expr::Binary { op: BinOp::Lt, .. }));
    }

    #[test]
    fn parse_logical_operators() {
        let expr = parse_expr_ok("a && b || c");
        // || has lower precedence, so: (a && b) || c
        match expr {
            Expr::Binary {
                op: BinOp::Or,
                left,
                ..
            } => {
                assert!(matches!(*left, Expr::Binary { op: BinOp::And, .. }));
            }
            _ => panic!("expected Or at top level"),
        }
    }

    #[test]
    fn parse_bitwise_operators() {
        let expr = parse_expr_ok("a & b | c ^ d");
        // Precedence: & > ^ > |, so: (a & b) | (c ^ d)
        assert!(matches!(
            expr,
            Expr::Binary {
                op: BinOp::BitOr,
                ..
            }
        ));
    }

    // ── Unary expressions ──────────────────────────────────────────────

    #[test]
    fn parse_unary_negation() {
        let expr = parse_expr_ok("-42");
        assert!(matches!(
            expr,
            Expr::Unary {
                op: UnaryOp::Neg,
                ..
            }
        ));
    }

    #[test]
    fn parse_unary_not() {
        let expr = parse_expr_ok("!true");
        assert!(matches!(
            expr,
            Expr::Unary {
                op: UnaryOp::Not,
                ..
            }
        ));
    }

    #[test]
    fn parse_unary_ref() {
        let expr = parse_expr_ok("&x");
        assert!(matches!(
            expr,
            Expr::Unary {
                op: UnaryOp::Ref,
                ..
            }
        ));
    }

    #[test]
    fn parse_unary_ref_mut() {
        let expr = parse_expr_ok("&mut x");
        assert!(matches!(
            expr,
            Expr::Unary {
                op: UnaryOp::RefMut,
                ..
            }
        ));
    }

    // ── Function calls ─────────────────────────────────────────────────

    #[test]
    fn parse_function_call_no_args() {
        let expr = parse_expr_ok("foo()");
        match expr {
            Expr::Call { args, .. } => assert_eq!(args.len(), 0),
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_function_call_with_args() {
        let expr = parse_expr_ok("add(1, 2)");
        match expr {
            Expr::Call { args, .. } => assert_eq!(args.len(), 2),
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn parse_named_args() {
        let expr = parse_expr_ok("add(a: 1, b: 2)");
        match expr {
            Expr::Call { args, .. } => {
                assert_eq!(args[0].name.as_deref(), Some("a"));
                assert_eq!(args[1].name.as_deref(), Some("b"));
            }
            _ => panic!("expected Call"),
        }
    }

    // ── Method calls & field access ────────────────────────────────────

    #[test]
    fn parse_field_access() {
        let expr = parse_expr_ok("obj.field");
        assert!(matches!(expr, Expr::Field { field, .. } if field == "field"));
    }

    #[test]
    fn parse_method_call() {
        let expr = parse_expr_ok("obj.method(1, 2)");
        match expr {
            Expr::MethodCall { method, args, .. } => {
                assert_eq!(method, "method");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected MethodCall"),
        }
    }

    #[test]
    fn parse_chained_field_access() {
        let expr = parse_expr_ok("a.b.c");
        match expr {
            Expr::Field { object, field, .. } => {
                assert_eq!(field, "c");
                assert!(matches!(*object, Expr::Field { field, .. } if field == "b"));
            }
            _ => panic!("expected chained Field"),
        }
    }

    // ── Index ──────────────────────────────────────────────────────────

    #[test]
    fn parse_index_access() {
        let expr = parse_expr_ok("arr[0]");
        assert!(matches!(expr, Expr::Index { .. }));
    }

    // ── Grouped & Tuple ────────────────────────────────────────────────

    #[test]
    fn parse_grouped_expression() {
        let expr = parse_expr_ok("(1 + 2)");
        assert!(matches!(expr, Expr::Grouped { .. }));
    }

    #[test]
    fn parse_tuple_expression() {
        let expr = parse_expr_ok("(1, 2, 3)");
        match expr {
            Expr::Tuple { elements, .. } => assert_eq!(elements.len(), 3),
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn parse_empty_tuple() {
        let expr = parse_expr_ok("()");
        match expr {
            Expr::Tuple { elements, .. } => assert_eq!(elements.len(), 0),
            _ => panic!("expected empty Tuple"),
        }
    }

    // ── Array ──────────────────────────────────────────────────────────

    #[test]
    fn parse_array_literal() {
        let expr = parse_expr_ok("[1, 2, 3]");
        match expr {
            Expr::Array { elements, .. } => assert_eq!(elements.len(), 3),
            _ => panic!("expected Array"),
        }
    }

    #[test]
    fn parse_empty_array() {
        let expr = parse_expr_ok("[]");
        match expr {
            Expr::Array { elements, .. } => assert_eq!(elements.len(), 0),
            _ => panic!("expected empty Array"),
        }
    }

    // ── Pipeline ───────────────────────────────────────────────────────

    #[test]
    fn parse_pipeline() {
        let expr = parse_expr_ok("5 |> double |> add_one");
        match expr {
            Expr::Pipe { left, .. } => {
                assert!(matches!(*left, Expr::Pipe { .. }));
            }
            _ => panic!("expected Pipe"),
        }
    }

    // ── Block ──────────────────────────────────────────────────────────

    #[test]
    fn parse_block_with_final_expr() {
        let expr = parse_expr_ok("{ let x = 1; x + 1 }");
        match expr {
            Expr::Block { stmts, expr, .. } => {
                assert_eq!(stmts.len(), 1);
                assert!(expr.is_some());
            }
            _ => panic!("expected Block"),
        }
    }

    #[test]
    fn parse_block_all_stmts() {
        let expr = parse_expr_ok("{ let x = 1; let y = 2; }");
        match expr {
            Expr::Block { stmts, expr, .. } => {
                assert_eq!(stmts.len(), 2);
                assert!(expr.is_none());
            }
            _ => panic!("expected Block"),
        }
    }

    // ── If expression ──────────────────────────────────────────────────

    #[test]
    fn parse_if_expr() {
        let expr = parse_expr_ok("if x > 0 { 1 } else { 0 }");
        match expr {
            Expr::If { else_branch, .. } => {
                assert!(else_branch.is_some());
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn parse_if_else_if() {
        let expr = parse_expr_ok("if a { 1 } else if b { 2 } else { 3 }");
        match expr {
            Expr::If { else_branch, .. } => {
                let else_br = else_branch.unwrap();
                assert!(matches!(*else_br, Expr::If { .. }));
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn parse_if_without_else() {
        let expr = parse_expr_ok("if x { 1 }");
        match expr {
            Expr::If { else_branch, .. } => {
                assert!(else_branch.is_none());
            }
            _ => panic!("expected If"),
        }
    }

    // ── Match expression ───────────────────────────────────────────────

    #[test]
    fn parse_match_expr() {
        let expr = parse_expr_ok("match x { 0 => 1, _ => 2 }");
        match expr {
            Expr::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
            }
            _ => panic!("expected Match"),
        }
    }

    #[test]
    fn parse_match_with_guard() {
        let expr = parse_expr_ok("match x { n if n > 0 => n, _ => 0 }");
        match expr {
            Expr::Match { arms, .. } => {
                assert!(arms[0].guard.is_some());
                assert!(arms[1].guard.is_none());
            }
            _ => panic!("expected Match"),
        }
    }

    // ── While / For ────────────────────────────────────────────────────

    #[test]
    fn parse_while_loop() {
        let expr = parse_expr_ok("while x > 0 { x }");
        assert!(matches!(expr, Expr::While { label: _, .. }));
    }

    #[test]
    fn parse_for_loop() {
        let expr = parse_expr_ok("for i in items { i }");
        match expr {
            Expr::For {
                label: _, variable, ..
            } => assert_eq!(variable, "i"),
            _ => panic!("expected For"),
        }
    }

    // ── Closure ────────────────────────────────────────────────────────

    #[test]
    fn parse_closure_single_param() {
        let expr = parse_expr_ok("|x| x * 2");
        match expr {
            Expr::Closure { params, .. } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name, "x");
            }
            _ => panic!("expected Closure"),
        }
    }

    #[test]
    fn parse_closure_typed_params() {
        let expr = parse_expr_ok("|x: i32, y: i32| x + y");
        match expr {
            Expr::Closure { params, .. } => {
                assert_eq!(params.len(), 2);
                assert!(params[0].ty.is_some());
            }
            _ => panic!("expected Closure"),
        }
    }

    #[test]
    fn parse_closure_empty_params() {
        let expr = parse_expr_ok("|| 42");
        match expr {
            Expr::Closure { params, .. } => assert_eq!(params.len(), 0),
            _ => panic!("expected Closure"),
        }
    }

    // ── Try operator ───────────────────────────────────────────────────

    #[test]
    fn parse_try_operator() {
        let expr = parse_expr_ok("foo()?");
        assert!(matches!(expr, Expr::Try { .. }));
    }

    // ── Cast ───────────────────────────────────────────────────────────

    #[test]
    fn parse_cast_expression() {
        let expr = parse_expr_ok("x as i64");
        match expr {
            Expr::Cast { ty, .. } => {
                assert!(matches!(ty, TypeExpr::Simple { ref name, .. } if name == "i64"));
            }
            _ => panic!("expected Cast"),
        }
    }

    // ── Let statement ──────────────────────────────────────────────────

    #[test]
    fn parse_let_immutable() {
        let item = first_item("let x = 42");
        match item {
            Item::Stmt(Stmt::Let {
                mutable, name, ty, ..
            }) => {
                assert!(!mutable);
                assert_eq!(name, "x");
                assert!(ty.is_none());
            }
            _ => panic!("expected Let"),
        }
    }

    #[test]
    fn parse_let_mutable_with_type() {
        let item = first_item("let mut x: i32 = 0");
        match item {
            Item::Stmt(Stmt::Let {
                mutable, name, ty, ..
            }) => {
                assert!(mutable);
                assert_eq!(name, "x");
                assert!(ty.is_some());
            }
            _ => panic!("expected Let"),
        }
    }

    // ── Assignment ─────────────────────────────────────────────────────

    #[test]
    fn parse_simple_assignment() {
        let item = first_item("x = 42");
        match item {
            Item::Stmt(Stmt::Expr { expr, .. }) => {
                assert!(matches!(
                    *expr,
                    Expr::Assign {
                        op: AssignOp::Assign,
                        ..
                    }
                ));
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_compound_assignment() {
        let item = first_item("x += 1");
        match item {
            Item::Stmt(Stmt::Expr { expr, .. }) => {
                assert!(matches!(
                    *expr,
                    Expr::Assign {
                        op: AssignOp::AddAssign,
                        ..
                    }
                ));
            }
            _ => panic!("expected compound assignment"),
        }
    }

    // ── Function definition ────────────────────────────────────────────

    #[test]
    fn parse_simple_fn() {
        let item = first_item("fn add(a: i32, b: i32) -> i32 { a + b }");
        match item {
            Item::FnDef(fd) => {
                assert_eq!(fd.name, "add");
                assert_eq!(fd.params.len(), 2);
                assert!(fd.return_type.is_some());
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_fn_no_params_no_return() {
        let item = first_item("fn hello() { }");
        match item {
            Item::FnDef(fd) => {
                assert_eq!(fd.name, "hello");
                assert!(fd.params.is_empty());
                assert!(fd.return_type.is_none());
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_annotated_fn() {
        let item = first_item("@kernel fn init() { }");
        match item {
            Item::FnDef(fd) => {
                assert_eq!(fd.annotation.unwrap().name, "kernel");
                assert_eq!(fd.name, "init");
            }
            _ => panic!("expected FnDef"),
        }
    }

    // ── Struct definition ──────────────────────────────────────────────

    #[test]
    fn parse_struct_def() {
        let item = first_item("struct Point { x: f64, y: f64 }");
        match item {
            Item::StructDef(sd) => {
                assert_eq!(sd.name, "Point");
                assert_eq!(sd.fields.len(), 2);
            }
            _ => panic!("expected StructDef"),
        }
    }

    #[test]
    fn parse_struct_with_generic() {
        let item = first_item("struct Wrapper<T> { value: T }");
        match item {
            Item::StructDef(sd) => {
                assert_eq!(sd.generic_params.len(), 1);
                assert_eq!(sd.generic_params[0].name, "T");
            }
            _ => panic!("expected StructDef"),
        }
    }

    // ── Enum definition ────────────────────────────────────────────────

    #[test]
    fn parse_enum_def() {
        let item = first_item("enum Shape { Circle(f64), Rect(f64, f64) }");
        match item {
            Item::EnumDef(ed) => {
                assert_eq!(ed.name, "Shape");
                assert_eq!(ed.variants.len(), 2);
                assert_eq!(ed.variants[0].name, "Circle");
                assert_eq!(ed.variants[0].fields.len(), 1);
                assert_eq!(ed.variants[1].fields.len(), 2);
            }
            _ => panic!("expected EnumDef"),
        }
    }

    #[test]
    fn parse_enum_unit_variants() {
        let item = first_item("enum Color { Red, Green, Blue }");
        match item {
            Item::EnumDef(ed) => {
                assert_eq!(ed.variants.len(), 3);
                assert!(ed.variants[0].fields.is_empty());
            }
            _ => panic!("expected EnumDef"),
        }
    }

    // ── Impl block ─────────────────────────────────────────────────────

    #[test]
    fn parse_inherent_impl() {
        let item = first_item("impl Point { fn new() -> Point { Point { x: 0.0, y: 0.0 } } }");
        match item {
            Item::ImplBlock(ib) => {
                assert!(ib.trait_name.is_none());
                assert_eq!(ib.target_type, "Point");
                assert_eq!(ib.methods.len(), 1);
            }
            _ => panic!("expected ImplBlock"),
        }
    }

    #[test]
    fn parse_trait_impl() {
        let item = first_item("impl Display for Point { fn fmt() { } }");
        match item {
            Item::ImplBlock(ib) => {
                assert_eq!(ib.trait_name.as_deref(), Some("Display"));
                assert_eq!(ib.target_type, "Point");
            }
            _ => panic!("expected ImplBlock"),
        }
    }

    // ── Trait definition ───────────────────────────────────────────────

    #[test]
    fn parse_trait_def() {
        let item = first_item("trait Summary { fn summarize() -> str { } }");
        match item {
            Item::TraitDef(td) => {
                assert_eq!(td.name, "Summary");
                assert_eq!(td.methods.len(), 1);
            }
            _ => panic!("expected TraitDef"),
        }
    }

    // ── Const definition ───────────────────────────────────────────────

    #[test]
    fn parse_const_def() {
        let item = first_item("const MAX: usize = 1024");
        match item {
            Item::ConstDef(cd) => {
                assert_eq!(cd.name, "MAX");
            }
            _ => panic!("expected ConstDef"),
        }
    }

    // ── Use declaration ────────────────────────────────────────────────

    #[test]
    fn parse_use_simple() {
        let item = first_item("use std::io::println");
        match item {
            Item::UseDecl(ud) => {
                assert_eq!(ud.path, vec!["std", "io", "println"]);
                assert!(matches!(ud.kind, UseKind::Simple));
            }
            _ => panic!("expected UseDecl"),
        }
    }

    #[test]
    fn parse_use_glob() {
        let item = first_item("use std::io::*");
        match item {
            Item::UseDecl(ud) => {
                assert!(matches!(ud.kind, UseKind::Glob));
            }
            _ => panic!("expected UseDecl"),
        }
    }

    #[test]
    fn parse_use_group() {
        let item = first_item("use std::io::{println, read_line}");
        match item {
            Item::UseDecl(ud) => match ud.kind {
                UseKind::Group(names) => {
                    assert_eq!(names, vec!["println", "read_line"]);
                }
                _ => panic!("expected Group"),
            },
            _ => panic!("expected UseDecl"),
        }
    }

    // ── Module declaration ─────────────────────────────────────────────

    #[test]
    fn parse_mod_with_body() {
        let item = first_item("mod math { fn add(a: i32, b: i32) -> i32 { a + b } }");
        match item {
            Item::ModDecl(md) => {
                assert_eq!(md.name, "math");
                assert!(md.body.is_some());
                assert_eq!(md.body.unwrap().len(), 1);
            }
            _ => panic!("expected ModDecl"),
        }
    }

    #[test]
    fn parse_mod_without_body() {
        let item = first_item("mod external");
        match item {
            Item::ModDecl(md) => {
                assert_eq!(md.name, "external");
                assert!(md.body.is_none());
            }
            _ => panic!("expected ModDecl"),
        }
    }

    // ── Type expressions ───────────────────────────────────────────────

    #[test]
    fn parse_type_simple() {
        let item = first_item("let x: i32 = 0");
        match item {
            Item::Stmt(Stmt::Let { ty: Some(ty), .. }) => {
                assert!(matches!(ty, TypeExpr::Simple { ref name, .. } if name == "i32"));
            }
            _ => panic!("expected Let with type"),
        }
    }

    #[test]
    fn parse_type_generic() {
        let item = first_item("let x: Vec<i32> = 0");
        match item {
            Item::Stmt(Stmt::Let { ty: Some(ty), .. }) => {
                assert!(matches!(ty, TypeExpr::Generic { ref name, .. } if name == "Vec"));
            }
            _ => panic!("expected Let with generic type"),
        }
    }

    #[test]
    fn parse_type_reference() {
        let item = first_item("let x: &mut i32 = 0");
        match item {
            Item::Stmt(Stmt::Let { ty: Some(ty), .. }) => {
                assert!(matches!(ty, TypeExpr::Reference { mutable: true, .. }));
            }
            _ => panic!("expected Let with ref type"),
        }
    }

    #[test]
    fn parse_type_array() {
        let item = first_item("let x: [f32; 4] = 0");
        match item {
            Item::Stmt(Stmt::Let { ty: Some(ty), .. }) => {
                assert!(matches!(ty, TypeExpr::Array { size: 4, .. }));
            }
            _ => panic!("expected Let with array type"),
        }
    }

    #[test]
    fn parse_type_fn() {
        let item = first_item("let f: fn(i32, i32) -> bool = 0");
        match item {
            Item::Stmt(Stmt::Let { ty: Some(ty), .. }) => {
                assert!(matches!(ty, TypeExpr::Fn { .. }));
            }
            _ => panic!("expected Let with fn type"),
        }
    }

    #[test]
    fn parse_type_fn_void() {
        let item = first_item("let f: fn() = 0");
        match item {
            Item::Stmt(Stmt::Let { ty: Some(ty), .. }) => match ty {
                TypeExpr::Fn {
                    params,
                    return_type,
                    ..
                } => {
                    assert!(params.is_empty());
                    assert!(
                        matches!(*return_type, TypeExpr::Simple { ref name, .. } if name == "void")
                    );
                }
                _ => panic!("expected Fn type"),
            },
            _ => panic!("expected Let with fn type"),
        }
    }

    #[test]
    fn parse_type_fn_nested() {
        let item = first_item("let f: fn(i64) -> fn(i64) -> i64 = 0");
        match item {
            Item::Stmt(Stmt::Let { ty: Some(ty), .. }) => match ty {
                TypeExpr::Fn {
                    params,
                    return_type,
                    ..
                } => {
                    assert_eq!(params.len(), 1);
                    assert!(matches!(*return_type, TypeExpr::Fn { .. }));
                }
                _ => panic!("expected Fn type"),
            },
            _ => panic!("expected Let with fn type"),
        }
    }

    // ── Patterns ───────────────────────────────────────────────────────

    #[test]
    fn parse_wildcard_pattern() {
        let expr = parse_expr_ok("match x { _ => 0 }");
        match expr {
            Expr::Match { arms, .. } => {
                assert!(matches!(arms[0].pattern, Pattern::Wildcard { .. }));
            }
            _ => panic!("expected Match"),
        }
    }

    #[test]
    fn parse_enum_pattern() {
        let expr = parse_expr_ok("match x { Shape::Circle(r) => r, _ => 0 }");
        match expr {
            Expr::Match { arms, .. } => match &arms[0].pattern {
                Pattern::Enum {
                    enum_name,
                    variant,
                    fields,
                    ..
                } => {
                    assert_eq!(enum_name, "Shape");
                    assert_eq!(variant, "Circle");
                    assert_eq!(fields.len(), 1);
                }
                _ => panic!("expected Enum pattern"),
            },
            _ => panic!("expected Match"),
        }
    }

    // ── Path expressions ───────────────────────────────────────────────

    #[test]
    fn parse_path_expression() {
        let expr = parse_expr_ok("std::io::println");
        match expr {
            Expr::Path { segments, .. } => {
                assert_eq!(segments, vec!["std", "io", "println"]);
            }
            _ => panic!("expected Path"),
        }
    }

    // ── Range ──────────────────────────────────────────────────────────

    #[test]
    fn parse_range_expression() {
        let expr = parse_expr_ok("0..10");
        match expr {
            Expr::Range {
                inclusive,
                start,
                end,
                ..
            } => {
                assert!(!inclusive);
                assert!(start.is_some());
                assert!(end.is_some());
            }
            _ => panic!("expected Range"),
        }
    }

    #[test]
    fn parse_inclusive_range() {
        let expr = parse_expr_ok("0..=10");
        assert!(matches!(
            expr,
            Expr::Range {
                inclusive: true,
                ..
            }
        ));
    }

    // ── Struct init ────────────────────────────────────────────────────

    #[test]
    fn parse_struct_init() {
        let expr = parse_expr_ok("Point { x: 1.0, y: 2.0 }");
        match expr {
            Expr::StructInit { name, fields, .. } => {
                assert_eq!(name, "Point");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "x");
                assert_eq!(fields[1].name, "y");
            }
            _ => panic!("expected StructInit"),
        }
    }

    // ── Complex programs ───────────────────────────────────────────────

    #[test]
    fn parse_complete_program() {
        let source = r#"
            fn factorial(n: i32) -> i32 {
                if n <= 1 {
                    1
                } else {
                    n * factorial(n - 1)
                }
            }

            let result = factorial(10)
        "#;
        let prog = parse_ok(source);
        assert_eq!(prog.items.len(), 2);
    }

    #[test]
    fn parse_struct_with_methods() {
        let source = r#"
            struct Point { x: f64, y: f64 }
            impl Point {
                fn new(x: f64, y: f64) -> Point {
                    Point { x: x, y: y }
                }
            }
        "#;
        let prog = parse_ok(source);
        assert_eq!(prog.items.len(), 2);
    }

    // ── Error cases ────────────────────────────────────────────────────

    #[test]
    fn parse_error_unexpected_token() {
        let errors = parse_err("fn {}");
        assert!(!errors.is_empty());
    }

    #[test]
    fn parse_error_missing_closing_paren() {
        let errors = parse_err("fn foo(x: i32 { }");
        assert!(!errors.is_empty());
    }

    // ── Return / Break / Continue ──────────────────────────────────────

    #[test]
    fn parse_return_statement() {
        let item = first_item("return 42");
        match item {
            Item::Stmt(Stmt::Return { value, .. }) => {
                assert!(value.is_some());
            }
            _ => panic!("expected Return"),
        }
    }

    #[test]
    fn parse_return_void() {
        let item = first_item("return");
        match item {
            Item::Stmt(Stmt::Return { value, .. }) => {
                assert!(value.is_none());
            }
            _ => panic!("expected Return"),
        }
    }

    #[test]
    fn parse_break_continue() {
        let prog = parse_ok("break; continue");
        assert_eq!(prog.items.len(), 2);
    }

    // ── Matmul operator ────────────────────────────────────────────────

    #[test]
    fn parse_matmul_operator() {
        let expr = parse_expr_ok("a @ b");
        assert!(matches!(
            expr,
            Expr::Binary {
                op: BinOp::MatMul,
                ..
            }
        ));
    }

    // ── S5.1 Generic function parsing ─────────────────────────────────

    #[test]
    fn parse_generic_fn_single_param() {
        let item = first_item("fn identity<T>(x: T) -> T { x }");
        match item {
            Item::FnDef(fd) => {
                assert_eq!(fd.name, "identity");
                assert_eq!(fd.generic_params.len(), 1);
                assert_eq!(fd.generic_params[0].name, "T");
                assert!(fd.generic_params[0].bounds.is_empty());
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_generic_fn_multiple_params() {
        let item = first_item("fn pair<T, U>(a: T, b: U) -> T { a }");
        match item {
            Item::FnDef(fd) => {
                assert_eq!(fd.generic_params.len(), 2);
                assert_eq!(fd.generic_params[0].name, "T");
                assert_eq!(fd.generic_params[1].name, "U");
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_generic_fn_with_bounds() {
        let item = first_item("fn max<T: Ord>(a: T, b: T) -> T { a }");
        match item {
            Item::FnDef(fd) => {
                assert_eq!(fd.generic_params.len(), 1);
                assert_eq!(fd.generic_params[0].name, "T");
                assert_eq!(fd.generic_params[0].bounds.len(), 1);
                assert_eq!(fd.generic_params[0].bounds[0].name, "Ord");
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_generic_fn_with_multiple_bounds() {
        let item = first_item("fn show<T: Display + Ord>(x: T) -> T { x }");
        match item {
            Item::FnDef(fd) => {
                assert_eq!(fd.generic_params[0].bounds.len(), 2);
                assert_eq!(fd.generic_params[0].bounds[0].name, "Display");
                assert_eq!(fd.generic_params[0].bounds[1].name, "Ord");
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_generic_enum() {
        let item = first_item("enum Option<T> { Some(T), None }");
        match item {
            Item::EnumDef(ed) => {
                assert_eq!(ed.name, "Option");
                assert_eq!(ed.generic_params.len(), 1);
                assert_eq!(ed.generic_params[0].name, "T");
                assert_eq!(ed.variants.len(), 2);
            }
            _ => panic!("expected EnumDef"),
        }
    }

    #[test]
    fn parse_generic_impl_block() {
        // Note: target_type is currently just a String, so `Wrapper<T>` isn't supported yet.
        // This tests `impl<T> Wrapper { ... }` which parses the generic on the impl level.
        let item = first_item("impl<T> Wrapper { fn get(self: T) -> T { self } }");
        match item {
            Item::ImplBlock(ib) => {
                assert_eq!(ib.generic_params.len(), 1);
                assert_eq!(ib.generic_params[0].name, "T");
                assert_eq!(ib.target_type, "Wrapper");
            }
            _ => panic!("expected ImplBlock"),
        }
    }

    // ── Where clauses (S5.1) ────────────────────────────────────────────

    #[test]
    fn parse_where_clause_single_bound() {
        let item = first_item("fn show<T>(x: T) -> T where T: Display { x }");
        match item {
            Item::FnDef(fd) => {
                assert_eq!(fd.generic_params.len(), 1);
                assert!(fd.generic_params[0].bounds.is_empty());
                assert_eq!(fd.where_clauses.len(), 1);
                assert_eq!(fd.where_clauses[0].name, "T");
                assert_eq!(fd.where_clauses[0].bounds.len(), 1);
                assert_eq!(fd.where_clauses[0].bounds[0].name, "Display");
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_where_clause_multiple_bounds() {
        let item = first_item("fn show<T>(x: T) -> T where T: Display + Ord { x }");
        match item {
            Item::FnDef(fd) => {
                assert_eq!(fd.where_clauses.len(), 1);
                assert_eq!(fd.where_clauses[0].bounds.len(), 2);
                assert_eq!(fd.where_clauses[0].bounds[0].name, "Display");
                assert_eq!(fd.where_clauses[0].bounds[1].name, "Ord");
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_where_clause_multiple_params() {
        let item = first_item("fn foo<T, U>(a: T, b: U) -> T where T: Display, U: Ord { a }");
        match item {
            Item::FnDef(fd) => {
                assert_eq!(fd.where_clauses.len(), 2);
                assert_eq!(fd.where_clauses[0].name, "T");
                assert_eq!(fd.where_clauses[1].name, "U");
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_fn_without_where_clause() {
        let item = first_item("fn add(a: i64, b: i64) -> i64 { a + b }");
        match item {
            Item::FnDef(fd) => {
                assert!(fd.where_clauses.is_empty());
            }
            _ => panic!("expected FnDef"),
        }
    }

    // ── Extern function declarations (S7.1) ────────────────────────────

    #[test]
    fn parse_extern_fn_simple() {
        let item = first_item("extern fn abs(x: i32) -> i32");
        match item {
            Item::ExternFn(efn) => {
                assert_eq!(efn.name, "abs");
                assert!(efn.annotation.is_none());
                assert!(efn.abi.is_none());
                assert_eq!(efn.params.len(), 1);
                assert_eq!(efn.params[0].name, "x");
            }
            _ => panic!("expected ExternFn"),
        }
    }

    #[test]
    fn parse_extern_fn_with_abi() {
        let item = first_item("extern(\"C\") fn printf(fmt: i64) -> i32");
        match item {
            Item::ExternFn(efn) => {
                assert_eq!(efn.name, "printf");
                assert_eq!(efn.abi, Some("C".to_string()));
                assert_eq!(efn.params.len(), 1);
            }
            _ => panic!("expected ExternFn"),
        }
    }

    #[test]
    fn parse_extern_fn_with_ffi_annotation() {
        let item = first_item("@ffi extern(\"C\") fn malloc(size: u64) -> u64");
        match item {
            Item::ExternFn(efn) => {
                assert_eq!(efn.name, "malloc");
                assert!(efn.annotation.is_some());
                assert_eq!(efn.annotation.as_ref().unwrap().name, "ffi");
                assert_eq!(efn.abi, Some("C".to_string()));
                assert_eq!(efn.params.len(), 1);
            }
            _ => panic!("expected ExternFn"),
        }
    }

    #[test]
    fn parse_extern_fn_no_return() {
        let item = first_item("extern fn exit(code: i32)");
        match item {
            Item::ExternFn(efn) => {
                assert_eq!(efn.name, "exit");
                assert!(efn.return_type.is_none());
            }
            _ => panic!("expected ExternFn"),
        }
    }

    #[test]
    fn parse_extern_fn_multiple_params() {
        let item = first_item("extern fn memcpy(dst: u64, src: u64, n: u64) -> u64");
        match item {
            Item::ExternFn(efn) => {
                assert_eq!(efn.name, "memcpy");
                assert_eq!(efn.params.len(), 3);
                assert_eq!(efn.params[0].name, "dst");
                assert_eq!(efn.params[1].name, "src");
                assert_eq!(efn.params[2].name, "n");
            }
            _ => panic!("expected ExternFn"),
        }
    }

    // ── Type alias (S8.3) ──────────────────────────────────────────────

    #[test]
    fn parse_type_alias_simple() {
        let item = first_item("type Meters = f64");
        match item {
            Item::TypeAlias(ta) => {
                assert_eq!(ta.name, "Meters");
                assert!(matches!(&ta.ty, TypeExpr::Simple { name, .. } if name == "f64"));
            }
            _ => panic!("expected TypeAlias"),
        }
    }

    #[test]
    fn parse_type_alias_to_int() {
        let item = first_item("type Count = i32");
        match item {
            Item::TypeAlias(ta) => {
                assert_eq!(ta.name, "Count");
                assert!(matches!(&ta.ty, TypeExpr::Simple { name, .. } if name == "i32"));
            }
            _ => panic!("expected TypeAlias"),
        }
    }

    // ── Never type (S8.4) ──────────────────────────────────────────────

    #[test]
    fn parse_never_type_return() {
        let item = first_item("fn diverge() -> ! { while true { 0 } }");
        match item {
            Item::FnDef(f) => {
                assert_eq!(f.name, "diverge");
                assert!(
                    matches!(&f.return_type, Some(TypeExpr::Simple { name, .. }) if name == "never")
                );
            }
            _ => panic!("expected FnDef"),
        }
    }

    // ── pub visibility modifier ────────────────────────────────────────

    #[test]
    fn parse_pub_fn() {
        let item = first_item("pub fn greet() -> void { 0 }");
        match item {
            Item::FnDef(f) => {
                assert!(f.is_pub);
                assert_eq!(f.name, "greet");
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_non_pub_fn() {
        let item = first_item("fn private() -> void { 0 }");
        match item {
            Item::FnDef(f) => {
                assert!(!f.is_pub);
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_pub_struct() {
        let item = first_item("pub struct Point { x: f64 }");
        match item {
            Item::StructDef(s) => {
                assert!(s.is_pub);
                assert_eq!(s.name, "Point");
            }
            _ => panic!("expected StructDef"),
        }
    }

    #[test]
    fn parse_pub_enum() {
        let item = first_item("pub enum Color { Red, Green, Blue }");
        match item {
            Item::EnumDef(e) => {
                assert!(e.is_pub);
                assert_eq!(e.name, "Color");
            }
            _ => panic!("expected EnumDef"),
        }
    }

    #[test]
    fn parse_pub_const() {
        let item = first_item("pub const MAX: i64 = 100");
        match item {
            Item::ConstDef(c) => {
                assert!(c.is_pub);
                assert_eq!(c.name, "MAX");
            }
            _ => panic!("expected ConstDef"),
        }
    }

    #[test]
    fn parse_pub_trait() {
        let item = first_item("pub trait Drawable { fn draw() -> void { 0 } }");
        match item {
            Item::TraitDef(t) => {
                assert!(t.is_pub);
                assert_eq!(t.name, "Drawable");
            }
            _ => panic!("expected TraitDef"),
        }
    }

    #[test]
    fn parse_pub_with_annotation() {
        let item = first_item("pub @kernel fn boot() -> void { 0 }");
        match item {
            Item::FnDef(f) => {
                assert!(f.is_pub);
                assert!(f.annotation.is_some());
                assert_eq!(f.name, "boot");
            }
            _ => panic!("expected FnDef"),
        }
    }

    // ── &self / &mut self sugar ───────────────────────────────────────────

    #[test]
    fn parse_ref_self_param() {
        let item = first_item("impl Foo { fn get(&self) -> i64 { 0 } }");
        match item {
            Item::ImplBlock(ib) => {
                let method = &ib.methods[0];
                assert_eq!(method.params[0].name, "self");
                assert!(matches!(
                    &method.params[0].ty,
                    TypeExpr::Reference { mutable: false, .. }
                ));
            }
            _ => panic!("expected ImplBlock"),
        }
    }

    #[test]
    fn parse_ref_mut_self_param() {
        let item = first_item("impl Foo { fn set(&mut self, x: i64) -> void { 0 } }");
        match item {
            Item::ImplBlock(ib) => {
                let method = &ib.methods[0];
                assert_eq!(method.params[0].name, "self");
                assert!(matches!(
                    &method.params[0].ty,
                    TypeExpr::Reference { mutable: true, .. }
                ));
                assert_eq!(method.params.len(), 2);
                assert_eq!(method.params[1].name, "x");
            }
            _ => panic!("expected ImplBlock"),
        }
    }

    #[test]
    fn parse_bare_self_still_works() {
        let item = first_item("impl Foo { fn consume(self) -> i64 { 0 } }");
        match item {
            Item::ImplBlock(ib) => {
                let method = &ib.methods[0];
                assert_eq!(method.params[0].name, "self");
                assert!(matches!(
                    &method.params[0].ty,
                    TypeExpr::Simple { name, .. } if name == "Self"
                ));
            }
            _ => panic!("expected ImplBlock"),
        }
    }

    // ── Async/Await ──────────────────────────────────────────────────

    #[test]
    fn parse_async_fn() {
        let item = first_item("async fn fetch() -> i64 { 42 }");
        match item {
            Item::FnDef(f) => {
                assert!(f.is_async);
                assert_eq!(f.name, "fetch");
            }
            _ => panic!("expected async fn"),
        }
    }

    #[test]
    fn parse_await_expr() {
        let expr = parse_expr_ok("x.await");
        assert!(matches!(expr, Expr::Await { .. }));
    }

    #[test]
    fn parse_async_block_expr() {
        let expr = parse_expr_ok("async { 42 }");
        assert!(matches!(expr, Expr::AsyncBlock { .. }));
    }

    #[test]
    fn parse_async_fn_with_params() {
        let item = first_item("async fn load(url: str, timeout: i64) -> str { url }");
        match item {
            Item::FnDef(f) => {
                assert!(f.is_async);
                assert_eq!(f.name, "load");
                assert_eq!(f.params.len(), 2);
            }
            _ => panic!("expected async fn"),
        }
    }

    // ── Inline Assembly ───────────────────────────────────────────────

    #[test]
    fn parse_asm_simple() {
        let expr = parse_expr_ok("asm!(\"nop\")");
        match expr {
            Expr::InlineAsm {
                template, operands, ..
            } => {
                assert_eq!(template, "nop");
                assert!(operands.is_empty());
            }
            _ => panic!("expected InlineAsm"),
        }
    }

    #[test]
    fn parse_asm_with_operands() {
        let expr = parse_expr_ok("asm!(\"mov {}, {}\", out(reg) x, in(reg) y)");
        match expr {
            Expr::InlineAsm {
                template, operands, ..
            } => {
                assert_eq!(template, "mov {}, {}");
                assert_eq!(operands.len(), 2);
                assert!(
                    matches!(&operands[0], AsmOperand::Out { constraint, .. } if constraint == "reg")
                );
                assert!(
                    matches!(&operands[1], AsmOperand::In { constraint, .. } if constraint == "reg")
                );
            }
            _ => panic!("expected InlineAsm"),
        }
    }

    #[test]
    fn parse_asm_const_operand() {
        let expr = parse_expr_ok("asm!(\"int {}\", const 0x80)");
        match expr {
            Expr::InlineAsm { operands, .. } => {
                assert_eq!(operands.len(), 1);
                assert!(matches!(&operands[0], AsmOperand::Const { .. }));
            }
            _ => panic!("expected InlineAsm"),
        }
    }

    #[test]
    fn parse_asm_options_nomem_nostack() {
        let expr = parse_expr_ok("asm!(\"nop\", options(nomem, nostack))");
        match expr {
            Expr::InlineAsm { options, .. } => {
                assert_eq!(options.len(), 2);
                assert_eq!(options[0], AsmOption::Nomem);
                assert_eq!(options[1], AsmOption::Nostack);
            }
            _ => panic!("expected InlineAsm"),
        }
    }

    #[test]
    fn parse_asm_clobber_abi() {
        let expr = parse_expr_ok("asm!(\"syscall\", clobber_abi(\"C\"))");
        match expr {
            Expr::InlineAsm { clobber_abi, .. } => {
                assert_eq!(clobber_abi, Some("C".to_string()));
            }
            _ => panic!("expected InlineAsm"),
        }
    }

    #[test]
    fn parse_asm_options_with_operands() {
        let expr =
            parse_expr_ok("asm!(\"add {}, {}\", out(reg) r, in(reg) x, options(pure, nomem))");
        match expr {
            Expr::InlineAsm {
                operands, options, ..
            } => {
                assert_eq!(operands.len(), 2);
                assert_eq!(options.len(), 2);
                assert_eq!(options[0], AsmOption::Pure);
                assert_eq!(options[1], AsmOption::Nomem);
            }
            _ => panic!("expected InlineAsm"),
        }
    }

    #[test]
    fn parse_asm_all_option_kinds() {
        let expr = parse_expr_ok(
            "asm!(\"nop\", options(nomem, nostack, readonly, preserves_flags, pure, att_syntax))",
        );
        match expr {
            Expr::InlineAsm { options, .. } => {
                assert_eq!(options.len(), 6);
                assert_eq!(options[0], AsmOption::Nomem);
                assert_eq!(options[1], AsmOption::Nostack);
                assert_eq!(options[2], AsmOption::Readonly);
                assert_eq!(options[3], AsmOption::PreservesFlags);
                assert_eq!(options[4], AsmOption::Pure);
                assert_eq!(options[5], AsmOption::AttSyntax);
            }
            _ => panic!("expected InlineAsm"),
        }
    }

    #[test]
    fn parse_asm_clobber_and_options_combined() {
        let expr =
            parse_expr_ok("asm!(\"int 0x80\", in(reg) x, clobber_abi(\"C\"), options(nostack))");
        match expr {
            Expr::InlineAsm {
                operands,
                options,
                clobber_abi,
                ..
            } => {
                assert_eq!(operands.len(), 1);
                assert_eq!(clobber_abi, Some("C".to_string()));
                assert_eq!(options.len(), 1);
                assert_eq!(options[0], AsmOption::Nostack);
            }
            _ => panic!("expected InlineAsm"),
        }
    }

    #[test]
    fn parse_global_asm_item() {
        let src = "global_asm!(\".section .text\\n.align 4\")";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            crate::parser::ast::Item::GlobalAsm(ga) => {
                assert_eq!(ga.template, ".section .text\n.align 4");
            }
            other => panic!("expected GlobalAsm, got {:?}", other),
        }
    }

    #[test]
    fn parse_chained_await() {
        let expr = parse_expr_ok("fetch().await");
        match expr {
            Expr::Await { expr, .. } => {
                assert!(matches!(*expr, Expr::Call { .. }));
            }
            _ => panic!("expected Await wrapping Call"),
        }
    }

    // ── Lifetime annotation tests ───────────────────────────────────────

    #[test]
    fn parse_fn_with_single_lifetime() {
        let item = first_item("fn foo<'a>(x: &'a i32) -> &'a i32 { x }");
        match item {
            Item::FnDef(f) => {
                assert_eq!(f.lifetime_params.len(), 1);
                assert_eq!(f.lifetime_params[0].name, "a");
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_fn_with_multiple_lifetimes() {
        let item = first_item("fn foo<'a, 'b>(x: &'a i32, y: &'b i32) -> &'a i32 { x }");
        match item {
            Item::FnDef(f) => {
                assert_eq!(f.lifetime_params.len(), 2);
                assert_eq!(f.lifetime_params[0].name, "a");
                assert_eq!(f.lifetime_params[1].name, "b");
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_fn_with_lifetime_and_generics() {
        let item = first_item("fn foo<'a, T>(x: &'a T) -> &'a T { x }");
        match item {
            Item::FnDef(f) => {
                assert_eq!(f.lifetime_params.len(), 1);
                assert_eq!(f.lifetime_params[0].name, "a");
                assert_eq!(f.generic_params.len(), 1);
                assert_eq!(f.generic_params[0].name, "T");
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_fn_with_no_lifetimes() {
        let item = first_item("fn foo(x: i32) -> i32 { x }");
        match item {
            Item::FnDef(f) => {
                assert!(f.lifetime_params.is_empty());
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_struct_with_lifetime() {
        let item = first_item("struct Ref<'a> { val: &'a i32 }");
        match item {
            Item::StructDef(s) => {
                assert_eq!(s.lifetime_params.len(), 1);
                assert_eq!(s.lifetime_params[0].name, "a");
            }
            _ => panic!("expected StructDef"),
        }
    }

    #[test]
    fn parse_enum_with_lifetime() {
        let item = first_item("enum Cow<'a> { Borrowed(&'a i32), Owned(i32) }");
        match item {
            Item::EnumDef(e) => {
                assert_eq!(e.lifetime_params.len(), 1);
                assert_eq!(e.lifetime_params[0].name, "a");
            }
            _ => panic!("expected EnumDef"),
        }
    }

    #[test]
    fn parse_trait_with_lifetime() {
        let item = first_item("trait Readable<'a> { fn read(&self) -> &'a i32 }");
        match item {
            Item::TraitDef(t) => {
                assert_eq!(t.lifetime_params.len(), 1);
                assert_eq!(t.lifetime_params[0].name, "a");
            }
            _ => panic!("expected TraitDef"),
        }
    }

    #[test]
    fn parse_impl_with_lifetime() {
        let item = first_item("impl<'a> Ref { fn get(&self) -> i32 { 0 } }");
        match item {
            Item::ImplBlock(i) => {
                assert_eq!(i.lifetime_params.len(), 1);
                assert_eq!(i.lifetime_params[0].name, "a");
            }
            _ => panic!("expected ImplBlock"),
        }
    }

    #[test]
    fn parse_reference_type_with_lifetime() {
        let item = first_item("fn foo<'a>(x: &'a i32) -> i32 { 0 }");
        match item {
            Item::FnDef(f) => {
                // Check the param type has a lifetime
                let param_type = &f.params[0].ty;
                match param_type {
                    TypeExpr::Reference { lifetime, .. } => {
                        assert_eq!(lifetime.as_deref(), Some("a"));
                    }
                    _ => panic!("expected Reference type with lifetime"),
                }
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_reference_type_without_lifetime() {
        let item = first_item("fn foo(x: &i32) -> i32 { 0 }");
        match item {
            Item::FnDef(f) => {
                let param_type = &f.params[0].ty;
                match param_type {
                    TypeExpr::Reference { lifetime, .. } => {
                        assert!(lifetime.is_none());
                    }
                    _ => panic!("expected Reference type without lifetime"),
                }
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_mutable_reference_with_lifetime() {
        let item = first_item("fn foo<'a>(x: &'a mut i32) -> &'a mut i32 { x }");
        match item {
            Item::FnDef(f) => {
                let param_type = &f.params[0].ty;
                match param_type {
                    TypeExpr::Reference {
                        lifetime, mutable, ..
                    } => {
                        assert_eq!(lifetime.as_deref(), Some("a"));
                        assert!(*mutable);
                    }
                    _ => panic!("expected mutable Reference type with lifetime"),
                }
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_static_lifetime_reference() {
        let item = first_item("fn foo(x: &'static str) -> i32 { 0 }");
        match item {
            Item::FnDef(f) => {
                let param_type = &f.params[0].ty;
                match param_type {
                    TypeExpr::Reference { lifetime, .. } => {
                        assert_eq!(lifetime.as_deref(), Some("static"));
                    }
                    _ => panic!("expected Reference type with 'static lifetime"),
                }
            }
            _ => panic!("expected FnDef"),
        }
    }
}
