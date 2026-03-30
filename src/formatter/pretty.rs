//! Pretty printer that walks AST nodes and emits formatted source code.

use crate::lexer::Comment;
use crate::parser::ast::*;

/// AST-based code formatter that emits consistently styled Fajar Lang source.
pub struct Formatter<'src> {
    /// The original source text (for extracting literal text when needed).
    source: &'src str,
    /// Comments collected from the source, sorted by position.
    comments: Vec<Comment>,
    /// Index of next comment to emit.
    comment_idx: usize,
    /// The output buffer.
    output: String,
    /// Current indentation level (each level = 4 spaces).
    indent: usize,
}

const INDENT_WIDTH: usize = 4;

impl<'src> Formatter<'src> {
    /// Creates a new formatter with the given source and comments.
    pub fn new(source: &'src str, comments: Vec<Comment>) -> Self {
        Self {
            source,
            comments,
            comment_idx: 0,
            output: String::new(),
            indent: 0,
        }
    }

    /// Returns the formatted output, ensuring a trailing newline.
    pub fn finish(mut self) -> String {
        // Emit any remaining comments
        self.emit_remaining_comments();
        // Ensure trailing newline
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        // Collapse multiple trailing newlines
        while self.output.ends_with("\n\n\n") {
            self.output.pop();
        }
        self.output
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    fn push(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn push_char(&mut self, c: char) {
        self.output.push(c);
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    fn indent_str(&self) -> String {
        " ".repeat(self.indent * INDENT_WIDTH)
    }

    fn write_indent(&mut self) {
        let indent = self.indent_str();
        self.push(&indent);
    }

    fn emit_comments_before(&mut self, pos: usize) {
        while self.comment_idx < self.comments.len() && self.comments[self.comment_idx].pos < pos {
            let comment = self.comments[self.comment_idx].clone();
            self.comment_idx += 1;
            // Ensure we're on a new line for the comment
            if !self.output.is_empty() && !self.output.ends_with('\n') {
                self.newline();
            }
            self.write_indent();
            self.push(&comment.text);
            self.newline();
        }
    }

    fn emit_remaining_comments(&mut self) {
        while self.comment_idx < self.comments.len() {
            let comment = self.comments[self.comment_idx].clone();
            self.comment_idx += 1;
            if !self.output.is_empty() && !self.output.ends_with('\n') {
                self.newline();
            }
            self.write_indent();
            self.push(&comment.text);
            self.newline();
        }
    }

    // ── Program ─────────────────────────────────────────────────────────

    /// Formats a complete program.
    pub fn format_program(&mut self, program: &Program) {
        for (i, item) in program.items.iter().enumerate() {
            self.emit_comments_before(item_span(item).start);
            self.format_item(item);
            // Blank line between top-level items
            if i + 1 < program.items.len() {
                self.newline();
            }
        }
    }

    // ── Items ───────────────────────────────────────────────────────────

    fn format_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(f) => self.format_fn_def(f),
            Item::StructDef(s) => self.format_struct_def(s),
            Item::EnumDef(e) => self.format_enum_def(e),
            Item::UnionDef(u) => {
                self.write_indent();
                self.push(&format!("union {} {{", u.name));
                self.newline();
                self.indent += 1;
                for field in &u.fields {
                    self.write_indent();
                    self.push(&format!("{}: {},", field.name, field.ty));
                    self.newline();
                }
                self.indent -= 1;
                self.write_indent();
                self.push("}");
                self.newline();
            }
            Item::ImplBlock(i) => self.format_impl_block(i),
            Item::TraitDef(t) => self.format_trait_def(t),
            Item::ConstDef(c) => self.format_const_def(c),
            Item::ServiceDef(_svc) => { /* service: formatted as module */ }
            Item::StaticDef(s) => {
                self.write_indent();
                let mut decl = String::from("static ");
                if s.is_mut {
                    decl.push_str("mut ");
                }
                decl.push_str(&s.name);
                decl.push_str(": ");
                decl.push_str(&format!("{:?}", s.ty));
                self.push(&decl);
            }
            Item::UseDecl(u) => self.format_use_decl(u),
            Item::ModDecl(m) => self.format_mod_decl(m),
            Item::ExternFn(efn) => self.format_extern_fn(efn),
            Item::TypeAlias(ta) => self.format_type_alias(ta),
            Item::GlobalAsm(ga) => {
                self.write_indent();
                self.push(&format!("global_asm!(\"{}\")", ga.template));
                self.newline();
            }
            Item::EffectDecl(_) => {
                // Effect declarations: formatting not yet implemented
            }
            Item::MacroRulesDef(m) => {
                self.write_indent();
                self.push(&format!("macro_rules! {} {{ ... }}", m.name));
                self.newline();
            }
            Item::Stmt(s) => self.format_stmt(s),
        }
    }

    fn format_annotation(&mut self, ann: &Annotation) {
        self.write_indent();
        self.push("@");
        self.push(&ann.name);
        self.newline();
    }

    fn format_fn_def(&mut self, f: &FnDef) {
        if let Some(ann) = &f.annotation {
            self.format_annotation(ann);
        }
        self.write_indent();
        self.push("fn ");
        self.push(&f.name);
        self.format_generic_params_with_lifetimes(&f.lifetime_params, &f.generic_params);
        self.push_char('(');
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            self.push(&p.name);
            self.push(": ");
            self.format_type_expr(&p.ty);
        }
        self.push_char(')');
        if let Some(ret) = &f.return_type {
            self.push(" -> ");
            self.format_type_expr(ret);
        }
        self.push(" ");
        self.format_block_body(&f.body);
        self.newline();
    }

    fn format_type_alias(&mut self, ta: &TypeAlias) {
        self.write_indent();
        self.push("type ");
        self.push(&ta.name);
        self.push(" = ");
        self.format_type_expr(&ta.ty);
        self.newline();
    }

    fn format_extern_fn(&mut self, efn: &ExternFn) {
        if let Some(ann) = &efn.annotation {
            self.format_annotation(ann);
        }
        self.write_indent();
        if let Some(abi) = &efn.abi {
            self.push(&format!("extern(\"{abi}\") fn "));
        } else {
            self.push("extern fn ");
        }
        self.push(&efn.name);
        self.push_char('(');
        for (i, p) in efn.params.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            self.push(&p.name);
            self.push(": ");
            self.format_type_expr(&p.ty);
        }
        self.push_char(')');
        if let Some(ret) = &efn.return_type {
            self.push(" -> ");
            self.format_type_expr(ret);
        }
        self.newline();
    }

    fn format_struct_def(&mut self, s: &StructDef) {
        if let Some(ann) = &s.annotation {
            self.format_annotation(ann);
        }
        self.write_indent();
        self.push("struct ");
        self.push(&s.name);
        self.format_generic_params_with_lifetimes(&s.lifetime_params, &s.generic_params);
        self.push(" {");
        self.newline();
        self.indent += 1;
        for field in &s.fields {
            self.emit_comments_before(field.span.start);
            self.write_indent();
            self.push(&field.name);
            self.push(": ");
            self.format_type_expr(&field.ty);
            self.push(",");
            self.newline();
        }
        self.indent -= 1;
        self.write_indent();
        self.push("}");
        self.newline();
    }

    fn format_enum_def(&mut self, e: &EnumDef) {
        if let Some(ann) = &e.annotation {
            self.format_annotation(ann);
        }
        self.write_indent();
        self.push("enum ");
        self.push(&e.name);
        self.format_generic_params_with_lifetimes(&e.lifetime_params, &e.generic_params);
        self.push(" {");
        self.newline();
        self.indent += 1;
        for variant in &e.variants {
            self.emit_comments_before(variant.span.start);
            self.write_indent();
            self.push(&variant.name);
            if !variant.fields.is_empty() {
                self.push_char('(');
                for (i, ty) in variant.fields.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.format_type_expr(ty);
                }
                self.push_char(')');
            }
            self.push(",");
            self.newline();
        }
        self.indent -= 1;
        self.write_indent();
        self.push("}");
        self.newline();
    }

    fn format_impl_block(&mut self, imp: &ImplBlock) {
        self.write_indent();
        self.push("impl");
        self.format_generic_params_with_lifetimes(&imp.lifetime_params, &imp.generic_params);
        if let Some(trait_name) = &imp.trait_name {
            self.push(" ");
            self.push(trait_name);
            self.push(" for");
        }
        self.push(" ");
        self.push(&imp.target_type);
        self.push(" {");
        self.newline();
        self.indent += 1;
        for (i, method) in imp.methods.iter().enumerate() {
            self.emit_comments_before(method.span.start);
            self.format_fn_def(method);
            if i + 1 < imp.methods.len() {
                self.newline();
            }
        }
        self.indent -= 1;
        self.write_indent();
        self.push("}");
        self.newline();
    }

    fn format_trait_def(&mut self, t: &TraitDef) {
        self.write_indent();
        self.push("trait ");
        self.push(&t.name);
        self.format_generic_params_with_lifetimes(&t.lifetime_params, &t.generic_params);
        self.push(" {");
        self.newline();
        self.indent += 1;
        for method in &t.methods {
            self.emit_comments_before(method.span.start);
            self.format_fn_def(method);
        }
        self.indent -= 1;
        self.write_indent();
        self.push("}");
        self.newline();
    }

    fn format_const_def(&mut self, c: &ConstDef) {
        if let Some(ann) = &c.annotation {
            self.format_annotation(ann);
        }
        self.write_indent();
        self.push("const ");
        self.push(&c.name);
        self.push(": ");
        self.format_type_expr(&c.ty);
        self.push(" = ");
        self.format_expr(&c.value);
        self.newline();
    }

    fn format_use_decl(&mut self, u: &UseDecl) {
        self.write_indent();
        self.push("use ");
        let path_str = u.path.join("::");
        self.push(&path_str);
        match &u.kind {
            UseKind::Simple => {}
            UseKind::Glob => self.push("::*"),
            UseKind::Group(names) => {
                self.push("::{");
                for (i, name) in names.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.push(name);
                }
                self.push("}");
            }
        }
        self.newline();
    }

    fn format_mod_decl(&mut self, m: &ModDecl) {
        self.write_indent();
        self.push("mod ");
        self.push(&m.name);
        if let Some(body) = &m.body {
            self.push(" {");
            self.newline();
            self.indent += 1;
            for (i, item) in body.iter().enumerate() {
                self.emit_comments_before(item_span(item).start);
                self.format_item(item);
                if i + 1 < body.len() {
                    self.newline();
                }
            }
            self.indent -= 1;
            self.write_indent();
            self.push("}");
        }
        self.newline();
    }

    fn format_generic_params_with_lifetimes(
        &mut self,
        lifetime_params: &[LifetimeParam],
        params: &[GenericParam],
    ) {
        if lifetime_params.is_empty() && params.is_empty() {
            return;
        }
        self.push_char('<');
        let mut first = true;
        for lp in lifetime_params {
            if !first {
                self.push(", ");
            }
            first = false;
            self.push("'");
            self.push(&lp.name);
        }
        for p in params {
            if !first {
                self.push(", ");
            }
            first = false;
            self.push(&p.name);
            if !p.bounds.is_empty() {
                self.push(": ");
                for (j, b) in p.bounds.iter().enumerate() {
                    if j > 0 {
                        self.push(" + ");
                    }
                    self.push(&b.name);
                    if !b.type_args.is_empty() {
                        self.push_char('<');
                        for (k, ta) in b.type_args.iter().enumerate() {
                            if k > 0 {
                                self.push(", ");
                            }
                            self.format_type_expr(ta);
                        }
                        self.push_char('>');
                    }
                }
            }
        }
        self.push_char('>');
    }

    // ── Statements ──────────────────────────────────────────────────────

    fn format_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                mutable,
                name,
                ty,
                value,
                ..
            } => {
                self.write_indent();
                self.push("let ");
                if *mutable {
                    self.push("mut ");
                }
                self.push(name);
                if let Some(ty) = ty {
                    self.push(": ");
                    self.format_type_expr(ty);
                }
                self.push(" = ");
                self.format_expr(value);
                self.newline();
            }
            Stmt::Const {
                name, ty, value, ..
            } => {
                self.write_indent();
                self.push("const ");
                self.push(name);
                self.push(": ");
                self.format_type_expr(ty);
                self.push(" = ");
                self.format_expr(value);
                self.newline();
            }
            Stmt::Expr { expr, .. } => {
                self.write_indent();
                self.format_expr(expr);
                self.newline();
            }
            Stmt::Return { value, .. } => {
                self.write_indent();
                self.push("return");
                if let Some(val) = value {
                    self.push(" ");
                    self.format_expr(val);
                }
                self.newline();
            }
            Stmt::Break { value, .. } => {
                self.write_indent();
                self.push("break");
                if let Some(val) = value {
                    self.push(" ");
                    self.format_expr(val);
                }
                self.newline();
            }
            Stmt::Continue { .. } => {
                self.write_indent();
                self.push("continue");
                self.newline();
            }
            Stmt::Item(item) => self.format_item(item),
        }
    }

    // ── Expressions ─────────────────────────────────────────────────────

    fn format_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal { kind, span, .. } => self.format_literal(kind, *span),
            Expr::Ident { name, .. } => self.push(name),
            Expr::Binary {
                left, op, right, ..
            } => {
                self.format_expr(left);
                self.push(" ");
                self.push(&op.to_string());
                self.push(" ");
                self.format_expr(right);
            }
            Expr::Unary { op, operand, .. } => {
                self.push(&op.to_string());
                // Space after &mut
                if matches!(op, UnaryOp::RefMut) {
                    self.push(" ");
                }
                self.format_expr(operand);
            }
            Expr::Call { callee, args, .. } => {
                self.format_expr(callee);
                self.push_char('(');
                self.format_call_args(args);
                self.push_char(')');
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                self.format_expr(receiver);
                self.push(".");
                self.push(method);
                self.push_char('(');
                self.format_call_args(args);
                self.push_char(')');
            }
            Expr::Field { object, field, .. } => {
                self.format_expr(object);
                self.push(".");
                self.push(field);
            }
            Expr::Index { object, index, .. } => {
                self.format_expr(object);
                self.push_char('[');
                self.format_expr(index);
                self.push_char(']');
            }
            Expr::Block { stmts, expr, .. } => {
                self.format_block(stmts, expr.as_deref());
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.push("if ");
                self.format_expr(condition);
                self.push(" ");
                self.format_block_body(then_branch);
                if let Some(else_b) = else_branch {
                    self.push(" else ");
                    if matches!(else_b.as_ref(), Expr::If { .. }) {
                        self.format_expr(else_b);
                    } else {
                        self.format_block_body(else_b);
                    }
                }
            }
            Expr::Match { subject, arms, .. } => {
                self.push("match ");
                self.format_expr(subject);
                self.push(" {");
                self.newline();
                self.indent += 1;
                for arm in arms {
                    self.emit_comments_before(arm.span.start);
                    self.write_indent();
                    self.format_pattern(&arm.pattern);
                    if let Some(guard) = &arm.guard {
                        self.push(" if ");
                        self.format_expr(guard);
                    }
                    self.push(" => ");
                    self.format_expr(&arm.body);
                    self.push(",");
                    self.newline();
                }
                self.indent -= 1;
                self.write_indent();
                self.push("}");
            }
            Expr::While {
                label: _,
                condition,
                body,
                ..
            } => {
                self.push("while ");
                self.format_expr(condition);
                self.push(" ");
                self.format_block_body(body);
            }
            Expr::For {
                label: _,
                variable,
                iterable,
                body,
                ..
            } => {
                self.push("for ");
                self.push(variable);
                self.push(" in ");
                self.format_expr(iterable);
                self.push(" ");
                self.format_block_body(body);
            }
            Expr::Loop { label: _, body, .. } => {
                self.push("loop ");
                self.format_block_body(body);
            }
            Expr::Assign {
                target, op, value, ..
            } => {
                self.format_expr(target);
                self.push(" ");
                self.push(&op.to_string());
                self.push(" ");
                self.format_expr(value);
            }
            Expr::Pipe { left, right, .. } => {
                self.format_expr(left);
                self.push(" |> ");
                self.format_expr(right);
            }
            Expr::Array { elements, .. } => {
                self.push_char('[');
                for (i, el) in elements.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.format_expr(el);
                }
                self.push_char(']');
            }
            Expr::ArrayRepeat { value, count, .. } => {
                self.push_char('[');
                self.format_expr(value);
                self.push("; ");
                self.format_expr(count);
                self.push_char(']');
            }
            Expr::Tuple { elements, .. } => {
                self.push_char('(');
                for (i, el) in elements.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.format_expr(el);
                }
                self.push_char(')');
            }
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                if let Some(s) = start {
                    self.format_expr(s);
                }
                if *inclusive {
                    self.push("..=");
                } else {
                    self.push("..");
                }
                if let Some(e) = end {
                    self.format_expr(e);
                }
            }
            Expr::Cast { expr, ty, .. } => {
                self.format_expr(expr);
                self.push(" as ");
                self.format_type_expr(ty);
            }
            Expr::Try { expr, .. } => {
                self.format_expr(expr);
                self.push_char('?');
            }
            Expr::Await { expr, .. } => {
                self.format_expr(expr);
                self.push(".await");
            }
            Expr::AsyncBlock { body, .. } => {
                self.push("async ");
                self.format_expr(body);
            }
            Expr::InlineAsm { template, .. } => {
                self.push("asm!(\"");
                self.push(template);
                self.push("\")");
            }
            Expr::FString { parts, .. } => {
                self.push("f\"");
                for part in parts {
                    match part {
                        FStringExprPart::Literal(s) => self.push(s),
                        FStringExprPart::Expr(expr) => {
                            self.push_char('{');
                            self.format_expr(expr);
                            self.push_char('}');
                        }
                    }
                }
                self.push_char('"');
            }
            Expr::Closure {
                params,
                return_type,
                body,
                ..
            } => {
                self.push_char('|');
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.push(&p.name);
                    if let Some(ty) = &p.ty {
                        self.push(": ");
                        self.format_type_expr(ty);
                    }
                }
                self.push_char('|');
                if let Some(ret) = return_type {
                    self.push(" -> ");
                    self.format_type_expr(ret);
                }
                self.push(" ");
                self.format_expr(body);
            }
            Expr::StructInit { name, fields, .. } => {
                self.push(name);
                self.push(" { ");
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.push(&f.name);
                    self.push(": ");
                    self.format_expr(&f.value);
                }
                self.push(" }");
            }
            Expr::Grouped { expr, .. } => {
                self.push_char('(');
                self.format_expr(expr);
                self.push_char(')');
            }
            Expr::Path { segments, .. } => {
                self.push(&segments.join("::"));
            }
            Expr::HandleEffect { .. } => {
                // Effect handle expression: formatting not yet implemented
                self.push("handle { ... }");
            }
            Expr::ResumeExpr { .. } => {
                self.push("resume");
            }
            Expr::Comptime { body, .. } => {
                self.push("comptime ");
                self.format_expr(body);
            }
            Expr::MacroInvocation { name, args, .. } => {
                self.push(&format!("{}!(", name));
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.format_expr(arg);
                }
                self.push(")");
            }
            Expr::Yield { value, .. } => {
                self.push("yield");
                if let Some(v) = value {
                    self.push(" ");
                    self.format_expr(v);
                }
            }
        }
    }

    fn format_literal(&mut self, kind: &LiteralKind, span: crate::lexer::token::Span) {
        match kind {
            LiteralKind::Int(n) => {
                // Try to preserve original formatting (hex, bin, etc.) from source
                let original = &self.source[span.start..span.end];
                if original.starts_with("0x")
                    || original.starts_with("0X")
                    || original.starts_with("0b")
                    || original.starts_with("0B")
                    || original.starts_with("0o")
                    || original.starts_with("0O")
                {
                    self.push(original);
                } else {
                    self.push(&n.to_string());
                }
            }
            LiteralKind::Float(f) => {
                let s = format!("{f}");
                // Ensure float always has decimal point
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    self.push(&s);
                } else {
                    self.push(&s);
                    self.push(".0");
                }
            }
            LiteralKind::String(s) => {
                self.push("\"");
                self.push(&escape_string(s));
                self.push("\"");
            }
            LiteralKind::RawString(s) => {
                self.push("r\"");
                self.push(s);
                self.push("\"");
            }
            LiteralKind::Char(c) => {
                self.push("'");
                self.push(&escape_char(*c));
                self.push("'");
            }
            LiteralKind::Bool(b) => {
                self.push(if *b { "true" } else { "false" });
            }
            LiteralKind::Null => self.push("null"),
        }
    }

    fn format_call_args(&mut self, args: &[CallArg]) {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            if let Some(name) = &arg.name {
                self.push(name);
                self.push(": ");
            }
            self.format_expr(&arg.value);
        }
    }

    /// Formats a block expression: `{ stmts; expr }`.
    fn format_block(&mut self, stmts: &[Stmt], final_expr: Option<&Expr>) {
        self.push("{");
        if stmts.is_empty() && final_expr.is_none() {
            self.push("}");
            return;
        }
        self.newline();
        self.indent += 1;
        for stmt in stmts {
            self.emit_comments_before(stmt_span(stmt).start);
            self.format_stmt(stmt);
        }
        if let Some(expr) = final_expr {
            self.emit_comments_before(expr.span().start);
            self.write_indent();
            self.format_expr(expr);
            self.newline();
        }
        self.indent -= 1;
        self.write_indent();
        self.push("}");
    }

    /// Formats an expression that should be a block body (for fn, if, while, etc.).
    fn format_block_body(&mut self, expr: &Expr) {
        match expr {
            Expr::Block { stmts, expr, .. } => {
                self.format_block(stmts, expr.as_deref());
            }
            _ => {
                // Wrap non-block in braces
                self.push("{ ");
                self.format_expr(expr);
                self.push(" }");
            }
        }
    }

    // ── Types ───────────────────────────────────────────────────────────

    fn format_type_expr(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Simple { name, .. } => self.push(name),
            TypeExpr::Generic { name, args, .. } => {
                self.push(name);
                self.push_char('<');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.format_type_expr(a);
                }
                self.push_char('>');
            }
            TypeExpr::Tensor {
                element_type, dims, ..
            } => {
                self.push("Tensor<");
                self.format_type_expr(element_type);
                self.push(">[");
                for (i, d) in dims.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    match d {
                        Some(n) => self.push(&n.to_string()),
                        None => self.push("*"),
                    }
                }
                self.push("]");
            }
            TypeExpr::Pointer { mutable, inner, .. } => {
                if *mutable {
                    self.push("*mut ");
                } else {
                    self.push("*const ");
                }
                self.format_type_expr(inner);
            }
            TypeExpr::Reference {
                lifetime,
                mutable,
                inner,
                ..
            } => {
                self.push("&");
                if let Some(lt) = lifetime {
                    self.push("'");
                    self.push(lt);
                    self.push(" ");
                }
                if *mutable {
                    self.push("mut ");
                }
                self.format_type_expr(inner);
            }
            TypeExpr::Tuple { elements, .. } => {
                self.push_char('(');
                for (i, e) in elements.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.format_type_expr(e);
                }
                self.push_char(')');
            }
            TypeExpr::Array { element, size, .. } => {
                self.push("[");
                self.format_type_expr(element);
                self.push("; ");
                self.push(&size.to_string());
                self.push("]");
            }
            TypeExpr::Slice { element, .. } => {
                self.push("[");
                self.format_type_expr(element);
                self.push("]");
            }
            TypeExpr::Fn {
                params,
                return_type,
                ..
            } => {
                self.push("fn(");
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.format_type_expr(p);
                }
                self.push(") -> ");
                self.format_type_expr(return_type);
            }
            TypeExpr::Path { segments, .. } => {
                self.push(&segments.join("::"));
            }
            TypeExpr::DynTrait { trait_name, .. } => {
                self.push(&format!("dyn {trait_name}"));
            }
        }
    }

    // ── Patterns ────────────────────────────────────────────────────────

    fn format_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Literal { kind, .. } => match kind {
                LiteralKind::Int(n) => self.push(&n.to_string()),
                LiteralKind::Float(f) => self.push(&f.to_string()),
                LiteralKind::String(s) => {
                    self.push("\"");
                    self.push(&escape_string(s));
                    self.push("\"");
                }
                LiteralKind::Char(c) => {
                    self.push("'");
                    self.push(&escape_char(*c));
                    self.push("'");
                }
                LiteralKind::Bool(b) => self.push(if *b { "true" } else { "false" }),
                LiteralKind::Null => self.push("null"),
                LiteralKind::RawString(s) => {
                    self.push("r\"");
                    self.push(s);
                    self.push("\"");
                }
            },
            Pattern::Ident { name, .. } => self.push(name),
            Pattern::Wildcard { .. } => self.push("_"),
            Pattern::Tuple { elements, .. } => {
                self.push_char('(');
                for (i, p) in elements.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.format_pattern(p);
                }
                self.push_char(')');
            }
            Pattern::Struct { name, fields, .. } => {
                self.push(name);
                self.push(" { ");
                for (i, fp) in fields.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.push(&fp.name);
                    if let Some(pat) = &fp.pattern {
                        self.push(": ");
                        self.format_pattern(pat);
                    }
                }
                self.push(" }");
            }
            Pattern::Enum {
                enum_name,
                variant,
                fields,
                ..
            } => {
                if !enum_name.is_empty() {
                    self.push(enum_name);
                    self.push("::");
                }
                self.push(variant);
                if !fields.is_empty() {
                    self.push_char('(');
                    for (i, p) in fields.iter().enumerate() {
                        if i > 0 {
                            self.push(", ");
                        }
                        self.format_pattern(p);
                    }
                    self.push_char(')');
                }
            }
            Pattern::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                self.format_expr(start);
                if *inclusive {
                    self.push("..=");
                } else {
                    self.push("..");
                }
                self.format_expr(end);
            }
            Pattern::Or { patterns, .. } => {
                for (i, p) in patterns.iter().enumerate() {
                    if i > 0 {
                        self.push(" | ");
                    }
                    self.format_pattern(p);
                }
            }
        }
    }
}

// ── Utilities ───────────────────────────────────────────────────────────

/// Gets the span of an item for comment positioning.
fn item_span(item: &Item) -> crate::lexer::token::Span {
    match item {
        Item::FnDef(f) => f.span,
        Item::StructDef(s) => s.span,
        Item::EnumDef(e) => e.span,
        Item::UnionDef(u) => u.span,
        Item::ImplBlock(i) => i.span,
        Item::TraitDef(t) => t.span,
        Item::ConstDef(c) => c.span,
        Item::StaticDef(s) => s.span,
        Item::ServiceDef(svc) => svc.span,
        Item::UseDecl(u) => u.span,
        Item::ModDecl(m) => m.span,
        Item::ExternFn(e) => e.span,
        Item::TypeAlias(ta) => ta.span,
        Item::GlobalAsm(ga) => ga.span,
        Item::EffectDecl(ed) => ed.span,
        Item::MacroRulesDef(m) => m.span,
        Item::Stmt(s) => stmt_span(s),
    }
}

/// Gets the span of a statement for comment positioning.
fn stmt_span(stmt: &Stmt) -> crate::lexer::token::Span {
    match stmt {
        Stmt::Let { span, .. }
        | Stmt::Const { span, .. }
        | Stmt::Expr { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::Break { span, .. }
        | Stmt::Continue { span, .. } => *span,
        Stmt::Item(item) => item_span(item),
    }
}

/// Escapes a string for output (restores escape sequences).
fn escape_string(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\0' => out.push_str("\\0"),
            c => out.push(c),
        }
    }
    out
}

/// Escapes a character for char literal output.
fn escape_char(c: char) -> String {
    match c {
        '\n' => "\\n".to_string(),
        '\t' => "\\t".to_string(),
        '\r' => "\\r".to_string(),
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        '\0' => "\\0".to_string(),
        c => c.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::formatter::format;

    #[test]
    fn format_simple_function() {
        let src = "fn   add(  a:i32,b:i32 )->i32{a+b}";
        let result = format(src).unwrap();
        assert_eq!(result, "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n");
    }

    #[test]
    fn format_let_statement() {
        let src = "let   x:i32   =   42";
        let result = format(src).unwrap();
        assert_eq!(result, "let x: i32 = 42\n");
    }

    #[test]
    fn format_let_mut_statement() {
        let src = "let  mut  counter  =  0";
        let result = format(src).unwrap();
        assert_eq!(result, "let mut counter = 0\n");
    }

    #[test]
    fn format_struct_def() {
        let src = "struct Point{x:f64,y:f64}";
        let result = format(src).unwrap();
        assert!(result.contains("struct Point {"));
        assert!(result.contains("    x: f64,"));
        assert!(result.contains("    y: f64,"));
        assert!(result.contains("}"));
    }

    #[test]
    fn format_enum_def() {
        let src = "enum Shape{Circle(f64),Rect(f64,f64)}";
        let result = format(src).unwrap();
        assert!(result.contains("enum Shape {"));
        assert!(result.contains("    Circle(f64),"));
        assert!(result.contains("    Rect(f64, f64),"));
    }

    #[test]
    fn format_if_else() {
        let src = "if   a>b  {a}  else  {b}";
        let result = format(src).unwrap();
        assert_eq!(result, "if a > b {\n    a\n} else {\n    b\n}\n");
    }

    #[test]
    fn format_while_loop() {
        let src = "while  x<10{x=x+1}";
        let result = format(src).unwrap();
        assert!(result.contains("while x < 10 {"));
    }

    #[test]
    fn format_for_loop() {
        let src = "for  i  in  0..10{println(i)}";
        let result = format(src).unwrap();
        assert!(result.contains("for i in 0..10 {"));
    }

    #[test]
    fn format_preserves_comments() {
        let src = "// hello world\nlet x = 42";
        let result = format(src).unwrap();
        assert!(result.contains("// hello world"));
        assert!(result.contains("let x = 42"));
    }

    #[test]
    fn format_idempotent() {
        let src = "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
        let first = format(src).unwrap();
        let second = format(&first).unwrap();
        assert_eq!(first, second, "formatter must be idempotent");
    }

    #[test]
    fn format_trailing_newline() {
        let src = "let x = 1";
        let result = format(src).unwrap();
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn format_match_expression() {
        let src = "match x{0=>\"zero\",_=>\"other\"}";
        let result = format(src).unwrap();
        assert!(result.contains("match x {"));
        assert!(result.contains("    0 => \"zero\","));
        assert!(result.contains("    _ => \"other\","));
    }

    #[test]
    fn format_array_literal() {
        let src = "let  arr  =  [1,2,3]";
        let result = format(src).unwrap();
        assert_eq!(result, "let arr = [1, 2, 3]\n");
    }

    #[test]
    fn format_pipeline() {
        let src = "5|>double|>add_one";
        let result = format(src).unwrap();
        assert_eq!(result, "5 |> double |> add_one\n");
    }

    #[test]
    fn format_block_comment() {
        let src = "/* comment */\nlet x = 1";
        let result = format(src).unwrap();
        assert!(result.contains("/* comment */"));
    }

    #[test]
    fn format_impl_block() {
        let src = "impl Point{fn new(x:f64,y:f64)->Point{Point{x:x,y:y}}}";
        let result = format(src).unwrap();
        assert!(result.contains("impl Point {"));
        assert!(result.contains("    fn new(x: f64, y: f64) -> Point {"));
    }

    #[test]
    fn format_const_def() {
        let src = "const  MAX:i32=100";
        let result = format(src).unwrap();
        assert_eq!(result, "const MAX: i32 = 100\n");
    }

    #[test]
    fn format_cast_expression() {
        let src = "let x = 42 as f64";
        let result = format(src).unwrap();
        assert!(result.contains("42 as f64"));
    }

    #[test]
    fn format_loop_expression() {
        let src = "loop{break 42}";
        let result = format(src).unwrap();
        assert!(result.contains("loop {"));
        assert!(result.contains("break 42"));
    }
}
