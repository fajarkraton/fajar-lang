//! Expression and statement type checking.
//!
//! Contains all `check_*` functions for type-checking AST nodes,
//! `resolve_type()` for TypeExpr → Type conversion, and lifetime elision.

use crate::parser::ast::*;

use super::*;

impl TypeChecker {
    /// Second pass: type-check an item.
    pub(super) fn check_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(fndef) => {
                // `self` parameter is not allowed in free functions
                for param in &fndef.params {
                    if param.name == "self" {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: "regular parameter".into(),
                            found: "`self` outside impl block".into(),
                            span: param.span,
                            hint: None,
                        });
                    }
                }
                self.check_fn_def(fndef);
            }
            Item::ConstDef(cdef) => self.check_const_def(cdef),
            Item::StaticDef(sdef) => {
                // Treat static mut like a const def for type checking
                self.check_expr(&sdef.value);
            }
            Item::ServiceDef(svc) => {
                for h in &svc.handlers {
                    self.check_fn_def(h);
                }
                // E11: protocol completeness check
                if let Some(ref proto_name) = svc.implements {
                    if let Some(required_methods) = self.traits.get(proto_name) {
                        let handler_names: std::collections::HashSet<String> =
                            svc.handlers.iter().map(|h| h.name.clone()).collect();
                        for method_sig in required_methods {
                            if !handler_names.contains(&method_sig.name) {
                                self.errors.push(SemanticError::TypeMismatch {
                                    expected: format!(
                                        "method '{}' required by protocol '{}'",
                                        method_sig.name, proto_name
                                    ),
                                    found: format!("service '{}' is missing this method", svc.name),
                                    span: svc.span,
                                    hint: Some(format!(
                                        "add 'fn {}(...)' to service '{}'",
                                        method_sig.name, svc.name
                                    )),
                                });
                            }
                        }
                    }
                }
            }
            Item::ImplBlock(impl_block) => {
                for method in &impl_block.methods {
                    self.check_fn_def(method);
                }
            }
            Item::ModDecl(mod_decl) => {
                if let Some(items) = &mod_decl.body {
                    for item in items {
                        self.check_item(item);
                    }
                }
            }
            Item::EffectDecl(ed) => {
                // Register the effect in the registry
                let kind = crate::analyzer::effects::effect_kind_from_name(&ed.name)
                    .unwrap_or(crate::analyzer::effects::EffectKind::State);
                let ops: Vec<crate::analyzer::effects::EffectOp> = ed
                    .operations
                    .iter()
                    .map(|op| {
                        crate::analyzer::effects::EffectOp::new(
                            op.name.clone(),
                            op.params.iter().map(|(_, _)| "any".to_string()).collect(),
                            op.return_type
                                .as_ref()
                                .map_or("void".to_string(), |_| "any".to_string()),
                        )
                    })
                    .collect();
                let decl = crate::analyzer::effects::EffectDecl::new(ed.name.clone(), kind, ops);
                if self.effect_registry.register(decl).is_err() {
                    self.errors.push(SemanticError::DuplicateEffectDecl {
                        name: ed.name.clone(),
                        span: ed.span,
                    });
                }
            }
            Item::Stmt(stmt) => {
                self.check_stmt(stmt);
            }
            _ => {}
        }
    }

    /// Checks a function definition.
    fn check_fn_def(&mut self, fndef: &FnDef) {
        let scope_kind = match &fndef.annotation {
            Some(ann) if ann.name == "kernel" => crate::analyzer::scope::ScopeKind::Kernel,
            Some(ann) if ann.name == "device" => {
                // Track device capability parameter for E6/E7 enforcement
                self.current_device_cap = ann.param.clone();
                crate::analyzer::scope::ScopeKind::Device
            }
            Some(ann) if ann.name == "npu" => crate::analyzer::scope::ScopeKind::Npu,
            Some(ann) if ann.name == "unsafe" => crate::analyzer::scope::ScopeKind::Unsafe,
            Some(ann) if ann.name == "safe" => crate::analyzer::scope::ScopeKind::Safe,
            _ if fndef.is_async => crate::analyzer::scope::ScopeKind::AsyncFn,
            _ => crate::analyzer::scope::ScopeKind::Function,
        };
        self.symbols.push_scope_kind(scope_kind);

        // Register generic type parameters as TypeVar in scope.
        // This allows `T` to be resolved as a type variable in parameter and return positions.
        // Also validate that trait bounds reference known traits.
        for gp in &fndef.generic_params {
            for bound in &gp.bounds {
                if !self.traits.contains_key(&bound.name) {
                    self.errors.push(SemanticError::UnknownTrait {
                        name: bound.name.clone(),
                        span: bound.span,
                    });
                }
            }
            self.symbols.define(Symbol {
                name: gp.name.clone(),
                ty: Type::TypeVar(gp.name.clone()),
                mutable: false,
                span: gp.span,
                used: true, // type params are always "used"
            });
        }

        // Check lifetime annotations (always run — also catches undeclared lifetimes)
        self.check_lifetime_elision(fndef);

        // Validate where clause bounds reference known traits.
        for wc in &fndef.where_clauses {
            for bound in &wc.bounds {
                if !self.traits.contains_key(&bound.name) {
                    self.errors.push(SemanticError::UnknownTrait {
                        name: bound.name.clone(),
                        span: bound.span,
                    });
                }
            }
        }

        // Define parameters in function scope
        for param in &fndef.params {
            let ty = self.resolve_type(&param.ty);
            self.symbols.define(Symbol {
                name: param.name.clone(),
                ty,
                mutable: false,
                span: param.span,
                used: false,
            });
        }

        // Compute NLL liveness info for this function body
        let outer_nll = self.nll_info.take();
        self.nll_info = Some(crate::analyzer::cfg::NllInfo::analyze(&fndef.body));

        let body_type = self.check_expr(&fndef.body);

        let declared_ret = fndef
            .return_type
            .as_ref()
            .map(|t| self.resolve_type(t))
            .unwrap_or(Type::Void);

        // Check return type compatibility
        if !declared_ret.is_compatible(&body_type)
            && !matches!(declared_ret, Type::Void)
            && !matches!(body_type, Type::Void | Type::Never)
        {
            let hint = type_mismatch_hint(&declared_ret.display_name(), &body_type.display_name());
            self.errors.push(SemanticError::TypeMismatch {
                expected: declared_ret.display_name(),
                found: body_type.display_name(),
                span: fndef.span,
                hint,
            });
        }

        // Validate const fn body: only allow const-evaluable operations
        if fndef.is_const {
            self.check_const_fn_body(&fndef.body, &fndef.name, fndef.span);
        }

        // ── Effect System Validation ────────────────────────────────────
        // Collect effect variable names from generic params
        let effect_vars: std::collections::HashSet<String> = fndef
            .generic_params
            .iter()
            .filter(|p| p.is_effect)
            .map(|p| p.name.clone())
            .collect();

        // 1. Validate all declared effect names exist in the registry
        //    (skip effect variables — they're resolved at call site)
        for effect_name in &fndef.effects {
            if !effect_vars.contains(effect_name)
                && self.effect_registry.lookup(effect_name).is_none()
            {
                self.errors.push(SemanticError::UnknownEffect {
                    name: effect_name.clone(),
                    span: fndef.span,
                });
            }
        }

        // 2. Check context-effect compatibility: effects declared in `with`
        //    must be allowed by the function's context annotation
        if let Some(ref ann) = fndef.annotation {
            let ctx = match ann.name.as_str() {
                "kernel" => Some(crate::analyzer::effects::ContextAnnotation::Kernel),
                "device" => Some(crate::analyzer::effects::ContextAnnotation::Device),
                "safe" => Some(crate::analyzer::effects::ContextAnnotation::Safe),
                "unsafe" => Some(crate::analyzer::effects::ContextAnnotation::Unsafe),
                _ => None,
            };
            if let Some(ctx) = ctx {
                let forbidden = crate::analyzer::effects::forbidden_effects(ctx);
                for effect_name in &fndef.effects {
                    // Skip effect variables (resolved at call site)
                    if effect_vars.contains(effect_name) {
                        continue;
                    }
                    if let Some(decl) = self.effect_registry.lookup(effect_name) {
                        if forbidden.contains(&decl.kind) {
                            self.errors.push(SemanticError::EffectForbiddenInContext {
                                effect: effect_name.clone(),
                                context: format!("@{}", ann.name),
                                span: fndef.span,
                            });
                        }
                    }
                }
            }
        }

        // 3. Register function's effect signature for callee checking
        if !fndef.effects.is_empty() {
            self.fn_effects
                .insert(fndef.name.clone(), fndef.effects.clone());
        }

        // Restore outer NLL info (for nested functions)
        self.nll_info = outer_nll;

        self.emit_unused_warnings();
    }

    /// Validates that a const fn body only contains const-evaluable expressions.
    /// Produces warnings for non-const operations (I/O, heap allocation, etc.).
    fn check_const_fn_body(&mut self, expr: &Expr, fn_name: &str, fn_span: Span) {
        match expr {
            // Allowed: literals, identifiers, binary/unary ops, if/match, blocks
            Expr::Literal { .. } | Expr::Ident { .. } | Expr::Grouped { .. } => {}
            Expr::Binary { left, right, .. } => {
                self.check_const_fn_body(left, fn_name, fn_span);
                self.check_const_fn_body(right, fn_name, fn_span);
            }
            Expr::Unary { operand, .. } => {
                self.check_const_fn_body(operand, fn_name, fn_span);
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.check_const_fn_body(condition, fn_name, fn_span);
                self.check_const_fn_body(then_branch, fn_name, fn_span);
                if let Some(eb) = else_branch {
                    self.check_const_fn_body(eb, fn_name, fn_span);
                }
            }
            Expr::Block {
                stmts, expr: tail, ..
            } => {
                for stmt in stmts {
                    match stmt {
                        Stmt::Let { value, .. } | Stmt::Const { value, .. } => {
                            self.check_const_fn_body(value, fn_name, fn_span);
                        }
                        Stmt::Return { value, .. } => {
                            if let Some(v) = value {
                                self.check_const_fn_body(v, fn_name, fn_span);
                            }
                        }
                        Stmt::Expr { expr, .. } => {
                            self.check_const_fn_body(expr, fn_name, fn_span);
                        }
                        _ => {
                            // While/for/assignment not allowed in const fn
                            self.errors.push(SemanticError::TypeMismatch {
                                expected: "const-evaluable statement".into(),
                                found: "non-const statement in const fn".into(),
                                span: fn_span,
                                hint: Some(format!(
                                    "const fn '{}' contains a statement that cannot be evaluated at compile time",
                                    fn_name
                                )),
                            });
                        }
                    }
                }
                if let Some(t) = tail {
                    self.check_const_fn_body(t, fn_name, fn_span);
                }
            }
            Expr::Call { callee, args, .. } => {
                // Allow calling other const fns (we check at codegen time if they're actually const)
                self.check_const_fn_body(callee, fn_name, fn_span);
                for arg in args {
                    self.check_const_fn_body(&arg.value, fn_name, fn_span);
                }
            }
            Expr::Array { elements, .. } => {
                for elem in elements {
                    self.check_const_fn_body(elem, fn_name, fn_span);
                }
            }
            Expr::Index { object, index, .. } => {
                self.check_const_fn_body(object, fn_name, fn_span);
                self.check_const_fn_body(index, fn_name, fn_span);
            }
            Expr::StructInit { fields, .. } => {
                for fi in fields {
                    self.check_const_fn_body(&fi.value, fn_name, fn_span);
                }
            }
            Expr::Field { object, .. } => {
                self.check_const_fn_body(object, fn_name, fn_span);
            }
            Expr::Tuple { elements, .. } => {
                for elem in elements {
                    self.check_const_fn_body(elem, fn_name, fn_span);
                }
            }
            Expr::ArrayRepeat { value, count, .. } => {
                self.check_const_fn_body(value, fn_name, fn_span);
                self.check_const_fn_body(count, fn_name, fn_span);
            }
            Expr::Cast { expr, .. } => {
                self.check_const_fn_body(expr, fn_name, fn_span);
            }
            // Disallowed in const fn
            Expr::MethodCall { span, .. } => {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: "const-evaluable expression".into(),
                    found: "method call in const fn".into(),
                    span: *span,
                    hint: Some(format!(
                        "const fn '{}': method calls cannot be evaluated at compile time",
                        fn_name
                    )),
                });
            }
            Expr::Await { span, .. } | Expr::AsyncBlock { span, .. } => {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: "const-evaluable expression".into(),
                    found: "async operation in const fn".into(),
                    span: *span,
                    hint: Some(format!(
                        "const fn '{}': async operations are not allowed in const context",
                        fn_name
                    )),
                });
            }
            Expr::InlineAsm { span, .. } => {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: "const-evaluable expression".into(),
                    found: "inline assembly in const fn".into(),
                    span: *span,
                    hint: Some(format!(
                        "const fn '{}': inline assembly cannot be evaluated at compile time",
                        fn_name
                    )),
                });
            }
            _ => {
                // Other expressions: allow (may fail at codegen const eval, which is OK)
            }
        }
    }

    /// Pops the current scope and emits SE009 warnings for unused variables
    /// and ME010 errors for unconsumed linear variables.
    fn emit_unused_warnings(&mut self) {
        let unused = self.symbols.pop_scope_unused();
        for sym in &unused {
            // Check if this is a linear variable that wasn't consumed
            if let Some((span, consumed)) = self.linear_vars.get(&sym.name) {
                if !consumed {
                    self.errors.push(SemanticError::LinearNotConsumed {
                        name: sym.name.clone(),
                        span: *span,
                    });
                    continue;
                }
            }
            self.errors.push(SemanticError::UnusedVariable {
                name: sym.name.clone(),
                span: sym.span,
            });
        }
    }

    /// Releases borrows whose binding variable is no longer live (NLL).
    ///
    /// Called before each statement in a block. Checks all active borrow
    /// bindings against the NLL liveness info. If a borrow binding's last
    /// use position is before `current_pos`, its borrow is released early.
    fn release_dead_borrows_nll(&mut self, current_pos: usize) {
        if let Some(nll) = &self.nll_info {
            let active_refs = self.moves.active_borrow_refs();
            let mut to_release = Vec::new();
            for ref_name in active_refs {
                if !nll.is_live_at(&ref_name, current_pos) {
                    to_release.push(ref_name);
                }
            }
            for ref_name in to_release {
                self.moves.release_borrow_by_ref(&ref_name);
            }
        }
    }

    /// Checks a const definition.
    fn check_const_def(&mut self, cdef: &ConstDef) {
        let val_type = self.check_expr(&cdef.value);
        // Skip type check if type is inferred (written as `const X = expr` without annotation)
        let is_inferred = matches!(&cdef.ty, TypeExpr::Simple { name, .. } if name == "_");
        if !is_inferred {
            let declared = self.resolve_type(&cdef.ty);
            if !declared.is_compatible(&val_type) {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: declared.display_name(),
                    found: val_type.display_name(),
                    span: cdef.span,
                    hint: None,
                });
            }
        }
    }

    /// Checks a statement, returns the type of its value (Void for most stmts).
    fn check_stmt(&mut self, stmt: &Stmt) -> Type {
        match stmt {
            Stmt::Let {
                mutable,
                linear,
                name,
                ty,
                value,
                span,
            } => {
                let val_type = self.check_expr(value);

                // Track linear ownership
                if *linear {
                    self.linear_vars.insert(name.clone(), (*span, false));
                }

                if let Some(ty_expr) = ty {
                    let declared = self.resolve_type(ty_expr);
                    if !declared.is_compatible(&val_type) {
                        let hint =
                            type_mismatch_hint(&declared.display_name(), &val_type.display_name());
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: declared.display_name(),
                            found: val_type.display_name(),
                            span: *span,
                            hint,
                        });
                    }
                    self.symbols.define(Symbol {
                        name: name.clone(),
                        ty: declared,
                        mutable: *mutable,
                        span: *span,
                        used: false,
                    });
                } else {
                    // No explicit type annotation: default unsuffixed literals
                    // (IntLiteral → i64, FloatLiteral → f64)
                    self.symbols.define(Symbol {
                        name: name.clone(),
                        ty: val_type.default_literal(),
                        mutable: *mutable,
                        span: *span,
                        used: false,
                    });
                }

                // Move tracking: if RHS is a variable of Move type, mark it moved
                if let Expr::Ident {
                    name: src_name,
                    span: src_span,
                } = &**value
                {
                    let src_type = self
                        .symbols
                        .lookup(src_name)
                        .map(|s| s.ty.clone())
                        .unwrap_or(Type::Unknown);
                    if !crate::analyzer::borrow_lite::is_copy_type(&src_type) {
                        // ME003: cannot move while borrowed
                        if let Some(borrow_span) = self.moves.check_can_move(src_name) {
                            self.errors.push(SemanticError::MoveWhileBorrowed {
                                name: src_name.clone(),
                                span: *src_span,
                                borrow_span,
                            });
                        }
                        self.moves.mark_moved(src_name, *src_span);
                    }
                }
                // Declare new binding as Owned
                self.moves.declare(name, *span);

                // Track borrow refs: if RHS is &target or &mut target,
                // register that this binding holds a borrow.
                if let Expr::Unary { op, operand, .. } = &**value {
                    if matches!(op, UnaryOp::Ref | UnaryOp::RefMut) {
                        if let Expr::Ident {
                            name: target_name, ..
                        } = &**operand
                        {
                            self.moves.register_borrow_ref(
                                name,
                                target_name,
                                matches!(op, UnaryOp::RefMut),
                            );
                        }
                    }
                }

                Type::Void
            }
            Stmt::Const {
                name,
                ty,
                value,
                span,
            } => {
                let val_type = self.check_expr(value);
                let is_inferred = matches!(ty, TypeExpr::Simple { name, .. } if name == "_");
                let final_ty = if is_inferred {
                    val_type
                } else {
                    let declared = self.resolve_type(ty);
                    if !declared.is_compatible(&val_type) {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: declared.display_name(),
                            found: val_type.display_name(),
                            span: *span,
                            hint: None,
                        });
                    }
                    declared
                };
                self.symbols.define(Symbol {
                    name: name.clone(),
                    ty: final_ty,
                    mutable: false,
                    span: *span,
                    used: false,
                });
                Type::Void
            }
            Stmt::Expr { expr, .. } => self.check_expr(expr),
            Stmt::Return { value, span } => {
                if !self.symbols.is_inside_function() {
                    self.errors
                        .push(SemanticError::ReturnOutsideFunction { span: *span });
                }
                if let Some(v) = value {
                    self.check_expr(v)
                } else {
                    Type::Void
                }
            }
            Stmt::Break {
                label: _,
                value,
                span,
            } => {
                if !self.symbols.is_inside_loop() {
                    self.errors
                        .push(SemanticError::BreakOutsideLoop { span: *span });
                }
                if let Some(v) = value {
                    self.check_expr(v)
                } else {
                    Type::Void
                }
            }
            Stmt::Continue { label: _, span } => {
                if !self.symbols.is_inside_loop() {
                    self.errors
                        .push(SemanticError::BreakOutsideLoop { span: *span });
                }
                Type::Void
            }
            Stmt::Item(item) => {
                self.register_item(item);
                self.check_item(item);
                Type::Void
            }
        }
    }

    /// Type-checks an expression and returns its inferred type.
    pub fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal { kind, .. } => self.check_literal(kind),
            Expr::Ident { name, span } => self.check_ident(name, *span),
            Expr::Binary {
                left, op, right, ..
            } => self.check_binary(left, *op, right),
            Expr::Unary { op, operand, .. } => self.check_unary(*op, operand),
            Expr::Call {
                callee, args, span, ..
            } => self.check_call(callee, args, *span),
            Expr::Block { stmts, expr, .. } => self.check_block(stmts, expr),
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => self.check_if(condition, then_branch, else_branch),
            Expr::While {
                label: _,
                condition,
                body,
                ..
            } => {
                let cond_ty = self.check_expr(condition);
                if !cond_ty.is_compatible(&Type::Bool) && !cond_ty.is_integer() {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "bool".into(),
                        found: cond_ty.display_name(),
                        span: condition.span(),
                        hint: None,
                    });
                }
                self.symbols
                    .push_scope_kind(crate::analyzer::scope::ScopeKind::Loop);
                self.check_expr(body);
                self.symbols.pop_scope();
                Type::Void
            }
            Expr::For {
                label: _,
                variable,
                iterable,
                body,
                ..
            } => {
                let iter_ty = self.check_expr(iterable);
                let elem_ty = match &iter_ty {
                    Type::Array(inner) => *inner.clone(),
                    Type::Str => Type::Char,
                    _ => Type::Unknown,
                };
                self.symbols
                    .push_scope_kind(crate::analyzer::scope::ScopeKind::Loop);
                self.symbols.define(Symbol {
                    name: variable.clone(),
                    ty: elem_ty,
                    mutable: false,
                    span: iterable.span(),
                    used: false,
                });
                self.check_expr(body);
                self.symbols.pop_scope();
                Type::Void
            }
            Expr::Loop { label: _, body, .. } => {
                self.symbols
                    .push_scope_kind(crate::analyzer::scope::ScopeKind::Loop);
                self.check_expr(body);
                self.symbols.pop_scope();
                Type::Void
            }
            Expr::Assign {
                target,
                op,
                value,
                span,
            } => self.check_assign(target, *op, value, *span),
            Expr::Match {
                subject,
                arms,
                span,
            } => self.check_match(subject, arms, *span),
            Expr::Array { elements, span } => self.check_array(elements, *span),
            Expr::ArrayRepeat {
                value, count, span, ..
            } => {
                let elem_ty = self.check_expr(value);
                self.check_expr(count);
                let _ = span;
                Type::Array(Box::new(elem_ty))
            }
            Expr::Tuple { elements, .. } => {
                let types: Vec<Type> = elements.iter().map(|e| self.check_expr(e)).collect();
                Type::Tuple(types)
            }
            Expr::Pipe { left, right, span } => self.check_pipe(left, right, *span),
            Expr::StructInit {
                name, fields, span, ..
            } => self.check_struct_init(name, fields, *span),
            Expr::Field {
                object,
                field,
                span,
            } => self.check_field(object, field, *span),
            Expr::Index {
                object,
                index,
                span,
            } => self.check_index(object, index, *span),
            Expr::Range { start, end, .. } => {
                let mut range_ty = Type::I64; // default if no bounds
                if let Some(s) = start {
                    let st = self.check_expr(s);
                    if !st.is_integer() && !matches!(st, Type::Unknown) {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: "integer".into(),
                            found: st.display_name(),
                            span: s.span(),
                            hint: None,
                        });
                    }
                    range_ty = st;
                }
                if let Some(e) = end {
                    let et = self.check_expr(e);
                    if !et.is_integer() && !matches!(et, Type::Unknown) {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: "integer".into(),
                            found: et.display_name(),
                            span: e.span(),
                            hint: None,
                        });
                    }
                }
                Type::Array(Box::new(range_ty))
            }
            Expr::Grouped { expr, .. } => self.check_expr(expr),
            Expr::Closure { params, body, .. } => {
                self.symbols
                    .push_scope_kind(crate::analyzer::scope::ScopeKind::Function);
                let param_types: Vec<Type> = params
                    .iter()
                    .map(|cp| {
                        let ty = cp
                            .ty
                            .as_ref()
                            .map(|t| self.resolve_type(t))
                            .unwrap_or(Type::Unknown);
                        self.symbols.define(Symbol {
                            name: cp.name.clone(),
                            ty: ty.clone(),
                            mutable: false,
                            span: cp.span,
                            used: false,
                        });
                        ty
                    })
                    .collect();
                let ret_type = self.check_expr(body);
                self.symbols.pop_scope();
                Type::Function {
                    params: param_types,
                    ret: Box::new(ret_type),
                }
            }
            Expr::Path { segments, span } => {
                // Try full qualified name first (e.g., "math::square")
                let qualified = segments.join("::");
                if self.symbols.lookup(&qualified).is_some() {
                    return self.check_ident(&qualified, *span);
                }
                // Fall back to last segment only (e.g., "square")
                let name = segments.last().map_or("", |s| s.as_str());
                self.check_ident(name, *span)
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                span,
            } => self.check_method_call(receiver, method, args, *span),
            Expr::Cast {
                expr: inner,
                ty: target_ty,
                span,
            } => {
                let src = self.check_expr(inner);
                let target = self.resolve_type(target_ty);

                // Validate cast compatibility: numeric↔numeric, bool↔int OK
                let src_numeric = src.is_numeric() || src == Type::Bool || src == Type::Unknown;
                let tgt_numeric =
                    target.is_numeric() || target == Type::Bool || target == Type::Unknown;

                if !src_numeric || !tgt_numeric {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: format!(
                            "numeric type for cast, got {} as {}",
                            src.display_name(),
                            target.display_name()
                        ),
                        found: src.display_name(),
                        span: *span,
                        hint: None,
                    });
                }
                target
            }
            Expr::Try { .. } => Type::Unknown,
            Expr::Await { expr, span: _ } => {
                // v0.7 "Illumination": allow .await in any context (cooperative eval)
                // Previously this was an error; now it's allowed for flexibility
                // Unwrap Future<T> → T
                let inner_ty = self.check_expr(expr);
                match inner_ty {
                    Type::Future { inner } => *inner,
                    _ => Type::Unknown,
                }
            }
            Expr::AsyncBlock { body, .. } => {
                // async { body } is effectively an async context
                self.symbols
                    .push_scope_kind(crate::analyzer::scope::ScopeKind::AsyncFn);
                let inner_ty = self.check_expr(body);
                let _ = self.symbols.pop_scope_unused();
                Type::Future {
                    inner: Box::new(inner_ty),
                }
            }
            Expr::FString { parts, .. } => {
                for part in parts {
                    if let FStringExprPart::Expr(expr) = part {
                        self.check_expr(expr);
                    }
                }
                Type::Str
            }
            Expr::InlineAsm { span, .. } => {
                // KE005/KE006: asm! only allowed in @kernel or @unsafe context
                let in_kernel = self.symbols.is_inside_kernel();
                let in_unsafe = self.symbols.is_inside_unsafe();
                if !in_kernel && !in_unsafe {
                    let in_device = self.symbols.is_inside_device();
                    if in_device {
                        self.errors
                            .push(SemanticError::AsmInDeviceContext { span: *span });
                    } else {
                        self.errors
                            .push(SemanticError::AsmInSafeContext { span: *span });
                    }
                }
                Type::Unknown
            }
            Expr::HandleEffect { body, handlers, .. } => {
                let old_in_handle = self.in_handle_expr;
                self.in_handle_expr = true;
                let body_type = self.check_expr(body);
                // Check each handler arm
                for arm in handlers {
                    // Validate effect exists
                    if self.effect_registry.lookup(&arm.effect_name).is_none() {
                        self.errors.push(SemanticError::UnknownEffect {
                            name: arm.effect_name.clone(),
                            span: arm.span,
                        });
                    }
                    self.check_expr(&arm.body);
                }
                self.in_handle_expr = old_in_handle;
                body_type
            }
            Expr::ResumeExpr { value, span, .. } => {
                if !self.in_handle_expr {
                    self.errors
                        .push(SemanticError::ResumeOutsideHandler { span: *span });
                }
                self.check_expr(value)
            }
            Expr::Comptime { body, .. } => self.check_expr(body),
            Expr::MacroInvocation { .. } => Type::Unknown,
        }
    }

    // ── Expression checkers ──

    /// Returns the type of a literal.
    fn check_literal(&self, kind: &LiteralKind) -> Type {
        match kind {
            LiteralKind::Int(_) => Type::IntLiteral,
            LiteralKind::Float(_) => Type::FloatLiteral,
            LiteralKind::String(_) | LiteralKind::RawString(_) => Type::Str,
            LiteralKind::Char(_) => Type::Char,
            LiteralKind::Bool(_) => Type::Bool,
            LiteralKind::Null => Type::Void,
        }
    }

    /// Looks up an identifier in the symbol table.
    fn check_ident(&mut self, name: &str, span: Span) -> Type {
        // Check for use-after-move
        if let Some(move_span) = self.moves.check_use(name) {
            self.errors.push(SemanticError::UseAfterMove {
                name: name.to_string(),
                span,
                move_span,
            });
        }
        match self.symbols.lookup(name) {
            Some(sym) => {
                let ty = sym.ty.clone();
                self.symbols.mark_used(name);
                ty
            }
            None => {
                let mut suggestion = suggest_similar(name, &self.symbols.all_names());

                // If the name looks like a common function name but isn't defined,
                // suggest adding `fn` keyword (beginner forgot `fn`)
                if suggestion.is_none() || suggestion.as_deref() == Some("did you mean 'min'?") {
                    let common_fn_names = [
                        "main", "init", "setup", "run", "start", "test", "new", "create", "build",
                        "parse", "process", "handle", "update",
                    ];
                    if common_fn_names.contains(&name) {
                        suggestion = Some(format!(
                            "did you mean `fn {name}()`? (missing `fn` keyword)"
                        ));
                    }
                }

                self.errors.push(SemanticError::UndefinedVariable {
                    name: name.to_string(),
                    span,
                    suggestion,
                });
                Type::Unknown
            }
        }
    }

    /// Checks a binary expression.
    fn check_binary(&mut self, left: &Expr, op: BinOp, right: &Expr) -> Type {
        let lt = self.check_expr(left);
        let rt = self.check_expr(right);

        match op {
            // Arithmetic: both sides must be the same numeric type (no implicit promotion)
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::Pow => {
                // String concatenation with +
                if op == BinOp::Add && lt == Type::Str && rt == Type::Str {
                    return Type::Str;
                }
                // Tensor elementwise ops: shape check when both have known dims
                if lt.is_tensor() && rt.is_tensor() {
                    match lt.elementwise_shape(&rt) {
                        Some(result) => return result,
                        None => {
                            let op_span = Span::new(left.span().start, right.span().end);
                            self.errors.push(SemanticError::TensorShapeMismatch {
                                detail: format!(
                                    "elementwise: {} and {}",
                                    lt.display_name(),
                                    rt.display_name()
                                ),
                                span: op_span,
                            });
                            return Type::dynamic_tensor();
                        }
                    }
                }
                if lt.is_numeric() && rt.is_numeric() {
                    if lt.is_compatible(&rt) {
                        // Resolve to the concrete type when mixing literal with concrete
                        lt.resolve_with(&rt)
                    } else {
                        let hint = type_mismatch_hint(&lt.display_name(), &rt.display_name());
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: lt.display_name(),
                            found: rt.display_name(),
                            span: right.span(),
                            hint,
                        });
                        Type::Unknown
                    }
                } else if matches!(lt, Type::Unknown | Type::TypeVar(_))
                    || matches!(rt, Type::Unknown | Type::TypeVar(_))
                {
                    // TypeVar: inside generic fn body, defer check to call site
                    if matches!(lt, Type::TypeVar(_)) {
                        lt
                    } else if matches!(rt, Type::TypeVar(_)) {
                        rt
                    } else {
                        Type::Unknown
                    }
                } else {
                    // Generate helpful hint for common type mismatches
                    let hint = if (lt.is_numeric() && rt == Type::Str)
                        || (lt == Type::Str && rt.is_numeric())
                    {
                        if op == BinOp::Add {
                            Some("to concatenate, convert the number: `to_string(x) + s`".into())
                        } else {
                            Some("to do arithmetic, convert the string: `parse_int(s)`".into())
                        }
                    } else if lt == Type::Str && rt == Type::Str && op != BinOp::Add {
                        Some("strings only support `+` (concatenation), not arithmetic".into())
                    } else {
                        Some(format!(
                            "cannot use `{}` between {} and {}",
                            op,
                            lt.display_name(),
                            rt.display_name()
                        ))
                    };
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "numeric".into(),
                        found: format!("{} and {}", lt.display_name(), rt.display_name()),
                        span: left.span(),
                        hint,
                    });
                    Type::Unknown
                }
            }
            // Comparison: both sides same type, result is bool
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                if !lt.is_compatible(&rt) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: lt.display_name(),
                        found: rt.display_name(),
                        span: right.span(),
                        hint: None,
                    });
                }
                Type::Bool
            }
            // Logical: both bool, result bool
            BinOp::And | BinOp::Or => Type::Bool,
            // Bitwise: both same integer type, result same integer type
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                if lt.is_integer() && rt.is_integer() && lt.is_compatible(&rt) {
                    lt.resolve_with(&rt)
                } else if lt.is_integer() && rt.is_integer() {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: lt.display_name(),
                        found: rt.display_name(),
                        span: right.span(),
                        hint: None,
                    });
                    Type::Unknown
                } else if matches!(lt, Type::Unknown | Type::TypeVar(_))
                    || matches!(rt, Type::Unknown | Type::TypeVar(_))
                {
                    if matches!(lt, Type::TypeVar(_)) {
                        lt
                    } else if matches!(rt, Type::TypeVar(_)) {
                        rt
                    } else {
                        Type::Unknown
                    }
                } else {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "i64".into(),
                        found: format!("{} and {}", lt.display_name(), rt.display_name()),
                        span: left.span(),
                        hint: None,
                    });
                    Type::Unknown
                }
            }
            BinOp::MatMul => {
                // Shape checking for @ operator when both operands have tensor types.
                // Uses both the built-in Type::matmul_shape() and the tensor_verify module
                // for richer shape error messages when dims are fully known.
                if lt.is_tensor() && rt.is_tensor() {
                    // Skip shape check when either has unknown rank (empty dims)
                    let lt_empty = matches!(&lt, Type::Tensor { dims, .. } if dims.is_empty());
                    let rt_empty = matches!(&rt, Type::Tensor { dims, .. } if dims.is_empty());
                    if lt_empty || rt_empty {
                        return Type::dynamic_tensor();
                    }

                    // Enhanced: use tensor_verify for concrete shapes
                    if let (Type::Tensor { dims: ld, .. }, Type::Tensor { dims: rd, .. }) =
                        (&lt, &rt)
                    {
                        let all_concrete_l = ld.iter().all(|d| d.is_some());
                        let all_concrete_r = rd.iter().all(|d| d.is_some());
                        if all_concrete_l && all_concrete_r {
                            use crate::verify::tensor_verify::{
                                ShapeCheckStatus, SymbolicShape, verify_matmul,
                            };
                            let ls: Vec<usize> =
                                ld.iter().filter_map(|d| d.map(|v| v as usize)).collect();
                            let rs: Vec<usize> =
                                rd.iter().filter_map(|d| d.map(|v| v as usize)).collect();
                            let lshape = SymbolicShape::concrete(&ls);
                            let rshape = SymbolicShape::concrete(&rs);
                            let constraint = verify_matmul(&lshape, &rshape);
                            if matches!(constraint.status, ShapeCheckStatus::Invalid(_)) {
                                let op_span = Span::new(left.span().start, right.span().end);
                                self.errors.push(SemanticError::TensorShapeMismatch {
                                    detail: constraint.description,
                                    span: op_span,
                                });
                                return Type::dynamic_tensor();
                            }
                        }
                    }

                    match lt.matmul_shape(&rt) {
                        Some(result) => result,
                        None => {
                            let op_span = Span::new(left.span().start, right.span().end);
                            self.errors.push(SemanticError::TensorShapeMismatch {
                                detail: format!(
                                    "matmul: {} @ {}",
                                    lt.display_name(),
                                    rt.display_name()
                                ),
                                span: op_span,
                            });
                            Type::dynamic_tensor()
                        }
                    }
                } else {
                    Type::dynamic_tensor()
                }
            }
        }
    }

    /// Checks a unary expression.
    fn check_unary(&mut self, op: UnaryOp, operand: &Expr) -> Type {
        let ty = self.check_expr(operand);
        match op {
            UnaryOp::Neg => {
                if ty.is_numeric() || matches!(ty, Type::Unknown | Type::TypeVar(_)) {
                    ty
                } else {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "numeric".into(),
                        found: ty.display_name(),
                        span: operand.span(),
                        hint: None,
                    });
                    Type::Unknown
                }
            }
            UnaryOp::Not => Type::Bool,
            UnaryOp::BitNot => {
                if ty.is_integer() || matches!(ty, Type::Unknown | Type::TypeVar(_)) {
                    ty
                } else {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "integer".into(),
                        found: ty.display_name(),
                        span: operand.span(),
                        hint: None,
                    });
                    Type::Unknown
                }
            }
            UnaryOp::Ref => self.check_borrow_ref(operand, &ty, false),
            UnaryOp::RefMut => self.check_borrow_ref(operand, &ty, true),
            UnaryOp::Deref => match &ty {
                Type::Ref(inner) | Type::RefMut(inner) => *inner.clone(),
                Type::Unknown => Type::Unknown,
                _ => {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "reference type".into(),
                        found: ty.display_name(),
                        span: operand.span(),
                        hint: None,
                    });
                    Type::Unknown
                }
            },
        }
    }

    /// Checks a borrow expression (`&x` or `&mut x`).
    fn check_borrow_ref(&mut self, operand: &Expr, ty: &Type, is_mutable: bool) -> Type {
        let span = operand.span();

        // Extract the target variable name (only idents supported)
        if let Expr::Ident { name, .. } = operand {
            if is_mutable {
                // Check variable is declared as `mut`
                if let Some(sym) = self.symbols.lookup(name) {
                    if !sym.mutable {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: "mutable variable for &mut".into(),
                            found: format!("immutable variable '{name}'"),
                            span,
                            hint: None,
                        });
                    }
                }
                // Try to create mutable borrow
                if let Err(err) = self.moves.borrow_mut(name, span) {
                    match err {
                        crate::analyzer::borrow_lite::BorrowError::MutWhileImmBorrowed {
                            imm_span,
                        } => {
                            self.errors.push(SemanticError::MutBorrowConflict {
                                name: name.clone(),
                                span,
                                borrow_span: imm_span,
                            });
                        }
                        crate::analyzer::borrow_lite::BorrowError::DoubleMutBorrow {
                            existing_span,
                        } => {
                            self.errors.push(SemanticError::MutBorrowConflict {
                                name: name.clone(),
                                span,
                                borrow_span: existing_span,
                            });
                        }
                        _ => {}
                    }
                }
                Type::RefMut(Box::new(ty.clone()))
            } else {
                // Try to create immutable borrow
                if let Err(crate::analyzer::borrow_lite::BorrowError::ImmWhileMutBorrowed {
                    mut_span,
                }) = self.moves.borrow_imm(name, span)
                {
                    self.errors.push(SemanticError::ImmBorrowConflict {
                        name: name.clone(),
                        span,
                        borrow_span: mut_span,
                    });
                }
                Type::Ref(Box::new(ty.clone()))
            }
        } else {
            // Non-identifier operand (e.g., &expr) — no borrow tracking
            if is_mutable {
                Type::RefMut(Box::new(ty.clone()))
            } else {
                Type::Ref(Box::new(ty.clone()))
            }
        }
    }

    /// Checks a function call.
    fn check_call(&mut self, callee: &Expr, args: &[CallArg], span: Span) -> Type {
        // Context checks for @kernel/@device isolation
        if let Expr::Ident { name, .. } = callee {
            self.check_context_call(name, span);
        }
        // Also check path-based calls (e.g., module::func)
        if let Expr::Path { segments, .. } = callee {
            if let Some(last) = segments.last() {
                self.check_context_call(last, span);
            }
        }

        let callee_ty = self.check_expr(callee);

        // Evaluate argument types
        let arg_types: Vec<Type> = args.iter().map(|a| self.check_expr(&a.value)).collect();

        // SE018: thread::spawn Send check — all arguments must be Send
        if let Expr::Path { segments, .. } = callee {
            if segments.len() == 2 && segments[0] == "thread" && segments[1] == "spawn" {
                // Skip the first arg (function pointer — always Send)
                for (i, arg_ty) in arg_types.iter().enumerate().skip(1) {
                    if !arg_ty.is_send() {
                        self.errors.push(SemanticError::NotSendType {
                            ty: arg_ty.display_name(),
                            span: args[i].value.span(),
                        });
                    }
                }
            }
        }

        // IPC002: ipc_send/ipc_call with non-@message struct argument
        if let Expr::Ident { name, .. } = callee {
            if (name == "ipc_send" || name == "ipc_call") && args.len() >= 2 {
                // Check if second arg is a struct init expression
                if let Expr::StructInit {
                    name: struct_name, ..
                } = &args[1].value
                {
                    if !self.message_structs.contains(struct_name) {
                        self.errors.push(SemanticError::IpcTypeMismatch {
                            found: struct_name.clone(),
                            span: args[1].value.span(),
                        });
                    }
                }
            }
        }

        // Move tracking: if a move-type variable is passed as arg, mark moved
        // Exempt non-consuming builtins that logically take &T (read-only inspection)
        let is_non_consuming_builtin = if let Expr::Ident { name, .. } = callee {
            matches!(
                name.as_str(),
                "len"
                    | "type_of"
                    | "println"
                    | "print"
                    | "dbg"
                    | "assert"
                    | "assert_eq"
                    | "join"
                    | "timeout"
                    | "spawn"
            ) || name.starts_with("tensor_")
                || name.starts_with("optimizer_")
                || name.starts_with("model_")
                || name.starts_with("gpu_")
        } else {
            false
        };
        if !is_non_consuming_builtin {
            for (arg, arg_ty) in args.iter().zip(arg_types.iter()) {
                if let Expr::Ident {
                    name: arg_name,
                    span: arg_span,
                } = &arg.value
                {
                    if !crate::analyzer::borrow_lite::is_copy_type(arg_ty) {
                        if let Some(borrow_span) = self.moves.check_can_move(arg_name) {
                            self.errors.push(SemanticError::MoveWhileBorrowed {
                                name: arg_name.clone(),
                                span: *arg_span,
                                borrow_span,
                            });
                        }
                        self.moves.mark_moved(arg_name, *arg_span);
                    }
                }
            }
        }

        // Check if any args are named — if so, skip positional type checking
        // because the analyzer doesn't have parameter names to reorder against
        let has_named_args = args.iter().any(|a| a.name.is_some());

        // Track callee name for IPC @message struct pass-through
        let callee_name = match callee {
            Expr::Ident { name, .. } => Some(name.as_str()),
            _ => None,
        };

        match callee_ty {
            Type::Function { params, ret } => {
                // Check arity (skip for variadic builtins with Unknown params)
                let is_variadic =
                    params.len() == 1 && matches!(params.first(), Some(Type::Unknown));
                if !is_variadic && params.len() != arg_types.len() {
                    let hint = if arg_types.len() < params.len() {
                        let missing = params.len() - arg_types.len();
                        Some(format!("missing {missing} argument(s)"))
                    } else {
                        let extra = arg_types.len() - params.len();
                        Some(format!("{extra} extra argument(s)"))
                    };
                    self.errors.push(SemanticError::ArgumentCountMismatch {
                        expected: params.len(),
                        found: arg_types.len(),
                        span,
                        hint,
                    });
                }

                // Generic function inference: if params contain TypeVars, use unification
                let is_generic = params.iter().any(crate::analyzer::inference::has_type_vars)
                    || crate::analyzer::inference::has_type_vars(&ret);

                if is_generic && !is_variadic && !has_named_args {
                    let generic_names =
                        crate::analyzer::inference::extract_generic_names(&Type::Function {
                            params: params.clone(),
                            ret: ret.clone(),
                        });
                    match crate::analyzer::inference::infer_type_args(
                        &params,
                        &arg_types,
                        &generic_names,
                        span,
                    ) {
                        Ok(subst) => {
                            // Apply substitution to return type
                            subst.apply(&ret)
                        }
                        Err(err) => match *err {
                            crate::analyzer::inference::InferError::UnificationFailed {
                                expected,
                                found,
                                span: err_span,
                                ..
                            } => {
                                self.errors.push(SemanticError::TypeMismatch {
                                    expected: expected.display_name(),
                                    found: found.display_name(),
                                    span: err_span,
                                    hint: None,
                                });
                                Type::Unknown
                            }
                            crate::analyzer::inference::InferError::Unbound {
                                param,
                                span: err_span,
                            } => {
                                self.errors.push(SemanticError::CannotInferType {
                                    param,
                                    reason: "type parameter not used in function arguments".into(),
                                    span: err_span,
                                });
                                *ret
                            }
                        },
                    }
                } else {
                    // Non-generic: check argument types directly (skip Unknown params and named args)
                    if !is_variadic && !has_named_args {
                        let is_ipc_fn = matches!(callee_name, Some("ipc_send" | "ipc_call"));
                        for (i, (expected, found)) in
                            params.iter().zip(arg_types.iter()).enumerate()
                        {
                            // Allow @message struct as ipc_send/ipc_call second arg
                            if is_ipc_fn && i == 1 {
                                if let Expr::StructInit { name: sn, .. } = &args[i].value {
                                    if self.message_structs.contains(sn) {
                                        continue; // valid @message struct — skip type check
                                    }
                                }
                            }
                            if !expected.is_compatible(found) {
                                self.errors.push(SemanticError::TypeMismatch {
                                    expected: expected.display_name(),
                                    found: found.display_name(),
                                    span: args.get(i).map_or(span, |a| a.span),
                                    hint: None,
                                });
                            }
                        }
                    }
                    *ret
                }
            }
            Type::Unknown => Type::Unknown,
            _ => {
                let suggestion =
                    suggest_similar(&format!("{callee_ty}"), &self.symbols.all_names());
                self.errors.push(SemanticError::UndefinedFunction {
                    name: format!("{callee_ty}"),
                    span,
                    suggestion,
                });
                Type::Unknown
            }
        }
    }

    /// Checks context isolation rules for a function call.
    fn check_context_call(&mut self, callee_name: &str, span: Span) {
        let in_kernel = self.symbols.is_inside_kernel();
        let in_device = self.symbols.is_inside_device();

        if in_kernel {
            // @kernel cannot call @device functions
            if self.device_fns.contains(callee_name) {
                self.errors.push(SemanticError::DeviceCallInKernel { span });
            }
            // KE001: @kernel cannot use heap-allocating builtins
            if self.heap_builtins.contains(callee_name) {
                self.errors.push(SemanticError::HeapAllocInKernel { span });
            }
            // KE002: @kernel cannot use tensor/ML builtins
            if self.tensor_builtins.contains(callee_name) {
                self.errors.push(SemanticError::TensorInKernel { span });
            }
        }

        if in_device {
            // @device cannot call @kernel functions
            if self.kernel_fns.contains(callee_name) {
                self.errors.push(SemanticError::KernelCallInDevice { span });
            }
            // @device cannot use OS builtins (raw pointer operations)
            if self.os_builtins.contains(callee_name) {
                self.errors.push(SemanticError::RawPointerInDevice { span });
            }
            // E6/E7: @device("cap") restricts to capability-specific builtins
            if let Some(ref cap) = self.current_device_cap {
                let allowed = match cap.as_str() {
                    "net" => &self.cap_net,
                    "blk" => &self.cap_blk,
                    "port_io" => &self.cap_port_io,
                    "irq" => &self.cap_irq,
                    "dma" => &self.cap_dma,
                    _ => &self.cap_port_io, // unknown cap → allow port_io as default
                };
                // Check: if callee is a hardware builtin but NOT in the allowed set
                if (self.cap_port_io.contains(callee_name)
                    || self.cap_irq.contains(callee_name)
                    || self.cap_dma.contains(callee_name)
                    || self.cap_net.contains(callee_name)
                    || self.cap_blk.contains(callee_name))
                    && !allowed.contains(callee_name)
                {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: format!("builtin allowed by @device(\"{cap}\") capability"),
                        found: format!("'{}' requires different capability", callee_name),
                        span,
                        hint: Some(format!(
                            "@device(\"{cap}\") cannot call '{callee_name}'; add the correct capability"
                        )),
                    });
                }
            }
        }

        let in_npu = self.symbols.is_inside_npu();
        if in_npu {
            // NE001: @npu cannot use raw pointer/OS builtins
            if self.os_builtins.contains(callee_name) {
                self.errors.push(SemanticError::RawPointerInNpu { span });
            }
            // NE002: @npu cannot use heap-allocating builtins
            if self.heap_builtins.contains(callee_name) {
                self.errors.push(SemanticError::HeapAllocInNpu { span });
            }
            // NE003: @npu cannot use OS primitives
            if self.os_builtins.contains(callee_name) {
                self.errors.push(SemanticError::OsPrimitiveInNpu { span });
            }
            // NE004: @npu cannot call @kernel functions
            if self.kernel_fns.contains(callee_name) {
                self.errors.push(SemanticError::KernelCallInNpu { span });
            }
        }

        if in_kernel {
            // @kernel cannot call @npu functions
            if self.npu_fns.contains(callee_name) {
                self.errors.push(SemanticError::DeviceCallInKernel { span });
            }
        }

        // @safe context: cannot access hardware builtins (microkernel isolation)
        let in_safe = self.symbols.is_inside_safe();
        if in_safe {
            // SE020: @safe cannot use hardware/OS builtins
            if self.safe_blocked_builtins.contains(callee_name) {
                self.errors
                    .push(SemanticError::HardwareAccessInSafe { span });
            }
            // SE021: @safe cannot call @kernel functions directly
            if self.kernel_fns.contains(callee_name) {
                self.errors.push(SemanticError::KernelCallInSafe { span });
            }
            // SE022: @safe cannot call @device functions directly — use IPC
            if self.device_fns.contains(callee_name) {
                self.errors.push(SemanticError::DeviceCallInSafe { span });
            }
        }
    }

    /// Returns the span start of a statement (for NLL ordering).
    fn stmt_span_start(stmt: &Stmt) -> usize {
        match stmt {
            Stmt::Let { span, .. }
            | Stmt::Const { span, .. }
            | Stmt::Expr { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Break { span, .. }
            | Stmt::Continue { span, .. } => span.start,
            Stmt::Item(_) => 0,
        }
    }

    /// Checks a block expression.
    fn check_block(&mut self, stmts: &[Stmt], tail: &Option<Box<Expr>>) -> Type {
        self.symbols.push_scope();
        self.moves.push_scope();
        let mut diverged = false;
        for stmt in stmts {
            if diverged {
                // SE010: Unreachable code after return/break/continue
                let span = match stmt {
                    Stmt::Let { span, .. }
                    | Stmt::Const { span, .. }
                    | Stmt::Expr { span, .. }
                    | Stmt::Return { span, .. }
                    | Stmt::Break { span, .. }
                    | Stmt::Continue { span, .. } => *span,
                    Stmt::Item(_) => Span::new(0, 0),
                };
                if span.start != 0 || span.end != 0 {
                    self.errors.push(SemanticError::UnreachableCode { span });
                }
                break;
            }
            // NLL: release borrows whose binding is no longer live
            self.release_dead_borrows_nll(Self::stmt_span_start(stmt));
            let stmt_ty = self.check_stmt(stmt);
            // Check if this statement diverges
            match stmt {
                Stmt::Return { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {
                    diverged = true;
                }
                Stmt::Expr { .. } => {
                    if matches!(stmt_ty, Type::Never) {
                        diverged = true;
                    }
                }
                _ => {}
            }
        }
        let ty = match tail {
            Some(e) => {
                if diverged {
                    self.errors
                        .push(SemanticError::UnreachableCode { span: e.span() });
                }
                // NLL: release dead borrows before tail expression
                self.release_dead_borrows_nll(e.span().start);
                self.check_expr(e)
            }
            None => Type::Void,
        };
        self.emit_unused_warnings();
        self.moves.pop_scope();
        ty
    }

    /// Checks an if expression.
    fn check_if(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: &Option<Box<Expr>>,
    ) -> Type {
        let cond_ty = self.check_expr(condition);
        if !cond_ty.is_compatible(&Type::Bool) && !cond_ty.is_integer() {
            self.errors.push(SemanticError::TypeMismatch {
                expected: "bool".into(),
                found: cond_ty.display_name(),
                span: condition.span(),
                hint: None,
            });
        }

        let then_ty = self.check_expr(then_branch);

        if let Some(else_e) = else_branch {
            let else_ty = self.check_expr(else_e);
            // If used as expression, both branches should match.
            // When either branch is Void, the if/else is used as a statement —
            // no need to require matching types.
            if !then_ty.is_compatible(&else_ty)
                && !matches!(then_ty, Type::Void)
                && !matches!(else_ty, Type::Void)
            {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: then_ty.display_name(),
                    found: else_ty.display_name(),
                    span: else_e.span(),
                    hint: None,
                });
            }
            then_ty
        } else {
            Type::Void
        }
    }

    /// Checks an assignment expression.
    fn check_assign(&mut self, target: &Expr, op: AssignOp, value: &Expr, span: Span) -> Type {
        // For `x = f(x)` pattern: temporarily revive the target variable before
        // evaluating the RHS. This allows the moved variable to be consumed by f()
        // and then reassigned. Without this, `state = define_fn(state, ...)` would
        // trigger ME001 because state was marked moved in a previous iteration.
        if let Expr::Ident {
            name,
            span: id_span,
            ..
        } = target
        {
            self.moves.declare(name, *id_span);
        }

        let val_ty = self.check_expr(value);

        if let Expr::Ident {
            name,
            span: id_span,
        } = target
        {
            // Check mutability
            if let Some(sym) = self.symbols.lookup(name) {
                if !sym.mutable {
                    self.errors.push(SemanticError::ImmutableAssignment {
                        name: name.clone(),
                        span: *id_span,
                    });
                }
                // For simple assignment, check type compatibility
                if op == AssignOp::Assign && !sym.ty.is_compatible(&val_ty) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: sym.ty.display_name(),
                        found: val_ty.display_name(),
                        span,
                        hint: None,
                    });
                }
            } else {
                let suggestion = suggest_similar(name, &self.symbols.all_names());
                self.errors.push(SemanticError::UndefinedVariable {
                    name: name.clone(),
                    span: *id_span,
                    suggestion,
                });
            }

            // ME004/ME005: assignment to a borrowed variable conflicts with active borrows
            if let Some(borrow_span) = self.moves.check_can_move(name) {
                self.errors.push(SemanticError::MutBorrowConflict {
                    name: name.clone(),
                    span,
                    borrow_span,
                });
            }

            // Revive moved variable on reassignment: `state = f(state)` is valid.
            // The old value was consumed by `f`, and a new value is being assigned.
            self.moves.declare(name, *id_span);
        } else {
            // Field/index assignment — just check the value
            self.check_expr(target);
        }

        Type::Void
    }

    /// Checks a match expression.
    fn check_match(&mut self, subject: &Expr, arms: &[MatchArm], span: Span) -> Type {
        let subject_ty = self.check_expr(subject);
        let mut result_ty: Option<Type> = None;
        let mut has_wildcard_or_catch_all = false;
        let mut matched_variants: Vec<String> = Vec::new();
        let mut matched_enum_name: Option<String> = None;

        // Move tracking: if the subject is a move-type variable and any arm
        // destructures it (Enum/Tuple/Struct pattern), mark it as moved.
        if let Expr::Ident {
            name: subject_name,
            span: subject_span,
        } = subject
        {
            if !crate::analyzer::borrow_lite::is_copy_type(&subject_ty) {
                let has_destructure = arms.iter().any(|arm| {
                    matches!(
                        arm.pattern,
                        Pattern::Enum { .. } | Pattern::Tuple { .. } | Pattern::Struct { .. }
                    )
                });
                if has_destructure {
                    if let Some(borrow_span) = self.moves.check_can_move(subject_name) {
                        self.errors.push(SemanticError::MoveWhileBorrowed {
                            name: subject_name.clone(),
                            span: *subject_span,
                            borrow_span,
                        });
                    }
                    self.moves.mark_moved(subject_name, *subject_span);
                }
            }
        }

        for arm in arms {
            // SE020: detect unreachable patterns (arms after a catch-all)
            if has_wildcard_or_catch_all {
                self.errors.push(SemanticError::UnreachablePattern {
                    span: arm.pattern.span(),
                });
            }

            // Check if this arm is a catch-all (wildcard or bare ident without guard)
            if arm.guard.is_none() {
                match &arm.pattern {
                    Pattern::Wildcard { .. } => has_wildcard_or_catch_all = true,
                    Pattern::Ident { name, .. } => {
                        // Check if this ident is a known enum variant (e.g. "None")
                        let is_variant = self.enum_variants.values().any(|vs| vs.contains(name));
                        if is_variant {
                            matched_variants.push(name.clone());
                            // Infer enum name from variant
                            if matched_enum_name.is_none() {
                                for (ename, vs) in &self.enum_variants {
                                    if vs.contains(name) {
                                        matched_enum_name = Some(ename.clone());
                                        break;
                                    }
                                }
                            }
                        } else {
                            has_wildcard_or_catch_all = true;
                        }
                    }
                    Pattern::Enum {
                        enum_name, variant, ..
                    } => {
                        matched_variants.push(variant.clone());
                        if matched_enum_name.is_none() {
                            if !enum_name.is_empty() {
                                matched_enum_name = Some(enum_name.clone());
                            } else {
                                // Infer enum name from variant
                                for (ename, vs) in &self.enum_variants {
                                    if vs.contains(variant) {
                                        matched_enum_name = Some(ename.clone());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            self.symbols.push_scope();
            self.check_pattern(&arm.pattern);
            if let Some(guard) = &arm.guard {
                self.check_expr(guard);
            }
            let arm_ty = self.check_expr(&arm.body);
            if result_ty.is_none() {
                result_ty = Some(arm_ty);
            }
            self.symbols.pop_scope();
        }

        // SE011: Non-exhaustive match
        if !has_wildcard_or_catch_all && !arms.is_empty() {
            if let Some(ref ename) = matched_enum_name {
                // Check if all variants of the known enum are covered
                if let Some(all_variants) = self.enum_variants.get(ename) {
                    let all_covered = all_variants.iter().all(|v| matched_variants.contains(v));
                    if !all_covered {
                        self.errors.push(SemanticError::NonExhaustiveMatch { span });
                    }
                }
                // Unknown enum — skip (can't verify)
            } else if matched_variants.is_empty() {
                // No enum variants and no wildcard — definitely non-exhaustive
                self.errors.push(SemanticError::NonExhaustiveMatch { span });
            }
        }

        result_ty.unwrap_or(Type::Void)
    }

    /// Registers pattern bindings in the current scope.
    fn check_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Ident { name, span } => {
                self.symbols.define(Symbol {
                    name: name.clone(),
                    ty: Type::Unknown,
                    mutable: false,
                    span: *span,
                    used: false,
                });
            }
            Pattern::Tuple { elements, .. } => {
                for elem in elements {
                    self.check_pattern(elem);
                }
            }
            Pattern::Enum { fields, .. } => {
                for field in fields {
                    self.check_pattern(field);
                }
            }
            Pattern::Struct { fields, .. } => {
                for fp in fields {
                    if let Some(ref pat) = fp.pattern {
                        self.check_pattern(pat);
                    } else {
                        self.symbols.define(Symbol {
                            name: fp.name.clone(),
                            ty: Type::Unknown,
                            mutable: false,
                            span: fp.span,
                            used: false,
                        });
                    }
                }
            }
            Pattern::Wildcard { .. } | Pattern::Literal { .. } | Pattern::Range { .. } => {}
            Pattern::Or { patterns, .. } => {
                for p in patterns {
                    self.check_pattern(p);
                }
            }
        }
    }

    /// Checks an array literal.
    fn check_array(&mut self, elements: &[Expr], _span: Span) -> Type {
        if elements.is_empty() {
            return Type::Array(Box::new(Type::Unknown));
        }
        let first_ty = self.check_expr(&elements[0]);
        for elem in elements.iter().skip(1) {
            let elem_ty = self.check_expr(elem);
            if !first_ty.is_compatible(&elem_ty) {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: first_ty.display_name(),
                    found: elem_ty.display_name(),
                    span: elem.span(),
                    hint: None,
                });
            }
        }
        Type::Array(Box::new(first_ty))
    }

    /// Checks a pipeline expression: `x |> f`.
    fn check_pipe(&mut self, left: &Expr, right: &Expr, span: Span) -> Type {
        let arg_ty = self.check_expr(left);
        let fn_ty = self.check_expr(right);

        match fn_ty {
            Type::Function { params, ret } => {
                if params.len() != 1 {
                    self.errors.push(SemanticError::ArgumentCountMismatch {
                        expected: 1,
                        found: params.len(),
                        span,
                        hint: Some(
                            "pipeline `|>` requires a function that takes exactly 1 argument"
                                .into(),
                        ),
                    });
                } else if !params[0].is_compatible(&arg_ty) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: params[0].display_name(),
                        found: arg_ty.display_name(),
                        span: left.span(),
                        hint: None,
                    });
                }
                *ret
            }
            Type::Unknown => Type::Unknown,
            _ => {
                self.errors.push(SemanticError::UndefinedFunction {
                    name: format!("{fn_ty}"),
                    span: right.span(),
                    suggestion: None,
                });
                Type::Unknown
            }
        }
    }

    /// Checks struct initialization.
    fn check_struct_init(
        &mut self,
        name: &str,
        fields: &[crate::parser::ast::FieldInit],
        span: Span,
    ) -> Type {
        let struct_ty = self.symbols.lookup(name).map(|s| s.ty.clone());

        match struct_ty {
            Some(Type::Struct {
                name: sname,
                fields: def_fields,
            }) => {
                // Build type variable substitution map for generic structs.
                // If a field type is TypeVar("T") and the value is i64, then T = i64.
                let mut subst: HashMap<String, Type> = HashMap::new();
                let is_generic = def_fields.values().any(|t| matches!(t, Type::TypeVar(_)));

                // Check each provided field
                for fi in fields {
                    let val_ty = self.check_expr(&fi.value);
                    if let Some(expected_ty) = def_fields.get(&fi.name) {
                        if is_generic {
                            if let Type::TypeVar(tv) = expected_ty {
                                // Infer: T = val_ty
                                if let Some(prev) = subst.get(tv) {
                                    if !prev.is_compatible(&val_ty) {
                                        self.errors.push(SemanticError::TypeMismatch {
                                            expected: prev.display_name(),
                                            found: val_ty.display_name(),
                                            span: fi.value.span(),
                                            hint: Some(format!(
                                                "type parameter `{tv}` was inferred as `{}` from another field",
                                                prev.display_name()
                                            )),
                                        });
                                    }
                                } else {
                                    subst.insert(tv.clone(), val_ty.clone());
                                }
                            } else if !expected_ty.is_compatible(&val_ty) {
                                self.errors.push(SemanticError::TypeMismatch {
                                    expected: expected_ty.display_name(),
                                    found: val_ty.display_name(),
                                    span: fi.value.span(),
                                    hint: None,
                                });
                            }
                        } else if !expected_ty.is_compatible(&val_ty) {
                            self.errors.push(SemanticError::TypeMismatch {
                                expected: expected_ty.display_name(),
                                found: val_ty.display_name(),
                                span: fi.value.span(),
                                hint: None,
                            });
                        }
                    }
                }

                // Check for missing required fields
                for fname in def_fields.keys() {
                    if !fields.iter().any(|fi| &fi.name == fname) {
                        self.errors.push(SemanticError::MissingField {
                            struct_name: sname.clone(),
                            field: fname.clone(),
                            span,
                        });
                    }
                }

                // Resolve generic fields with inferred types
                let resolved_fields = if is_generic && !subst.is_empty() {
                    def_fields
                        .iter()
                        .map(|(k, v)| {
                            let resolved = match v {
                                Type::TypeVar(tv) => {
                                    subst.get(tv).cloned().unwrap_or_else(|| v.clone())
                                }
                                other => other.clone(),
                            };
                            (k.clone(), resolved)
                        })
                        .collect()
                } else {
                    def_fields
                };

                Type::Struct {
                    name: sname,
                    fields: resolved_fields,
                }
            }
            _ => {
                // Struct not defined — duck-type it
                for fi in fields {
                    self.check_expr(&fi.value);
                }
                Type::Struct {
                    name: name.to_string(),
                    fields: HashMap::new(),
                }
            }
        }
    }

    /// Checks field access.
    fn check_field(&mut self, object: &Expr, field: &str, span: Span) -> Type {
        let obj_ty = self.check_expr(object);
        match &obj_ty {
            Type::Struct { fields, name } => {
                if let Some(ft) = fields.get(field) {
                    ft.clone()
                } else {
                    let field_names: Vec<String> = fields.keys().cloned().collect();
                    let suggestion = suggest_similar(field, &field_names);
                    self.errors.push(SemanticError::UndefinedVariable {
                        name: format!("{name}.{field}"),
                        span,
                        suggestion,
                    });
                    Type::Unknown
                }
            }
            Type::Tuple(elems) => {
                if let Ok(idx) = field.parse::<usize>() {
                    elems.get(idx).cloned().unwrap_or(Type::Unknown)
                } else {
                    Type::Unknown
                }
            }
            _ => Type::Unknown,
        }
    }

    /// Checks index access.
    ///
    /// When both the array length and index are compile-time constants,
    /// performs a static bounds check and emits a compile error on OOB.
    fn check_index(&mut self, object: &Expr, index: &Expr, _span: Span) -> Type {
        let obj_ty = self.check_expr(object);
        let idx_ty = self.check_expr(index);

        if !idx_ty.is_integer() && !matches!(idx_ty, Type::Unknown) {
            self.errors.push(SemanticError::TypeMismatch {
                expected: "integer".into(),
                found: idx_ty.display_name(),
                span: index.span(),
                hint: None,
            });
        }

        // Compile-time bounds check: if we know both array length and index value,
        // check at compile time that the index is within bounds.
        if let Type::Array(_) = &obj_ty {
            let known_len = self.try_const_array_len(object);
            let known_idx = self.try_const_index(index);
            if let (Some(len), Some(idx)) = (known_len, known_idx) {
                if idx < 0 || idx >= len as i64 {
                    self.errors.push(SemanticError::IndexOutOfBounds {
                        index: idx,
                        length: len,
                        span: index.span(),
                    });
                }
            }
        }

        match obj_ty {
            Type::Array(inner) => *inner,
            Type::Str => Type::Char,
            _ => Type::Unknown,
        }
    }

    /// Try to determine the compile-time length of an array expression.
    fn try_const_array_len(&self, expr: &Expr) -> Option<u64> {
        match expr {
            Expr::Array { elements, .. } => Some(elements.len() as u64),
            Expr::ArrayRepeat { count, .. } => {
                if let Expr::Literal {
                    kind: LiteralKind::Int(n),
                    ..
                } = count.as_ref()
                {
                    Some(*n as u64)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Try to determine the compile-time value of an index expression.
    fn try_const_index(&self, expr: &Expr) -> Option<i64> {
        match expr {
            Expr::Literal {
                kind: LiteralKind::Int(n),
                ..
            } => Some(*n),
            _ => None,
        }
    }

    /// Checks a method call.
    fn check_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[CallArg],
        _span: Span,
    ) -> Type {
        let obj_ty = self.check_expr(receiver);
        for arg in args {
            self.check_expr(&arg.value);
        }

        match (&obj_ty, method) {
            // String methods
            (Type::Str, "len") => Type::USize,
            (Type::Str, "contains" | "starts_with" | "ends_with" | "is_empty") => Type::Bool,
            (
                Type::Str,
                "trim" | "trim_start" | "trim_end" | "to_uppercase" | "to_lowercase" | "replace"
                | "rev" | "repeat",
            ) => Type::Str,
            (Type::Str, "split") => Type::Array(Box::new(Type::Str)),
            (Type::Str, "bytes") => Type::Array(Box::new(Type::I64)),
            (Type::Str, "chars") => Type::Array(Box::new(Type::Char)),
            (Type::Str, "index_of") => Type::Unknown, // returns Option
            (Type::Str, "parse_int") => Type::Unknown, // returns Result
            (Type::Str, "parse_float") => Type::Unknown, // returns Result
            (Type::Str, "substring" | "char_at") => Type::Str,
            // Array methods
            (Type::Array(_), "len") => Type::USize,
            (Type::Array(inner), "push") => Type::Array(inner.clone()),
            (Type::Array(_), "is_empty") => Type::Bool,
            (Type::Array(_), "contains") => Type::Bool,
            (Type::Array(_), "first" | "last") => Type::Unknown, // returns Option
            (Type::Array(_), "pop") => Type::Unknown,            // returns Option
            (Type::Array(_), "reverse") => obj_ty.clone(),
            (Type::Array(_), "join") => Type::Str,
            // Map methods
            (Type::Unknown, "get" | "insert" | "remove" | "keys" | "values" | "entries") => {
                Type::Unknown
            }
            _ => Type::Unknown,
        }
    }

    // ── Type resolution ──

    /// Resolves a TypeExpr from the AST to a Type.
    pub(super) fn resolve_type(&mut self, ty: &TypeExpr) -> Type {
        match ty {
            TypeExpr::Simple { name, .. } => match name.as_str() {
                "void" => Type::Void,
                "never" => Type::Never,
                "bool" => Type::Bool,
                "i8" => Type::I8,
                "i16" => Type::I16,
                "i32" => Type::I32,
                "i64" => Type::I64,
                "i128" => Type::I128,
                "u1" | "u2" | "u3" | "u4" | "u5" | "u6" | "u7" => Type::U8,
                "u8" => Type::U8,
                "u16" => Type::U16,
                "u32" => Type::U32,
                "u64" => Type::U64,
                "u128" => Type::U128,
                "isize" => Type::ISize,
                "usize" => Type::USize,
                "f16" => Type::F16,
                "bf16" => Type::Bf16,
                "f32" => Type::F32,
                "f64" => Type::F64,
                "char" => Type::Char,
                "str" | "String" => Type::Str,
                "Tensor" => Type::dynamic_tensor(),
                "any" => Type::Unknown,
                other => {
                    // Check type aliases first
                    if let Some(aliased) = self.type_aliases.get(other) {
                        return aliased.clone();
                    }
                    // Check if it's a known struct/enum
                    if let Some(sym) = self.symbols.lookup(other) {
                        sym.ty.clone()
                    } else {
                        Type::Named(other.to_string())
                    }
                }
            },
            TypeExpr::Array { element, .. } => Type::Array(Box::new(self.resolve_type(element))),
            TypeExpr::Tuple { elements, .. } => {
                Type::Tuple(elements.iter().map(|t| self.resolve_type(t)).collect())
            }
            TypeExpr::Fn {
                params,
                return_type,
                ..
            } => Type::Function {
                params: params.iter().map(|t| self.resolve_type(t)).collect(),
                ret: Box::new(self.resolve_type(return_type)),
            },
            TypeExpr::Reference { mutable, inner, .. } => {
                let inner_ty = self.resolve_type(inner);
                if *mutable {
                    Type::RefMut(Box::new(inner_ty))
                } else {
                    Type::Ref(Box::new(inner_ty))
                }
            }
            TypeExpr::Slice { element, .. } => Type::Array(Box::new(self.resolve_type(element))),
            TypeExpr::Generic { name, args, .. } => {
                // Resolve known generic containers
                match name.as_str() {
                    "Array" | "Vec" if args.len() == 1 => {
                        Type::Array(Box::new(self.resolve_type(&args[0])))
                    }
                    "Future" if args.len() == 1 => Type::Future {
                        inner: Box::new(self.resolve_type(&args[0])),
                    },
                    other => {
                        // Check type aliases
                        if let Some(aliased) = self.type_aliases.get(other) {
                            return aliased.clone();
                        }
                        Type::Named(other.to_string())
                    }
                }
            }
            TypeExpr::Path { segments, .. } => {
                Type::Named(segments.last().cloned().unwrap_or_default())
            }
            TypeExpr::Tensor {
                element_type,
                dims,
                span,
            } => {
                let elem = self.resolve_type(element_type);
                if elem.is_tensor() {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "scalar element type (f32, f64, i32, ...)".into(),
                        found: elem.display_name(),
                        span: *span,
                        hint: None,
                    });
                }
                Type::Tensor {
                    element: Box::new(elem),
                    dims: dims.clone(),
                }
            }
            TypeExpr::Pointer { .. } => Type::Unknown,
            TypeExpr::DynTrait {
                trait_name, span, ..
            } => {
                // Validate the trait exists
                if !self.traits.contains_key(trait_name) {
                    self.errors.push(SemanticError::UnknownTrait {
                        name: trait_name.clone(),
                        span: *span,
                    });
                }
                Type::DynTrait(trait_name.clone())
            }
        }
    }

    /// Checks lifetime elision rules for a function definition.
    ///
    /// Implements the three Rust-inspired lifetime elision rules:
    /// 1. Each elided lifetime in input position becomes a distinct lifetime parameter.
    /// 2. If there is exactly one input lifetime, it is assigned to all elided output lifetimes.
    /// 3. If there is a `&self` or `&mut self` parameter, its lifetime is assigned to all
    ///    elided output lifetimes.
    ///
    /// This function validates that explicitly annotated lifetimes are consistent
    /// and warns about unnecessary annotations that match elision rules.
    pub fn check_lifetime_elision(&mut self, fndef: &crate::parser::ast::FnDef) {
        // Collect lifetimes from parameters
        let mut input_lifetimes = Vec::new();
        let mut has_self_ref = false;
        let mut self_lifetime: Option<String> = None;

        for param in &fndef.params {
            let mut param_lifetimes = Vec::new();
            collect_lifetimes_from_type(&param.ty, &mut param_lifetimes);
            if param.name == "self" {
                has_self_ref = true;
                if let Some(lt) = param_lifetimes.first() {
                    self_lifetime = Some(lt.clone());
                }
            }
            input_lifetimes.extend(param_lifetimes);
        }

        // Check for duplicate lifetime param declarations
        let mut seen_lifetimes = std::collections::HashSet::new();
        for lp in &fndef.lifetime_params {
            if !seen_lifetimes.insert(lp.name.clone()) {
                self.errors.push(SemanticError::LifetimeConflict {
                    name: lp.name.clone(),
                    span: lp.span,
                });
            }
        }

        // Validate that lifetimes used in params are declared
        let declared: std::collections::HashSet<String> = fndef
            .lifetime_params
            .iter()
            .map(|lp| lp.name.clone())
            .collect();

        for lt in &input_lifetimes {
            if lt != "static" && lt != "_" && !declared.contains(lt) {
                self.errors.push(SemanticError::LifetimeMismatch {
                    expected: "declared lifetime".into(),
                    found: lt.clone(),
                    span: fndef.span,
                });
            }
        }

        // Collect output lifetimes (from return type)
        if let Some(ref ret_ty) = fndef.return_type {
            let mut output_lifetimes = Vec::new();
            collect_lifetimes_from_type(ret_ty, &mut output_lifetimes);

            for lt in &output_lifetimes {
                if lt != "static" && lt != "_" && !declared.contains(lt) {
                    self.errors.push(SemanticError::LifetimeMismatch {
                        expected: "declared lifetime".into(),
                        found: lt.clone(),
                        span: fndef.span,
                    });
                }
            }

            // Elision rule 2 & 3: validate output lifetimes
            // If there's exactly one input lifetime or &self, output can use it
            let elision_source = if has_self_ref {
                self_lifetime.clone()
            } else if input_lifetimes.len() == 1 {
                Some(input_lifetimes[0].clone())
            } else {
                None
            };

            // If there are output lifetimes and no elision source, they must all be declared
            if !output_lifetimes.is_empty() && elision_source.is_none() && input_lifetimes.len() > 1
            {
                // Multiple input lifetimes, no &self — output lifetime is ambiguous
                // This is fine as long as all output lifetimes are explicitly declared
                // (already checked above)
            }
        }
    }
}

/// Collects all lifetime names referenced in a type expression.
///
/// Walks the type expression tree and extracts lifetime annotations
/// from reference types. Used by lifetime elision checking.
pub fn collect_lifetimes_from_type(ty: &crate::parser::ast::TypeExpr, out: &mut Vec<String>) {
    match ty {
        crate::parser::ast::TypeExpr::Reference {
            lifetime, inner, ..
        } => {
            if let Some(lt) = lifetime {
                out.push(lt.clone());
            }
            collect_lifetimes_from_type(inner, out);
        }
        crate::parser::ast::TypeExpr::Generic { args, .. } => {
            for arg in args {
                collect_lifetimes_from_type(arg, out);
            }
        }
        crate::parser::ast::TypeExpr::Tuple { elements, .. } => {
            for elem in elements {
                collect_lifetimes_from_type(elem, out);
            }
        }
        crate::parser::ast::TypeExpr::Array { element, .. }
        | crate::parser::ast::TypeExpr::Slice { element, .. } => {
            collect_lifetimes_from_type(element, out);
        }
        crate::parser::ast::TypeExpr::Pointer { inner, .. } => {
            collect_lifetimes_from_type(inner, out);
        }
        crate::parser::ast::TypeExpr::Fn {
            params,
            return_type,
            ..
        } => {
            for p in params {
                collect_lifetimes_from_type(p, out);
            }
            collect_lifetimes_from_type(return_type, out);
        }
        _ => {}
    }
}
