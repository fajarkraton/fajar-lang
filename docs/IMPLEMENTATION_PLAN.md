# IMPLEMENTATION PLAN — Fajar Lang

> Rencana implementasi lengkap dari nol hingga production-ready.
> Setiap task memiliki: file target, input/output contract, dependency, test criteria.
> Update dokumen ini seiring progress.

---

## Status Overview

```
Current Step:  Phase 1 Sprint 1.7 (gap fixes) + Phase 2 Sprint 2.6 (gap fixes)
Next Action:   Step 1.7.1 — Missing Error Codes (LE008, PE007-PE010)
Phase 0:       ✅ COMPLETE (2026-03-05)
Phase 1:       🟡 Core done (1.1-1.6), gap fixes remaining (1.7)
Phase 2:       🟡 Core done (2.1-2.5), gap fixes remaining (2.6-2.10)
```

---

## Phase 0 — Project Scaffolding

**Goal:** Project Rust valid, `cargo build` dan `cargo test` sukses, semua directory dan placeholder files ada.

**Duration:** 1 session (< 1 jam)

### Step 0.1 — Initialize Cargo Project

**Action:**
```bash
cd "/home/primecore/Documents/Fajar Lang"
cargo init --name fajar-lang .
```

**Cargo.toml:**
```toml
[package]
name = "fajar-lang"
version = "0.1.0"
edition = "2021"
description = "Fajar Lang — Systems programming language for OS and AI/ML"
license = "MIT"

[[bin]]
name = "fj"
path = "src/main.rs"

[dependencies]
thiserror = "2.0"
miette = { version = "7.0", features = ["fancy"] }
clap = { version = "4.5", features = ["derive"] }
rustyline = "14.0"
ndarray = "0.16"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
indexmap = "2.0"

[dev-dependencies]
criterion = "0.5"
pretty_assertions = "1.4"

[profile.release]
opt-level = 3
lto = true
```

**Verify:** `cargo build` succeeds

### Step 0.2 — Create Directory Structure

```bash
mkdir -p src/{lexer,parser,analyzer,interpreter}
mkdir -p src/runtime/{os,ml}
mkdir -p src/stdlib
mkdir -p tests examples benches
```

### Step 0.3 — Create Placeholder Module Files

Setiap file harus memiliki module-level doc comment dan content minimal agar `cargo build` sukses.

**File: `src/lib.rs`**
```rust
//! # Fajar Lang
//!
//! A systems programming language for OS development and AI/ML.

pub mod lexer;
pub mod parser;
// pub mod analyzer;    // uncomment in Phase 2
// pub mod interpreter; // uncomment in Sprint 1.4
// pub mod runtime;     // uncomment in Phase 3
```

**File: `src/main.rs`**
```rust
//! Fajar Lang CLI entry point.
fn main() {
    println!("Fajar Lang v0.1.0");
    println!("Use `fj --help` for usage (coming in Sprint 1.6)");
}
```

**File: `src/lexer/mod.rs`**
```rust
//! Fajar Lang lexer — converts source code into tokens.
pub mod token;
pub mod cursor;
```

**File: `src/lexer/token.rs`**
```rust
//! Token types for the Fajar Lang lexer.
```

**File: `src/lexer/cursor.rs`**
```rust
//! Source code cursor for character-by-character scanning.
```

**File: `src/parser/mod.rs`**
```rust
//! Fajar Lang parser — converts tokens to AST.
pub mod ast;
pub mod pratt;
```

**File: `src/parser/ast.rs`**
```rust
//! Abstract Syntax Tree node definitions.
```

**File: `src/parser/pratt.rs`**
```rust
//! Pratt expression parser with operator precedence.
```

**Verify:**
```bash
cargo build   # must succeed
cargo test    # 0 tests, 0 failures
cargo clippy  # review warnings
```

### Step 0.4 — Create Example Files

**File: `examples/hello.fj`**
```fajar
use std::io::println

fn main() -> void {
    println("Hello from Fajar Lang!")
}
```

**File: `examples/fibonacci.fj`**
```fajar
fn fibonacci(n: i32) -> i32 {
    if n <= 1 { n }
    else { fibonacci(n - 1) + fibonacci(n - 2) }
}

fn main() -> void {
    for i in 0..10 {
        println(fibonacci(i))
    }
}
```

### Step 0.5 — Git Init & First Commit

```bash
git init
git add .
git commit -m "chore: scaffold project structure with all modules"
git checkout -b phase-1
```

**Exit criteria Step 0:** `cargo build` clean, `cargo test` runs 0 tests, git initialized.

---

## Phase 1 — Core Language Foundation

**Goal:** Working tree-walking interpreter yang bisa menjalankan program Fajar Lang dasar.
**Duration:** 8-12 minggu | **Status:** NEXT

---

### Sprint 1.1 — Lexer

**Goal:** `tokenize("source") → Result<Vec<Token>, Vec<LexError>>`
**Duration:** 1-2 minggu

#### Step 1.1.1 — Span & Token Structs

**File:** `src/lexer/token.rs`

**Implement:**
```rust
/// Byte offset range in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// A single token from the Fajar Lang source.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub line: u32,
    pub col: u32,
}
```

**Tests:**
- `span_creation_and_merge`
- `token_has_correct_fields`

**Dependency:** None
**Commit:** `feat(lexer): define Span and Token structs`

#### Step 1.1.2 — TokenKind Enum

**File:** `src/lexer/token.rs`

**Implement:** Complete `TokenKind` enum with ALL variants:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // === Literals ===
    IntLit(i64),
    FloatLit(f64),
    StringLit(String),
    BoolLit(bool),
    CharLit(char),

    // === Identifier ===
    Ident(String),

    // === Control Flow Keywords ===
    If, Else, Match, While, For, In, Return, Break, Continue,

    // === Declaration Keywords ===
    Let, Mut, Fn, Struct, Enum, Impl, Trait, Type, Const,

    // === Module Keywords ===
    Use, Mod, Pub, Extern, As,

    // === Literal Keywords ===
    True, False, Null,

    // === Type Keywords ===
    Bool, I8, I16, I32, I64, I128, Isize,
    U8, U16, U32, U64, U128, Usize,
    F32, F64, Str, Char, Void, Never,

    // === ML Keywords ===
    Tensor, Grad, Loss, Layer, Model,

    // === OS Keywords ===
    Ptr, Addr, Page, Region, Irq, Syscall,

    // === Arithmetic Operators ===
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %
    StarStar,   // **
    At,         // @ (matrix multiply)

    // === Comparison Operators ===
    EqEq,       // ==
    BangEq,     // !=
    Lt,         // <
    Gt,         // >
    LtEq,       // <=
    GtEq,       // >=

    // === Logical Operators ===
    AmpAmp,     // &&
    PipePipe,   // ||
    Bang,       // !

    // === Bitwise Operators ===
    Amp,        // &
    Pipe,       // |
    Caret,      // ^
    Tilde,      // ~
    LtLt,       // <<
    GtGt,       // >>

    // === Assignment Operators ===
    Eq,         // =
    PlusEq,     // +=
    MinusEq,    // -=
    StarEq,     // *=
    SlashEq,    // /=
    PercentEq,  // %=
    AmpEq,      // &=
    PipeEq,     // |=
    CaretEq,    // ^=
    LtLtEq,     // <<=
    GtGtEq,     // >>=

    // === Delimiters ===
    LParen,     // (
    RParen,     // )
    LBrace,     // {
    RBrace,     // }
    LBracket,   // [
    RBracket,   // ]

    // === Punctuation ===
    Semicolon,  // ;
    Colon,      // :
    ColonColon, // ::
    Comma,      // ,
    Dot,        // .
    DotDot,     // ..
    DotDotEq,   // ..=
    Arrow,      // ->
    FatArrow,   // =>
    PipeArrow,  // |>

    // === Annotations ===
    AtSign,     // @ (for annotations, context-dependent with At)

    // === Special ===
    Eof,
}
```

**Tests:**
- `token_kind_equality` — verify all variants can be compared
- `token_kind_debug_display` — verify Debug output

**Dependency:** Step 1.1.1
**Commit:** `feat(lexer): define complete TokenKind enum with all variants`

#### Step 1.1.3 — Keyword Lookup Table

**File:** `src/lexer/token.rs`

**Implement:**
```rust
use std::collections::HashMap;
use std::sync::LazyLock;

static KEYWORDS: LazyLock<HashMap<&'static str, TokenKind>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("if", TokenKind::If);
    m.insert("else", TokenKind::Else);
    // ... all 50+ keywords
    m
});

/// Look up whether an identifier is a keyword.
pub fn lookup_keyword(ident: &str) -> Option<TokenKind> {
    KEYWORDS.get(ident).cloned()
}
```

**Tests:**
- `lookup_finds_all_keywords` — iterate all keyword strings, verify lookup succeeds
- `lookup_returns_none_for_identifiers` — `lookup_keyword("foo")` returns None
- `lookup_is_case_sensitive` — `lookup_keyword("IF")` returns None

**Dependency:** Step 1.1.2
**Commit:** `feat(lexer): implement keyword lookup table`

#### Step 1.1.4 — LexError Type

**File:** `src/lexer/token.rs` (atau file baru `src/lexer/error.rs`)

**Implement:**
```rust
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum LexError {
    #[error("[LE001] unexpected character '{ch}' at {line}:{col}")]
    UnexpectedChar { ch: char, line: u32, col: u32, span: Span },

    #[error("[LE002] unterminated string literal starting at {line}:{col}")]
    UnterminatedString { line: u32, col: u32, span: Span },

    #[error("[LE003] unterminated block comment")]
    UnterminatedComment { span: Span },

    #[error("[LE004] invalid number literal")]
    InvalidNumber { span: Span },

    #[error("[LE005] invalid escape sequence")]
    InvalidEscape { span: Span },

    #[error("[LE006] number literal overflow")]
    NumberOverflow { span: Span },

    #[error("[LE007] empty character literal")]
    EmptyCharLiteral { span: Span },

    #[error("[LE008] multi-character character literal")]
    MultiCharLiteral { span: Span },
}
```

**Tests:**
- `lex_error_display_messages` — verify error messages format correctly

**Dependency:** Step 1.1.1
**Commit:** `feat(lexer): define LexError types with error codes`

#### Step 1.1.5 — Cursor Struct

**File:** `src/lexer/cursor.rs`

**Implement:**
```rust
pub struct Cursor<'src> {
    source: &'src str,
    pos: usize,
    line: u32,
    col: u32,
}

impl<'src> Cursor<'src> {
    pub fn new(source: &'src str) -> Self;
    pub fn peek(&self) -> Option<char>;
    pub fn peek_next(&self) -> Option<char>;     // lookahead 2
    pub fn advance(&mut self) -> Option<char>;
    pub fn is_eof(&self) -> bool;
    pub fn remaining(&self) -> &'src str;
    pub fn pos(&self) -> usize;
    pub fn line(&self) -> u32;
    pub fn col(&self) -> u32;
    pub fn span_from(&self, start: usize) -> Span;
}
```

**Tests:**
- `cursor_peek_returns_first_char`
- `cursor_advance_moves_position`
- `cursor_tracks_line_and_col_on_newline`
- `cursor_is_eof_at_end`
- `cursor_handles_empty_source`
- `cursor_handles_unicode`
- `cursor_peek_next_lookahead`

**Dependency:** Step 1.1.1
**Commit:** `feat(lexer): implement Cursor with peek, advance, position tracking`

#### Step 1.1.6 — Lexer Core: Whitespace & Comments

**File:** `src/lexer/mod.rs`

**Implement:**
```rust
pub fn tokenize(source: &str) -> Result<Vec<Token>, Vec<LexError>>;

// Internal helpers:
fn skip_whitespace(cursor: &mut Cursor);
fn skip_line_comment(cursor: &mut Cursor);
fn skip_block_comment(cursor: &mut Cursor) -> Result<(), LexError>;
```

**Tests:**
- `tokenize_empty_string_returns_only_eof`
- `tokenize_whitespace_only_returns_eof`
- `tokenize_skips_line_comments`
- `tokenize_skips_block_comments`
- `tokenize_reports_unterminated_block_comment`

**Dependency:** Step 1.1.5
**Commit:** `feat(lexer): implement whitespace and comment skipping`

#### Step 1.1.7 — Lexer: Identifiers & Keywords

**File:** `src/lexer/mod.rs`

**Implement:**
```rust
fn scan_identifier_or_keyword(cursor: &mut Cursor) -> Token;
```

Identifier rule: starts with `[a-zA-Z_]`, followed by `[a-zA-Z0-9_]*`.
After scanning, lookup in keyword table → return keyword token or `Ident`.

**Tests:**
- `lexer_produces_ident_for_unknown_word`
- `lexer_produces_keyword_for_let`
- `lexer_produces_keyword_for_all_keywords` (parametrized)
- `lexer_handles_underscore_prefix`
- `lexer_handles_identifier_with_numbers`

**Dependency:** Step 1.1.3, 1.1.6
**Commit:** `feat(lexer): implement identifier and keyword scanning`

#### Step 1.1.8 — Lexer: Number Literals

**File:** `src/lexer/mod.rs`

**Implement:**
```rust
fn scan_number(cursor: &mut Cursor) -> Result<TokenKind, LexError>;
// Handles: decimal, hex (0x), binary (0b), octal (0o),
//          float (3.14), scientific (1.0e-4),
//          underscore separators (1_000_000)
```

**Tests:**
- `lexer_produces_int_for_decimal` — `42`
- `lexer_produces_int_for_hex` — `0xFF`
- `lexer_produces_int_for_binary` — `0b1010`
- `lexer_produces_int_for_octal` — `0o17`
- `lexer_produces_float_for_decimal_point` — `3.14`
- `lexer_produces_float_for_scientific` — `1.0e-4`
- `lexer_handles_underscore_separator` — `1_000_000`
- `lexer_reports_error_for_invalid_hex` — `0xGG`
- `lexer_reports_error_for_overflow` — huge number

**Dependency:** Step 1.1.6
**Commit:** `feat(lexer): implement number literal scanning (dec/hex/bin/oct/float/sci)`

#### Step 1.1.9 — Lexer: String Literals

**File:** `src/lexer/mod.rs`

**Implement:**
```rust
fn scan_string(cursor: &mut Cursor) -> Result<TokenKind, LexError>;
// Regular strings: "hello\nworld"
// Raw strings: r"no \n escape"
// Escape sequences: \n \t \r \\ \" \0 \xHH
```

**Tests:**
- `lexer_produces_string_for_simple` — `"hello"`
- `lexer_handles_escape_sequences` — `"hello\nworld"`
- `lexer_handles_raw_strings` — `r"no \n escape"`
- `lexer_reports_unterminated_string`
- `lexer_reports_invalid_escape`
- `lexer_handles_empty_string` — `""`

**Dependency:** Step 1.1.6
**Commit:** `feat(lexer): implement string literal scanning with escapes and raw strings`

#### Step 1.1.10 — Lexer: Char Literals

**File:** `src/lexer/mod.rs`

**Implement:**
```rust
fn scan_char(cursor: &mut Cursor) -> Result<TokenKind, LexError>;
// 'a', '\n', '\\'
```

**Tests:**
- `lexer_produces_char_literal` — `'a'`
- `lexer_handles_char_escape` — `'\n'`
- `lexer_reports_empty_char` — `''`
- `lexer_reports_multi_char` — `'ab'`

**Dependency:** Step 1.1.6
**Commit:** `feat(lexer): implement char literal scanning`

#### Step 1.1.11 — Lexer: Operators & Punctuation

**File:** `src/lexer/mod.rs`

**Implement:**
```rust
fn scan_operator_or_punctuation(cursor: &mut Cursor) -> Result<Token, LexError>;
// Must handle multi-char operators: ==, !=, <=, >=, ->, =>, |>, ::
// Must handle ambiguity: * vs ** vs *=
// Must handle: .. vs ..=
```

**Logic:** Longest-match — try longest operator first, then fall back.

**Tests:**
- `lexer_produces_single_char_operators` — `+ - * / % < >`
- `lexer_produces_double_char_operators` — `== != <= >= -> => |> :: ..`
- `lexer_produces_triple_char_operators` — `..= <<= >>=`
- `lexer_distinguishes_star_starstar_stareq` — `*` vs `**` vs `*=`
- `lexer_produces_all_delimiters` — `( ) { } [ ]`
- `lexer_produces_all_punctuation` — `; : , .`
- `lexer_reports_unexpected_char` — `$`

**Dependency:** Step 1.1.6
**Commit:** `feat(lexer): implement operator and punctuation scanning`

#### Step 1.1.12 — Lexer: Integration & Error Collection

**File:** `src/lexer/mod.rs`

**Implement:** Complete `tokenize()` function yang menggabungkan semua scanner:

```rust
pub fn tokenize(source: &str) -> Result<Vec<Token>, Vec<LexError>> {
    let mut cursor = Cursor::new(source);
    let mut tokens = Vec::new();
    let mut errors = Vec::new();

    while !cursor.is_eof() {
        skip_whitespace(&mut cursor);
        if cursor.is_eof() { break; }

        match scan_token(&mut cursor) {
            Ok(token) => tokens.push(token),
            Err(err) => {
                errors.push(err);
                cursor.advance(); // skip problematic char
            }
        }
    }

    tokens.push(Token::eof(cursor.pos(), cursor.line(), cursor.col()));

    if errors.is_empty() { Ok(tokens) } else { Err(errors) }
}
```

**Tests (Integration — file `tests/lexer_tests.rs`):**
- `tokenize_hello_world_program` — full `hello.fj` source
- `tokenize_let_declaration` — `let x: i32 = 42`
- `tokenize_function_definition` — `fn add(a: i32, b: i32) -> i32 { a + b }`
- `tokenize_if_else_expression` — `if x > 0 { x } else { -x }`
- `tokenize_pipeline_operator` — `5 |> double |> add_one`
- `tokenize_annotation` — `@kernel fn init() { }`
- `tokenize_tensor_operations` — `let y = x @ w + b`
- `tokenize_collects_multiple_errors` — source with 3+ errors, all reported
- `tokenize_eof_always_last` — invariant check

**Dependency:** Steps 1.1.7–1.1.11
**Commit:** `feat(lexer): complete tokenize() with error collection and integration tests`

**Sprint 1.1 Exit Criteria:**
- [ ] `cargo test` — all lexer tests pass
- [ ] Every TokenKind variant produced by at least 1 test
- [ ] Error collection works (multiple errors in one source)
- [ ] EOF always last token
- [ ] `cargo clippy -- -D warnings` clean

---

### Sprint 1.2 — AST Definition

**Goal:** Complete AST node types for all Fajar Lang constructs.
**Duration:** 1 minggu

#### Step 1.2.1 — Core Expression Nodes

**File:** `src/parser/ast.rs`

**Implement:**
```rust
use crate::lexer::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // Literals
    Literal(LiteralKind, Span),
    Ident(String, Span),

    // Operators
    Binary(Box<Expr>, BinOp, Box<Expr>, Span),
    Unary(UnaryOp, Box<Expr>, Span),

    // Access
    Call { callee: Box<Expr>, args: Vec<Expr>, span: Span },
    MethodCall { receiver: Box<Expr>, method: String, args: Vec<Expr>, span: Span },
    Field { object: Box<Expr>, field: String, span: Span },
    Index { object: Box<Expr>, index: Box<Expr>, span: Span },

    // Control flow
    Block(Vec<Stmt>, Option<Box<Expr>>, Span),
    If { cond: Box<Expr>, then_branch: Box<Expr>, else_branch: Option<Box<Expr>>, span: Span },
    Match { subject: Box<Expr>, arms: Vec<MatchArm>, span: Span },
    While { cond: Box<Expr>, body: Box<Expr>, span: Span },
    For { var: String, iter: Box<Expr>, body: Box<Expr>, span: Span },

    // Jump
    Return(Option<Box<Expr>>, Span),
    Break(Option<Box<Expr>>, Span),
    Continue(Span),

    // Special
    Assign { target: Box<Expr>, op: AssignOp, value: Box<Expr>, span: Span },
    Pipe(Box<Expr>, Box<Expr>, Span),
    Range { start: Box<Expr>, end: Box<Expr>, inclusive: bool, span: Span },

    // Collections
    Array(Vec<Expr>, Span),
    Tuple(Vec<Expr>, Span),
    TensorLiteral(Vec<Expr>, Span),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralKind {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Char(char),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod, Pow, MatMul,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
    Pipe,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Neg,    // -
    Not,    // !
    Deref,  // *
    Ref,    // &
    RefMut, // &mut
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssignOp {
    Assign, AddAssign, SubAssign, MulAssign, DivAssign,
    ModAssign, BitAndAssign, BitOrAssign, BitXorAssign,
    ShlAssign, ShrAssign,
}
```

**Tests:**
- `expr_literal_creation`
- `expr_binary_nested`
- `expr_has_span`

**Dependency:** Lexer Span type
**Commit:** `feat(parser): define Expr enum with 20+ variants`

#### Step 1.2.2 — Statement Nodes

**File:** `src/parser/ast.rs`

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let { name: String, mutable: bool, ty: Option<TypeExpr>, value: Expr, span: Span },
    Const { name: String, ty: TypeExpr, value: Expr, span: Span },
    Expr(Expr),
    Item(Item),
}
```

**Dependency:** Step 1.2.1
**Commit:** `feat(parser): define Stmt enum`

#### Step 1.2.3 — Item Nodes (Top-Level Declarations)

**File:** `src/parser/ast.rs`

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    FnDef(FnDef),
    StructDef(StructDef),
    EnumDef(EnumDef),
    ImplBlock(ImplBlock),
    TraitDef(TraitDef),
    UseDecl(UseDecl),
    ModDecl(ModDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeExpr>,
    pub body: Expr, // Block expression
    pub annotations: Vec<Annotation>,
    pub generic_params: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<StructField>,
    pub generic_params: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub generic_params: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub data: Vec<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplBlock {
    pub target: String,
    pub trait_name: Option<String>,
    pub methods: Vec<FnDef>,
    pub generic_params: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraitDef {
    pub name: String,
    pub methods: Vec<FnDef>,
    pub generic_params: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    pub path: Vec<String>,
    pub kind: UseKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UseKind {
    Simple,                   // use foo::bar
    Glob,                     // use foo::*
    Selective(Vec<String>),   // use foo::{a, b, c}
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModDecl {
    pub name: String,
    pub items: Option<Vec<Item>>,
    pub span: Span,
}
```

**Dependency:** Step 1.2.2
**Commit:** `feat(parser): define Item nodes (fn, struct, enum, impl, trait, use, mod)`

#### Step 1.2.4 — TypeExpr, Pattern, Annotation, MatchArm

**File:** `src/parser/ast.rs`

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Simple(String, Span),                                    // i32, bool, MyStruct
    Generic(String, Vec<TypeExpr>, Span),                    // Vec<i32>, Option<T>
    Tensor { dtype: String, dims: Vec<TensorDim>, span: Span }, // Tensor<f32>[784, 128]
    Pointer { mutable: bool, inner: Box<TypeExpr>, span: Span }, // *const T, *mut T
    Tuple(Vec<TypeExpr>, Span),                              // (i32, f64)
    Array { elem: Box<TypeExpr>, size: Box<Expr>, span: Span }, // [i32; 5]
    Slice(Box<TypeExpr>, Span),                              // [i32]
    Fn { params: Vec<TypeExpr>, ret: Box<TypeExpr>, span: Span }, // fn(i32) -> bool
}

#[derive(Debug, Clone, PartialEq)]
pub enum TensorDim {
    Fixed(usize),
    Dynamic, // *
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Literal(LiteralKind, Span),
    Ident(String, Span),
    Wildcard(Span),
    Tuple(Vec<Pattern>, Span),
    Struct { name: String, fields: Vec<(String, Option<Pattern>)>, span: Span },
    Enum { path: String, variant: String, data: Vec<Pattern>, span: Span },
    Range { start: Box<Expr>, end: Box<Expr>, inclusive: bool, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub name: String,
    pub args: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub items: Vec<Item>,
    pub stmts: Vec<Stmt>,
    pub span: Span,
}
```

**Dependency:** Step 1.2.3
**Commit:** `feat(parser): define TypeExpr, Pattern, Annotation, MatchArm, Program`

#### Step 1.2.5 — Span Utility Methods & Display Impls

**File:** `src/lexer/token.rs` + `src/parser/ast.rs`

**Implement:**
```rust
impl Span {
    pub fn new(start: usize, end: usize) -> Self;
    pub fn merge(self, other: Span) -> Span;
    pub fn len(&self) -> usize;
}

impl Expr {
    pub fn span(&self) -> Span; // extract span from any Expr variant
}
```

Plus `Display` implementations for key types.

**Dependency:** Step 1.2.4
**Commit:** `feat(parser): add Span utilities and Display impls for AST nodes`

**Sprint 1.2 Exit Criteria:**
- [ ] Every AST variant exists and compiles
- [ ] Every node carries a Span
- [ ] `cargo test` passes
- [ ] `cargo clippy` clean

---

### Sprint 1.3 — Parser

**Goal:** `parse(tokens) → Result<Program, Vec<ParseError>>`
**Duration:** 2-3 minggu

#### Step 1.3.1 — ParseError Type & Token Cursor

**File:** `src/parser/mod.rs`

```rust
// ParseError
#[derive(Debug, Error, Clone)]
pub enum ParseError {
    #[error("[PE001] unexpected token: expected {expected}, got {got}")]
    UnexpectedToken { expected: String, got: String, span: Span },
    #[error("[PE002] expected expression")]
    ExpectedExpression { span: Span },
    // ... PE001-PE010
}

// Token cursor
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<ParseError>,
}

impl Parser {
    fn peek(&self) -> &Token;
    fn peek_kind(&self) -> &TokenKind;
    fn advance(&mut self) -> &Token;
    fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError>;
    fn is_eof(&self) -> bool;
    fn synchronize(&mut self); // error recovery: skip to next statement boundary
}
```

**Tests:**
- `parser_cursor_peek_and_advance`
- `parser_expect_succeeds_on_match`
- `parser_expect_returns_error_on_mismatch`
- `parser_synchronize_skips_to_next_statement`

**Dependency:** Sprint 1.1 + Sprint 1.2
**Commit:** `feat(parser): implement ParseError and token cursor`

#### Step 1.3.2 — Pratt Expression Parser

**File:** `src/parser/pratt.rs`

**Implement:** Pratt parser with binding power table from SKILLS.md:

```rust
impl BinOp {
    fn binding_power(&self) -> (u8, u8) {
        match self {
            BinOp::Pipe      => (2, 3),
            BinOp::Or        => (4, 5),
            BinOp::And       => (6, 7),
            BinOp::Eq | BinOp::Ne => (8, 9),
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => (10, 11),
            BinOp::Add | BinOp::Sub => (12, 13),
            BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::MatMul => (14, 15),
            BinOp::Pow => (17, 16),  // right associative
            _ => todo!()
        }
    }
}

fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ParseError>;
```

**Tests:**
- `parser_simple_addition` — `1 + 2`
- `parser_precedence_mul_over_add` — `1 + 2 * 3` → `1 + (2 * 3)`
- `parser_left_associative` — `1 - 2 - 3` → `(1 - 2) - 3`
- `parser_right_associative_power` — `2 ** 3 ** 4` → `2 ** (3 ** 4)`
- `parser_pipeline_operator` — `x |> f |> g`
- `parser_matrix_multiply` — `a @ b`
- `parser_parenthesized_expression` — `(1 + 2) * 3`
- `parser_unary_negation` — `-x`
- `parser_unary_not` — `!true`

**Dependency:** Step 1.3.1
**Commit:** `feat(parser): implement Pratt expression parser with precedence`

#### Step 1.3.3 — Parse Literals & Primary Expressions

**File:** `src/parser/mod.rs`

**Implement:** parse_literal, parse_ident, parse_grouped, parse_array, parse_tuple

**Tests per literal type.**

**Dependency:** Step 1.3.2
**Commit:** `feat(parser): parse literals and primary expressions`

#### Step 1.3.4 — Parse Function Calls, Method Calls, Field Access, Index

**Tests:**
- `parser_function_call` — `foo(1, 2, 3)`
- `parser_method_call` — `x.bar(1)`
- `parser_field_access` — `point.x`
- `parser_index_access` — `arr[0]`
- `parser_chained_access` — `a.b.c[0].d()`

**Dependency:** Step 1.3.3
**Commit:** `feat(parser): parse calls, method calls, field access, indexing`

#### Step 1.3.5 — Parse Let/Const Statements

**Tests:**
- `parser_let_with_type` — `let x: i32 = 42`
- `parser_let_without_type` — `let x = 42`
- `parser_let_mutable` — `let mut x = 0`
- `parser_const` — `const MAX: usize = 1024`

**Dependency:** Step 1.3.3
**Commit:** `feat(parser): parse let and const statements`

#### Step 1.3.6 — Parse If/Else Expressions

**Tests:**
- `parser_if_only` — `if x > 0 { x }`
- `parser_if_else` — `if x > 0 { x } else { -x }`
- `parser_if_else_if` — chained
- `parser_if_as_expression` — `let max = if a > b { a } else { b }`

**Dependency:** Step 1.3.5
**Commit:** `feat(parser): parse if/else expressions`

#### Step 1.3.7 — Parse While/For Loops

**Tests:**
- `parser_while_loop` — `while x > 0 { x -= 1 }`
- `parser_for_in_range` — `for i in 0..10 { ... }`
- `parser_for_in_collection` — `for item in items { ... }`

**Dependency:** Step 1.3.6
**Commit:** `feat(parser): parse while and for loops`

#### Step 1.3.8 — Parse Match Expressions

**Tests:**
- `parser_match_literals` — `match x { 0 => "zero", _ => "other" }`
- `parser_match_with_guard` — `x if x < 0 => "neg"`
- `parser_match_enum_destructuring`
- `parser_match_range_pattern` — `1..=9 => "digit"`

**Dependency:** Step 1.3.7
**Commit:** `feat(parser): parse match expressions with patterns and guards`

#### Step 1.3.9 — Parse Function Definitions

**Tests:**
- `parser_fn_basic` — `fn add(a: i32, b: i32) -> i32 { a + b }`
- `parser_fn_no_params` — `fn greet() -> void { ... }`
- `parser_fn_no_return_type` — `fn greet() { ... }`
- `parser_fn_with_annotation` — `@kernel fn init() { ... }`

**Dependency:** Step 1.3.8
**Commit:** `feat(parser): parse function definitions with annotations`

#### Step 1.3.10 — Parse Struct/Enum Definitions

**Tests:**
- `parser_struct` — `struct Point { x: f64, y: f64 }`
- `parser_enum` — `enum Direction { North, South, Custom(f64, f64) }`

**Dependency:** Step 1.3.9
**Commit:** `feat(parser): parse struct and enum definitions`

#### Step 1.3.11 — Parse Impl Blocks & Traits

**Tests:**
- `parser_impl_block` — `impl Point { fn distance(...) { ... } }`
- `parser_impl_trait_for` — `impl Module for LinearLayer { ... }`
- `parser_trait_def` — `trait Module { fn forward(...) -> Tensor }`

**Dependency:** Step 1.3.10
**Commit:** `feat(parser): parse impl blocks and trait definitions`

#### Step 1.3.12 — Parse Use/Mod Declarations

**Tests:**
- `parser_use_simple` — `use std::io::println`
- `parser_use_glob` — `use math::*`
- `parser_use_selective` — `use os::{alloc, free}`
- `parser_mod_inline` — `mod math { ... }`

**Dependency:** Step 1.3.11
**Commit:** `feat(parser): parse use and mod declarations`

#### Step 1.3.13 — Parse Assignments & Block Expressions

**Tests:**
- `parser_assignment` — `x = 42`
- `parser_compound_assignment` — `x += 1`
- `parser_block_returns_last_expr` — `{ let a = 5; a + 1 }`
- `parser_range_expression` — `0..10`, `0..=10`

**Dependency:** Step 1.3.12
**Commit:** `feat(parser): parse assignments, blocks, and ranges`

#### Step 1.3.14 — Parser Error Recovery

**Implement:** `synchronize()` — skip tokens until next statement boundary.
**Collect** all errors, don't stop at first.

**Tests:**
- `parser_recovers_and_reports_multiple_errors`
- `parser_skips_to_next_fn_after_error`

**Dependency:** Step 1.3.13
**Commit:** `feat(parser): implement error recovery with synchronization`

#### Step 1.3.15 — Parser Integration Tests

**File:** `tests/parser_tests.rs`

**Tests:**
- `parse_complete_hello_world` — full program
- `parse_function_with_body`
- `parse_struct_and_impl`
- `parse_complex_expressions`
- `parse_annotations_kernel_device`

**Dependency:** Step 1.3.14
**Commit:** `test(parser): add comprehensive integration tests`

**Sprint 1.3 Exit Criteria:**
- [ ] Every Expr variant can be parsed from tokens
- [ ] Every Stmt and Item type can be parsed
- [ ] Operator precedence follows spec exactly (11 levels)
- [ ] Error recovery works — multiple errors collected
- [ ] `cargo test` all pass

---

### Sprint 1.4 — Environment & Values

**Goal:** Value representation and scope chain for interpreter.
**Duration:** 1 minggu

#### Step 1.4.1 — Value Enum

**File:** `src/interpreter/value.rs`

Implement full `Value` enum (see Architecture section). Plus `Display`, `PartialEq`.

**Commit:** `feat(interpreter): define Value enum with all runtime types`

#### Step 1.4.2 — Environment (Scope Chain)

**File:** `src/interpreter/env.rs`

```rust
pub struct Environment {
    bindings: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    pub fn new(parent: Option<Rc<RefCell<Environment>>>) -> Rc<RefCell<Self>>;
    pub fn define(&mut self, name: String, value: Value);
    pub fn lookup(&self, name: &str) -> Option<Value>;
    pub fn assign(&mut self, name: &str, value: Value) -> Result<(), RuntimeError>;
    pub fn push_scope(parent: Rc<RefCell<Environment>>) -> Rc<RefCell<Environment>>;
}
```

**Tests:**
- `env_define_and_lookup`
- `env_nested_scope_shadows_parent`
- `env_lookup_in_parent_scope`
- `env_assign_existing_variable`
- `env_assign_undefined_returns_error`

**Commit:** `feat(interpreter): implement Environment with scope chain`

#### Step 1.4.3 — Function Values & Closures

**File:** `src/interpreter/value.rs`

```rust
pub struct FnValue {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Expr,
    pub closure_env: Rc<RefCell<Environment>>,
}
```

**Tests:**
- `fn_value_captures_closure_environment`

**Commit:** `feat(interpreter): implement FnValue with closure capture`

**Sprint 1.4 Exit Criteria:**
- [ ] Value enum covers all runtime types
- [ ] Environment scope chain works with nested push/pop
- [ ] Closures capture parent environment

---

### Sprint 1.5 — Interpreter

**Goal:** Tree-walking evaluation of all expressions and statements.
**Duration:** 2-3 minggu

#### Step 1.5.1 — Interpreter Struct & Initialization

**File:** `src/interpreter/mod.rs` + `src/interpreter/eval.rs`

```rust
pub struct Interpreter {
    env: Rc<RefCell<Environment>>,
}

impl Interpreter {
    pub fn new() -> Self;
    pub fn eval_source(&mut self, source: &str) -> Result<Value, FjError>;
    pub fn eval_program(&mut self, program: &Program) -> Result<Value, RuntimeError>;
}
```

**Commit:** `feat(interpreter): scaffold Interpreter struct`

#### Step 1.5.2 — Eval Literals & Identifiers

**Tests:** `assert_eval!("42", Value::Int(42))`, `assert_eval!("true", Value::Bool(true))`

#### Step 1.5.3 — Eval Arithmetic, Comparison, Logical

**Tests:**
- `eval_addition` — `1 + 2` → `3`
- `eval_precedence` — `1 + 2 * 3` → `7`
- `eval_float_division` — `10.0 / 3.0` → `3.333...`
- `eval_comparison` — `5 > 3` → `true`
- `eval_logical_and_or` — `true && false` → `false`
- `eval_power` — `2 ** 10` → `1024`

#### Step 1.5.4 — Eval Variable Operations (Let, Assign)

**Tests:**
- `eval_let_and_lookup` — `let x = 42; x` → `42`
- `eval_mutable_assignment` — `let mut x = 0; x = 5; x` → `5`
- `eval_immutable_assignment_error` — `let x = 0; x = 5` → Error

#### Step 1.5.5 — Eval Block Expressions

**Tests:** `eval_block_returns_last_expr` — `{ let a = 5; let b = 10; a + b }` → `15`

#### Step 1.5.6 — Eval If/Else

**Tests:** `eval_if_true_branch`, `eval_if_false_branch`, `eval_if_as_expression`

#### Step 1.5.7 — Eval While/For Loops

**Tests:** `eval_while_loop_counter`, `eval_for_in_range`

#### Step 1.5.8 — Eval Function Definition & Call

**Tests:**
- `eval_fn_call` — define `add`, call `add(1, 2)` → `3`
- `eval_recursive_fibonacci` — `fibonacci(10)` → `55`
- `eval_closure` — function captures variable from outer scope

#### Step 1.5.9 — Eval Return/Break/Continue

**Tests:** `eval_early_return`, `eval_break_from_while`, `eval_continue_in_for`

#### Step 1.5.10 — Eval Match

**Tests:** `eval_match_literal`, `eval_match_wildcard`, `eval_match_range_pattern`

#### Step 1.5.11 — Eval Struct/Enum Instantiation

**Tests:** `eval_struct_creation`, `eval_field_access`, `eval_enum_variant`

#### Step 1.5.12 — Built-in Functions

**Implement:** `print`, `println`, `eprintln`, `len`, `type_of`, `assert!`, `assert_eq!`

**Tests:** `eval_println_output`, `eval_len_array`, `eval_type_of`

#### Step 1.5.13-15 — Integration Tests

**File:** `tests/eval_tests.rs`

- `e2e_hello_world` — run `examples/hello.fj`
- `e2e_fibonacci` — run `examples/fibonacci.fj`, verify output
- `e2e_structs` — struct creation, method calls

**Sprint 1.5 Exit Criteria:**
- [ ] All expressions evaluate correctly
- [ ] All statements execute correctly
- [ ] Recursion works (fibonacci(30))
- [ ] Closures work
- [ ] Built-in functions work
- [ ] Integration tests pass

---

### Sprint 1.6 — CLI & REPL

**Goal:** `fj run`, `fj repl`, `fj check`, `fj dump-tokens`, `fj dump-ast`
**Duration:** 1 minggu

#### Step 1.6.1 — Clap CLI

**File:** `src/main.rs`

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fj", version, about = "Fajar Lang interpreter")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run { file: String },
    Repl,
    Check { file: String },
    DumpTokens { file: String },
    DumpAst { file: String },
}
```

**Commit:** `feat(cli): implement clap CLI with subcommands`

#### Step 1.6.2 — REPL with Rustyline

**Implement:** Interactive REPL with history, multi-line support.

**Commit:** `feat(cli): implement REPL with rustyline`

#### Step 1.6.3 — Error Display with Miette

**Implement:** Convert `FjError` to miette diagnostic for beautiful output.

**Commit:** `feat(cli): integrate miette for error display`

#### Step 1.6.4 — Exit Codes & Final Polish

**Exit codes:** 0 = success, 1 = runtime error, 2 = compile error, 3 = usage error

**Commit:** `feat(cli): add proper exit codes`

**Sprint 1.6 Exit Criteria:**
- [x] `fj run examples/hello.fj` — prints output ✅
- [x] `fj repl` — interactive evaluation ✅
- [x] `fj check examples/hello.fj` — shows errors or "OK" ✅
- [x] `fj dump-tokens examples/hello.fj` — shows token list ✅
- [x] `fj dump-ast examples/hello.fj` — shows AST (debug format) ✅
- [ ] Error messages have source highlighting ← **moved to Sprint 2.9 (miette)**

---

### Sprint 1.7 — Phase 1 Gap Fixes (GAP ANALYSIS)

**Problem:** Gap analysis revealed 8 areas where implementation diverges from specification.

#### Step 1.7.1 — Missing Error Codes

**Files:** `src/lexer/mod.rs`, `src/parser/mod.rs`

**Missing from Lexer (1 code):**
- [ ] LE008 MultiCharLiteral — `'ab'` → error (currently LE003 catches this generically)

**Missing from Parser (4 codes):**
- [ ] PE007 InvalidPattern — invalid pattern in match expression
- [ ] PE008 DuplicateField — `Point { x: 1, x: 2 }` (duplicate field in struct init)
- [ ] PE009 TrailingSeparator — trailing comma warning `fn f(a, b,)`
- [ ] PE010 InvalidAnnotation — `@unknown fn f() { }` (unknown annotation)

**Tests:** One test per new error code, verifying error message includes code.

#### Step 1.7.2 — Span::merge Utility

**File:** `src/lexer/token.rs`

```rust
impl Span {
    pub fn merge(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}
```

**Tests:** `span_merge_combines_two_spans`, `span_merge_overlapping`

#### Step 1.7.3 — Exit Code Spec Compliance

**File:** `src/main.rs`

**Current:** All errors return `ExitCode::from(1)`.
**Required:** 0=success, 1=runtime error, 2=compile error (lex/parse/semantic), 3=usage error.

- [ ] Lex/parse/semantic errors → `ExitCode::from(2)`
- [ ] Runtime errors → `ExitCode::from(1)`
- [ ] Usage errors (file not found, bad args) → `ExitCode::from(3)`

#### Step 1.7.4 — Integration Test Files

**Files:** `tests/lexer_tests.rs`, `tests/parser_tests.rs`, `tests/eval_tests.rs`

- [ ] `tests/lexer_tests.rs` — tokenize full programs, verify token sequences
- [ ] `tests/parser_tests.rs` — parse complete programs, verify AST structure
- [ ] `tests/eval_tests.rs` — E2E tests: run .fj files, verify output

**Tests per file:**
- lexer: `tokenize_hello_world_program`, `tokenize_function_definition`, `tokenize_collects_multiple_errors`
- parser: `parse_complete_hello_world`, `parse_struct_and_impl`, `parse_annotations`
- eval: `e2e_hello_world`, `e2e_fibonacci`, `e2e_factorial`

#### Step 1.7.5 — Missing Interpreter API Methods

**File:** `src/interpreter/eval.rs`

```rust
impl Interpreter {
    /// Convenience: lex + parse + eval in one call.
    pub fn eval_source(&mut self, source: &str) -> Result<Value, FjError>;

    /// Call a named function with arguments.
    pub fn call_fn(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError>;
}
```

**Tests:** `eval_source_simple_expr`, `call_fn_by_name`

#### Step 1.7.6 — FjError Typed Variants

**File:** `src/lib.rs`

**Current:** `FjError::Lex(Vec<String>)` — wraps as strings, losing structured data.
**Required:** `FjError::Lex(Vec<LexError>)` — wraps actual error types.

```rust
pub enum FjError {
    Lex(Vec<LexError>),
    Parse(Vec<ParseError>),
    Semantic(Vec<SemanticError>),
    Runtime(RuntimeError),
}
```

**Impact:** Update all `FjError` construction sites. Enables miette integration (Sprint 2.9).

#### Step 1.7.7 — Unused Dependencies Audit

**File:** `Cargo.toml`

| Dependency | Status | Action |
|---|---|---|
| `serde` + `serde_json` | Unused in src/ | Keep — needed for `dump-ast --json` (add usage) OR remove |
| `indexmap` | Unused in src/ | Remove — HashMap sufficient for now |
| `ndarray` | Placeholder only | Keep — Phase 4 tensor backend |
| `criterion` (dev) | benches/ empty | Keep — Phase 2+ benchmarks |
| `pretty_assertions` (dev) | Not imported | Keep — will use in integration tests |

- [ ] Remove `indexmap` from Cargo.toml (or use it in SymbolTable)
- [ ] Decide: add `serde` to AST for `dump-ast --json`, or remove until needed
- [ ] Document: ndarray/criterion kept as future dependencies

#### Step 1.7.8 — CHANGELOG Update

**File:** `docs/CHANGELOG.md`

- [ ] Move "In Progress: Sprint 1.1" to [Unreleased] Added section
- [ ] Add all Phase 1 completions (Lexer, AST, Parser, Environment, Interpreter, CLI/REPL)
- [ ] Add test count, example programs, REPL features

---

### Phase 1 Final Exit Criteria

- [x] `examples/hello.fj` runs: prints "Hello from Fajar Lang!"
- [x] `examples/fibonacci.fj` computes fibonacci(30) correctly
- [x] REPL evaluates expressions interactively
- [ ] All error messages have source highlighting via miette ← **Sprint 2.9**
- [x] `cargo test` — 100% passing (350 tests, target was 100+)
- [x] `cargo clippy -- -D warnings` — clean
- [x] `cargo fmt -- --check` — clean
- [ ] Update `docs/CHANGELOG.md` ← **Step 1.7.8**

---

## Phase 2 — Type System

**Goal:** Static type checking yang menangkap error sebelum runtime.
**Duration:** 6-8 minggu | **Prereq:** Phase 1 complete
**Status:** 🟡 Core done (Sprint 2.1-2.5), gap fixes remaining (Sprint 2.6-2.10)

### Sprint 2.1 — Type Representation ✅

- [x] `src/analyzer/type_check.rs` — Type enum (14 variants: Void, Never, I64, F64, Bool, Char, Str, Array, Tuple, Struct, Enum, Function, Unknown, Named)
- [x] Type environment via SymbolTable (scoped type lookups)
- [x] Type::is_compatible() with Unknown (error recovery) and Never (diverging)

### Sprint 2.2 — Symbol Table & Scope Resolution ✅

- [x] `src/analyzer/scope.rs` — SymbolTable with Vec<Vec<Symbol>> stack
- [x] Symbol struct (name, ty: Type, mutable, span)
- [x] push_scope/pop_scope/define/lookup — innermost-first resolution
- [x] 8 scope tests

### Sprint 2.3 — Type Checker (Expressions) ✅

- [x] TypeChecker struct + two-pass analyze() entry point
- [x] check_expr dispatches all 24 expression types
- [x] check_binary: arithmetic (numeric), comparison (same type), logical (bool), bitwise (integer)
- [x] check_call: arity + argument type check, variadic builtin handling
- [x] 11 builtin function signatures registered

### Sprint 2.4 — Type Checker (Statements & Items) ✅

- [x] Check let/const bindings (type annotation vs inferred)
- [x] Check function definitions (params, return type, body)
- [x] Check struct/enum definitions (first pass registers, second pass checks)
- [x] Check assignment (mutability SE007 + type match SE004)
- [x] Check for/while loops

### Sprint 2.5 — Integration & Pipeline ✅

- [x] Wire analyzer into CLI `check` and `run` commands
- [x] 28 type checker tests + 8 scope tests = 36 total
- [x] CLI catches SE004, SE005, SE007. All examples pass.

### Sprint 2.6 — Distinct Integer/Float Types (GAP FIX)

**Problem:** resolve_type() collapses all integer types to I64, all float types to F64.
Exit criteria requires `let x: i32 = 42; let y: i64 = x` → COMPILE ERROR.

**File:** `src/analyzer/type_check.rs`

**Implement:**
- [ ] Expand Type enum: I8, I16, I32, I64, I128, U8, U16, U32, U64, U128, ISize, USize, F32, F64
- [ ] Update resolve_type() to map each TypeExpr::Simple to its own Type variant
- [ ] Add numeric type promotion rules: no implicit cast, require explicit `as` cast
- [ ] Update is_numeric(), is_integer(), display_name(), is_compatible()
- [ ] Update interpreter Value interaction: runtime uses i64/f64 internally, type checker enforces distinctions
- [ ] Update all existing tests to use correct types

**Tests:**
- `i32_not_assignable_to_i64` — `let x: i32 = 1; let y: i64 = x` → SE004
- `explicit_cast_allowed` — `let y: i64 = x as i64` → OK
- `f32_not_assignable_to_f64` — `let x: f32 = 1.0; let y: f64 = x` → SE004
- `same_type_assignment_ok` — `let x: i32 = 1; let y: i32 = x` → OK
- `integer_literal_defaults_to_i64` — `let x = 42` → x: i64

### Sprint 2.7 — Missing Semantic Error Codes (GAP FIX)

**Problem:** ERROR_CODES.md specifies 12 SE codes. Only 9 implemented (SE001-SE008, SE012).
Missing: SE009 (UnusedVariable), SE010 (UnreachableCode), SE011 (NonExhaustiveMatch).

**File:** `src/analyzer/type_check.rs`

**Implement:**
- [ ] SE009 UnusedVariable: track variable reads, report unused after scope exit (warning)
- [ ] SE010 UnreachableCode: detect statements after return/break in a block (warning)
- [ ] SE011 NonExhaustiveMatch: check match arms cover all cases (or have wildcard `_`)
- [ ] Add SemanticWarning vs SemanticError distinction (or severity field)

**Tests:**
- `unused_variable_warning` — `let x = 42` (never used) → SE009
- `unreachable_code_warning` — `return 1; let x = 2` → SE010
- `non_exhaustive_match_error` — `match x { 0 => "zero" }` (missing default) → SE011
- `wildcard_makes_match_exhaustive` — `match x { 0 => "zero", _ => "other" }` → OK
- `used_variable_no_warning` — `let x = 42; println(x)` → no SE009

### Sprint 2.8 — ScopeKind & Context Tracking (GAP FIX)

**Problem:** ARCHITECTURE.md specifies `Scope { symbols, kind: ScopeKind }`.
Current implementation uses `Vec<Vec<Symbol>>` without scope kind tracking.

**File:** `src/analyzer/scope.rs`

**Implement:**
- [ ] ScopeKind enum: Module, Function, Block, Loop, Kernel, Device, Unsafe
- [ ] Scope struct: { symbols: Vec<Symbol>, kind: ScopeKind }
- [ ] Refactor SymbolTable from Vec<Vec<Symbol>> to Vec<Scope>
- [ ] push_scope(kind: ScopeKind), current_scope_kind() query
- [ ] is_inside(ScopeKind) — walk stack to check if inside specific scope type
- [ ] Validate break/continue: only valid inside Loop scope
- [ ] Validate return: only valid inside Function scope

**Tests:**
- `break_outside_loop_error` — `break` at top level → error
- `continue_outside_loop_error` — `continue` at top level → error
- `return_outside_function_error` — `return 1` at top level → error
- `break_inside_loop_ok` — `while true { break }` → OK
- `return_inside_function_ok` — `fn f() -> i64 { return 1 }` → OK
- `nested_scope_kind_tracking` — function > block > loop → all kinds tracked

### Sprint 2.9 — miette Error Display (GAP FIX)

**Problem:** Cargo.toml has miette dependency but it's unused. All errors display as plain
`eprintln!` text. CLAUDE.md Phase 1 exit criteria requires "source highlighting via miette".

**Files:** `src/analyzer/type_check.rs`, `src/lexer/mod.rs`, `src/parser/mod.rs`, `src/interpreter/eval.rs`, `src/main.rs`

**Implement:**
- [ ] Implement `miette::Diagnostic` for SemanticError (error code, help text, source span labels)
- [ ] Implement `miette::Diagnostic` for LexError
- [ ] Implement `miette::Diagnostic` for ParseError
- [ ] Implement `miette::Diagnostic` for RuntimeError
- [ ] Pass source text through pipeline so errors can reference source (NamedSource)
- [ ] Update CLI to use `miette::set_hook()` + `miette::Report` for error display
- [ ] Test: verify error output shows source line, caret highlighting, help text

**Example expected output:**
```
error[SE004]: type mismatch
  --> test.fj:1:15
   |
1  | let x: i64 = "hello"
   |               ^^^^^^^ expected i64, found str
   |
   = help: consider using to_int() to convert string to integer
```

### Sprint 2.10 — Phase 2 Exit Gate

**Checklist (all must pass):**
- [ ] All 12 SE codes (SE001-SE012) implemented and tested
- [ ] `fj check` shows miette-formatted error output with source highlighting
- [ ] `let x: i32 = 42; let y: i64 = x` → SE004 compile error
- [ ] break/continue validated inside loop scope only
- [ ] return validated inside function scope only
- [ ] `cargo test` all passing, `cargo clippy -- -D warnings` clean
- [ ] Update PLANNING.md — Phase 2 complete

### Deferred to Later Phases

- [~] Type Inference (Hindley-Milner Lite) — constraint generation + unification → Phase 5
- [~] Generic Type Parameters — monomorphization at call sites → Phase 5
- [~] Tensor Type & Shape Checking — TE001-TE008 → Phase 4
- [~] Context Annotation Enforcement — @kernel KE001-KE004, @device DE001-DE003 → Phase 3/4
- [~] borrow_lite.rs — ownership/move semantics ME001-ME008 → Phase 3
- [~] TypedProgram output — analyzer returns typed AST → Phase 5 (compiler backend)

---

## Phase 3 — OS Runtime

**Goal:** OS-level programming capabilities (simulated).
**Duration:** 8-10 minggu | **Prereq:** Phase 2 complete

### Sprint 3.1 — Memory Manager

- [ ] `src/runtime/os/memory.rs` — MemoryManager struct
- [ ] Heap simulation (Vec<u8> backing store)
- [ ] `alloc!(size)` → region allocation
- [ ] `free!(region)` → deallocation
- [ ] Bounds checking, overlapping region detection

### Sprint 3.2 — Virtual Memory & Page Tables

- [ ] VirtAddr, PhysAddr as distinct newtype structs
- [ ] PageFlags (READ, WRITE, EXEC, USER)
- [ ] `map_page!(va, pa, flags)` — create mapping
- [ ] `unmap_page!(va)` — remove mapping
- [ ] Page table data structure (HashMap<VirtAddr, (PhysAddr, PageFlags)>)

### Sprint 3.3 — Interrupt Handling

- [ ] `src/runtime/os/irq.rs` — IrqTable struct
- [ ] `irq_register!(num, handler)` — register handler function
- [ ] `irq_enable!()` / `irq_disable!()`
- [ ] IRQ dispatch simulation
- [ ] Standard IRQ numbers: TIMER=0x20, KEYBOARD=0x21, SERIAL=0x24

### Sprint 3.4 — System Calls

- [ ] `src/runtime/os/syscall.rs` — SyscallTable struct
- [ ] `syscall_define!(num, handler)` — register syscall
- [ ] Standard syscalls: READ=0, WRITE=1, OPEN=2, CLOSE=3, EXIT=60
- [ ] Dispatch mechanism

### Sprint 3.5 — Port I/O Simulation

- [ ] `port_write!(port, value)` — simulate x86 port output
- [ ] `port_read!(port)` — simulate x86 port input
- [ ] Port registry with simulated devices

### Sprint 3.6 — OS Stdlib

- [ ] `stdlib/os.fj` — Fajar Lang OS standard library
- [ ] `src/stdlib/os.rs` — Rust bindings for OS builtins

### Sprint 3.7 — Integration Tests

- [ ] `tests/os_tests.rs`
- [ ] Test: kernel init sequence
- [ ] Test: alloc + free cycle
- [ ] Test: page mapping + protection flags
- [ ] Test: IRQ register + dispatch
- [ ] Test: context violation → compile error

**Phase 3 Exit Criteria:**
- [x] `examples/memory_map.fj` runs correctly
- [x] `@kernel` functions can use OS primitives
- [x] `@device` functions CANNOT use OS primitives (compile error)
- [x] Memory allocator handles alloc/free correctly

### Sprint 3.10 — Phase 3 Gap Fixes

> Post-completion documentation audit revealed these gaps. Must be resolved before Phase 4.

#### 3.10.1 — KE001/KE002 Context Enforcement (HIGH PRIORITY)

**Problem:** `HeapAllocInKernel` (KE001) and `TensorInKernel` (KE002) error variants exist in SemanticError enum, but `check_context_call()` has only a placeholder comment — no actual enforcement logic.

**Fix:**
- In `src/analyzer/type_check.rs` → `check_context_call()`:
  - Add `heap_builtins: HashSet<String>` field to TypeChecker (contains: `push`, `pop`, `to_string`)
  - Inside `if in_kernel {}` block: check if callee is in `heap_builtins` → emit `HeapAllocInKernel`
  - Inside `if in_kernel {}` block: check if callee is in future `ml_builtins` set → emit `TensorInKernel` (placeholder set, populated in Phase 4)
- Tests:
  - `@kernel fn f() { let a = [1,2]; push(a, 3) }` → KE001
  - `@kernel fn f() { to_string(42) }` → KE001
  - Verify KE003/DE001/DE002 still pass (regression)

#### 3.10.2 — Syscall Builtins (MEDIUM PRIORITY)

**Problem:** SyscallTable runtime exists but `syscall_define`/`syscall_dispatch` are not registered as interpreter builtins. Users cannot define or dispatch syscalls from .fj code.

**Fix:**
- In `src/interpreter/eval.rs`:
  - Register `syscall_define` and `syscall_dispatch` in `register_builtins()`
  - `builtin_syscall_define(num: Int, name: Str)` → calls `self.os.syscall.define()`
  - `builtin_syscall_dispatch(num: Int, args: Array)` → calls `self.os.syscall.dispatch()`
- In `src/analyzer/type_check.rs`:
  - Register type signatures for `syscall_define` and `syscall_dispatch`
  - Add both to `os_builtins` set
- Tests in `tests/os_tests.rs`:
  - Define syscall 60 as "exit", dispatch, verify log

#### 3.10.3 — OS Stdlib Files (MEDIUM PRIORITY)

**Problem:** IMPLEMENTATION_PLAN Sprint 3.6 requires `stdlib/os.fj` and `src/stdlib/os.rs` but neither exists.

**Fix:**
- Create `stdlib/os.fj`:
  ```fajar
  // Fajar Lang OS Standard Library
  // Wrappers and documentation for OS primitives
  // These functions are built-in; this file serves as documentation and future extension point
  ```
- Create `src/stdlib/mod.rs` — module declaration
- Create `src/stdlib/os.rs` — re-exports OS builtin names, documents their signatures
- Wire into `src/lib.rs` module tree

#### 3.10.4 — Missing Integration Tests (LOW PRIORITY)

**Problem:** IMPLEMENTATION_PLAN Sprint 3.7 specifies tests not yet written.

**Fix:**
- Add to `tests/os_tests.rs`:
  - `os_kernel_init_sequence` — full alloc→write→read→page_map→free pipeline
  - `os_irq_register_and_dispatch_from_fj` — register handler by name, enable, verify
  - `os_syscall_define_and_dispatch` — define + dispatch from .fj code

#### 3.10.5 — Final Exit Gate

- [ ] All KE/DE error codes enforced with tests
- [ ] Syscall builtins callable from .fj code
- [ ] stdlib/os.fj exists
- [ ] All integration tests passing
- [ ] `cargo test` all pass, `cargo clippy -- -D warnings` clean
- [ ] Update PLANNING.md, CHANGELOG.md

---

## Phase 4 — ML/AI Runtime

**Goal:** Native tensor operations dan automatic differentiation.
**Duration:** 10-14 minggu | **Prereq:** Phase 2 complete (can parallel with Phase 3)

### Sprint 4.1 — TensorValue Struct

- [ ] `src/runtime/ml/tensor.rs` — TensorValue with ndarray backend
- [ ] Shape tracking, dtype, requires_grad flag
- [ ] Tensor creation: zeros, ones, randn, xavier, eye, from_data
- [ ] Display implementation

### Sprint 4.2 — Basic Tensor Operations

- [ ] Element-wise: add, sub, mul, div
- [ ] Matrix multiply: matmul (@ operator)
- [ ] Transpose, reshape, flatten
- [ ] Broadcasting rules
- [ ] Shape validation

### Sprint 4.3 — Activation Functions

- [ ] relu, sigmoid, tanh, softmax, gelu, leaky_relu
- [ ] Each as pure function on TensorValue
- [ ] Numerical stability (softmax log-sum-exp trick)

### Sprint 4.4 — Computation Graph (Autograd)

- [ ] `src/runtime/ml/autograd.rs` — GradFn trait
- [ ] Dynamic tape-based computation graph
- [ ] Track operations for backward pass
- [ ] `requires_grad` flag propagation

### Sprint 4.5 — Backward Pass

- [ ] Implement backward for all ops (add, mul, matmul, relu, sigmoid, etc.)
- [ ] `tensor.backward()` — reverse-mode autodiff
- [ ] `tensor.grad()` — access gradient
- [ ] Chain rule application
- [ ] Numerical gradient check (analytical vs numerical, epsilon=1e-5)

### Sprint 4.6 — Loss Functions

- [ ] MSE loss
- [ ] Cross-entropy loss
- [ ] Binary cross-entropy loss
- [ ] L1 loss
- [ ] All with autograd support

### Sprint 4.7 — Optimizers

- [ ] `src/runtime/ml/optim.rs`
- [ ] SGD (with momentum optional)
- [ ] Adam (lr, beta1, beta2, epsilon)
- [ ] `optimizer.step()` — update parameters
- [ ] `optimizer.zero_grad()` — reset gradients

### Sprint 4.8 — Layer Abstractions

- [ ] Dense/Linear layer
- [ ] Conv2d (basic)
- [ ] Dropout, BatchNorm, LayerNorm
- [ ] Attention (basic self-attention)

### Sprint 4.9 — ML Stdlib & Integration

- [ ] `stdlib/nn.fj` — Fajar Lang ML standard library
- [ ] `src/stdlib/nn.rs` — Rust bindings
- [ ] @device annotation dispatch (CPU)
- [ ] SIMD acceleration via ndarray BLAS features

### Sprint 4.10 — ML Integration Tests

- [ ] `tests/ml_tests.rs`
- [ ] Test: MNIST forward pass (784→128→10)
- [ ] Test: XOR training (verify convergence in <1000 epochs)
- [ ] Test: Gradient correctness (numerical vs analytical for all ops)
- [ ] Test: Autograd memory (no leaks in training loop)

**Phase 4 Exit Criteria:**
- [ ] `examples/mnist_forward.fj` runs correctly
- [ ] XOR problem converges
- [ ] All gradients match numerical gradients (< 1e-4)
- [ ] `@device` functions can use tensor ops
- [ ] `@kernel` functions CANNOT use tensor ops (compile error)

---

## Phase 5 — Tooling & Compiler Backend

**Goal:** Developer experience + native compilation option.
**Duration:** 12+ minggu | **Prereq:** Phase 4 complete

### Sprint 5.1 — Code Formatter

- [ ] `fj fmt` — idempotent code formatter
- [ ] Indentation, spacing, line breaks
- [ ] `fj fmt --check` for CI

### Sprint 5.2 — LSP Server

- [ ] Language Server Protocol implementation
- [ ] Go-to-definition, hover types, diagnostics
- [ ] VS Code extension with syntax highlighting

### Sprint 5.3 — Package Manager

- [ ] `fj.toml` — project manifest
- [ ] `fj add <package>` — add dependency
- [ ] `fj build` — build project
- [ ] Package registry (basic)

### Sprint 5.4 — Bytecode VM

- [ ] Bytecode instruction set
- [ ] Compiler: AST → bytecode
- [ ] VM: execute bytecode
- [ ] Target: 10-100x speedup over tree-walking

### Sprint 5.5 — LLVM Backend (Optional)

- [ ] LLVM IR generation from AST
- [ ] Native binary compilation
- [ ] Link with system libraries

### Sprint 5.6 — GPU Backend

- [ ] wgpu integration for `@device(gpu)`
- [ ] GPU shader generation for tensor ops
- [ ] Runtime dispatch: CPU vs GPU

---

## Phase 6 — Standard Library

**Duration:** 8 minggu | **Prereq:** Phase 5 complete

- [ ] `std::collections` — Vec, HashMap, HashSet, VecDeque, BTreeMap
- [ ] `std::io` — file I/O, stdin/stdout, buffered I/O
- [ ] `std::string` — manipulation, formatting, regex
- [ ] `std::math` — trig, log, constants
- [ ] `std::convert` — From, Into, TryFrom
- [ ] `os::` completion — all OS primitives polished
- [ ] `nn::` completion — all ML ops polished
- [ ] `nn::data` — dataset loading, normalization, train/test split

---

## Phase 7 — Production Hardening

**Duration:** Ongoing | **Prereq:** Phase 6 complete

- [ ] Comprehensive fuzzing (AFL++, libFuzzer)
- [ ] Performance benchmarks & optimization
- [ ] Memory safety audit (cargo-audit, Miri)
- [ ] Security review (especially OS primitives)
- [ ] Documentation site (mdBook or similar)
- [ ] Example: minimal OS kernel in Fajar Lang
- [ ] Example: MNIST classifier with training
- [ ] Example: AI-powered kernel monitor (cross-domain)
- [ ] Compliance assessment: MISRA, DO-178C, IEC 62304, ISO 26262

---

## Version Targets

| Version | Phase | Key Feature |
|---------|-------|-------------|
| 0.1.0 | Phase 1 | Working interpreter, REPL, basic programs |
| 0.2.0 | Phase 2 | Type system, context annotations enforced |
| 0.3.0 | Phase 3 | OS runtime (memory, IRQ, syscall) |
| 0.4.0 | Phase 4 | ML runtime (tensor, autograd, training) |
| 1.0.0 | Phase 5-7 | Bytecode VM, LLVM, GPU, full stdlib, production-ready |

---

## Task Dependencies Graph

```
Phase 0: Scaffolding
    │
    ▼
Sprint 1.1: Lexer ──► Sprint 1.2: AST ──► Sprint 1.3: Parser
                                                  │
                                                  ▼
                              Sprint 1.4: Env/Values ──► Sprint 1.5: Interpreter
                                                              │
                                                              ▼
                                                    Sprint 1.6: CLI/REPL
                                                              │
                                                              ▼
                                              ┌── Sprint 2.1-2.8: Type System ──┐
                                              │                                  │
                                              ▼                                  ▼
                                    Phase 3: OS Runtime            Phase 4: ML Runtime
                                    (can run in parallel)          (can run in parallel)
                                              │                                  │
                                              └──────────┬───────────────────────┘
                                                         ▼
                                              Phase 5: Tooling + Compiler
                                                         │
                                                         ▼
                                              Phase 6: Standard Library
                                                         │
                                                         ▼
                                              Phase 7: Production Hardening
```

---

## Metrics & Tracking

| Phase | Est. Tasks | Tests Target | Status |
|-------|-----------|-------------|--------|
| 0 | 5 | 0 | NEXT |
| 1 | ~50 | 150+ | PENDING |
| 2 | ~30 | 100+ | LOCKED |
| 3 | ~25 | 80+ | LOCKED |
| 4 | ~35 | 120+ | LOCKED |
| 5 | ~30 | 80+ | LOCKED |
| 6 | ~20 | 60+ | LOCKED |
| 7 | ~15 | 50+ | LOCKED |
| **Total** | **~210** | **~640+** | |

---

*Implementation Plan Version: 1.0 | Created: 2026-03-05*
*Source: Synthesized from 24 reference documents in docs/*
