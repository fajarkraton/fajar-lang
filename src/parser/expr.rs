//! Expression parsing (Pratt parser, literals, calls, blocks, control flow).

use super::*;

impl Parser {
    pub(super) fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
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

                // Prevent `(` on a new line from chaining as a function call.
                // E.g., `foo()\n(x + 1)` should be two statements, not `foo()(x + 1)`.
                // Only break for LParen; Dot and LBracket are fine on new lines.
                if kind == TokenKind::LParen {
                    let next_line = self.peek().line;
                    let prev_line = self.prev_line();
                    if next_line > prev_line {
                        break;
                    }
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
                let (l_bp, r_bp) = infix_binding_power(&kind).ok_or_else(|| {
                    let tok = self.peek();
                    ParseError::UnexpectedToken {
                        expected: "infix operator".into(),
                        found: format!("{kind:?}"),
                        line: tok.line,
                        col: tok.col,
                        span: tok.span,
                    }
                })?;
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
                let (l_bp, _r_bp) = infix_binding_power(&kind).ok_or_else(|| {
                    let tok = self.peek();
                    ParseError::UnexpectedToken {
                        expected: "range operator".into(),
                        found: format!("{kind:?}"),
                        line: tok.line,
                        col: tok.col,
                        span: tok.span,
                    }
                })?;
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

            // Labeled loop: 'name: while/for/loop
            TokenKind::Lifetime(name) => {
                let label = name.clone();
                self.advance(); // eat 'label
                self.expect(&TokenKind::Colon)?; // eat :
                match self.peek().kind {
                    TokenKind::While => self.parse_while_expr_with_label(Some(label)),
                    TokenKind::For => self.parse_for_expr_with_label(Some(label)),
                    TokenKind::Loop => self.parse_loop_expr_with_label(Some(label)),
                    _ => {
                        let tok = self.peek();
                        Err(ParseError::UnexpectedToken {
                            expected: "while, for, or loop after label".into(),
                            found: format!("{}", tok.kind),
                            line: tok.line,
                            col: tok.col,
                            span: tok.span,
                        })
                    }
                }
            }

            // While loop
            TokenKind::While => self.parse_while_expr_with_label(None),

            // For loop
            TokenKind::For => self.parse_for_expr_with_label(None),

            // Loop (infinite)
            TokenKind::Loop => self.parse_loop_expr_with_label(None),

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

            // Comptime block: comptime { body }
            TokenKind::Comptime => {
                let start = token.span.start;
                self.advance(); // eat `comptime`
                let body = self.parse_block_expr()?;
                let end = body.span().end;
                Ok(Expr::Comptime {
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

    /// Parses one arm of a handle expression: `Effect::op(p1, p2) => { body }`.
    fn parse_effect_handler_arm(&mut self) -> Result<EffectHandlerArm, ParseError> {
        let start = self.peek().span.start;

        // Parse Effect::op
        let (effect_name, _) = self.expect_ident()?;
        self.expect(&TokenKind::ColonColon)?;
        let (op_name, _) = self.expect_ident()?;

        // Parse (param_names)
        self.expect(&TokenKind::LParen)?;
        let mut param_names = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            let (pname, _) = self.expect_ident()?;
            param_names.push(pname);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen)?;

        // => { body }
        self.expect(&TokenKind::FatArrow)?;
        let body = self.parse_block_expr()?;
        let end = body.span().end;

        Ok(EffectHandlerArm {
            effect_name,
            op_name,
            param_names,
            body: Box::new(body),
            span: Span::new(start, end),
        })
    }

    /// Parses an identifier expression, potentially a path (`a::b::c`) or struct init.
    fn parse_ident_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        let (name, name_span) = self.expect_ident()?;

        // Contextual keyword: `handle { body } with { handlers }`
        if name == "handle" && self.at(&TokenKind::LBrace) {
            let body = self.parse_block_expr()?;
            // Expect contextual `with`
            if !matches!(self.peek_kind(), TokenKind::Ident(s) if s == "with") {
                let tok = self.peek().clone();
                return Err(ParseError::UnexpectedToken {
                    expected: "`with` after handle body".into(),
                    found: format!("{}", tok.kind),
                    line: tok.line,
                    col: tok.col,
                    span: tok.span,
                });
            }
            self.advance(); // eat `with`
            self.expect(&TokenKind::LBrace)?;
            let mut handlers = Vec::new();
            while !self.at(&TokenKind::RBrace) && !self.at_eof() {
                let arm = self.parse_effect_handler_arm()?;
                handlers.push(arm);
            }
            let end = self.peek().span.end;
            self.expect(&TokenKind::RBrace)?;
            return Ok(Expr::HandleEffect {
                body: Box::new(body),
                handlers,
                span: Span::new(start, end),
            });
        }

        // Contextual keyword: `resume(value)`
        if name == "resume" && self.at(&TokenKind::LParen) {
            self.advance(); // eat `(`
            let value = self.parse_expr(0)?;
            let end = self.peek().span.end;
            self.expect(&TokenKind::RParen)?;
            return Ok(Expr::ResumeExpr {
                value: Box::new(value),
                span: Span::new(start, end),
            });
        }

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
        // Only if: (a) identifier starts with uppercase, (b) next is `{`,
        // and (c) inside the brace there's `ident:` (field init pattern).
        // This prevents `while i < MAX {` from being treated as struct init.
        if self.at(&TokenKind::LBrace) && name.starts_with(|c: char| c.is_uppercase()) {
            // Look ahead: if `{ ident :` pattern, it's struct init.
            // If `{ <statement>` (e.g., `{ let`, `{ putc`, `{ i`), it's a block.
            let is_struct_init = if let Some(tok1) = self.tokens.get(self.pos + 1) {
                if let TokenKind::Ident(_) = &tok1.kind {
                    // Check if token after ident is `:` (field init)
                    self.tokens
                        .get(self.pos + 2)
                        .is_some_and(|tok2| matches!(tok2.kind, TokenKind::Colon))
                } else {
                    // `{ }` (empty struct) or `{ RBrace`
                    matches!(tok1.kind, TokenKind::RBrace)
                }
            } else {
                false
            };
            if is_struct_init {
                return self.parse_struct_init(name, start);
            }
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
    pub(super) fn parse_block_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::LBrace)?;

        let mut stmts = Vec::new();
        let mut final_expr: Option<Box<Expr>> = None;

        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            // Try parsing as an item/statement
            match self.peek_kind() {
                TokenKind::Let
                | TokenKind::Const
                | TokenKind::Return
                | TokenKind::Break
                | TokenKind::Continue => {
                    stmts.push(self.parse_stmt()?);
                    // Drain pending stmts from tuple destructuring
                    while let Some(pending) = self.pending_stmts.pop() {
                        stmts.push(pending);
                    }
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
        let mut pattern = self.parse_pattern()?;

        // Or-pattern: `0 | 1 | 2 => ...`
        if matches!(self.peek_kind(), TokenKind::Pipe) {
            let mut patterns = vec![pattern];
            while self.eat(&TokenKind::Pipe) {
                patterns.push(self.parse_pattern()?);
            }
            let end = patterns.last().map_or(start, |p| p.span().end);
            pattern = Pattern::Or {
                patterns,
                span: Span::new(start, end),
            };
        }

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

    /// Parses a while loop: `['label:] while cond { body }`.
    fn parse_while_expr_with_label(&mut self, label: Option<String>) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::While)?;
        let condition = Box::new(self.parse_expr(0)?);
        let body = Box::new(self.parse_block_expr()?);
        let end = body.span().end;
        Ok(Expr::While {
            label,
            condition,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses a for loop: `['label:] for var in iter { body }`.
    fn parse_for_expr_with_label(&mut self, label: Option<String>) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::For)?;
        let (variable, _) = self.expect_ident()?;
        self.expect(&TokenKind::In)?;
        let iterable = Box::new(self.parse_expr(0)?);
        let body = Box::new(self.parse_block_expr()?);
        let end = body.span().end;
        Ok(Expr::For {
            label,
            variable,
            iterable,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses a loop: `['label:] loop { body }`.
    fn parse_loop_expr_with_label(&mut self, label: Option<String>) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Loop)?;
        let body = Box::new(self.parse_block_expr()?);
        let end = body.span().end;
        Ok(Expr::Loop {
            label,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses an array literal: `[a, b, c]`.
    fn parse_array_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::LBracket)?;

        // Check for empty array
        if self.at(&TokenKind::RBracket) {
            let end_tok = self.expect(&TokenKind::RBracket)?;
            return Ok(Expr::Array {
                elements: Vec::new(),
                span: Span::new(start, end_tok.span.end),
            });
        }

        // Parse first element
        let first = self.parse_expr(0)?;

        // Check for repeat syntax: [expr; count]
        if self.eat(&TokenKind::Semi) {
            let count = self.parse_expr(0)?;
            let end_tok = self.expect(&TokenKind::RBracket)?;
            return Ok(Expr::ArrayRepeat {
                value: Box::new(first),
                count: Box::new(count),
                span: Span::new(start, end_tok.span.end),
            });
        }

        // Regular array: [a, b, c, ...]
        let mut elements = vec![first];
        while self.eat(&TokenKind::Comma) {
            if self.at(&TokenKind::RBracket) {
                break; // trailing comma
            }
            elements.push(self.parse_expr(0)?);
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
    pub(super) fn parse_global_asm(&mut self) -> Result<GlobalAsm, ParseError> {
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
                    "lateout" => Ok(AsmOperand::LateOut { constraint, expr }),
                    _ => Err(ParseError::UnexpectedToken {
                        expected: "in, out, inout, lateout, or const".into(),
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
    pub(super) fn parse_type_expr(&mut self) -> Result<TypeExpr, ParseError> {
        let token = self.peek().clone();

        match &token.kind {
            // Reference type: &T, &mut T, &'a T, &'a mut T
            TokenKind::Amp => {
                self.advance();
                // Check for optional lifetime annotation
                let lifetime = if let TokenKind::Lifetime(name) = self.peek_kind().clone() {
                    self.advance();
                    Some(name)
                } else {
                    None
                };
                let mutable = self.eat(&TokenKind::Mut);
                let inner = self.parse_type_expr()?;
                let span = Span::new(token.span.start, inner.span().end);
                Ok(TypeExpr::Reference {
                    lifetime,
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
                });
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
