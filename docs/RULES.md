# Rules — Fajar Lang Coding Conventions

> Aturan ini berlaku untuk semua kode di project ini. Tidak ada pengecualian tanpa diskusi eksplisit.

## 1. Core Principles

1. **CORRECTNESS** first, then performance, then elegance
2. **EXPLICIT** over implicit — no hidden behavior
3. **ERRORS** are values — never panic in library code
4. **TESTS** before implementation — TDD always
5. **SMALL** functions — max 50 lines per function
6. **ONE** concern per module — strict single responsibility

## 2. Rust Code Style

### 2.1 Naming Conventions

```rust
// Types, Traits, Enums: PascalCase
pub struct TokenKind { ... }
pub trait GradFn { ... }
pub enum FjError { ... }

// Functions, variables, modules: snake_case
fn tokenize(source: &str) -> ... { }
let token_count = tokens.len();
mod lexer { }

// Constants, statics: SCREAMING_SNAKE_CASE
const MAX_RECURSION_DEPTH: usize = 1024;
static KEYWORD_MAP: &[(&str, TokenKind)] = &[ ... ];

// Lifetime parameters: single lowercase letters or short words
fn parse<'src>(tokens: &'src [Token]) -> ...
fn with_context<'a, 'ctx>(ctx: &'ctx Context, value: &'a Value) -> ...

// Type parameters: PascalCase, descriptive
fn map<T, U>(f: impl Fn(T) -> U, value: T) -> U
```

### 2.2 Error Handling Rules

```rust
// RULE: Never use .unwrap() in library code (src/)
// ALLOWED: .unwrap() in tests only
// ALLOWED: .expect("reason") with meaningful message in main.rs only

// BAD
let token = tokens.get(pos).unwrap();

// GOOD
let token = tokens.get(pos).ok_or(ParseError::unexpected_eof(pos))?;

// RULE: Use thiserror for all error types
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LexError {
    #[error("unexpected character '{ch}' at line {line}, col {col}")]
    UnexpectedChar { ch: char, line: u32, col: u32 },

    #[error("unterminated string literal starting at line {line}")]
    UnterminatedString { line: u32 },
}

// RULE: Collect errors, don't stop at first
// BAD
fn lex(src: &str) -> Result<Vec<Token>, LexError> {
    // stops at first error
}

// GOOD
fn lex(src: &str) -> Result<Vec<Token>, Vec<LexError>> {
    let mut errors = Vec::new();
    let mut tokens = Vec::new();
    // collect all errors
    if errors.is_empty() { Ok(tokens) } else { Err(errors) }
}
```

### 2.3 Safety Rules

```rust
// RULE: No unsafe code outside src/runtime/os/
// RULE: Every unsafe block must have SAFETY comment

// BAD
unsafe { *ptr = value; }

// GOOD
// SAFETY: ptr was allocated by alloc!() in the same function,
// and size was bounds-checked against the heap region.
unsafe { *ptr = value; }
```

### 2.4 Keyword Table

```rust
// RULE: Keywords use static lookup, not match chain
lazy_static! {
    static ref KEYWORDS: HashMap<&'static str, TokenKind> = {
        let mut m = HashMap::new();
        m.insert("let", TokenKind::Let);
        m.insert("fn", TokenKind::Fn);
        // ...
        m
    };
}

fn lookup(keyword: &str) -> Option<TokenKind> { ... }
```

### 2.5 Module Organization

```rust
// RULE: Each module file starts with module-level doc comment
//! # Lexer
//!
//! Converts Fajar Lang source code into a stream of tokens.
//! See [`tokenize`] for the main entry point.

// RULE: Public API at top, private helpers at bottom
pub fn tokenize(source: &str) -> Result<Vec<Token>, Vec<LexError>> { ... }
pub struct Token { ... }
pub enum TokenKind { ... }

// Private helpers below
fn is_digit(c: char) -> bool { c.is_ascii_digit() }
fn is_alpha(c: char) -> bool { c.is_alphabetic() || c == '_' }
```

## 3. Testing Rules

### 3.1 Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // RULE: Test names describe behavior, not implementation
    // BAD
    #[test]
    fn test_lex() { ... }

    // GOOD
    #[test]
    fn lexer_produces_int_token_for_decimal_literal() { ... }

    #[test]
    fn lexer_reports_error_for_unterminated_string() { ... }

    #[test]
    fn parser_handles_left_associative_addition() { ... }
}
```

### 3.2 Test Coverage Requirements

| Component | Required Coverage |
|-----------|-------------------|
| Lexer | Every token type |
| Parser | Every AST node type |
| Analyzer | Every error kind |
| Interpreter | Every built-in function |
| OS Runtime | Every primitive (simulated) |
| ML Runtime | Every op + autograd correctness |

### 3.3 Test Utilities

```rust
// Helper macros for tests
macro_rules! assert_tokens {
    ($src:expr, $($kind:expr),+) => {
        let tokens = tokenize($src).unwrap();
        let kinds: Vec<_> = tokens.iter()
            .filter(|t| t.kind != TokenKind::Eof)
            .map(|t| t.kind.clone()).collect();
        assert_eq!(kinds, vec![$($kind),+]);
    };
}

macro_rules! assert_eval {
    ($src:expr, $expected:expr) => {
        let mut interp = Interpreter::new();
        let result = interp.eval_source($src).unwrap();
        assert_eq!(result, $expected);
    };
}

// Usage
assert_tokens!("let x = 42", TokenKind::Let, TokenKind::Ident("x".into()),
    TokenKind::Eq, TokenKind::IntLit(42));
assert_eval!("1 + 2 * 3", Value::Int(7));
```

## 4. Documentation Rules

```rust
// RULE: All pub items must have doc comments

/// Represents a single token in the Fajar Lang source code.
///
/// Tokens are produced by the lexer and consumed by the parser.
/// Each token carries its kind, source location (span), and line/column info.
pub struct Token {
    /// The kind of token (keyword, literal, operator, etc.)
    pub kind: TokenKind,
    /// Byte offset range in the source string
    pub span: Span,
    /// 1-indexed line number
    pub line: u32,
    /// 1-indexed column number (in characters, not bytes)
    pub col: u32,
}
```

## 5. Architecture Rules

### 5.1 Dependency Direction

```
ALLOWED dependency direction:
    main.rs → interpreter → analyzer → parser → lexer
    interpreter → runtime/os
    interpreter → runtime/ml
    runtime/ml → (ndarray)

FORBIDDEN:
    lexer → parser (no upward deps)
    parser → interpreter
    runtime/os → runtime/ml (they are siblings, not parent/child)
```

### 5.2 Module Boundaries

```rust
// RULE: No cross-module struct field access — use methods
// BAD
let kind = token.kind;  // from outside lexer module

// GOOD (with pub getter)
impl Token {
    pub fn kind(&self) -> &TokenKind { &self.kind }
}
let kind = token.kind();

// EXCEPTION: Simple data structs like Span can have pub fields
pub struct Span {
    pub start: usize,
    pub end: usize,
}
```

### 5.3 Runtime Context Separation

```rust
// RULE: OS and ML features must not bleed into each other's modules
// RULE: Context validation happens in Analyzer, not Interpreter

// BAD — doing context check in interpreter
fn eval_builtin(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if name == "alloc" && !self.is_kernel_context() {
        return Err(RuntimeError::Custom("alloc only in @kernel".into()));
    }
}

// GOOD — analyzer catches this at compile time
// analyzer/type_check.rs validates @kernel context during analysis
// interpreter trusts the analyzer's work
```

## 6. Invariants (Must Always Be True)

1. Every AST node has a valid Span
2. EOF token is always last in token stream
3. Lexer produces no whitespace tokens
4. Parser never returns partial AST (either full program or error)
5. Analyzer: every symbol is defined before use
6. Interpreter: `Value::Null` only for void-typed expressions
7. TensorValue: `shape.iter().product() == data.len()`
8. MemoryManager: no overlapping allocated regions
9. Autograd: `backward()` only called after `forward()`
10. All tests must pass before any commit

## 7. Code Review Checklist

Before marking any task as DONE:

- [ ] No `.unwrap()` in `src/` (only in tests)
- [ ] No `unsafe` without SAFETY comment
- [ ] All `pub` items have doc comments
- [ ] All tests pass: `cargo test`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Code formatted: `cargo fmt`
- [ ] New functions have at least 1 test
- [ ] TASKS.md updated
- [ ] No `TODO:` comments left in final code (use GitHub issues instead)

---

*Rules Version: 1.0 | Non-negotiable unless discussed explicitly*
