//! Token types for the Fajar Lang lexer.
//!
//! Defines [`Token`], [`TokenKind`], and [`Span`] used throughout the compiler pipeline.
//! Every token carries its kind, byte-offset span, and line/column info for error reporting.

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

/// Byte-offset range in the source string.
///
/// `start` is inclusive, `end` is exclusive: `source[start..end]` gives the token text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
}

impl Span {
    /// Creates a new span from start (inclusive) to end (exclusive).
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the length of this span in bytes.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns `true` if the span has zero length.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Merges two spans into one that covers both.
    pub fn merge(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

/// A single token produced by the lexer.
///
/// Tokens are the atomic units of the Fajar Lang source code.
/// Each token carries its kind, source location (span), and line/column info.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The kind of token (keyword, literal, operator, etc.).
    pub kind: TokenKind,
    /// Byte offset range in the source string.
    pub span: Span,
    /// 1-indexed line number.
    pub line: u32,
    /// 1-indexed column number (in characters, not bytes).
    pub col: u32,
}

impl Token {
    /// Creates a new token.
    pub fn new(kind: TokenKind, span: Span, line: u32, col: u32) -> Self {
        Self {
            kind,
            span,
            line,
            col,
        }
    }
}

/// A part of an f-string literal.
#[derive(Debug, Clone, PartialEq)]
pub enum FStringPart {
    /// Literal text segment.
    Literal(String),
    /// Expression source code to be parsed and evaluated.
    Expr(String),
}

/// All possible token kinds in the Fajar Lang lexer.
///
/// Organized into: keywords, type keywords, ML keywords, OS keywords,
/// annotations, operators, delimiters, literals, identifiers, and EOF.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ── Control Flow Keywords ──────────────────────────────────────────
    /// `if`
    If,
    /// `else`
    Else,
    /// `match`
    Match,
    /// `while`
    While,
    /// `for`
    For,
    /// `loop`
    Loop,
    /// `in`
    In,
    /// `return`
    Return,
    /// `break`
    Break,
    /// `continue`
    Continue,
    /// `async`
    Async,
    /// `await`
    Await,

    // ── Declaration Keywords ───────────────────────────────────────────
    /// `let`
    Let,
    /// `mut`
    Mut,
    /// `fn`
    Fn,
    /// `struct`
    Struct,
    /// `enum`
    Enum,
    /// `union`
    Union,
    /// `impl`
    Impl,
    /// `trait`
    Trait,
    /// `type`
    Type,
    /// `const`
    Const,
    /// `dyn`
    Dyn,

    // ── Module Keywords ────────────────────────────────────────────────
    /// `use`
    Use,
    /// `mod`
    Mod,
    /// `pub`
    Pub,
    /// `extern`
    Extern,
    /// `as`
    As,
    /// `where`
    Where,

    // ── Literal Keywords ───────────────────────────────────────────────
    /// `true`
    True,
    /// `false`
    False,
    /// `null`
    Null,

    // ── Built-in Type Keywords ─────────────────────────────────────────
    /// `bool`
    BoolType,
    /// `i8`
    I8,
    /// `i16`
    I16,
    /// `i32`
    I32,
    /// `i64`
    I64,
    /// `i128`
    I128,
    /// `isize`
    Isize,
    /// `u8`
    U8,
    /// `u16`
    U16,
    /// `u32`
    U32,
    /// `u64`
    U64,
    /// `u128`
    U128,
    /// `usize`
    Usize,
    /// `f16`
    F16Type,
    /// `bf16`
    Bf16Type,
    /// `f32`
    F32Type,
    /// `f64`
    F64Type,
    /// `str`
    StrType,
    /// `char`
    CharType,
    /// `void`
    Void,
    /// `never`
    Never,

    // ── ML Keywords ────────────────────────────────────────────────────
    /// `tensor`
    Tensor,
    /// `grad`
    Grad,
    /// `loss`
    Loss,
    /// `layer`
    Layer,
    /// `model`
    Model,

    // ── OS Keywords ────────────────────────────────────────────────────
    /// `ptr`
    Ptr,
    /// `addr`
    Addr,
    /// `page`
    Page,
    /// `region`
    Region,
    /// `irq`
    Irq,
    /// `syscall`
    Syscall,

    // ── Annotations ────────────────────────────────────────────────────
    /// `@kernel`
    AtKernel,
    /// `@device`
    AtDevice,
    /// `@npu`
    AtNpu,
    /// `@safe`
    AtSafe,
    /// `@unsafe`
    AtUnsafe,
    /// `@ffi`
    AtFfi,
    /// `@panic_handler`
    AtPanicHandler,
    /// `@no_std`
    AtNoStd,
    /// `@entry`
    AtEntry,
    /// `@repr_c`
    AtReprC,
    /// `@repr_packed`
    AtReprPacked,
    /// `@simd`
    AtSimd,
    /// `@section`
    AtSection,
    /// `@test`
    AtTest,
    /// `@should_panic`
    AtShouldPanic,
    /// `@ignore`
    AtIgnore,
    /// `@infer`
    AtInfer,
    /// `@interrupt`
    AtInterrupt,

    // ── Arithmetic Operators ───────────────────────────────────────────
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `**`
    StarStar,
    /// `@` (matrix multiply)
    At,

    // ── Comparison Operators ───────────────────────────────────────────
    /// `==`
    EqEq,
    /// `!=`
    BangEq,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    LtEq,
    /// `>=`
    GtEq,

    // ── Logical Operators ──────────────────────────────────────────────
    /// `&&`
    AmpAmp,
    /// `||`
    PipePipe,
    /// `!`
    Bang,

    // ── Bitwise Operators ──────────────────────────────────────────────
    /// `&`
    Amp,
    /// `|`
    Pipe,
    /// `^`
    Caret,
    /// `~`
    Tilde,
    /// `<<`
    LtLt,
    /// `>>`
    GtGt,

    // ── Assignment Operators ───────────────────────────────────────────
    /// `=`
    Eq,
    /// `+=`
    PlusEq,
    /// `-=`
    MinusEq,
    /// `*=`
    StarEq,
    /// `/=`
    SlashEq,
    /// `%=`
    PercentEq,
    /// `&=`
    AmpEq,
    /// `|=`
    PipeEq,
    /// `^=`
    CaretEq,
    /// `<<=`
    LtLtEq,
    /// `>>=`
    GtGtEq,

    // ── Range Operators ────────────────────────────────────────────────
    /// `..`
    DotDot,
    /// `..=`
    DotDotEq,

    // ── Pipeline Operator ──────────────────────────────────────────────
    /// `|>`
    PipeGt,

    // ── Delimiters ─────────────────────────────────────────────────────
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,

    // ── Punctuation ────────────────────────────────────────────────────
    /// `;`
    Semi,
    /// `:`
    Colon,
    /// `::`
    ColonColon,
    /// `,`
    Comma,
    /// `.`
    Dot,
    /// `->`
    Arrow,
    /// `=>`
    FatArrow,
    /// `?`
    Question,

    // ── Doc Comments ──────────────────────────────────────────────────
    /// A `///` doc comment line (content after `///`, trimmed of leading space).
    DocComment(String),

    /// An f-string literal: `f"Hello {name}"`.
    /// Parts alternate between literal text and expression source code.
    FStringLit(Vec<FStringPart>),

    // ── Literals ───────────────────────────────────────────────────────
    /// Integer literal (e.g. `42`, `0xFF`, `0b1010`, `0o17`).
    IntLit(i64),
    /// Float literal (e.g. `3.14`, `1.0e-4`).
    FloatLit(f64),
    /// String literal (e.g. `"hello"`).
    StringLit(String),
    /// Raw string literal (e.g. `r"raw \n string"`).
    RawStringLit(String),
    /// Character literal (e.g. `'a'`).
    CharLit(char),

    // ── Identifiers ────────────────────────────────────────────────────
    /// An identifier (e.g. `foo`, `my_var`, `MyStruct`).
    Ident(String),

    // ── Lifetime ─────────────────────────────────────────────────────
    /// A lifetime annotation (e.g. `'a`, `'static`, `'_`).
    Lifetime(String),

    // ── Special ────────────────────────────────────────────────────────
    /// End of file marker. Always the last token in the stream.
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Keywords
            TokenKind::If => write!(f, "if"),
            TokenKind::Else => write!(f, "else"),
            TokenKind::Match => write!(f, "match"),
            TokenKind::While => write!(f, "while"),
            TokenKind::For => write!(f, "for"),
            TokenKind::Loop => write!(f, "loop"),
            TokenKind::In => write!(f, "in"),
            TokenKind::Return => write!(f, "return"),
            TokenKind::Break => write!(f, "break"),
            TokenKind::Continue => write!(f, "continue"),
            TokenKind::Async => write!(f, "async"),
            TokenKind::Await => write!(f, "await"),
            TokenKind::Let => write!(f, "let"),
            TokenKind::Mut => write!(f, "mut"),
            TokenKind::Fn => write!(f, "fn"),
            TokenKind::Struct => write!(f, "struct"),
            TokenKind::Enum => write!(f, "enum"),
            TokenKind::Union => write!(f, "union"),
            TokenKind::Impl => write!(f, "impl"),
            TokenKind::Trait => write!(f, "trait"),
            TokenKind::Type => write!(f, "type"),
            TokenKind::Const => write!(f, "const"),
            TokenKind::Dyn => write!(f, "dyn"),
            TokenKind::Use => write!(f, "use"),
            TokenKind::Mod => write!(f, "mod"),
            TokenKind::Pub => write!(f, "pub"),
            TokenKind::Extern => write!(f, "extern"),
            TokenKind::As => write!(f, "as"),
            TokenKind::Where => write!(f, "where"),
            TokenKind::True => write!(f, "true"),
            TokenKind::False => write!(f, "false"),
            TokenKind::Null => write!(f, "null"),
            // Type keywords
            TokenKind::BoolType => write!(f, "bool"),
            TokenKind::I8 => write!(f, "i8"),
            TokenKind::I16 => write!(f, "i16"),
            TokenKind::I32 => write!(f, "i32"),
            TokenKind::I64 => write!(f, "i64"),
            TokenKind::I128 => write!(f, "i128"),
            TokenKind::Isize => write!(f, "isize"),
            TokenKind::U8 => write!(f, "u8"),
            TokenKind::U16 => write!(f, "u16"),
            TokenKind::U32 => write!(f, "u32"),
            TokenKind::U64 => write!(f, "u64"),
            TokenKind::U128 => write!(f, "u128"),
            TokenKind::Usize => write!(f, "usize"),
            TokenKind::F16Type => write!(f, "f16"),
            TokenKind::Bf16Type => write!(f, "bf16"),
            TokenKind::F32Type => write!(f, "f32"),
            TokenKind::F64Type => write!(f, "f64"),
            TokenKind::StrType => write!(f, "str"),
            TokenKind::CharType => write!(f, "char"),
            TokenKind::Void => write!(f, "void"),
            TokenKind::Never => write!(f, "never"),
            // ML keywords
            TokenKind::Tensor => write!(f, "tensor"),
            TokenKind::Grad => write!(f, "grad"),
            TokenKind::Loss => write!(f, "loss"),
            TokenKind::Layer => write!(f, "layer"),
            TokenKind::Model => write!(f, "model"),
            // OS keywords
            TokenKind::Ptr => write!(f, "ptr"),
            TokenKind::Addr => write!(f, "addr"),
            TokenKind::Page => write!(f, "page"),
            TokenKind::Region => write!(f, "region"),
            TokenKind::Irq => write!(f, "irq"),
            TokenKind::Syscall => write!(f, "syscall"),
            // Annotations
            TokenKind::AtKernel => write!(f, "@kernel"),
            TokenKind::AtDevice => write!(f, "@device"),
            TokenKind::AtNpu => write!(f, "@npu"),
            TokenKind::AtSafe => write!(f, "@safe"),
            TokenKind::AtUnsafe => write!(f, "@unsafe"),
            TokenKind::AtFfi => write!(f, "@ffi"),
            TokenKind::AtPanicHandler => write!(f, "@panic_handler"),
            TokenKind::AtNoStd => write!(f, "@no_std"),
            TokenKind::AtEntry => write!(f, "@entry"),
            TokenKind::AtReprC => write!(f, "@repr_c"),
            TokenKind::AtReprPacked => write!(f, "@repr_packed"),
            TokenKind::AtSimd => write!(f, "@simd"),
            TokenKind::AtTest => write!(f, "@test"),
            TokenKind::AtShouldPanic => write!(f, "@should_panic"),
            TokenKind::AtIgnore => write!(f, "@ignore"),
            TokenKind::AtSection => write!(f, "@section"),
            // Operators
            TokenKind::Plus => write!(f, "+"),
            TokenKind::Minus => write!(f, "-"),
            TokenKind::Star => write!(f, "*"),
            TokenKind::Slash => write!(f, "/"),
            TokenKind::Percent => write!(f, "%"),
            TokenKind::StarStar => write!(f, "**"),
            TokenKind::At => write!(f, "@"),
            TokenKind::EqEq => write!(f, "=="),
            TokenKind::BangEq => write!(f, "!="),
            TokenKind::Lt => write!(f, "<"),
            TokenKind::Gt => write!(f, ">"),
            TokenKind::LtEq => write!(f, "<="),
            TokenKind::GtEq => write!(f, ">="),
            TokenKind::AmpAmp => write!(f, "&&"),
            TokenKind::PipePipe => write!(f, "||"),
            TokenKind::Bang => write!(f, "!"),
            TokenKind::Amp => write!(f, "&"),
            TokenKind::Pipe => write!(f, "|"),
            TokenKind::Caret => write!(f, "^"),
            TokenKind::Tilde => write!(f, "~"),
            TokenKind::LtLt => write!(f, "<<"),
            TokenKind::GtGt => write!(f, ">>"),
            TokenKind::Eq => write!(f, "="),
            TokenKind::PlusEq => write!(f, "+="),
            TokenKind::MinusEq => write!(f, "-="),
            TokenKind::StarEq => write!(f, "*="),
            TokenKind::SlashEq => write!(f, "/="),
            TokenKind::PercentEq => write!(f, "%="),
            TokenKind::AmpEq => write!(f, "&="),
            TokenKind::PipeEq => write!(f, "|="),
            TokenKind::CaretEq => write!(f, "^="),
            TokenKind::LtLtEq => write!(f, "<<="),
            TokenKind::GtGtEq => write!(f, ">>="),
            TokenKind::DotDot => write!(f, ".."),
            TokenKind::DotDotEq => write!(f, "..="),
            TokenKind::PipeGt => write!(f, "|>"),
            // Delimiters
            TokenKind::LParen => write!(f, "("),
            TokenKind::RParen => write!(f, ")"),
            TokenKind::LBrace => write!(f, "{{"),
            TokenKind::RBrace => write!(f, "}}"),
            TokenKind::LBracket => write!(f, "["),
            TokenKind::RBracket => write!(f, "]"),
            // Punctuation
            TokenKind::Semi => write!(f, ";"),
            TokenKind::Colon => write!(f, ":"),
            TokenKind::ColonColon => write!(f, "::"),
            TokenKind::Comma => write!(f, ","),
            TokenKind::Dot => write!(f, "."),
            TokenKind::Arrow => write!(f, "->"),
            TokenKind::FatArrow => write!(f, "=>"),
            TokenKind::Question => write!(f, "?"),
            // Doc comments
            TokenKind::DocComment(s) => write!(f, "/// {s}"),
            TokenKind::FStringLit(_) => write!(f, "f\"...\""),
            // Literals
            TokenKind::IntLit(v) => write!(f, "{v}"),
            TokenKind::FloatLit(v) => write!(f, "{v}"),
            TokenKind::StringLit(s) => write!(f, "\"{s}\""),
            TokenKind::RawStringLit(s) => write!(f, "r\"{s}\""),
            TokenKind::CharLit(c) => write!(f, "'{c}'"),
            // Identifier
            TokenKind::Ident(name) => write!(f, "{name}"),
            // Lifetime
            TokenKind::Lifetime(name) => write!(f, "'{name}"),
            // Special
            TokenKind::AtInfer => write!(f, "@infer"),
            TokenKind::AtInterrupt => write!(f, "@interrupt"),
            TokenKind::Eof => write!(f, "EOF"),
        }
    }
}

/// Static keyword lookup table.
///
/// Maps keyword strings to their [`TokenKind`]. Used by the lexer to distinguish
/// keywords from identifiers.
pub static KEYWORDS: LazyLock<HashMap<&'static str, TokenKind>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // Control flow
    m.insert("if", TokenKind::If);
    m.insert("else", TokenKind::Else);
    m.insert("match", TokenKind::Match);
    m.insert("while", TokenKind::While);
    m.insert("for", TokenKind::For);
    m.insert("loop", TokenKind::Loop);
    m.insert("in", TokenKind::In);
    m.insert("return", TokenKind::Return);
    m.insert("break", TokenKind::Break);
    m.insert("continue", TokenKind::Continue);
    m.insert("async", TokenKind::Async);
    m.insert("await", TokenKind::Await);
    // Declarations
    m.insert("let", TokenKind::Let);
    m.insert("mut", TokenKind::Mut);
    m.insert("fn", TokenKind::Fn);
    m.insert("struct", TokenKind::Struct);
    m.insert("enum", TokenKind::Enum);
    m.insert("union", TokenKind::Union);
    m.insert("impl", TokenKind::Impl);
    m.insert("trait", TokenKind::Trait);
    m.insert("type", TokenKind::Type);
    m.insert("const", TokenKind::Const);
    m.insert("dyn", TokenKind::Dyn);
    // Module
    m.insert("use", TokenKind::Use);
    m.insert("mod", TokenKind::Mod);
    m.insert("pub", TokenKind::Pub);
    m.insert("extern", TokenKind::Extern);
    m.insert("as", TokenKind::As);
    m.insert("where", TokenKind::Where);
    // Literals
    m.insert("true", TokenKind::True);
    m.insert("false", TokenKind::False);
    m.insert("null", TokenKind::Null);
    // Built-in types
    m.insert("bool", TokenKind::BoolType);
    m.insert("i8", TokenKind::I8);
    m.insert("i16", TokenKind::I16);
    m.insert("i32", TokenKind::I32);
    m.insert("i64", TokenKind::I64);
    m.insert("i128", TokenKind::I128);
    m.insert("isize", TokenKind::Isize);
    m.insert("u8", TokenKind::U8);
    m.insert("u16", TokenKind::U16);
    m.insert("u32", TokenKind::U32);
    m.insert("u64", TokenKind::U64);
    m.insert("u128", TokenKind::U128);
    m.insert("usize", TokenKind::Usize);
    m.insert("f16", TokenKind::F16Type);
    m.insert("bf16", TokenKind::Bf16Type);
    m.insert("f32", TokenKind::F32Type);
    m.insert("f64", TokenKind::F64Type);
    m.insert("str", TokenKind::StrType);
    m.insert("char", TokenKind::CharType);
    m.insert("void", TokenKind::Void);
    m.insert("never", TokenKind::Never);
    // ML keywords
    m.insert("tensor", TokenKind::Tensor);
    m.insert("grad", TokenKind::Grad);
    m.insert("loss", TokenKind::Loss);
    m.insert("layer", TokenKind::Layer);
    m.insert("model", TokenKind::Model);
    // OS keywords
    m.insert("ptr", TokenKind::Ptr);
    m.insert("addr", TokenKind::Addr);
    m.insert("page", TokenKind::Page);
    m.insert("region", TokenKind::Region);
    m.insert("irq", TokenKind::Irq);
    m.insert("syscall", TokenKind::Syscall);
    m
});

/// Annotation lookup table.
///
/// Maps annotation names (without `@` prefix) to their [`TokenKind`].
pub static ANNOTATIONS: LazyLock<HashMap<&'static str, TokenKind>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("kernel", TokenKind::AtKernel);
    m.insert("device", TokenKind::AtDevice);
    m.insert("npu", TokenKind::AtNpu);
    m.insert("safe", TokenKind::AtSafe);
    m.insert("unsafe", TokenKind::AtUnsafe);
    m.insert("ffi", TokenKind::AtFfi);
    m.insert("panic_handler", TokenKind::AtPanicHandler);
    m.insert("no_std", TokenKind::AtNoStd);
    m.insert("entry", TokenKind::AtEntry);
    m.insert("repr_c", TokenKind::AtReprC);
    m.insert("repr_packed", TokenKind::AtReprPacked);
    m.insert("simd", TokenKind::AtSimd);
    m.insert("test", TokenKind::AtTest);
    m.insert("should_panic", TokenKind::AtShouldPanic);
    m.insert("ignore", TokenKind::AtIgnore);
    m.insert("section", TokenKind::AtSection);
    m.insert("infer", TokenKind::AtInfer);
    m.insert("interrupt", TokenKind::AtInterrupt);
    m
});

/// Looks up a keyword by name, returning the corresponding [`TokenKind`] if found.
pub fn lookup_keyword(name: &str) -> Option<TokenKind> {
    KEYWORDS.get(name).cloned()
}

/// Looks up an annotation by name (without `@` prefix).
pub fn lookup_annotation(name: &str) -> Option<TokenKind> {
    ANNOTATIONS.get(name).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_new_and_len() {
        let span = Span::new(5, 10);
        assert_eq!(span.start, 5);
        assert_eq!(span.end, 10);
        assert_eq!(span.len(), 5);
        assert!(!span.is_empty());
    }

    #[test]
    fn span_empty() {
        let span = Span::new(3, 3);
        assert!(span.is_empty());
        assert_eq!(span.len(), 0);
    }

    #[test]
    fn token_new_creates_token() {
        let token = Token::new(TokenKind::Let, Span::new(0, 3), 1, 1);
        assert_eq!(token.kind, TokenKind::Let);
        assert_eq!(token.span, Span::new(0, 3));
        assert_eq!(token.line, 1);
        assert_eq!(token.col, 1);
    }

    #[test]
    fn lookup_keyword_returns_keyword_for_known_words() {
        assert_eq!(lookup_keyword("let"), Some(TokenKind::Let));
        assert_eq!(lookup_keyword("fn"), Some(TokenKind::Fn));
        assert_eq!(lookup_keyword("if"), Some(TokenKind::If));
        assert_eq!(lookup_keyword("return"), Some(TokenKind::Return));
        assert_eq!(lookup_keyword("true"), Some(TokenKind::True));
        assert_eq!(lookup_keyword("false"), Some(TokenKind::False));
        assert_eq!(lookup_keyword("null"), Some(TokenKind::Null));
    }

    #[test]
    fn lookup_keyword_returns_none_for_identifiers() {
        assert_eq!(lookup_keyword("foo"), None);
        assert_eq!(lookup_keyword("myVar"), None);
        assert_eq!(lookup_keyword(""), None);
    }

    #[test]
    fn lookup_keyword_covers_all_type_keywords() {
        assert_eq!(lookup_keyword("bool"), Some(TokenKind::BoolType));
        assert_eq!(lookup_keyword("i8"), Some(TokenKind::I8));
        assert_eq!(lookup_keyword("i16"), Some(TokenKind::I16));
        assert_eq!(lookup_keyword("i32"), Some(TokenKind::I32));
        assert_eq!(lookup_keyword("i64"), Some(TokenKind::I64));
        assert_eq!(lookup_keyword("i128"), Some(TokenKind::I128));
        assert_eq!(lookup_keyword("isize"), Some(TokenKind::Isize));
        assert_eq!(lookup_keyword("u8"), Some(TokenKind::U8));
        assert_eq!(lookup_keyword("u16"), Some(TokenKind::U16));
        assert_eq!(lookup_keyword("u32"), Some(TokenKind::U32));
        assert_eq!(lookup_keyword("u64"), Some(TokenKind::U64));
        assert_eq!(lookup_keyword("u128"), Some(TokenKind::U128));
        assert_eq!(lookup_keyword("usize"), Some(TokenKind::Usize));
        assert_eq!(lookup_keyword("f32"), Some(TokenKind::F32Type));
        assert_eq!(lookup_keyword("f64"), Some(TokenKind::F64Type));
        assert_eq!(lookup_keyword("str"), Some(TokenKind::StrType));
        assert_eq!(lookup_keyword("char"), Some(TokenKind::CharType));
        assert_eq!(lookup_keyword("void"), Some(TokenKind::Void));
        assert_eq!(lookup_keyword("never"), Some(TokenKind::Never));
    }

    #[test]
    fn lookup_keyword_covers_ml_keywords() {
        assert_eq!(lookup_keyword("tensor"), Some(TokenKind::Tensor));
        assert_eq!(lookup_keyword("grad"), Some(TokenKind::Grad));
        assert_eq!(lookup_keyword("loss"), Some(TokenKind::Loss));
        assert_eq!(lookup_keyword("layer"), Some(TokenKind::Layer));
        assert_eq!(lookup_keyword("model"), Some(TokenKind::Model));
    }

    #[test]
    fn lookup_keyword_covers_os_keywords() {
        assert_eq!(lookup_keyword("ptr"), Some(TokenKind::Ptr));
        assert_eq!(lookup_keyword("addr"), Some(TokenKind::Addr));
        assert_eq!(lookup_keyword("page"), Some(TokenKind::Page));
        assert_eq!(lookup_keyword("region"), Some(TokenKind::Region));
        assert_eq!(lookup_keyword("irq"), Some(TokenKind::Irq));
        assert_eq!(lookup_keyword("syscall"), Some(TokenKind::Syscall));
    }

    #[test]
    fn lookup_annotation_returns_annotation_for_known_names() {
        assert_eq!(lookup_annotation("kernel"), Some(TokenKind::AtKernel));
        assert_eq!(lookup_annotation("device"), Some(TokenKind::AtDevice));
        assert_eq!(lookup_annotation("npu"), Some(TokenKind::AtNpu));
        assert_eq!(lookup_annotation("safe"), Some(TokenKind::AtSafe));
        assert_eq!(lookup_annotation("unsafe"), Some(TokenKind::AtUnsafe));
        assert_eq!(lookup_annotation("ffi"), Some(TokenKind::AtFfi));
        assert_eq!(lookup_annotation("test"), Some(TokenKind::AtTest));
        assert_eq!(
            lookup_annotation("should_panic"),
            Some(TokenKind::AtShouldPanic)
        );
        assert_eq!(lookup_annotation("ignore"), Some(TokenKind::AtIgnore));
    }

    #[test]
    fn lookup_annotation_returns_none_for_unknown() {
        assert_eq!(lookup_annotation("unknown"), None);
        assert_eq!(lookup_annotation(""), None);
    }

    #[test]
    fn token_kind_display_keywords() {
        assert_eq!(format!("{}", TokenKind::Let), "let");
        assert_eq!(format!("{}", TokenKind::Fn), "fn");
        assert_eq!(format!("{}", TokenKind::Return), "return");
    }

    #[test]
    fn token_kind_display_operators() {
        assert_eq!(format!("{}", TokenKind::Plus), "+");
        assert_eq!(format!("{}", TokenKind::StarStar), "**");
        assert_eq!(format!("{}", TokenKind::PipeGt), "|>");
        assert_eq!(format!("{}", TokenKind::Arrow), "->");
        assert_eq!(format!("{}", TokenKind::FatArrow), "=>");
    }

    #[test]
    fn token_kind_display_literals() {
        assert_eq!(format!("{}", TokenKind::IntLit(42)), "42");
        assert_eq!(format!("{}", TokenKind::FloatLit(3.14)), "3.14");
        assert_eq!(format!("{}", TokenKind::StringLit("hi".into())), "\"hi\"");
        assert_eq!(format!("{}", TokenKind::CharLit('a')), "'a'");
    }

    #[test]
    fn token_kind_display_lifetime() {
        assert_eq!(format!("{}", TokenKind::Lifetime("a".into())), "'a");
        assert_eq!(
            format!("{}", TokenKind::Lifetime("static".into())),
            "'static"
        );
        assert_eq!(format!("{}", TokenKind::Lifetime("_".into())), "'_");
    }

    #[test]
    fn token_kind_display_eof() {
        assert_eq!(format!("{}", TokenKind::Eof), "EOF");
    }

    #[test]
    fn token_kind_display_ident() {
        assert_eq!(format!("{}", TokenKind::Ident("foo".into())), "foo");
    }
}
