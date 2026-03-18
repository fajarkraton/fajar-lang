//! Item and statement parsing (fn, struct, enum, trait, impl, let, return).

use super::*;

impl Parser {
    pub(super) fn parse_fn_def(
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

        // Optional generic params (lifetimes + type params)
        let (lifetime_params, generic_params) = self.try_parse_lifetime_and_generic_params()?;

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
            lifetime_params,
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
    pub(super) fn parse_extern_fn(
        &mut self,
        annotation: Option<Annotation>,
    ) -> Result<ExternFn, ParseError> {
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
    pub(super) fn parse_type_alias(&mut self, is_pub: bool) -> Result<TypeAlias, ParseError> {
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
                    lifetime: None,
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
    /// Parses optional generic parameters, including lifetime parameters.
    ///
    /// Lifetime parameters (e.g., `'a`, `'b`) must appear before type parameters.
    /// Returns `(lifetime_params, generic_params)`.
    fn try_parse_lifetime_and_generic_params(
        &mut self,
    ) -> Result<(Vec<LifetimeParam>, Vec<GenericParam>), ParseError> {
        if !self.eat(&TokenKind::Lt) {
            return Ok((Vec::new(), Vec::new()));
        }

        let mut lifetime_params = Vec::new();
        let mut generic_params = Vec::new();

        // Parse lifetime params first (they come before type params)
        while let TokenKind::Lifetime(name) = self.peek_kind().clone() {
            let start = self.peek().span.start;
            let end = self.peek().span.end;
            self.advance(); // consume lifetime token
            lifetime_params.push(LifetimeParam {
                name,
                span: Span::new(start, end),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            // If the next token is `>`, stop
            if self.at(&TokenKind::Gt) {
                break;
            }
        }

        // Parse type params
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
            generic_params.push(GenericParam {
                name,
                bounds,
                span: Span::new(start, end),
            });

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        self.expect(&TokenKind::Gt)?;
        Ok((lifetime_params, generic_params))
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
    pub(super) fn parse_struct_def(
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
        let (lifetime_params, generic_params) = self.try_parse_lifetime_and_generic_params()?;

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
            lifetime_params,
            generic_params,
            fields,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses a union definition: `union Name { fields }`.
    pub(super) fn parse_union_def(
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
    pub(super) fn parse_enum_def(
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
        let (lifetime_params, generic_params) = self.try_parse_lifetime_and_generic_params()?;

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
            lifetime_params,
            generic_params,
            variants,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses an impl block: `impl [Trait for] Type { methods }`.
    pub(super) fn parse_impl_block(&mut self) -> Result<ImplBlock, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Impl)?;

        let (lifetime_params, generic_params) = self.try_parse_lifetime_and_generic_params()?;

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
            lifetime_params,
            generic_params,
            trait_name,
            target_type,
            methods,
            span: Span::new(start, end_tok.span.end),
        })
    }

    /// Parses a trait definition: `trait Name { methods }`.
    pub(super) fn parse_trait_def(&mut self, is_pub: bool) -> Result<TraitDef, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Trait)?;
        let (name, _) = self.expect_ident()?;
        let (lifetime_params, generic_params) = self.try_parse_lifetime_and_generic_params()?;

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
            lifetime_params,
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
        let (lifetime_params, generic_params) = self.try_parse_lifetime_and_generic_params()?;

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
            lifetime_params,
            generic_params,
            params,
            return_type,
            where_clauses,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses a const definition: `const NAME: Type = value`.
    pub(super) fn parse_const_def(
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
    pub(super) fn parse_use_decl(&mut self) -> Result<UseDecl, ParseError> {
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
    pub(super) fn parse_mod_decl(&mut self) -> Result<ModDecl, ParseError> {
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
    pub(super) fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek_kind() {
            TokenKind::Let => self.parse_let_stmt(),
            TokenKind::Const => self.parse_const_stmt(),
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

    /// Parses a const statement: `const NAME: Type = value`.
    fn parse_const_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Const)?;
        let (name, _) = self.expect_ident()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.parse_type_expr()?;
        self.expect(&TokenKind::Eq)?;
        let value = Box::new(self.parse_expr(0)?);
        let end = value.span().end;
        self.eat_semi();
        Ok(Stmt::Const {
            name,
            ty,
            value,
            span: Span::new(start, end),
        })
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
    pub(super) fn parse_return_stmt(&mut self) -> Result<Stmt, ParseError> {
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

    /// Parses a break statement: `break ['label] [expr]`.
    pub(super) fn parse_break_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Break)?;

        // Check for optional label: break 'outer
        let label = if let TokenKind::Lifetime(name) = &self.peek().kind {
            let l = Some(name.clone());
            self.advance();
            l
        } else {
            None
        };

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
            label,
            value,
            span: Span::new(start, end),
        })
    }

    /// Parses a continue statement: `continue ['label]`.
    pub(super) fn parse_continue_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Continue)?;

        // Check for optional label: continue 'outer
        let label = if let TokenKind::Lifetime(name) = &self.peek().kind {
            let l = Some(name.clone());
            self.advance();
            l
        } else {
            None
        };

        let end = self.prev_span().end;
        self.eat_semi();

        Ok(Stmt::Continue {
            label,
            span: Span::new(start, end),
        })
    }

    // ── Expressions (Pratt parser) ─────────────────────────────────────
}
