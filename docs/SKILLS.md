# Skills — Fajar Lang Implementation Patterns

> Panduan implementasi untuk komponen compiler dan interpreter. Gunakan sebagai referensi saat coding.

## 1. Lexer Skills

### 1.1 Cursor Pattern

```rust
struct Cursor<'src> {
    source: &'src str,
    pos: usize,        // byte offset
    line: u32,
    col: u32,
}

impl<'src> Cursor<'src> {
    fn peek(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        if ch == '\n' { self.line += 1; self.col = 1; }
        else { self.col += 1; }
        Some(ch)
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn remaining(&self) -> &'src str {
        &self.source[self.pos..]
    }
}
```

### 1.2 Number Scanning

```rust
fn scan_number(&mut self) -> Result<TokenKind, LexError> {
    let start = self.pos;

    // Check prefix: 0x, 0b, 0o
    if self.peek() == Some('0') {
        self.advance();
        match self.peek() {
            Some('x') | Some('X') => { self.advance(); return self.scan_hex(); }
            Some('b') | Some('B') => { self.advance(); return self.scan_binary(); }
            Some('o') | Some('O') => { self.advance(); return self.scan_octal(); }
            _ => {}
        }
    }

    // Decimal digits
    self.skip_digits();

    // Float: check for dot followed by digit
    if self.peek() == Some('.') && self.peek_next().map_or(false, |c| c.is_ascii_digit()) {
        self.advance(); // consume '.'
        self.skip_digits();
        // Scientific notation
        if matches!(self.peek(), Some('e') | Some('E')) {
            self.advance();
            if matches!(self.peek(), Some('+') | Some('-')) { self.advance(); }
            self.skip_digits();
        }
        let text = &self.source[start..self.pos];
        return Ok(TokenKind::FloatLit(text.parse().map_err(|_| LexError::InvalidNumber { span: self.span(start) })?));
    }

    let text = &self.source[start..self.pos];
    Ok(TokenKind::IntLit(text.replace('_', "").parse().map_err(|_| LexError::InvalidNumber { span: self.span(start) })?))
}
```

### 1.3 Error Recovery in Lexer

```rust
fn tokenize(source: &str) -> Result<Vec<Token>, Vec<LexError>> {
    let mut tokens = Vec::new();
    let mut errors = Vec::new();

    while !cursor.is_eof() {
        match cursor.scan_token() {
            Ok(token) => tokens.push(token),
            Err(error) => {
                errors.push(error);
                cursor.advance(); // skip problematic character
            }
        }
    }

    tokens.push(Token::eof(cursor.pos, cursor.line, cursor.col));

    if errors.is_empty() { Ok(tokens) } else { Err(errors) }
}
```

## 2. Parser Skills

### 2.1 Pratt Parser (Expression Parsing)

```rust
fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
    // Prefix (unary or primary)
    let mut lhs = match self.peek_kind() {
        TokenKind::IntLit(_) | TokenKind::FloatLit(_) => self.parse_literal()?,
        TokenKind::Ident(_) => self.parse_ident_or_call()?,
        TokenKind::LParen => self.parse_grouped()?,
        TokenKind::Minus | TokenKind::Bang => self.parse_unary()?,
        _ => return Err(ParseError::expected_expression(self.peek_span())),
    };

    // Infix (binary operators)
    loop {
        let op = match self.peek_kind() {
            TokenKind::Plus => BinOp::Add,
            TokenKind::Minus => BinOp::Sub,
            TokenKind::Star => BinOp::Mul,
            TokenKind::Slash => BinOp::Div,
            TokenKind::At => BinOp::MatMul,
            TokenKind::PipeArrow => BinOp::Pipe,
            _ => break,
        };

        let (l_bp, r_bp) = op.binding_power();
        if l_bp < min_bp { break; }

        self.advance(); // consume operator
        let rhs = self.parse_expr(r_bp)?;
        let span = lhs.span().merge(rhs.span());
        lhs = Expr::Binary(Box::new(lhs), op, Box::new(rhs), span);
    }

    Ok(lhs)
}
```

### 2.2 Binding Power Table

```rust
impl BinOp {
    fn binding_power(&self) -> (u8, u8) {
        match self {
            BinOp::Pipe      => (2, 3),    // |> left assoc
            BinOp::Or        => (4, 5),    // || left assoc
            BinOp::And       => (6, 7),    // && left assoc
            BinOp::Eq | BinOp::Ne => (8, 9),
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => (10, 11),
            BinOp::Add | BinOp::Sub => (12, 13),
            BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::MatMul => (14, 15),
            BinOp::Pow       => (17, 16),  // ** right assoc
        }
    }
}
```

### 2.3 Error Recovery in Parser

```rust
fn synchronize(&mut self) {
    while !self.is_eof() {
        match self.peek_kind() {
            // Statement boundaries — safe to resume
            TokenKind::Fn | TokenKind::Let | TokenKind::If |
            TokenKind::While | TokenKind::For | TokenKind::Return => return,
            _ => { self.advance(); }
        }
    }
}
```

## 3. Type System Skills

### 3.1 Hindley-Milner Type Inference (Lite)

```
Algorithm W (simplified):
1. For each expression, generate type constraints
2. Solve constraints via unification
3. Apply substitution to get final types

Example:
    let x = 42        → x: ?T, constraint: ?T = i64
    let y = x + 1.0   → constraint: ?T = f64, conflict!
```

### 3.2 Tensor Shape Tracking

```fajar
// Shape-typed tensors
tensor a: f32[3, 4]
tensor b: f32[4, 5]
let c = a @ b     // Shape: [3, 5] — verifiable at compile time

tensor d: f32[3, 3]
let e = a @ d     // ERROR: shape mismatch [3,4] @ [3,3]
```

## 4. Interpreter Skills

### 4.1 Environment (Scope Chain)

```rust
// Linked list of scopes
struct Environment {
    bindings: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    fn lookup(&self, name: &str) -> Option<Value> {
        if let Some(val) = self.bindings.get(name) {
            Some(val.clone())
        } else if let Some(parent) = &self.parent {
            parent.borrow().lookup(name)
        } else {
            None
        }
    }
}
```

### 4.2 Function Call Pattern

```rust
fn call_function(&mut self, func: &FnValue, args: Vec<Value>) -> Result<Value, RuntimeError> {
    // Create new scope with function's closure environment as parent
    let call_env = Environment::new(Some(func.closure_env.clone()));

    // Bind parameters
    for (param, arg) in func.params.iter().zip(args) {
        call_env.borrow_mut().define(param.name.clone(), arg);
    }

    // Save current env, switch to call env
    let prev_env = std::mem::replace(&mut self.env, call_env);
    let result = self.eval_block(&func.body);
    self.env = prev_env;

    result
}
```

## 5. Prompt Patterns for Claude Code

### For Architecture Decisions

```
"Consider the trade-offs between [option A] and [option B] for [component].
Key constraints: [list constraints].
Think through: correctness, performance, maintainability, and Rust idioms.
Recommend the better approach with reasoning."
```

### For Implementation

```
"Implement [function/module] following these contracts: [ARCHITECTURE.md excerpt].
Requirements: [from TASKS.md].
Rules: no panics, use thiserror, doc comments on all pub items.
Write tests first, then implementation."
```

### For Debugging

```
"This test is failing: [test code + output].
The relevant implementation: [code].
Step through the logic and identify the bug.
Provide the fix and explain why it's correct."
```

### For Code Review

```
"Review this implementation against RULES.md.
Check: error handling, safety, tests, documentation, performance.
List all violations and suggested fixes."
```

---

*Skills Version: 1.0 | Domain: Compilers + OS + ML*
