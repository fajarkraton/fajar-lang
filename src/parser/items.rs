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

        // Optional verification annotations: @requires(expr) @ensures(expr)
        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        while matches!(
            self.peek_kind(),
            TokenKind::AtRequires | TokenKind::AtEnsures
        ) {
            let is_requires = matches!(self.peek_kind(), TokenKind::AtRequires);
            self.advance(); // consume @requires or @ensures
            if matches!(self.peek_kind(), TokenKind::LParen) {
                self.advance(); // consume '('
                let expr = self.parse_expr(0)?;
                if matches!(self.peek_kind(), TokenKind::RParen) {
                    self.advance(); // consume ')'
                }
                if is_requires {
                    requires.push(Box::new(expr));
                } else {
                    ensures.push(Box::new(expr));
                }
            }
        }

        // Optional effect clause: `with IO, Alloc`
        let effects = self.parse_effect_clause()?;

        // Body
        let body = Box::new(self.parse_block_expr()?);
        let end = body.span().end;

        Ok(FnDef {
            is_pub,
            is_const: false,
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
            requires,
            ensures,
            effects,
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

    /// Parses an optional effect clause: `with IO, Alloc, Console`.
    ///
    /// Returns an empty vec if no `with` keyword is present.
    fn parse_effect_clause(&mut self) -> Result<Vec<String>, ParseError> {
        // `with` is a contextual keyword — check for Ident("with")
        if !matches!(self.peek_kind(), TokenKind::Ident(s) if s == "with") {
            return Ok(Vec::new());
        }
        self.advance(); // consume `with`

        let mut effects = Vec::new();
        // Parse comma-separated effect names
        let (name, _) = self.expect_ident()?;
        effects.push(name);
        while self.eat(&TokenKind::Comma) {
            // Stop if we see `{` (start of body)
            if self.at(&TokenKind::LBrace) {
                break;
            }
            let (name, _) = self.expect_ident()?;
            effects.push(name);
        }
        Ok(effects)
    }

    /// Parses an effect declaration: `effect Name { fn op(params) -> RetType }`.
    pub(super) fn parse_effect_decl(&mut self, is_pub: bool) -> Result<EffectDeclItem, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Effect)?;
        let (name, _) = self.expect_ident()?;

        self.expect(&TokenKind::LBrace)?;

        let mut operations = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let op = self.parse_effect_op()?;
            operations.push(op);
        }

        let end = self.peek().span.end;
        self.expect(&TokenKind::RBrace)?;

        Ok(EffectDeclItem {
            is_pub,
            name,
            operations,
            span: Span::new(start, end),
        })
    }

    /// Parses a single effect operation: `fn name(params) -> RetType`.
    fn parse_effect_op(&mut self) -> Result<EffectOpDef, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Fn)?;
        let (name, _) = self.expect_ident()?;

        self.expect(&TokenKind::LParen)?;
        let mut params = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            let (pname, _) = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let ty = self.parse_type_expr()?;
            params.push((pname, ty));
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen)?;

        let return_type = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let end = self.prev_span().end;

        Ok(EffectOpDef {
            name,
            params,
            return_type,
            span: Span::new(start, end),
        })
    }

    /// Parses a `macro_rules!` definition.
    ///
    /// ```text
    /// macro_rules! name {
    ///     (pattern) => { template }
    /// }
    /// ```
    pub(super) fn parse_macro_rules_def(&mut self) -> Result<Item, ParseError> {
        let start = self.peek().span.start;
        // consume `macro_rules`
        self.advance();
        // consume `!`
        self.expect(&TokenKind::Bang)?;
        let (name, _) = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;

        let mut arms = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let arm_start = self.peek().span.start;

            // Parse pattern: (...)
            self.expect(&TokenKind::LParen)?;
            let mut pattern = String::new();
            let mut depth = 1;
            while depth > 0 && !self.at_eof() {
                let tok = self.advance().clone();
                match &tok.kind {
                    TokenKind::LParen => {
                        depth += 1;
                        pattern.push('(');
                    }
                    TokenKind::RParen => {
                        depth -= 1;
                        if depth > 0 {
                            pattern.push(')');
                        }
                    }
                    _ => {
                        if !pattern.is_empty() {
                            pattern.push(' ');
                        }
                        pattern.push_str(&format!("{}", tok.kind));
                    }
                }
            }

            // => { template }
            self.expect(&TokenKind::FatArrow)?;
            let body = self.parse_block_expr()?;
            let arm_end = body.span().end;

            arms.push(MacroArm {
                pattern: pattern.trim().to_string(),
                body: Box::new(body),
                span: Span::new(arm_start, arm_end),
            });
        }

        let end = self.peek().span.end;
        self.expect(&TokenKind::RBrace)?;

        Ok(Item::MacroRulesDef(MacroRulesItem {
            name,
            arms,
            span: Span::new(start, end),
        }))
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

        // Parse type params (and comptime params)
        while !self.at(&TokenKind::Gt) && !self.at_eof() {
            let start = self.peek().span.start;

            // Check for `comptime` or `const` modifier: `const N: usize` or `comptime N: i64`
            let is_comptime = self.eat(&TokenKind::Comptime) || self.eat(&TokenKind::Const);

            // Check for effect variable: identifier followed by `: Effect`
            // e.g., `fn map<E: Effect>(...)` or just `<E>` in effect position
            let (name, _) = self.expect_ident()?;

            let mut bounds = Vec::new();
            let mut is_effect = false;
            let mut const_type: Option<String> = None;

            if self.eat(&TokenKind::Colon) {
                if is_comptime {
                    // For const generic params, parse a type name (not a trait bound).
                    // `const N: usize` — usize is a keyword, not an identifier.
                    let type_name = self.parse_const_param_type()?;
                    const_type = Some(type_name);
                } else {
                    // Check if bound is "Effect" — marks as effect variable
                    if matches!(self.peek_kind(), TokenKind::Ident(s) if s == "Effect") {
                        is_effect = true;
                    }
                    loop {
                        let bound = self.parse_trait_bound()?;
                        bounds.push(bound);
                        if !self.eat(&TokenKind::Plus) {
                            break;
                        }
                    }
                }
            }

            let end = self.prev_span().end;
            generic_params.push(GenericParam {
                name,
                bounds,
                is_comptime,
                is_effect,
                const_type,
                span: Span::new(start, end),
            });

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        self.expect(&TokenKind::Gt)?;
        Ok((lifetime_params, generic_params))
    }

    /// Parses the type annotation of a const generic parameter.
    ///
    /// Handles keyword types like `usize`, `i32`, `bool` that are not identifiers.
    /// `const N: usize` → returns `"usize"`.
    fn parse_const_param_type(&mut self) -> Result<String, ParseError> {
        let kind = self.peek_kind().clone();
        let type_name = match kind {
            TokenKind::Usize => {
                self.advance();
                "usize".to_string()
            }
            TokenKind::Isize => {
                self.advance();
                "isize".to_string()
            }
            TokenKind::I8 => {
                self.advance();
                "i8".to_string()
            }
            TokenKind::I16 => {
                self.advance();
                "i16".to_string()
            }
            TokenKind::I32 => {
                self.advance();
                "i32".to_string()
            }
            TokenKind::I64 => {
                self.advance();
                "i64".to_string()
            }
            TokenKind::I128 => {
                self.advance();
                "i128".to_string()
            }
            TokenKind::U8 => {
                self.advance();
                "u8".to_string()
            }
            TokenKind::U16 => {
                self.advance();
                "u16".to_string()
            }
            TokenKind::U32 => {
                self.advance();
                "u32".to_string()
            }
            TokenKind::U64 => {
                self.advance();
                "u64".to_string()
            }
            TokenKind::U128 => {
                self.advance();
                "u128".to_string()
            }
            TokenKind::BoolType => {
                self.advance();
                "bool".to_string()
            }
            TokenKind::Ident(name) => {
                let n = name.clone();
                self.advance();
                n
            }
            _ => {
                let tok = self.peek().clone();
                return Err(ParseError::ExpectedIdentifier {
                    found: format!("{}", tok.kind),
                    line: tok.line,
                    col: tok.col,
                    span: tok.span,
                });
            }
        };
        Ok(type_name)
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
            is_protocol: false,
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
            is_const: false,
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
            requires: vec![],
            ensures: vec![],
            effects: Vec::new(),
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
        let (name, name_span) = self.expect_ident()?;
        let ty = if self.eat(&TokenKind::Colon) {
            self.parse_type_expr()?
        } else {
            TypeExpr::Simple {
                name: "_".to_string(),
                span: name_span,
            }
        };
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
            TokenKind::Yield => self.parse_yield_expr_stmt(),
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
        let (name, name_span) = self.expect_ident()?;
        let ty = if self.eat(&TokenKind::Colon) {
            self.parse_type_expr()?
        } else {
            TypeExpr::Simple {
                name: "_".to_string(),
                span: name_span,
            }
        };
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

    /// Parses a let statement: `let [mut] [linear] name [: Type] = value`
    /// or tuple destructuring: `let (a, b, ...) = expr`
    fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Let)?;

        let mutable = self.eat(&TokenKind::Mut);
        let linear = self.eat(&TokenKind::Linear);

        // Tuple destructuring: let (a, b) = expr
        if *self.peek_kind() == TokenKind::LParen {
            return self.parse_let_tuple_destructure(start, mutable);
        }

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
            linear,
            name,
            ty,
            value,
            span: Span::new(start, end),
        })
    }

    /// Parses `let (a, b, ...) = expr` as sugar for:
    /// `let _tmp = expr; let a = _tmp.0; let b = _tmp.1; ...`
    /// Returns a block statement containing the desugared lets.
    fn parse_let_tuple_destructure(
        &mut self,
        start: usize,
        mutable: bool,
    ) -> Result<Stmt, ParseError> {
        self.expect(&TokenKind::LParen)?;
        let mut names = Vec::new();
        loop {
            let (name, _) = self.expect_ident()?;
            names.push(name);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen)?;
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expr(0)?;
        let end = value.span().end;
        self.eat_semi();

        // Store tuple destructuring info in first let binding.
        // The value is the tuple expression.
        // Variable names are stored with index-based field access.
        // We create a single let binding for the first variable with the
        // tuple value wrapped in a TupleIndex (field ".0"), and push
        // pending lets for remaining variables into self.pending_stmts.
        let span = Span::new(start, end);

        // For each name, create a let binding with tuple.N access
        // First: let a = (expr).0
        // Remaining: let b = a.__tuple_src.1 -- but we need the original tuple
        // Simplest: evaluate tuple once, access by index
        // Since tuples are values (stored as Tuple variant), field access .0/.1 works

        // Store the remaining lets for the parent block to consume
        // First: let __tup = expr
        // Then: let a = __tup.0, let b = __tup.1, ...
        // Pending stmts are pushed in reverse order (popped = correct order)
        let tup_name = format!("_tup{}", start);

        for i in (0..names.len()).rev() {
            self.pending_stmts.push(Stmt::Let {
                mutable,
                linear: false,
                name: names[i].clone(),
                ty: None,
                value: Box::new(Expr::Field {
                    object: Box::new(Expr::Ident {
                        name: tup_name.clone(),
                        span,
                    }),
                    field: format!("{i}"),
                    span,
                }),
                span,
            });
        }

        // Return the tuple let binding
        Ok(Stmt::Let {
            mutable: false,
            linear: false,
            name: tup_name,
            ty: None,
            value: Box::new(value),
            span,
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

    /// Parses a yield expression as a statement: `yield expr`.
    fn parse_yield_expr_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.peek().span.start;
        self.expect(&TokenKind::Yield)?;

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

        Ok(Stmt::Expr {
            expr: Box::new(Expr::Yield {
                value,
                span: Span::new(start, end),
            }),
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
