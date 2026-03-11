//! Fajar Lang parser — converts tokens to AST.
//!
//! Entry point: [`parse`] takes `Vec<Token>` and returns `Result<Program, Vec<ParseError>>`.
//! Uses recursive descent for declarations/statements and Pratt parsing for expressions.
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
pub mod pratt;

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
}

impl Parser {
    /// Creates a new parser.
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
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
                let mut cd = self.parse_const_def(is_pub, annotation)?;
                cd.doc_comment = doc_comment;
                Ok(Item::ConstDef(cd))
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
                if let TokenKind::Ident(ref name) = self.peek_kind() {
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
            | TokenKind::AtTest
            | TokenKind::AtShouldPanic
            | TokenKind::AtIgnore => {
                let token = self.advance().clone();
                let (name, param) = match &token.kind {
                    TokenKind::AtKernel => ("kernel", None),
                    TokenKind::AtDevice => ("device", None),
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

    /// Parses a function definition: `fn name(params) -> RetType { body }`.
    fn parse_fn_def(
        &mut self,
        is_pub: bool,
        annotation: Option<Annotation>,
    ) -> Result<FnDef, ParseError> {
        let start = if let Some(ref ann) = annotation {
            ann.span.start
        } else {
            self.peek().span.start
        };

        // Optional `async` keyword before `fn`
        let is_async = self.eat(&TokenKind::Async);

        self.expect(&TokenKind::Fn)?;
        let (name, _) = self.expect_ident()?;

        // Optional generic params
        let generic_params = self.try_parse_generic_params()?;

        // Parameters
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;

        // Optional return type
        let return_type = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        // Optional where clause
        let where_clauses = self.parse_where_clauses()?;

        // Body
        let body = Box::new(self.parse_block_expr()?);
        let end = body.span().end;

        Ok(FnDef {
            is_pub,
            is_async,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            doc_comment: None,
            annotation,
            name,
            generic_params,
            params,
            return_type,
            where_clauses,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses an extern function declaration: `extern fn name(params) -> RetType`.
    ///
    /// Optionally preceded by `@ffi("C")` annotation. No body — just a declaration.
    fn parse_extern_fn(&mut self, annotation: Option<Annotation>) -> Result<ExternFn, ParseError> {
        let start = if let Some(ref ann) = annotation {
            ann.span.start
        } else {
            self.peek().span.start
        };

        self.expect(&TokenKind::Extern)?;

        // Optional ABI string: extern("C") or just extern
        let abi = if self.eat(&TokenKind::LParen) {
            let abi_str = match self.peek_kind() {
                TokenKind::StringLit(_) => {
                    let tok = self.advance().clone();
                    if let TokenKind::StringLit(s) = &tok.kind {
                        s.clone()
                    } else {
                        unreachable!()
                    }
                }
                _ => {
                    let tok = self.peek().clone();
                    return Err(ParseError::UnexpectedToken {
                        expected: "ABI string (e.g., \"C\")".into(),
                        found: format!("{}", tok.kind),
                        line: tok.line,
                        col: tok.col,
                        span: tok.span,
                    });
                }
            };
            self.expect(&TokenKind::RParen)?;
            Some(abi_str)
        } else {
            None
        };

        self.expect(&TokenKind::Fn)?;
        let (name, _) = self.expect_ident()?;

        // Parameters
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;

        // Optional return type
        let return_type = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let end = self.prev_span().end;

        Ok(ExternFn {
            annotation,
            abi,
            name,
            params,
            return_type,
            span: Span::new(start, end),
        })
    }

    /// Parses a type alias: `type Name = TypeExpr`.
    fn parse_type_alias(&mut self, is_pub: bool) -> Result<TypeAlias, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Type)?;
        let (name, _) = self.expect_ident()?;
        self.expect(&TokenKind::Eq)?;
        let ty = self.parse_type_expr()?;
        let end = ty.span().end;
        Ok(TypeAlias {
            is_pub,
            name,
            ty,
            span: Span::new(start, end),
        })
    }

    /// Parses function parameters: `name: Type, name: Type`.
    ///
    /// Also handles `self`, `&self`, and `&mut self` sugar in impl methods.
    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            let start = self.peek().span.start;

            // Check for `&self` or `&mut self` sugar
            if self.at(&TokenKind::Amp) {
                if let Some(param) = self.try_parse_ref_self(start)? {
                    params.push(param);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                    continue;
                }
            }

            let (name, name_span) = self.expect_ident()?;

            // Bare `self` parameter (no type annotation) — used in impl methods
            if name == "self" && !self.at(&TokenKind::Colon) {
                let end = name_span.end;
                params.push(Param {
                    name,
                    ty: TypeExpr::Simple {
                        name: "Self".to_string(),
                        span: Span::new(start, end),
                    },
                    span: Span::new(start, end),
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                continue;
            }

            self.expect(&TokenKind::Colon)?;
            let ty = self.parse_type_expr()?;
            let end = ty.span().end;
            params.push(Param {
                name,
                ty,
                span: Span::new(start, end),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    /// Tries to parse `&self` or `&mut self` sugar.
    /// Returns `None` if the `&` is not followed by `self` or `mut self`.
    fn try_parse_ref_self(&mut self, start: usize) -> Result<Option<Param>, ParseError> {
        // Save position to backtrack if this isn't &self or &mut self
        let saved_pos = self.pos;

        self.advance(); // consume `&`

        let mutable = self.eat(&TokenKind::Mut);

        // Check if next token is `self`
        if let TokenKind::Ident(name) = self.peek_kind().clone() {
            if name == "self" {
                let end = self.peek().span.end;
                self.advance(); // consume `self`
                let self_type = TypeExpr::Simple {
                    name: "Self".to_string(),
                    span: Span::new(start, end),
                };
                let ty = TypeExpr::Reference {
                    mutable,
                    inner: Box::new(self_type),
                    span: Span::new(start, end),
                };
                return Ok(Some(Param {
                    name: "self".to_string(),
                    ty,
                    span: Span::new(start, end),
                }));
            }
        }

        // Not &self or &mut self — backtrack
        self.pos = saved_pos;
        Ok(None)
    }

    /// Tries to parse generic parameters: `<T, U: Bound>`.
    fn try_parse_generic_params(&mut self) -> Result<Vec<GenericParam>, ParseError> {
        if !self.eat(&TokenKind::Lt) {
            return Ok(Vec::new());
        }

        let mut params = Vec::new();
        while !self.at(&TokenKind::Gt) && !self.at_eof() {
            let start = self.peek().span.start;
            let (name, _) = self.expect_ident()?;

            let mut bounds = Vec::new();
            if self.eat(&TokenKind::Colon) {
                loop {
                    let bound = self.parse_trait_bound()?;
                    bounds.push(bound);
                    if !self.eat(&TokenKind::Plus) {
                        break;
                    }
                }
            }

            let end = self.prev_span().end;
            params.push(GenericParam {
                name,
                bounds,
                span: Span::new(start, end),
            });

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        self.expect(&TokenKind::Gt)?;
        Ok(params)
    }

    /// Parses a trait bound: `TraitName<TypeArgs>`.
    fn parse_trait_bound(&mut self) -> Result<TraitBound, ParseError> {
        let start = self.peek().span.start;
        let (name, _) = self.expect_ident()?;

        let mut type_args = Vec::new();
        if self.eat(&TokenKind::Lt) {
            while !self.at(&TokenKind::Gt) && !self.at_eof() {
                type_args.push(self.parse_type_expr()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Gt)?;
        }

        let end = self.prev_span().end;
        Ok(TraitBound {
            name,
            type_args,
            span: Span::new(start, end),
        })
    }

    /// Parses optional where clauses: `where T: Bound1, U: Bound2`.
    fn parse_where_clauses(&mut self) -> Result<Vec<ast::WhereClause>, ParseError> {
        use crate::lexer::token::TokenKind;
        if !self.eat(&TokenKind::Where) {
            return Ok(Vec::new());
        }

        let mut clauses = Vec::new();
        loop {
            // Stop at block start
            if self.at(&TokenKind::LBrace) || self.at_eof() {
                break;
            }

            let start = self.peek().span.start;
            let (name, _) = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;

            let mut bounds = Vec::new();
            loop {
                let bound = self.parse_trait_bound()?;
                bounds.push(bound);
                if !self.eat(&TokenKind::Plus) {
                    break;
                }
            }

            let end = self.prev_span().end;
            clauses.push(ast::WhereClause {
                name,
                bounds,
                span: Span::new(start, end),
            });

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        Ok(clauses)
    }

    /// Parses a struct definition: `struct Name { fields }`.
    fn parse_struct_def(
        &mut self,
        is_pub: bool,
        annotation: Option<Annotation>,
    ) -> Result<StructDef, ParseError> {
        let start = if let Some(ref ann) = annotation {
            ann.span.start
        } else {
            self.peek().span.start
        };

        self.expect(&TokenKind::Struct)?;
        let (name, _) = self.expect_ident()?;
        let generic_params = self.try_parse_generic_params()?;

        self.expect(&TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let fstart = self.peek().span.start;
            let (fname, _) = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let ty = self.parse_type_expr()?;
            let fend = ty.span().end;
            fields.push(Field {
                name: fname,
                ty,
                span: Span::new(fstart, fend),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end_tok = self.expect(&TokenKind::RBrace)?;

        Ok(StructDef {
            is_pub,
            doc_comment: None,
            annotation,
            name,
            generic_params,
            fields,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses a union definition: `union Name { fields }`.
    fn parse_union_def(
        &mut self,
        is_pub: bool,
        annotation: Option<Annotation>,
    ) -> Result<UnionDef, ParseError> {
        let start = if let Some(ref ann) = annotation {
            ann.span.start
        } else {
            self.peek().span.start
        };

        self.expect(&TokenKind::Union)?;
        let (name, _) = self.expect_ident()?;

        self.expect(&TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let fstart = self.peek().span.start;
            let (fname, _) = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let ty = self.parse_type_expr()?;
            let fend = ty.span().end;
            fields.push(Field {
                name: fname,
                ty,
                span: Span::new(fstart, fend),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end_tok = self.expect(&TokenKind::RBrace)?;

        Ok(UnionDef {
            is_pub,
            doc_comment: None,
            annotation,
            name,
            fields,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses an enum definition: `enum Name { variants }`.
    fn parse_enum_def(
        &mut self,
        is_pub: bool,
        annotation: Option<Annotation>,
    ) -> Result<EnumDef, ParseError> {
        let start = if let Some(ref ann) = annotation {
            ann.span.start
        } else {
            self.peek().span.start
        };

        self.expect(&TokenKind::Enum)?;
        let (name, _) = self.expect_ident()?;
        let generic_params = self.try_parse_generic_params()?;

        self.expect(&TokenKind::LBrace)?;
        let mut variants = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let vstart = self.peek().span.start;
            let (vname, _) = self.expect_ident()?;

            let mut fields = Vec::new();
            if self.eat(&TokenKind::LParen) {
                while !self.at(&TokenKind::RParen) && !self.at_eof() {
                    fields.push(self.parse_type_expr()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen)?;
            }

            let vend = self.prev_span().end;
            variants.push(Variant {
                name: vname,
                fields,
                span: Span::new(vstart, vend),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end_tok = self.expect(&TokenKind::RBrace)?;

        Ok(EnumDef {
            is_pub,
            doc_comment: None,
            annotation,
            name,
            generic_params,
            variants,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses an impl block: `impl [Trait for] Type { methods }`.
    fn parse_impl_block(&mut self) -> Result<ImplBlock, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Impl)?;

        let generic_params = self.try_parse_generic_params()?;

        let (first_name, _) = self.expect_ident()?;

        // Check for `Trait for Type` pattern
        let (trait_name, target_type) = if self.eat(&TokenKind::For) {
            let (target, _) = self.expect_ident()?;
            (Some(first_name), target)
        } else {
            (None, first_name)
        };

        self.expect(&TokenKind::LBrace)?;
        let mut methods = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let is_method_pub = if matches!(self.peek_kind(), TokenKind::Pub) {
                self.advance();
                true
            } else {
                false
            };
            let ann = self.try_parse_annotation();
            let method = self.parse_fn_def(is_method_pub, ann)?;
            methods.push(method);
        }
        let end_tok = self.expect(&TokenKind::RBrace)?;

        Ok(ImplBlock {
            doc_comment: None,
            generic_params,
            trait_name,
            target_type,
            methods,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses a trait definition: `trait Name { methods }`.
    fn parse_trait_def(&mut self, is_pub: bool) -> Result<TraitDef, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Trait)?;
        let (name, _) = self.expect_ident()?;
        let generic_params = self.try_parse_generic_params()?;

        self.expect(&TokenKind::LBrace)?;
        let mut methods = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let ann = self.try_parse_annotation();
            let method = self.parse_trait_method(ann)?;
            methods.push(method);
        }
        let end_tok = self.expect(&TokenKind::RBrace)?;

        Ok(TraitDef {
            is_pub,
            doc_comment: None,
            name,
            generic_params,
            methods,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses a trait method: signature with optional body.
    ///
    /// If no `{` follows the signature, an empty block body is used (abstract method).
    fn parse_trait_method(&mut self, annotation: Option<Annotation>) -> Result<FnDef, ParseError> {
        let start = if let Some(ref ann) = annotation {
            ann.span.start
        } else {
            self.peek().span.start
        };

        let is_async = self.eat(&TokenKind::Async);

        self.expect(&TokenKind::Fn)?;
        let (name, _) = self.expect_ident()?;
        let generic_params = self.try_parse_generic_params()?;

        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;

        let return_type = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let where_clauses = self.parse_where_clauses()?;

        // Body is optional for trait methods — if no `{`, use empty block
        let (body, end) = if self.at(&TokenKind::LBrace) {
            let b = Box::new(self.parse_block_expr()?);
            let e = b.span().end;
            (b, e)
        } else {
            let end = self.prev_span().end;
            let empty = Box::new(Expr::Block {
                stmts: vec![],
                expr: None,
                span: Span::new(end, end),
            });
            (empty, end)
        };

        Ok(FnDef {
            is_pub: false,
            is_async,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            doc_comment: None,
            annotation,
            name,
            generic_params,
            params,
            return_type,
            where_clauses,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses a const definition: `const NAME: Type = value`.
    fn parse_const_def(
        &mut self,
        is_pub: bool,
        annotation: Option<Annotation>,
    ) -> Result<ConstDef, ParseError> {
        let start = if let Some(ref ann) = annotation {
            ann.span.start
        } else {
            self.peek().span.start
        };

        self.expect(&TokenKind::Const)?;
        let (name, _) = self.expect_ident()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.parse_type_expr()?;
        self.expect(&TokenKind::Eq)?;
        let value = Box::new(self.parse_expr(0)?);
        let end = value.span().end;
        self.eat_semi();

        Ok(ConstDef {
            is_pub,
            doc_comment: None,
            annotation,
            name,
            ty,
            value,
            span: Span::new(start, end),
        })
    }

    /// Parses a use declaration: `use path::to::item`.
    fn parse_use_decl(&mut self) -> Result<UseDecl, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Use)?;

        let mut path = Vec::new();
        let (first, _) = self.expect_ident()?;
        path.push(first);

        while self.eat(&TokenKind::ColonColon) {
            // Check for glob or group
            if self.eat(&TokenKind::Star) {
                let end = self.prev_span().end;
                self.eat_semi();
                return Ok(UseDecl {
                    path,
                    kind: UseKind::Glob,
                    span: Span::new(start, end),
                });
            }
            if self.eat(&TokenKind::LBrace) {
                let mut names = Vec::new();
                while !self.at(&TokenKind::RBrace) && !self.at_eof() {
                    let (name, _) = self.expect_ident()?;
                    names.push(name);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RBrace)?;
                let end = self.prev_span().end;
                self.eat_semi();
                return Ok(UseDecl {
                    path,
                    kind: UseKind::Group(names),
                    span: Span::new(start, end),
                });
            }

            let (segment, _) = self.expect_ident()?;
            path.push(segment);
        }

        let end = self.prev_span().end;
        self.eat_semi();
        Ok(UseDecl {
            path,
            kind: UseKind::Simple,
            span: Span::new(start, end),
        })
    }

    /// Parses a module declaration: `mod name { items }` or `mod name`.
    fn parse_mod_decl(&mut self) -> Result<ModDecl, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Mod)?;
        let (name, _) = self.expect_ident()?;

        let body = if self.eat(&TokenKind::LBrace) {
            let mut items = Vec::new();
            while !self.at(&TokenKind::RBrace) && !self.at_eof() {
                match self.parse_item_or_stmt() {
                    Ok(item) => items.push(item),
                    Err(e) => {
                        self.errors.push(e);
                        self.synchronize();
                    }
                }
            }
            self.expect(&TokenKind::RBrace)?;
            Some(items)
        } else {
            self.eat_semi();
            None
        };

        let end = self.prev_span().end;
        Ok(ModDecl {
            name,
            body,
            span: Span::new(start, end),
        })
    }

    // ── Statements ─────────────────────────────────────────────────────

    /// Parses a statement.
    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek_kind() {
            TokenKind::Let => self.parse_let_stmt(),
            TokenKind::Return => self.parse_return_stmt(),
            TokenKind::Break => self.parse_break_stmt(),
            TokenKind::Continue => self.parse_continue_stmt(),
            _ => {
                let expr = self.parse_expr(0)?;
                let span = expr.span();

                // Check for assignment
                if let Some(op) = token_to_assignop(self.peek_kind()) {
                    self.advance();
                    let value = self.parse_expr(0)?;
                    let end = value.span().end;
                    self.eat_semi();
                    return Ok(Stmt::Expr {
                        expr: Box::new(Expr::Assign {
                            target: Box::new(expr),
                            op,
                            value: Box::new(value),
                            span: Span::new(span.start, end),
                        }),
                        span: Span::new(span.start, end),
                    });
                }

                self.eat_semi();
                Ok(Stmt::Expr {
                    expr: Box::new(expr),
                    span,
                })
            }
        }
    }

    /// Parses a let statement: `let [mut] name [: Type] = value`.
    fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Let)?;

        let mutable = self.eat(&TokenKind::Mut);
        let (name, _) = self.expect_ident()?;

        let ty = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        self.expect(&TokenKind::Eq)?;
        let value = Box::new(self.parse_expr(0)?);
        let end = value.span().end;
        self.eat_semi();

        Ok(Stmt::Let {
            mutable,
            name,
            ty,
            value,
            span: Span::new(start, end),
        })
    }

    /// Parses a return statement: `return [expr]`.
    fn parse_return_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Return)?;

        let value = if !self.at(&TokenKind::Semi) && !self.at(&TokenKind::RBrace) && !self.at_eof()
        {
            Some(Box::new(self.parse_expr(0)?))
        } else {
            None
        };

        let end = value
            .as_ref()
            .map_or(self.prev_span().end, |v| v.span().end);
        self.eat_semi();

        Ok(Stmt::Return {
            value,
            span: Span::new(start, end),
        })
    }

    /// Parses a break statement: `break [expr]`.
    fn parse_break_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Break)?;

        let value = if !self.at(&TokenKind::Semi) && !self.at(&TokenKind::RBrace) && !self.at_eof()
        {
            Some(Box::new(self.parse_expr(0)?))
        } else {
            None
        };

        let end = value
            .as_ref()
            .map_or(self.prev_span().end, |v| v.span().end);
        self.eat_semi();

        Ok(Stmt::Break {
            value,
            span: Span::new(start, end),
        })
    }

    /// Parses a continue statement: `continue`.
    fn parse_continue_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        let end_tok = self.expect(&TokenKind::Continue)?;
        self.eat_semi();

        Ok(Stmt::Continue {
            span: Span::new(start, end_tok.span.end),
        })
    }

    // ── Expressions (Pratt parser) ─────────────────────────────────────

    /// Parses an expression using Pratt parsing with the given minimum binding power.
    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        // ── Prefix / Primary ──
        let mut lhs = self.parse_prefix()?;

        loop {
            if self.at_eof() {
                break;
            }

            let kind = self.peek_kind().clone();

            // ── Postfix operators ──
            if let Some((l_bp, ())) = postfix_binding_power(&kind) {
                if l_bp < min_bp {
                    break;
                }

                lhs = self.parse_postfix(lhs)?;
                continue;
            }

            // ── Assignment (Level 1, Right-assoc, handled here) ──
            if let Some(op) = token_to_assignop(&kind) {
                // Assignment BP = 1 (lowest), right-assoc: (1, 2)
                if 1 < min_bp {
                    break;
                }
                self.advance();
                let rhs = self.parse_expr(2)?;
                let span = Span::new(lhs.span().start, rhs.span().end);
                lhs = Expr::Assign {
                    target: Box::new(lhs),
                    op,
                    value: Box::new(rhs),
                    span,
                };
                continue;
            }

            // ── `as` cast (Level 15) ──
            if matches!(kind, TokenKind::As) {
                let (l_bp, _) = infix_binding_power(&kind).unwrap_or((0, 0));
                if l_bp < min_bp {
                    break;
                }
                self.advance();
                let ty = self.parse_type_expr()?;
                let span = Span::new(lhs.span().start, ty.span().end);
                lhs = Expr::Cast {
                    expr: Box::new(lhs),
                    ty,
                    span,
                };
                continue;
            }

            // ── Pipeline |> ──
            if matches!(kind, TokenKind::PipeGt) {
                let (l_bp, r_bp) = infix_binding_power(&kind).unwrap();
                if l_bp < min_bp {
                    break;
                }
                self.advance();
                let rhs = self.parse_expr(r_bp)?;
                let span = Span::new(lhs.span().start, rhs.span().end);
                lhs = Expr::Pipe {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                    span,
                };
                continue;
            }

            // ── Range .. / ..= ──
            if matches!(kind, TokenKind::DotDot | TokenKind::DotDotEq) {
                let (l_bp, _r_bp) = infix_binding_power(&kind).unwrap();
                if l_bp < min_bp {
                    break;
                }
                let inclusive = matches!(kind, TokenKind::DotDotEq);
                self.advance();
                // Range end is optional
                let end = if !self.at_eof()
                    && !self.at(&TokenKind::Semi)
                    && !self.at(&TokenKind::RBrace)
                    && !self.at(&TokenKind::RParen)
                    && !self.at(&TokenKind::RBracket)
                    && !self.at(&TokenKind::Comma)
                    && !self.at(&TokenKind::LBrace)
                {
                    Some(Box::new(self.parse_expr(min_bp)?))
                } else {
                    None
                };
                let span_end = end.as_ref().map_or(self.prev_span().end, |e| e.span().end);
                let lhs_start = lhs.span().start;
                lhs = Expr::Range {
                    start: Some(Box::new(lhs)),
                    end,
                    inclusive,
                    span: Span::new(lhs_start, span_end),
                };
                continue;
            }

            // ── Infix binary operators ──
            if let Some((l_bp, r_bp)) = infix_binding_power(&kind) {
                if l_bp < min_bp {
                    break;
                }

                let op = token_to_binop(&kind);
                if let Some(op) = op {
                    self.advance();
                    let rhs = self.parse_expr(r_bp)?;
                    let span = Span::new(lhs.span().start, rhs.span().end);
                    lhs = Expr::Binary {
                        left: Box::new(lhs),
                        op,
                        right: Box::new(rhs),
                        span,
                    };
                    continue;
                }
            }

            break;
        }

        Ok(lhs)
    }

    /// Parses a prefix expression (unary operators and primary expressions).
    fn parse_prefix(&mut self) -> Result<Expr, ParseError> {
        let kind = self.peek_kind().clone();

        // Unary operators
        if let Some(((), r_bp)) = prefix_binding_power(&kind) {
            let token = self.advance().clone();

            // Special case: &mut
            let op = if matches!(kind, TokenKind::Amp) && self.eat(&TokenKind::Mut) {
                UnaryOp::RefMut
            } else {
                token_to_unaryop(&kind).expect("prefix_binding_power returned Some")
            };

            let operand = self.parse_expr(r_bp)?;
            let span = Span::new(token.span.start, operand.span().end);
            return Ok(Expr::Unary {
                op,
                operand: Box::new(operand),
                span,
            });
        }

        // Primary expressions
        self.parse_primary()
    }

    /// Parses a primary (atomic) expression.
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.peek().clone();

        match &token.kind {
            // Literals
            TokenKind::IntLit(v) => {
                let v = *v;
                self.advance();
                Ok(Expr::Literal {
                    kind: LiteralKind::Int(v),
                    span: token.span,
                })
            }
            TokenKind::FloatLit(v) => {
                let v = *v;
                self.advance();
                Ok(Expr::Literal {
                    kind: LiteralKind::Float(v),
                    span: token.span,
                })
            }
            TokenKind::StringLit(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::Literal {
                    kind: LiteralKind::String(s),
                    span: token.span,
                })
            }
            TokenKind::RawStringLit(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::Literal {
                    kind: LiteralKind::RawString(s),
                    span: token.span,
                })
            }
            TokenKind::FStringLit(parts) => {
                let parts = parts.clone();
                let fspan = token.span;
                self.advance();
                let mut expr_parts = Vec::new();
                for part in parts {
                    match part {
                        crate::lexer::token::FStringPart::Literal(s) => {
                            expr_parts.push(FStringExprPart::Literal(s));
                        }
                        crate::lexer::token::FStringPart::Expr(src) => {
                            // Parse the expression source code
                            let expr_tokens = crate::lexer::tokenize(&src).map_err(|_| {
                                ParseError::UnexpectedToken {
                                    expected: "valid expression in f-string".into(),
                                    found: src.clone(),
                                    line: token.line,
                                    col: token.col,
                                    span: fspan,
                                }
                            })?;
                            let mut sub_parser = Parser::new(expr_tokens);
                            let expr = sub_parser.parse_expr(0)?;
                            expr_parts.push(FStringExprPart::Expr(Box::new(expr)));
                        }
                    }
                }
                Ok(Expr::FString {
                    parts: expr_parts,
                    span: fspan,
                })
            }
            TokenKind::CharLit(c) => {
                let c = *c;
                self.advance();
                Ok(Expr::Literal {
                    kind: LiteralKind::Char(c),
                    span: token.span,
                })
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Literal {
                    kind: LiteralKind::Bool(true),
                    span: token.span,
                })
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Literal {
                    kind: LiteralKind::Bool(false),
                    span: token.span,
                })
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expr::Literal {
                    kind: LiteralKind::Null,
                    span: token.span,
                })
            }

            // Identifier (may be path like std::io::println)
            TokenKind::Ident(_) => self.parse_ident_expr(),

            // Contextual keywords used as identifiers (e.g., addr, page, layer)
            kind if Self::is_contextual_keyword(kind) => {
                let name = format!("{kind}");
                self.advance();
                Ok(Expr::Ident {
                    name,
                    span: token.span,
                })
            }

            // Grouped expression or tuple: (expr) or (a, b)
            TokenKind::LParen => self.parse_grouped_or_tuple(),

            // Block expression: { stmts; expr }
            TokenKind::LBrace => self.parse_block_expr(),

            // If expression
            TokenKind::If => self.parse_if_expr(),

            // Match expression
            TokenKind::Match => self.parse_match_expr(),

            // While loop
            TokenKind::While => self.parse_while_expr(),

            // For loop
            TokenKind::For => self.parse_for_expr(),

            // Loop (infinite)
            TokenKind::Loop => self.parse_loop_expr(),

            // Array literal: [a, b, c]
            TokenKind::LBracket => self.parse_array_expr(),

            // Async block: async { body }
            TokenKind::Async => {
                let start = token.span.start;
                self.advance(); // eat `async`
                let body = self.parse_block_expr()?;
                let end = body.span().end;
                Ok(Expr::AsyncBlock {
                    body: Box::new(body),
                    span: Span::new(start, end),
                })
            }

            // Closure: |params| body
            TokenKind::Pipe | TokenKind::PipePipe => self.parse_closure_expr(),

            // Return/break/continue as expressions
            TokenKind::Return => {
                let stmt = self.parse_return_stmt()?;
                match stmt {
                    Stmt::Return { value, span } => Ok(value.map_or(
                        Expr::Literal {
                            kind: LiteralKind::Null,
                            span,
                        },
                        |v| *v,
                    )),
                    _ => unreachable!(),
                }
            }

            _ => Err(ParseError::ExpectedExpression {
                line: token.line,
                col: token.col,
                span: token.span,
            }),
        }
    }

    /// Parses an identifier expression, potentially a path (`a::b::c`) or struct init.
    fn parse_ident_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        let (name, name_span) = self.expect_ident()?;

        // Handle asm!(...) — inline assembly macro
        if name == "asm" && self.at(&TokenKind::Bang) {
            return self.parse_inline_asm(start);
        }

        // Check for path: ident::ident::...
        if self.at(&TokenKind::ColonColon) {
            let mut segments = vec![name];
            while self.eat(&TokenKind::ColonColon) {
                let (seg, _) = self.expect_ident()?;
                segments.push(seg);
            }
            let end = self.prev_span().end;
            return Ok(Expr::Path {
                segments,
                span: Span::new(start, end),
            });
        }

        // Check for struct init: Name { field: value, ... }
        // Only if the identifier starts with uppercase (convention for struct names)
        if self.at(&TokenKind::LBrace) && name.starts_with(|c: char| c.is_uppercase()) {
            return self.parse_struct_init(name, start);
        }

        Ok(Expr::Ident {
            name,
            span: name_span,
        })
    }

    /// Parses struct initialization: `Name { field: value, ... }`.
    fn parse_struct_init(&mut self, name: String, start: usize) -> Result<Expr, ParseError> {
        self.expect(&TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut seen_fields = std::collections::HashSet::new();

        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let fstart = self.peek().span.start;
            let (fname, _) = self.expect_ident()?;

            // PE008: Check for duplicate fields
            if !seen_fields.insert(fname.clone()) {
                self.errors.push(ParseError::DuplicateField {
                    field: fname.clone(),
                    line: self.tokens[self.pos.saturating_sub(1)].line,
                    col: self.tokens[self.pos.saturating_sub(1)].col,
                    span: Span::new(fstart, self.peek().span.start),
                });
            }

            self.expect(&TokenKind::Colon)?;
            let value = self.parse_expr(0)?;
            let fend = value.span().end;
            fields.push(FieldInit {
                name: fname,
                value,
                span: Span::new(fstart, fend),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        let end_tok = self.expect(&TokenKind::RBrace)?;
        Ok(Expr::StructInit {
            name,
            fields,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses a grouped expression `(expr)` or tuple `(a, b, c)`.
    fn parse_grouped_or_tuple(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::LParen)?;

        // Empty tuple: ()
        if self.at(&TokenKind::RParen) {
            let end_tok = self.expect(&TokenKind::RParen)?;
            return Ok(Expr::Tuple {
                elements: vec![],
                span: Span::new(start, end_tok.span.end),
            });
        }

        let first = self.parse_expr(0)?;

        // Check for tuple: (a, b, ...)
        if self.eat(&TokenKind::Comma) {
            let mut elements = vec![first];
            while !self.at(&TokenKind::RParen) && !self.at_eof() {
                elements.push(self.parse_expr(0)?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            let end_tok = self.expect(&TokenKind::RParen)?;
            return Ok(Expr::Tuple {
                elements,
                span: Span::new(start, end_tok.span.end),
            });
        }

        // Single grouped expression: (expr)
        let end_tok = self.expect(&TokenKind::RParen)?;
        Ok(Expr::Grouped {
            expr: Box::new(first),
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses a block expression: `{ stmts; [expr] }`.
    fn parse_block_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::LBrace)?;

        let mut stmts = Vec::new();
        let mut final_expr: Option<Box<Expr>> = None;

        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            // Try parsing as an item/statement
            match self.peek_kind() {
                TokenKind::Let | TokenKind::Return | TokenKind::Break | TokenKind::Continue => {
                    stmts.push(self.parse_stmt()?);
                }
                TokenKind::Fn | TokenKind::Struct | TokenKind::Union | TokenKind::Enum => {
                    let item = self.parse_item_or_stmt()?;
                    stmts.push(Stmt::Item(Box::new(item)));
                }
                _ => {
                    let expr = self.parse_expr(0)?;

                    // Check for assignment after expression
                    if let Some(op) = token_to_assignop(self.peek_kind()) {
                        self.advance();
                        let value = self.parse_expr(0)?;
                        let span = Span::new(expr.span().start, value.span().end);
                        let assign_expr = Expr::Assign {
                            target: Box::new(expr),
                            op,
                            value: Box::new(value),
                            span,
                        };
                        self.eat_semi();
                        stmts.push(Stmt::Expr {
                            span,
                            expr: Box::new(assign_expr),
                        });
                        continue;
                    }

                    if self.eat(&TokenKind::Semi) {
                        // Expression with semicolon → statement
                        let span = expr.span();
                        stmts.push(Stmt::Expr {
                            expr: Box::new(expr),
                            span,
                        });
                    } else if self.at(&TokenKind::RBrace) {
                        // Last expression without semicolon → block value
                        final_expr = Some(Box::new(expr));
                    } else {
                        // Expression without semicolon, not at end → statement
                        let span = expr.span();
                        stmts.push(Stmt::Expr {
                            expr: Box::new(expr),
                            span,
                        });
                    }
                }
            }
        }

        let end_tok = self.expect(&TokenKind::RBrace)?;
        Ok(Expr::Block {
            stmts,
            expr: final_expr,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses an if expression: `if cond { then } [else { else_ }]`.
    fn parse_if_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::If)?;

        let condition = Box::new(self.parse_expr(0)?);
        let then_branch = Box::new(self.parse_block_expr()?);

        let else_branch = if self.eat(&TokenKind::Else) {
            if self.at(&TokenKind::If) {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                Some(Box::new(self.parse_block_expr()?))
            }
        } else {
            None
        };

        let end = else_branch
            .as_ref()
            .map_or(then_branch.span().end, |e| e.span().end);

        Ok(Expr::If {
            condition,
            then_branch,
            else_branch,
            span: Span::new(start, end),
        })
    }

    /// Parses a match expression: `match subject { arms }`.
    fn parse_match_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Match)?;

        let subject = Box::new(self.parse_expr(0)?);
        self.expect(&TokenKind::LBrace)?;

        let mut arms = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let arm = self.parse_match_arm()?;
            arms.push(arm);
            self.eat(&TokenKind::Comma);
        }

        let end_tok = self.expect(&TokenKind::RBrace)?;

        Ok(Expr::Match {
            subject,
            arms,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses a single match arm: `pattern [if guard] => body`.
    fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let start = self.peek().span.start;
        let pattern = self.parse_pattern()?;

        let guard = if self.eat(&TokenKind::If) {
            Some(Box::new(self.parse_expr(0)?))
        } else {
            None
        };

        self.expect(&TokenKind::FatArrow)?;
        let body = Box::new(self.parse_expr(0)?);
        let end = body.span().end;

        Ok(MatchArm {
            pattern,
            guard,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses a while loop: `while cond { body }`.
    fn parse_while_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::While)?;

        let condition = Box::new(self.parse_expr(0)?);
        let body = Box::new(self.parse_block_expr()?);
        let end = body.span().end;

        Ok(Expr::While {
            condition,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses a for loop: `for var in iter { body }`.
    fn parse_for_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::For)?;

        let (variable, _) = self.expect_ident()?;
        self.expect(&TokenKind::In)?;
        let iterable = Box::new(self.parse_expr(0)?);
        let body = Box::new(self.parse_block_expr()?);
        let end = body.span().end;

        Ok(Expr::For {
            variable,
            iterable,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses a loop expression: `loop { body }`.
    fn parse_loop_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Loop)?;
        let body = Box::new(self.parse_block_expr()?);
        let end = body.span().end;
        Ok(Expr::Loop {
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses an array literal: `[a, b, c]`.
    fn parse_array_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::LBracket)?;

        let mut elements = Vec::new();
        while !self.at(&TokenKind::RBracket) && !self.at_eof() {
            elements.push(self.parse_expr(0)?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        let end_tok = self.expect(&TokenKind::RBracket)?;
        Ok(Expr::Array {
            elements,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses an inline assembly expression: `asm!("template", operands...)`.
    ///
    /// Called after `asm` has been consumed. Expects `!` and `(` next.
    fn parse_inline_asm(&mut self, start: usize) -> Result<Expr, ParseError> {
        self.expect(&TokenKind::Bang)?;
        self.expect(&TokenKind::LParen)?;

        // First argument must be a string template
        let template = match self.peek_kind() {
            TokenKind::StringLit(s) => {
                let s = s.clone();
                self.advance();
                s
            }
            _ => {
                let tok = self.peek().clone();
                return Err(ParseError::UnexpectedToken {
                    expected: "assembly template string".into(),
                    found: format!("{}", tok.kind),
                    line: tok.line,
                    col: tok.col,
                    span: tok.span,
                });
            }
        };

        // Parse optional operands, options, and clobber_abi after comma
        let mut operands = Vec::new();
        let mut options = Vec::new();
        let mut clobber_abi = None;
        while self.eat(&TokenKind::Comma) {
            if self.at(&TokenKind::RParen) {
                break;
            }
            // Check for `options(...)` or `clobber_abi("...")`
            if let TokenKind::Ident(name) = self.peek_kind() {
                if name == "options" {
                    self.advance();
                    self.expect(&TokenKind::LParen)?;
                    while !self.at(&TokenKind::RParen) {
                        if let TokenKind::Ident(opt) = self.peek_kind() {
                            let opt = opt.clone();
                            self.advance();
                            match opt.as_str() {
                                "nomem" => options.push(AsmOption::Nomem),
                                "nostack" => options.push(AsmOption::Nostack),
                                "readonly" => options.push(AsmOption::Readonly),
                                "preserves_flags" => options.push(AsmOption::PreservesFlags),
                                "pure" => options.push(AsmOption::Pure),
                                "att_syntax" => options.push(AsmOption::AttSyntax),
                                _ => {
                                    return Err(ParseError::UnexpectedToken {
                                        expected: "asm option (nomem, nostack, readonly, preserves_flags, pure, att_syntax)".into(),
                                        found: opt,
                                        line: self.peek().line,
                                        col: self.peek().col,
                                        span: self.peek().span,
                                    });
                                }
                            }
                            // Eat optional comma between options
                            self.eat(&TokenKind::Comma);
                        } else {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    continue;
                } else if name == "clobber_abi" {
                    self.advance();
                    self.expect(&TokenKind::LParen)?;
                    if let TokenKind::StringLit(abi) = self.peek_kind() {
                        clobber_abi = Some(abi.clone());
                        self.advance();
                    } else {
                        let tok = self.peek().clone();
                        return Err(ParseError::UnexpectedToken {
                            expected: "ABI string (e.g. \"C\")".into(),
                            found: format!("{}", tok.kind),
                            line: tok.line,
                            col: tok.col,
                            span: tok.span,
                        });
                    }
                    self.expect(&TokenKind::RParen)?;
                    continue;
                }
            }
            // Parse operand: in(reg) expr | out(reg) expr | inout(reg) expr | const expr
            let op = self.parse_asm_operand()?;
            operands.push(op);
        }

        let end_tok = self.expect(&TokenKind::RParen)?;
        Ok(Expr::InlineAsm {
            template,
            operands,
            options,
            clobber_abi,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses a global assembly item: `global_asm!(".section .text\n...")`.
    ///
    /// Called when `global_asm` identifier is detected at top level.
    fn parse_global_asm(&mut self) -> Result<GlobalAsm, ParseError> {
        let start = self.peek().span.start;
        self.advance(); // consume "global_asm"
        self.expect(&TokenKind::Bang)?;
        self.expect(&TokenKind::LParen)?;

        let template = match self.peek_kind() {
            TokenKind::StringLit(s) => {
                let s = s.clone();
                self.advance();
                s
            }
            _ => {
                let tok = self.peek().clone();
                return Err(ParseError::UnexpectedToken {
                    expected: "assembly template string".into(),
                    found: format!("{}", tok.kind),
                    line: tok.line,
                    col: tok.col,
                    span: tok.span,
                });
            }
        };

        let end_tok = self.expect(&TokenKind::RParen)?;
        Ok(GlobalAsm {
            template,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses a single inline assembly operand.
    fn parse_asm_operand(&mut self) -> Result<AsmOperand, ParseError> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::Const => {
                self.advance();
                let expr = Box::new(self.parse_expr(0)?);
                Ok(AsmOperand::Const { expr })
            }
            // `in` is a keyword, so handle it specially
            TokenKind::In => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let constraint = match self.peek_kind() {
                    TokenKind::Ident(c) => {
                        let c = c.clone();
                        self.advance();
                        c
                    }
                    TokenKind::StringLit(c) => {
                        let c = c.clone();
                        self.advance();
                        c
                    }
                    _ => "reg".to_string(),
                };
                self.expect(&TokenKind::RParen)?;
                let expr = Box::new(self.parse_expr(0)?);
                Ok(AsmOperand::In { constraint, expr })
            }
            TokenKind::Ident(name) if name == "sym" => {
                self.advance();
                // sym operand: next token is the symbol name
                if let TokenKind::Ident(sym_name) = self.peek_kind() {
                    let sym_name = sym_name.clone();
                    self.advance();
                    Ok(AsmOperand::Sym { name: sym_name })
                } else {
                    Err(ParseError::UnexpectedToken {
                        expected: "symbol name after 'sym'".into(),
                        found: format!("{}", self.peek().kind),
                        line: self.peek().line,
                        col: self.peek().col,
                        span: self.peek().span,
                    })
                }
            }
            TokenKind::Ident(name) => {
                let direction = name.clone();
                self.advance();
                // Expect (constraint)
                self.expect(&TokenKind::LParen)?;
                let constraint = match self.peek_kind() {
                    TokenKind::Ident(c) => {
                        let c = c.clone();
                        self.advance();
                        c
                    }
                    TokenKind::StringLit(c) => {
                        let c = c.clone();
                        self.advance();
                        c
                    }
                    _ => "reg".to_string(),
                };
                self.expect(&TokenKind::RParen)?;
                let expr = Box::new(self.parse_expr(0)?);
                match direction.as_str() {
                    "in" => Ok(AsmOperand::In { constraint, expr }),
                    "out" => Ok(AsmOperand::Out { constraint, expr }),
                    "inout" => Ok(AsmOperand::InOut { constraint, expr }),
                    _ => Err(ParseError::UnexpectedToken {
                        expected: "in, out, inout, or const".into(),
                        found: direction,
                        line: tok.line,
                        col: tok.col,
                        span: tok.span,
                    }),
                }
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "asm operand (in/out/inout/const)".into(),
                found: format!("{}", tok.kind),
                line: tok.line,
                col: tok.col,
                span: tok.span,
            }),
        }
    }

    /// Parses a closure expression: `|params| body` or `|| body`.
    fn parse_closure_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;

        // Handle || (empty params) — PipePipe is lexed as one token
        let params = if self.eat(&TokenKind::PipePipe) {
            Vec::new()
        } else {
            self.expect(&TokenKind::Pipe)?;
            let mut params = Vec::new();
            while !self.at(&TokenKind::Pipe) && !self.at_eof() {
                let pstart = self.peek().span.start;
                let (name, _) = self.expect_ident()?;
                let ty = if self.eat(&TokenKind::Colon) {
                    Some(self.parse_type_expr()?)
                } else {
                    None
                };
                let pend = self.prev_span().end;
                params.push(ClosureParam {
                    name,
                    ty,
                    span: Span::new(pstart, pend),
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Pipe)?;
            params
        };

        let return_type = if self.eat(&TokenKind::Arrow) {
            Some(Box::new(self.parse_type_expr()?))
        } else {
            None
        };

        let body = if self.at(&TokenKind::LBrace) {
            Box::new(self.parse_block_expr()?)
        } else {
            Box::new(self.parse_expr(0)?)
        };
        let end = body.span().end;

        Ok(Expr::Closure {
            params,
            return_type,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses postfix operations: function call, indexing, field access, method call, try.
    fn parse_postfix(&mut self, lhs: Expr) -> Result<Expr, ParseError> {
        let kind = self.peek_kind().clone();
        match kind {
            // Try: expr?
            TokenKind::Question => {
                self.advance();
                let span = Span::new(lhs.span().start, self.prev_span().end);
                Ok(Expr::Try {
                    expr: Box::new(lhs),
                    span,
                })
            }

            // Function call: expr(args)
            TokenKind::LParen => {
                self.advance();
                let mut args = Vec::new();
                while !self.at(&TokenKind::RParen) && !self.at_eof() {
                    let arg_start = self.peek().span.start;

                    // Check for named argument: name: value
                    let (name, value) = if matches!(self.peek_kind(), TokenKind::Ident(_))
                        && matches!(self.peek_at(1).kind, TokenKind::Colon)
                    {
                        let (n, _) = self.expect_ident()?;
                        self.expect(&TokenKind::Colon)?;
                        let v = self.parse_expr(0)?;
                        (Some(n), v)
                    } else {
                        let v = self.parse_expr(0)?;
                        (None, v)
                    };

                    let arg_end = value.span().end;
                    args.push(CallArg {
                        name,
                        value,
                        span: Span::new(arg_start, arg_end),
                    });
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                let end_tok = self.expect(&TokenKind::RParen)?;
                let span = Span::new(lhs.span().start, end_tok.span.end);
                Ok(Expr::Call {
                    callee: Box::new(lhs),
                    args,
                    span,
                })
            }

            // Index: expr[index]
            TokenKind::LBracket => {
                self.advance();
                let index = self.parse_expr(0)?;
                let end_tok = self.expect(&TokenKind::RBracket)?;
                let span = Span::new(lhs.span().start, end_tok.span.end);
                Ok(Expr::Index {
                    object: Box::new(lhs),
                    index: Box::new(index),
                    span,
                })
            }

            // Field access, method call, or .await: expr.field, expr.method(args), expr.await
            // Also handles tuple index: expr.0, expr.1, etc.
            TokenKind::Dot => {
                self.advance();

                // Handle .await as a postfix expression
                if self.at(&TokenKind::Await) {
                    let await_span = self.peek().span;
                    self.advance();
                    let span = Span::new(lhs.span().start, await_span.end);
                    return Ok(Expr::Await {
                        expr: Box::new(lhs),
                        span,
                    });
                }

                let field_name = if let TokenKind::IntLit(n) = &self.peek_kind() {
                    let s = n.to_string();
                    self.advance();
                    s
                } else {
                    self.expect_ident()?.0
                };

                // Check if this is a method call: .method(args)
                if self.at(&TokenKind::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    while !self.at(&TokenKind::RParen) && !self.at_eof() {
                        let arg_start = self.peek().span.start;
                        let value = self.parse_expr(0)?;
                        let arg_end = value.span().end;
                        args.push(CallArg {
                            name: None,
                            value,
                            span: Span::new(arg_start, arg_end),
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    let end_tok = self.expect(&TokenKind::RParen)?;
                    let span = Span::new(lhs.span().start, end_tok.span.end);
                    Ok(Expr::MethodCall {
                        receiver: Box::new(lhs),
                        method: field_name,
                        args,
                        span,
                    })
                } else {
                    let span = Span::new(lhs.span().start, self.prev_span().end);
                    Ok(Expr::Field {
                        object: Box::new(lhs),
                        field: field_name,
                        span,
                    })
                }
            }

            _ => Ok(lhs),
        }
    }

    // ── Type expressions ───────────────────────────────────────────────

    /// Parses a type expression.
    fn parse_type_expr(&mut self) -> Result<TypeExpr, ParseError> {
        let token = self.peek().clone();

        match &token.kind {
            // Reference type: &T or &mut T
            TokenKind::Amp => {
                self.advance();
                let mutable = self.eat(&TokenKind::Mut);
                let inner = self.parse_type_expr()?;
                let span = Span::new(token.span.start, inner.span().end);
                Ok(TypeExpr::Reference {
                    mutable,
                    inner: Box::new(inner),
                    span,
                })
            }

            // Tuple type: (T1, T2) or function type: fn(T1) -> T2
            TokenKind::LParen => {
                self.advance();
                let mut elements = Vec::new();
                while !self.at(&TokenKind::RParen) && !self.at_eof() {
                    elements.push(self.parse_type_expr()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                let end_tok = self.expect(&TokenKind::RParen)?;
                Ok(TypeExpr::Tuple {
                    elements,
                    span: Span::new(token.span.start, end_tok.span.end),
                })
            }

            // Array or slice type: [T; N] or [T]
            TokenKind::LBracket => {
                self.advance();
                let element = self.parse_type_expr()?;
                if self.eat(&TokenKind::Semi) {
                    // Array: [T; N]
                    let size_token = self.peek().clone();
                    match &size_token.kind {
                        TokenKind::IntLit(n) => {
                            let n = *n as u64;
                            self.advance();
                            let end_tok = self.expect(&TokenKind::RBracket)?;
                            Ok(TypeExpr::Array {
                                element: Box::new(element),
                                size: n,
                                span: Span::new(token.span.start, end_tok.span.end),
                            })
                        }
                        _ => Err(ParseError::UnexpectedToken {
                            expected: "array size".into(),
                            found: format!("{}", size_token.kind),
                            line: size_token.line,
                            col: size_token.col,
                            span: size_token.span,
                        }),
                    }
                } else {
                    // Slice: [T]
                    let end_tok = self.expect(&TokenKind::RBracket)?;
                    Ok(TypeExpr::Slice {
                        element: Box::new(element),
                        span: Span::new(token.span.start, end_tok.span.end),
                    })
                }
            }

            // Function type: fn(params) -> RetType  or  fn(params)
            TokenKind::Fn => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let mut params = Vec::new();
                while !self.at(&TokenKind::RParen) && !self.at_eof() {
                    params.push(self.parse_type_expr()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen)?;
                let (return_type, end) = if self.eat(&TokenKind::Arrow) {
                    let ret = self.parse_type_expr()?;
                    let end = ret.span().end;
                    (ret, end)
                } else {
                    // Void function pointer: fn()
                    let prev = self.pos.saturating_sub(1);
                    let end_pos = if prev < self.tokens.len() {
                        self.tokens[prev].span.end
                    } else {
                        token.span.end
                    };
                    (
                        TypeExpr::Simple {
                            name: "void".to_string(),
                            span: Span::new(end_pos, end_pos),
                        },
                        end_pos,
                    )
                };
                Ok(TypeExpr::Fn {
                    params,
                    return_type: Box::new(return_type),
                    span: Span::new(token.span.start, end),
                })
            }

            // Trait object type: dyn Trait
            TokenKind::Dyn => {
                self.advance();
                let name_tok = self.peek().clone();
                let trait_name = match &name_tok.kind {
                    TokenKind::Ident(n) => n.clone(),
                    _ => {
                        return Err(ParseError::UnexpectedToken {
                            expected: "trait name".into(),
                            found: format!("{}", name_tok.kind),
                            line: name_tok.line,
                            col: name_tok.col,
                            span: name_tok.span,
                        });
                    }
                };
                self.advance();
                let span = Span::new(token.span.start, self.prev_span().end);
                Ok(TypeExpr::DynTrait { trait_name, span })
            }

            // Never type: !
            TokenKind::Bang => {
                self.advance();
                Ok(TypeExpr::Simple {
                    name: "never".to_string(),
                    span: token.span,
                })
            }

            // Named type or type keyword: i32, bool, Vec<T>, Tensor<f32>[3,4], path::Type
            _ => {
                let name = self.parse_type_name()?;
                let start = token.span.start;

                // Check for generic args: Name<T, U>
                if self.at(&TokenKind::Lt) {
                    // Disambiguate: is this `<` a generic arg or a comparison?
                    // For type position, always treat as generic.
                    self.advance();
                    let mut args = Vec::new();
                    while !self.at(&TokenKind::Gt) && !self.at_eof() {
                        args.push(self.parse_type_expr()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::Gt)?;

                    // Check for tensor dimensions: Tensor<f32>[3, 4]
                    if (name == "Tensor" || name == "tensor") && self.at(&TokenKind::LBracket) {
                        self.advance();
                        let mut dims = Vec::new();
                        while !self.at(&TokenKind::RBracket) && !self.at_eof() {
                            if self.eat(&TokenKind::Star) {
                                dims.push(None);
                            } else {
                                let dim_tok = self.peek().clone();
                                match &dim_tok.kind {
                                    TokenKind::IntLit(n) => {
                                        dims.push(Some(*n as u64));
                                        self.advance();
                                    }
                                    _ => {
                                        return Err(ParseError::UnexpectedToken {
                                            expected: "dimension size or *".into(),
                                            found: format!("{}", dim_tok.kind),
                                            line: dim_tok.line,
                                            col: dim_tok.col,
                                            span: dim_tok.span,
                                        });
                                    }
                                }
                            }
                            if !self.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                        let end_tok = self.expect(&TokenKind::RBracket)?;
                        let element_type = args.into_iter().next().unwrap_or(TypeExpr::Simple {
                            name: "f64".into(),
                            span: token.span,
                        });
                        return Ok(TypeExpr::Tensor {
                            element_type: Box::new(element_type),
                            dims,
                            span: Span::new(start, end_tok.span.end),
                        });
                    }

                    let end = self.prev_span().end;
                    return Ok(TypeExpr::Generic {
                        name,
                        args,
                        span: Span::new(start, end),
                    });
                }

                let end = self.prev_span().end;
                Ok(TypeExpr::Simple {
                    name,
                    span: Span::new(start, end),
                })
            }
        }
    }

    /// Parses a type name (identifier or type keyword).
    fn parse_type_name(&mut self) -> Result<String, ParseError> {
        let token = self.peek().clone();
        let name = match &token.kind {
            TokenKind::Ident(n) => n.clone(),
            // Type keywords
            TokenKind::BoolType => "bool".into(),
            TokenKind::I8 => "i8".into(),
            TokenKind::I16 => "i16".into(),
            TokenKind::I32 => "i32".into(),
            TokenKind::I64 => "i64".into(),
            TokenKind::I128 => "i128".into(),
            TokenKind::Isize => "isize".into(),
            TokenKind::U8 => "u8".into(),
            TokenKind::U16 => "u16".into(),
            TokenKind::U32 => "u32".into(),
            TokenKind::U64 => "u64".into(),
            TokenKind::U128 => "u128".into(),
            TokenKind::Usize => "usize".into(),
            TokenKind::F16Type => "f16".into(),
            TokenKind::Bf16Type => "bf16".into(),
            TokenKind::F32Type => "f32".into(),
            TokenKind::F64Type => "f64".into(),
            TokenKind::StrType => "str".into(),
            TokenKind::CharType => "char".into(),
            TokenKind::Void => "void".into(),
            TokenKind::Never => "never".into(),
            TokenKind::Tensor => "Tensor".into(),
            TokenKind::Ptr => "ptr".into(),
            _ => {
                return Err(ParseError::ExpectedType {
                    line: token.line,
                    col: token.col,
                    span: token.span,
                })
            }
        };
        self.advance();
        Ok(name)
    }

    // ── Patterns ───────────────────────────────────────────────────────

    /// Parses a pattern (for match arms).
    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let token = self.peek().clone();

        match &token.kind {
            // Wildcard: _
            TokenKind::Ident(name) if name == "_" => {
                self.advance();
                Ok(Pattern::Wildcard { span: token.span })
            }

            // Identifier or enum pattern
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();

                // Check for enum pattern: Name::Variant(...)
                if self.eat(&TokenKind::ColonColon) {
                    let (variant, _) = self.expect_ident()?;
                    let mut fields = Vec::new();
                    if self.eat(&TokenKind::LParen) {
                        while !self.at(&TokenKind::RParen) && !self.at_eof() {
                            fields.push(self.parse_pattern()?);
                            if !self.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                        self.expect(&TokenKind::RParen)?;
                    }
                    let end = self.prev_span().end;
                    return Ok(Pattern::Enum {
                        enum_name: name,
                        variant,
                        fields,
                        span: Span::new(token.span.start, end),
                    });
                }

                // Check for direct variant pattern: Some(x), Ok(v), Err(e)
                if self.at(&TokenKind::LParen) && name.starts_with(|c: char| c.is_uppercase()) {
                    self.advance(); // eat (
                    let mut fields = Vec::new();
                    while !self.at(&TokenKind::RParen) && !self.at_eof() {
                        fields.push(self.parse_pattern()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    let end = self.prev_span().end;
                    return Ok(Pattern::Enum {
                        enum_name: String::new(), // no explicit enum name
                        variant: name,
                        fields,
                        span: Span::new(token.span.start, end),
                    });
                }

                // Check for struct pattern: Name { field: pat, ... }
                if self.at(&TokenKind::LBrace) && name.starts_with(|c: char| c.is_uppercase()) {
                    self.advance();
                    let mut fields = Vec::new();
                    while !self.at(&TokenKind::RBrace) && !self.at_eof() {
                        let fstart = self.peek().span.start;
                        let (fname, _) = self.expect_ident()?;
                        let pattern = if self.eat(&TokenKind::Colon) {
                            Some(self.parse_pattern()?)
                        } else {
                            None
                        };
                        let fend = self.prev_span().end;
                        fields.push(FieldPattern {
                            name: fname,
                            pattern,
                            span: Span::new(fstart, fend),
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RBrace)?;
                    let end = self.prev_span().end;
                    return Ok(Pattern::Struct {
                        name,
                        fields,
                        span: Span::new(token.span.start, end),
                    });
                }

                Ok(Pattern::Ident {
                    name,
                    span: token.span,
                })
            }

            // Literal patterns (with optional range: 1..10 or 1..=10)
            TokenKind::IntLit(v) => {
                let v = *v;
                self.advance();
                // Check for range pattern: int..int or int..=int
                if self.at(&TokenKind::DotDot) || self.at(&TokenKind::DotDotEq) {
                    let inclusive = self.at(&TokenKind::DotDotEq);
                    self.advance();
                    let end_token = self.peek().clone();
                    if let TokenKind::IntLit(end_v) = &end_token.kind {
                        let end_v = *end_v;
                        self.advance();
                        let end_span = end_token.span;
                        return Ok(Pattern::Range {
                            start: Box::new(Expr::Literal {
                                kind: LiteralKind::Int(v),
                                span: token.span,
                            }),
                            end: Box::new(Expr::Literal {
                                kind: LiteralKind::Int(end_v),
                                span: end_span,
                            }),
                            inclusive,
                            span: Span::new(token.span.start, end_span.end),
                        });
                    }
                }
                Ok(Pattern::Literal {
                    kind: LiteralKind::Int(v),
                    span: token.span,
                })
            }
            TokenKind::FloatLit(v) => {
                let v = *v;
                self.advance();
                Ok(Pattern::Literal {
                    kind: LiteralKind::Float(v),
                    span: token.span,
                })
            }
            TokenKind::StringLit(s) => {
                let s = s.clone();
                self.advance();
                Ok(Pattern::Literal {
                    kind: LiteralKind::String(s),
                    span: token.span,
                })
            }
            TokenKind::True => {
                self.advance();
                Ok(Pattern::Literal {
                    kind: LiteralKind::Bool(true),
                    span: token.span,
                })
            }
            TokenKind::False => {
                self.advance();
                Ok(Pattern::Literal {
                    kind: LiteralKind::Bool(false),
                    span: token.span,
                })
            }

            // Tuple pattern: (a, b)
            TokenKind::LParen => {
                self.advance();
                let mut elements = Vec::new();
                while !self.at(&TokenKind::RParen) && !self.at_eof() {
                    elements.push(self.parse_pattern()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                let end_tok = self.expect(&TokenKind::RParen)?;
                Ok(Pattern::Tuple {
                    elements,
                    span: Span::new(token.span.start, end_tok.span.end),
                })
            }

            _ => Err(ParseError::ExpectedPattern {
                line: token.line,
                col: token.col,
                span: token.span,
            }),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

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
        assert!(matches!(expr, Expr::While { .. }));
    }

    #[test]
    fn parse_for_loop() {
        let expr = parse_expr_ok("for i in items { i }");
        match expr {
            Expr::For { variable, .. } => assert_eq!(variable, "i"),
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
}
