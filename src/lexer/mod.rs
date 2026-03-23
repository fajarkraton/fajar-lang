//! Fajar Lang lexer — converts source code to tokens.
//!
//! Entry point: [`tokenize`] takes a `&str` and returns `Result<Vec<Token>, Vec<LexError>>`.
//!
//! # Example
//!
//! ```
//! use fajar_lang::lexer::{tokenize, token::TokenKind};
//!
//! let tokens = tokenize("let x = 42").unwrap();
//! assert_eq!(tokens[0].kind, TokenKind::Let);
//! ```

pub mod cursor;
pub mod token;

use cursor::Cursor;
use token::{Span, Token, TokenKind, lookup_annotation, lookup_keyword};

use thiserror::Error;

/// Errors produced during tokenization.
///
/// Error codes follow the LE (Lex Error) numbering from ERROR_CODES.md.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum LexError {
    /// LE001: Unexpected character.
    #[error("[LE001] unexpected character '{ch}' at {line}:{col}")]
    UnexpectedChar {
        /// The offending character.
        ch: char,
        /// 1-indexed line number.
        line: u32,
        /// 1-indexed column number.
        col: u32,
        /// Byte offset span.
        span: Span,
    },

    /// LE002: Unterminated string literal.
    #[error("[LE002] unterminated string literal at {line}:{col}")]
    UnterminatedString {
        /// 1-indexed line number where the string started.
        line: u32,
        /// 1-indexed column number where the string started.
        col: u32,
        /// Byte offset span.
        span: Span,
    },

    /// LE003: Unterminated block comment.
    #[error("[LE003] unterminated block comment at {line}:{col}")]
    UnterminatedBlockComment {
        /// 1-indexed line number where the comment started.
        line: u32,
        /// 1-indexed column number where the comment started.
        col: u32,
        /// Byte offset span.
        span: Span,
    },

    /// LE004: Invalid number literal.
    #[error("[LE004] invalid number literal at {line}:{col}: {reason}")]
    InvalidNumber {
        /// Description of what's wrong.
        reason: String,
        /// 1-indexed line number.
        line: u32,
        /// 1-indexed column number.
        col: u32,
        /// Byte offset span.
        span: Span,
    },

    /// LE005: Invalid escape sequence.
    #[error("[LE005] invalid escape sequence '\\{ch}' at {line}:{col}")]
    InvalidEscape {
        /// The character after the backslash.
        ch: char,
        /// 1-indexed line number.
        line: u32,
        /// 1-indexed column number.
        col: u32,
        /// Byte offset span.
        span: Span,
    },

    /// LE006: Number overflow.
    #[error("[LE006] number literal overflow at {line}:{col}")]
    NumberOverflow {
        /// 1-indexed line number.
        line: u32,
        /// 1-indexed column number.
        col: u32,
        /// Byte offset span.
        span: Span,
    },

    /// LE007: Empty character literal.
    #[error("[LE007] empty character literal at {line}:{col}")]
    EmptyCharLiteral {
        /// 1-indexed line number.
        line: u32,
        /// 1-indexed column number.
        col: u32,
        /// Byte offset span.
        span: Span,
    },

    /// LE008: Multi-character character literal.
    #[error("[LE008] multi-character character literal at {line}:{col}")]
    MultiCharLiteral {
        /// 1-indexed line number.
        line: u32,
        /// 1-indexed column number.
        col: u32,
        /// Byte offset span.
        span: Span,
    },

    /// Invalid character literal (unterminated or bad escape).
    #[error("invalid character literal at {line}:{col}")]
    InvalidCharLiteral {
        /// 1-indexed line number.
        line: u32,
        /// 1-indexed column number.
        col: u32,
        /// Byte offset span.
        span: Span,
    },

    /// Unknown annotation.
    #[error("unknown annotation '@{name}' at {line}:{col}")]
    UnknownAnnotation {
        /// The annotation name (without @).
        name: String,
        /// 1-indexed line number.
        line: u32,
        /// 1-indexed column number.
        col: u32,
        /// Byte offset span.
        span: Span,
    },
}

/// A comment extracted from source code, preserving position for formatter reattachment.
#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    /// Byte offset where the comment starts in the source.
    pub pos: usize,
    /// The full comment text (including `//` or `/* */` delimiters).
    pub text: String,
    /// Whether this is a doc comment (`///` or `//!`).
    pub is_doc: bool,
    /// Whether this is a block comment (`/* */`).
    pub is_block: bool,
}

/// Tokenizes Fajar Lang source code into a vector of tokens.
///
/// Returns all tokens including an [`TokenKind::Eof`] as the last token.
/// Collects all errors encountered during tokenization.
///
/// # Arguments
///
/// * `source` - The source code to tokenize.
///
/// # Returns
///
/// * `Ok(Vec<Token>)` - All tokens with EOF at the end.
/// * `Err(Vec<LexError>)` - All errors encountered (may still have partial tokens).
///
/// # Examples
///
/// ```
/// use fajar_lang::lexer::tokenize;
///
/// let tokens = tokenize("42").unwrap();
/// assert_eq!(tokens.len(), 2); // IntLit(42), Eof
/// ```
pub fn tokenize(source: &str) -> Result<Vec<Token>, Vec<LexError>> {
    let mut cursor = Cursor::new(source);
    let mut tokens = Vec::new();
    let mut errors = Vec::new();

    while !cursor.is_eof() {
        skip_whitespace_and_comments(&mut cursor, &mut errors);

        if cursor.is_eof() {
            break;
        }

        let start_pos = cursor.pos();
        let start_line = cursor.line();
        let start_col = cursor.col();

        // Check for doc comment: `///` (not `////`)
        if cursor.peek() == Some('/')
            && cursor.peek_nth(1) == Some('/')
            && cursor.peek_nth(2) == Some('/')
            && cursor.peek_nth(3) != Some('/')
        {
            // Consume `///`
            cursor.advance();
            cursor.advance();
            cursor.advance();
            // Strip optional leading space
            if cursor.peek() == Some(' ') {
                cursor.advance();
            }
            // Read to end of line
            let content_start = cursor.pos();
            cursor.eat_while(|c| c != '\n');
            let content = cursor.slice(content_start, cursor.pos()).to_string();
            let span = Span::new(start_pos, cursor.pos());
            tokens.push(Token::new(
                TokenKind::DocComment(content),
                span,
                start_line,
                start_col,
            ));
            continue;
        }

        match scan_token(&mut cursor, &mut errors) {
            Some(kind) => {
                let span = Span::new(start_pos, cursor.pos());
                tokens.push(Token::new(kind, span, start_line, start_col));
            }
            None => {
                // Error already recorded in scan_token or skip functions
            }
        }
    }

    // Always end with EOF
    let eof_pos = cursor.pos();
    tokens.push(Token::new(
        TokenKind::Eof,
        Span::new(eof_pos, eof_pos),
        cursor.line(),
        cursor.col(),
    ));

    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}

/// Tokenizes source code and also collects comments with their positions.
///
/// Used by the formatter to preserve comments in formatted output.
///
/// # Returns
///
/// * `Ok((Vec<Token>, Vec<Comment>))` - Tokens and collected comments.
/// * `Err(Vec<LexError>)` - Errors encountered during tokenization.
pub fn tokenize_with_comments(source: &str) -> Result<(Vec<Token>, Vec<Comment>), Vec<LexError>> {
    let mut cursor = Cursor::new(source);
    let mut tokens = Vec::new();
    let mut comments = Vec::new();
    let mut errors = Vec::new();

    while !cursor.is_eof() {
        collect_whitespace_and_comments(&mut cursor, &mut comments, &mut errors);

        if cursor.is_eof() {
            break;
        }

        let start_pos = cursor.pos();
        let start_line = cursor.line();
        let start_col = cursor.col();

        // Check for doc comment: `///` (not `////`)
        if cursor.peek() == Some('/')
            && cursor.peek_nth(1) == Some('/')
            && cursor.peek_nth(2) == Some('/')
            && cursor.peek_nth(3) != Some('/')
        {
            cursor.advance();
            cursor.advance();
            cursor.advance();
            if cursor.peek() == Some(' ') {
                cursor.advance();
            }
            let content_start = cursor.pos();
            cursor.eat_while(|c| c != '\n');
            let content = cursor.slice(content_start, cursor.pos()).to_string();
            let span = Span::new(start_pos, cursor.pos());
            tokens.push(Token::new(
                TokenKind::DocComment(content),
                span,
                start_line,
                start_col,
            ));
            continue;
        }

        if let Some(kind) = scan_token(&mut cursor, &mut errors) {
            let span = Span::new(start_pos, cursor.pos());
            tokens.push(Token::new(kind, span, start_line, start_col));
        }
    }

    let eof_pos = cursor.pos();
    tokens.push(Token::new(
        TokenKind::Eof,
        Span::new(eof_pos, eof_pos),
        cursor.line(),
        cursor.col(),
    ));

    if errors.is_empty() {
        Ok((tokens, comments))
    } else {
        Err(errors)
    }
}

/// Skips whitespace and collects comments with position info.
fn collect_whitespace_and_comments(
    cursor: &mut Cursor<'_>,
    comments: &mut Vec<Comment>,
    errors: &mut Vec<LexError>,
) {
    loop {
        cursor.eat_while(|c| c.is_whitespace());

        if cursor.is_eof() {
            return;
        }

        if cursor.peek() == Some('/') {
            match cursor.peek_second() {
                Some('/') => {
                    // Check for doc comment: exactly `///` (not `////`)
                    if cursor.peek_nth(2) == Some('/') && cursor.peek_nth(3) != Some('/') {
                        // Doc comment — don't skip, let tokenize_with_comments handle it
                        return;
                    }
                    let start_pos = cursor.pos();
                    let comment_start = cursor.pos();
                    cursor.eat_while(|c| c != '\n');
                    let end_pos = cursor.pos();
                    let text = cursor.slice(comment_start, end_pos);
                    comments.push(Comment {
                        pos: start_pos,
                        text: text.to_string(),
                        is_doc: false,
                        is_block: false,
                    });
                    continue;
                }
                Some('*') => {
                    let start_line = cursor.line();
                    let start_col = cursor.col();
                    let start_pos = cursor.pos();
                    cursor.advance(); // '/'
                    cursor.advance(); // '*'
                    let mut depth = 1u32;
                    while depth > 0 {
                        match cursor.advance() {
                            Some('/') if cursor.peek() == Some('*') => {
                                cursor.advance();
                                depth += 1;
                            }
                            Some('*') if cursor.peek() == Some('/') => {
                                cursor.advance();
                                depth -= 1;
                            }
                            Some(_) => {}
                            None => {
                                errors.push(LexError::UnterminatedBlockComment {
                                    line: start_line,
                                    col: start_col,
                                    span: Span::new(start_pos, cursor.pos()),
                                });
                                return;
                            }
                        }
                    }
                    let end_pos = cursor.pos();
                    let text = cursor.slice(start_pos, end_pos);
                    comments.push(Comment {
                        pos: start_pos,
                        text: text.to_string(),
                        is_doc: false,
                        is_block: true,
                    });
                    continue;
                }
                _ => return,
            }
        }

        return;
    }
}

/// Skips whitespace and comments (single-line, multi-line, and doc comments).
fn skip_whitespace_and_comments(cursor: &mut Cursor<'_>, errors: &mut Vec<LexError>) {
    loop {
        // Skip whitespace
        cursor.eat_while(|c| c.is_whitespace());

        if cursor.is_eof() {
            return;
        }

        // Check for comments
        if cursor.peek() == Some('/') {
            match cursor.peek_second() {
                Some('/') => {
                    // Check for doc comment: exactly `///` (not `////`)
                    if cursor.peek_nth(2) == Some('/') && cursor.peek_nth(3) != Some('/') {
                        // Doc comment — don't skip, let tokenize() handle it
                        return;
                    }
                    // Regular single-line comment
                    cursor.eat_while(|c| c != '\n');
                    continue;
                }
                Some('*') => {
                    // Multi-line comment
                    let start_line = cursor.line();
                    let start_col = cursor.col();
                    let start_pos = cursor.pos();
                    cursor.advance(); // '/'
                    cursor.advance(); // '*'
                    let mut depth = 1u32;
                    while depth > 0 {
                        match cursor.advance() {
                            Some('/') if cursor.peek() == Some('*') => {
                                cursor.advance();
                                depth += 1;
                            }
                            Some('*') if cursor.peek() == Some('/') => {
                                cursor.advance();
                                depth -= 1;
                            }
                            Some(_) => {}
                            None => {
                                errors.push(LexError::UnterminatedBlockComment {
                                    line: start_line,
                                    col: start_col,
                                    span: Span::new(start_pos, cursor.pos()),
                                });
                                return;
                            }
                        }
                    }
                    continue;
                }
                _ => return,
            }
        }

        return;
    }
}

/// Scans a single token from the cursor position.
///
/// Returns `None` if the character is unexpected (error is pushed to `errors`).
fn scan_token(cursor: &mut Cursor<'_>, errors: &mut Vec<LexError>) -> Option<TokenKind> {
    let ch = cursor.peek()?;

    match ch {
        // Identifiers and keywords
        c if is_ident_start(c) => Some(scan_identifier_or_keyword(cursor)),

        // Number literals
        '0'..='9' => scan_number(cursor, errors),

        // String literals
        '"' => scan_string(cursor, errors),

        // Raw string literals
        'r' if cursor.peek_second() == Some('"') => {
            // This is handled by scan_identifier_or_keyword if 'r' is followed by '"'
            // But we already checked is_ident_start('r') above, so we need special handling
            // Actually 'r' is_ident_start, so it goes to identifier branch.
            // We handle raw strings there.
            unreachable!("'r' is caught by identifier branch")
        }

        // Character literals or lifetime annotations
        '\'' => scan_char_or_lifetime(cursor, errors),

        // Annotation @
        '@' => scan_annotation(cursor, errors),

        // Operators and punctuation
        _ => scan_operator_or_punct(cursor, errors),
    }
}

/// Returns `true` if `c` can start an identifier.
fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

/// Returns `true` if `c` can continue an identifier.
fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Scans an identifier or keyword token.
///
/// Also handles raw string literals (r"...").
fn scan_identifier_or_keyword(cursor: &mut Cursor<'_>) -> TokenKind {
    let start = cursor.pos();

    // Check for f-string: f"..."
    if cursor.peek() == Some('f') && cursor.peek_second() == Some('"') {
        cursor.advance(); // consume 'f'
        cursor.advance(); // consume '"'
        let mut parts = Vec::new();
        let mut current_lit = String::new();
        loop {
            match cursor.peek() {
                Some('"') => {
                    cursor.advance();
                    break;
                }
                Some('{') => {
                    // Check for {{ escape
                    if cursor.peek_second() == Some('{') {
                        cursor.advance();
                        cursor.advance();
                        current_lit.push('{');
                        continue;
                    }
                    // Flush literal
                    if !current_lit.is_empty() {
                        parts.push(token::FStringPart::Literal(std::mem::take(
                            &mut current_lit,
                        )));
                    }
                    cursor.advance(); // consume '{'
                    // Read expression until '}'
                    let mut expr_src = String::new();
                    let mut depth = 1;
                    while !cursor.is_eof() {
                        match cursor.peek() {
                            Some('{') => {
                                depth += 1;
                                expr_src.push('{');
                                cursor.advance();
                            }
                            Some('}') => {
                                depth -= 1;
                                if depth == 0 {
                                    cursor.advance();
                                    break;
                                }
                                expr_src.push('}');
                                cursor.advance();
                            }
                            Some(c) => {
                                expr_src.push(c);
                                cursor.advance();
                            }
                            None => break,
                        }
                    }
                    parts.push(token::FStringPart::Expr(expr_src));
                }
                Some('}') => {
                    // Check for }} escape
                    if cursor.peek_second() == Some('}') {
                        cursor.advance();
                        cursor.advance();
                        current_lit.push('}');
                        continue;
                    }
                    cursor.advance();
                    current_lit.push('}');
                }
                Some('\\') => {
                    cursor.advance();
                    match cursor.peek() {
                        Some('n') => {
                            current_lit.push('\n');
                            cursor.advance();
                        }
                        Some('t') => {
                            current_lit.push('\t');
                            cursor.advance();
                        }
                        Some('"') => {
                            current_lit.push('"');
                            cursor.advance();
                        }
                        Some('\\') => {
                            current_lit.push('\\');
                            cursor.advance();
                        }
                        _ => current_lit.push('\\'),
                    }
                }
                Some(c) => {
                    current_lit.push(c);
                    cursor.advance();
                }
                None => break,
            }
        }
        if !current_lit.is_empty() {
            parts.push(token::FStringPart::Literal(current_lit));
        }
        return TokenKind::FStringLit(parts);
    }

    // Check for raw string: r"..."
    if cursor.peek() == Some('r') && cursor.peek_second() == Some('"') {
        cursor.advance(); // consume 'r'
        cursor.advance(); // consume '"'
        let str_start = cursor.pos();
        // Read until closing "
        while cursor.peek() != Some('"') && !cursor.is_eof() {
            cursor.advance();
        }
        let content = cursor.source()[str_start..cursor.pos()].to_string();
        if !cursor.is_eof() {
            cursor.advance(); // consume closing '"'
        }
        return TokenKind::RawStringLit(content);
    }

    let word = cursor.eat_while(is_ident_continue);

    // Check keyword table first
    if let Some(kw) = lookup_keyword(word) {
        return kw;
    }

    TokenKind::Ident(cursor.source()[start..cursor.pos()].to_string())
}

/// Scans a number literal (integer or float).
///
/// Supports: decimal, hex (0x), binary (0b), octal (0o), float, scientific notation.
/// Underscore separators are allowed (e.g., `1_000_000`).
fn scan_number(cursor: &mut Cursor<'_>, errors: &mut Vec<LexError>) -> Option<TokenKind> {
    let start = cursor.pos();
    let start_line = cursor.line();
    let start_col = cursor.col();
    let first = cursor.advance()?; // consume first digit

    // Check for prefix: 0x, 0b, 0o
    if first == '0' {
        match cursor.peek() {
            Some('x' | 'X') => {
                cursor.advance(); // consume 'x'
                let digits = cursor.eat_while(|c| c.is_ascii_hexdigit() || c == '_');
                let clean: String = digits.chars().filter(|&c| c != '_').collect();
                if clean.is_empty() {
                    errors.push(LexError::InvalidNumber {
                        reason: "expected hex digits after 0x".into(),
                        line: start_line,
                        col: start_col,
                        span: Span::new(start, cursor.pos()),
                    });
                    return None;
                }
                return match i64::from_str_radix(&clean, 16) {
                    Ok(v) => Some(TokenKind::IntLit(v)),
                    Err(_) => {
                        errors.push(LexError::NumberOverflow {
                            line: start_line,
                            col: start_col,
                            span: Span::new(start, cursor.pos()),
                        });
                        None
                    }
                };
            }
            Some('b' | 'B') => {
                cursor.advance(); // consume 'b'
                let digits = cursor.eat_while(|c| c == '0' || c == '1' || c == '_');
                let clean: String = digits.chars().filter(|&c| c != '_').collect();
                if clean.is_empty() {
                    errors.push(LexError::InvalidNumber {
                        reason: "expected binary digits after 0b".into(),
                        line: start_line,
                        col: start_col,
                        span: Span::new(start, cursor.pos()),
                    });
                    return None;
                }
                return match i64::from_str_radix(&clean, 2) {
                    Ok(v) => Some(TokenKind::IntLit(v)),
                    Err(_) => {
                        errors.push(LexError::NumberOverflow {
                            line: start_line,
                            col: start_col,
                            span: Span::new(start, cursor.pos()),
                        });
                        None
                    }
                };
            }
            Some('o' | 'O') => {
                cursor.advance(); // consume 'o'
                let digits = cursor.eat_while(|c| ('0'..='7').contains(&c) || c == '_');
                let clean: String = digits.chars().filter(|&c| c != '_').collect();
                if clean.is_empty() {
                    errors.push(LexError::InvalidNumber {
                        reason: "expected octal digits after 0o".into(),
                        line: start_line,
                        col: start_col,
                        span: Span::new(start, cursor.pos()),
                    });
                    return None;
                }
                return match i64::from_str_radix(&clean, 8) {
                    Ok(v) => Some(TokenKind::IntLit(v)),
                    Err(_) => {
                        errors.push(LexError::NumberOverflow {
                            line: start_line,
                            col: start_col,
                            span: Span::new(start, cursor.pos()),
                        });
                        None
                    }
                };
            }
            _ => {}
        }
    }

    // Decimal digits (including first digit already consumed)
    cursor.eat_while(|c| c.is_ascii_digit() || c == '_');

    // Check for float: decimal point followed by digit
    let mut is_float = false;
    if cursor.peek() == Some('.') && cursor.peek_second().is_some_and(|c| c.is_ascii_digit()) {
        is_float = true;
        cursor.advance(); // consume '.'
        cursor.eat_while(|c| c.is_ascii_digit() || c == '_');
    }

    // Scientific notation: e/E followed by optional sign and digits
    if cursor.peek().is_some_and(|c| c == 'e' || c == 'E') {
        is_float = true;
        cursor.advance(); // consume 'e'/'E'
        if cursor.peek().is_some_and(|c| c == '+' || c == '-') {
            cursor.advance(); // consume sign
        }
        let exp_digits = cursor.eat_while(|c| c.is_ascii_digit() || c == '_');
        let clean_exp: String = exp_digits.chars().filter(|&c| c != '_').collect();
        if clean_exp.is_empty() {
            errors.push(LexError::InvalidNumber {
                reason: "expected digits after exponent".into(),
                line: start_line,
                col: start_col,
                span: Span::new(start, cursor.pos()),
            });
            return None;
        }
    }

    let text = cursor.source()[start..cursor.pos()].to_string();
    let clean: String = text.chars().filter(|&c| c != '_').collect();

    if is_float {
        match clean.parse::<f64>() {
            Ok(v) => Some(TokenKind::FloatLit(v)),
            Err(_) => {
                errors.push(LexError::InvalidNumber {
                    reason: "invalid float literal".into(),
                    line: start_line,
                    col: start_col,
                    span: Span::new(start, cursor.pos()),
                });
                None
            }
        }
    } else {
        match clean.parse::<i64>() {
            Ok(v) => Some(TokenKind::IntLit(v)),
            Err(_) => {
                errors.push(LexError::NumberOverflow {
                    line: start_line,
                    col: start_col,
                    span: Span::new(start, cursor.pos()),
                });
                None
            }
        }
    }
}

/// Scans a regular string literal (double-quoted, with escape processing).
fn scan_string(cursor: &mut Cursor<'_>, errors: &mut Vec<LexError>) -> Option<TokenKind> {
    let start = cursor.pos();
    let start_line = cursor.line();
    let start_col = cursor.col();
    cursor.advance(); // consume opening '"'

    let mut value = String::new();

    loop {
        match cursor.advance() {
            Some('"') => return Some(TokenKind::StringLit(value)),
            Some('\\') => {
                let esc_line = cursor.line();
                let esc_col = cursor.col();
                match cursor.advance() {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some('r') => value.push('\r'),
                    Some('\\') => value.push('\\'),
                    Some('"') => value.push('"'),
                    Some('0') => value.push('\0'),
                    Some(ch) => {
                        errors.push(LexError::InvalidEscape {
                            ch,
                            line: esc_line,
                            col: esc_col,
                            span: Span::new(cursor.pos() - ch.len_utf8() - 1, cursor.pos()),
                        });
                        value.push(ch);
                    }
                    None => {
                        errors.push(LexError::UnterminatedString {
                            line: start_line,
                            col: start_col,
                            span: Span::new(start, cursor.pos()),
                        });
                        return None;
                    }
                }
            }
            Some(ch) => value.push(ch),
            None => {
                errors.push(LexError::UnterminatedString {
                    line: start_line,
                    col: start_col,
                    span: Span::new(start, cursor.pos()),
                });
                return None;
            }
        }
    }
}

/// Disambiguates between a character literal and a lifetime annotation.
///
/// A lifetime starts with `'` followed by an identifier (e.g., `'a`, `'static`, `'_`),
/// and the identifier is NOT terminated by a closing `'`. A char literal is `'c'`.
///
/// Strategy: peek ahead to check if this is `'<ident>` without a closing `'` after one char,
/// or `'_` / `'<multi-char-ident>` which can only be a lifetime.
fn scan_char_or_lifetime(cursor: &mut Cursor<'_>, errors: &mut Vec<LexError>) -> Option<TokenKind> {
    // Look at what follows the `'`
    let next = cursor.peek_second();
    match next {
        // `'_` is always a lifetime (wildcard lifetime)
        Some('_') => {
            // Check if it's `'_'` (char literal for underscore)
            if cursor.peek_nth(2) == Some('\'') {
                scan_char(cursor, errors)
            } else {
                scan_lifetime(cursor)
            }
        }
        // `'<letter>` — could be char literal or lifetime
        Some(c) if c.is_alphabetic() => {
            // Look further: if char after the letter is `'`, it's a char literal like `'a'`
            // If it's another ident char (letter/digit/_), check if there's a closing `'`
            match cursor.peek_nth(2) {
                // Pattern: `'X'` — char literal
                Some('\'') => scan_char(cursor, errors),
                // Pattern: `'ab...` — could be lifetime OR multi-char literal attempt
                Some(c3) if is_ident_continue(c3) => {
                    // Scan ahead through ident chars to see if a closing `'` follows
                    // If pattern is `'ident'`, it's a char literal (multi-char error)
                    // If pattern is `'ident<non-quote>`, it's a lifetime
                    let mut n = 3;
                    while let Some(cn) = cursor.peek_nth(n) {
                        if is_ident_continue(cn) {
                            n += 1;
                        } else {
                            break;
                        }
                    }
                    // Check what's after the ident
                    if cursor.peek_nth(n) == Some('\'') {
                        // Pattern: `'abc'` — multi-char literal (delegated to scan_char)
                        scan_char(cursor, errors)
                    } else {
                        // Pattern: `'abc` without closing quote — lifetime
                        scan_lifetime(cursor)
                    }
                }
                // Pattern: `'a<non-ident>` — lifetime like `'a,` or `'a>`
                _ => scan_lifetime(cursor),
            }
        }
        // `'\\` — escape sequence, must be a char literal
        // `''` — empty char literal (error case)
        // Everything else: delegate to char scanner (will error if invalid)
        _ => scan_char(cursor, errors),
    }
}

/// Scans a lifetime token: `'a`, `'static`, `'_`.
fn scan_lifetime(cursor: &mut Cursor<'_>) -> Option<TokenKind> {
    let start = cursor.pos();
    cursor.advance(); // consume `'`

    // Consume the identifier part
    let name = if cursor.peek() == Some('_') {
        cursor.advance();
        "_".to_string()
    } else {
        cursor.eat_while(is_ident_continue).to_string()
    };

    if name.is_empty() {
        // Should not happen given our disambiguation, but be safe
        return None;
    }

    let _end = cursor.pos();
    let _ = start; // span is tracked by the caller
    Some(TokenKind::Lifetime(name))
}

/// Scans a character literal (single-quoted).
fn scan_char(cursor: &mut Cursor<'_>, errors: &mut Vec<LexError>) -> Option<TokenKind> {
    let start = cursor.pos();
    let start_line = cursor.line();
    let start_col = cursor.col();
    cursor.advance(); // consume opening '\''

    let ch = match cursor.advance() {
        Some('\\') => {
            // Escape sequence
            match cursor.advance() {
                Some('n') => '\n',
                Some('t') => '\t',
                Some('r') => '\r',
                Some('\\') => '\\',
                Some('\'') => '\'',
                Some('0') => '\0',
                _ => {
                    errors.push(LexError::InvalidCharLiteral {
                        line: start_line,
                        col: start_col,
                        span: Span::new(start, cursor.pos()),
                    });
                    // Skip to closing quote if present
                    if cursor.peek() == Some('\'') {
                        cursor.advance();
                    }
                    return None;
                }
            }
        }
        Some('\'') => {
            // Empty char literal '' → LE007
            errors.push(LexError::EmptyCharLiteral {
                line: start_line,
                col: start_col,
                span: Span::new(start, cursor.pos()),
            });
            return None;
        }
        Some(c) => c,
        None => {
            errors.push(LexError::InvalidCharLiteral {
                line: start_line,
                col: start_col,
                span: Span::new(start, cursor.pos()),
            });
            return None;
        }
    };

    // Expect closing '\''
    if cursor.eat('\'') {
        return Some(TokenKind::CharLit(ch));
    }

    // Not closed — could be multi-char like 'ab' or unterminated
    // Check if there are more chars before a closing quote
    if cursor.peek().is_some() && cursor.peek() != Some('\'') {
        // Multi-char literal — skip to closing quote
        while let Some(c) = cursor.peek() {
            if c == '\'' {
                cursor.advance();
                break;
            }
            if c == '\n' {
                break;
            }
            cursor.advance();
        }
        errors.push(LexError::MultiCharLiteral {
            line: start_line,
            col: start_col,
            span: Span::new(start, cursor.pos()),
        });
    } else {
        errors.push(LexError::InvalidCharLiteral {
            line: start_line,
            col: start_col,
            span: Span::new(start, cursor.pos()),
        });
    }
    None
}

/// Scans an annotation token (`@kernel`, `@device`, etc.).
fn scan_annotation(cursor: &mut Cursor<'_>, errors: &mut Vec<LexError>) -> Option<TokenKind> {
    let start = cursor.pos();
    let start_line = cursor.line();
    let start_col = cursor.col();
    cursor.advance(); // consume '@'

    // If next char is not ident start, it's the bare @ operator (matrix multiply)
    if !cursor.peek().is_some_and(is_ident_start) {
        return Some(TokenKind::At);
    }

    let name = cursor.eat_while(is_ident_continue);

    if let Some(ann) = lookup_annotation(name) {
        Some(ann)
    } else {
        errors.push(LexError::UnknownAnnotation {
            name: name.to_string(),
            line: start_line,
            col: start_col,
            span: Span::new(start, cursor.pos()),
        });
        None
    }
}

/// Scans an operator or punctuation token.
fn scan_operator_or_punct(
    cursor: &mut Cursor<'_>,
    errors: &mut Vec<LexError>,
) -> Option<TokenKind> {
    let start = cursor.pos();
    let start_line = cursor.line();
    let start_col = cursor.col();
    let ch = cursor.advance()?;

    let kind = match ch {
        '+' => {
            if cursor.eat('=') {
                TokenKind::PlusEq
            } else {
                TokenKind::Plus
            }
        }
        '-' => {
            if cursor.eat('>') {
                TokenKind::Arrow
            } else if cursor.eat('=') {
                TokenKind::MinusEq
            } else {
                TokenKind::Minus
            }
        }
        '*' => {
            if cursor.eat('*') {
                TokenKind::StarStar
            } else if cursor.eat('=') {
                TokenKind::StarEq
            } else {
                TokenKind::Star
            }
        }
        '/' => {
            if cursor.eat('=') {
                TokenKind::SlashEq
            } else {
                TokenKind::Slash
            }
        }
        '%' => {
            if cursor.eat('=') {
                TokenKind::PercentEq
            } else {
                TokenKind::Percent
            }
        }
        '=' => {
            if cursor.eat('>') {
                TokenKind::FatArrow
            } else if cursor.eat('=') {
                TokenKind::EqEq
            } else {
                TokenKind::Eq
            }
        }
        '!' => {
            if cursor.eat('=') {
                TokenKind::BangEq
            } else {
                TokenKind::Bang
            }
        }
        '<' => {
            if cursor.eat('<') {
                if cursor.eat('=') {
                    TokenKind::LtLtEq
                } else {
                    TokenKind::LtLt
                }
            } else if cursor.eat('=') {
                TokenKind::LtEq
            } else {
                TokenKind::Lt
            }
        }
        '>' => {
            if cursor.eat('>') {
                if cursor.eat('=') {
                    TokenKind::GtGtEq
                } else {
                    TokenKind::GtGt
                }
            } else if cursor.eat('=') {
                TokenKind::GtEq
            } else {
                TokenKind::Gt
            }
        }
        '&' => {
            if cursor.eat('&') {
                TokenKind::AmpAmp
            } else if cursor.eat('=') {
                TokenKind::AmpEq
            } else {
                TokenKind::Amp
            }
        }
        '|' => {
            if cursor.eat('|') {
                TokenKind::PipePipe
            } else if cursor.eat('>') {
                TokenKind::PipeGt
            } else if cursor.eat('=') {
                TokenKind::PipeEq
            } else {
                TokenKind::Pipe
            }
        }
        '^' => {
            if cursor.eat('=') {
                TokenKind::CaretEq
            } else {
                TokenKind::Caret
            }
        }
        '~' => TokenKind::Tilde,
        '(' => TokenKind::LParen,
        ')' => TokenKind::RParen,
        '{' => TokenKind::LBrace,
        '}' => TokenKind::RBrace,
        '[' => TokenKind::LBracket,
        ']' => TokenKind::RBracket,
        ';' => TokenKind::Semi,
        ':' => {
            if cursor.eat(':') {
                TokenKind::ColonColon
            } else {
                TokenKind::Colon
            }
        }
        ',' => TokenKind::Comma,
        '.' => {
            if cursor.eat('.') {
                if cursor.eat('=') {
                    TokenKind::DotDotEq
                } else {
                    TokenKind::DotDot
                }
            } else {
                TokenKind::Dot
            }
        }
        '?' => TokenKind::Question,
        '$' => TokenKind::Dollar,
        _ => {
            errors.push(LexError::UnexpectedChar {
                ch,
                line: start_line,
                col: start_col,
                span: Span::new(start, cursor.pos()),
            });
            return None;
        }
    };

    Some(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper macro: tokenizes source and asserts token kinds (excluding EOF).
    macro_rules! assert_tokens {
        ($src:expr, $($kind:expr),+ $(,)?) => {
            let tokens = tokenize($src).unwrap();
            let kinds: Vec<_> = tokens.iter()
                .filter(|t| t.kind != TokenKind::Eof)
                .map(|t| t.kind.clone())
                .collect();
            assert_eq!(kinds, vec![$($kind),+]);
        };
    }

    // ── Keywords ───────────────────────────────────────────────────────

    #[test]
    fn tokenize_control_flow_keywords() {
        assert_tokens!(
            "if else match while for in return break continue",
            TokenKind::If,
            TokenKind::Else,
            TokenKind::Match,
            TokenKind::While,
            TokenKind::For,
            TokenKind::In,
            TokenKind::Return,
            TokenKind::Break,
            TokenKind::Continue
        );
    }

    #[test]
    fn tokenize_declaration_keywords() {
        assert_tokens!(
            "let mut fn struct enum impl trait type const",
            TokenKind::Let,
            TokenKind::Mut,
            TokenKind::Fn,
            TokenKind::Struct,
            TokenKind::Enum,
            TokenKind::Impl,
            TokenKind::Trait,
            TokenKind::Type,
            TokenKind::Const
        );
    }

    #[test]
    fn tokenize_module_keywords() {
        assert_tokens!(
            "use mod pub extern as",
            TokenKind::Use,
            TokenKind::Mod,
            TokenKind::Pub,
            TokenKind::Extern,
            TokenKind::As
        );
    }

    #[test]
    fn tokenize_literal_keywords() {
        assert_tokens!(
            "true false null",
            TokenKind::True,
            TokenKind::False,
            TokenKind::Null
        );
    }

    #[test]
    fn tokenize_ml_keywords() {
        assert_tokens!(
            "tensor grad loss layer model",
            TokenKind::Tensor,
            TokenKind::Grad,
            TokenKind::Loss,
            TokenKind::Layer,
            TokenKind::Model
        );
    }

    #[test]
    fn tokenize_os_keywords() {
        assert_tokens!(
            "ptr addr page region irq syscall",
            TokenKind::Ptr,
            TokenKind::Addr,
            TokenKind::Page,
            TokenKind::Region,
            TokenKind::Irq,
            TokenKind::Syscall
        );
    }

    // ── Identifiers ────────────────────────────────────────────────────

    #[test]
    fn tokenize_identifiers() {
        assert_tokens!(
            "foo bar_baz _private MyStruct x123",
            TokenKind::Ident("foo".into()),
            TokenKind::Ident("bar_baz".into()),
            TokenKind::Ident("_private".into()),
            TokenKind::Ident("MyStruct".into()),
            TokenKind::Ident("x123".into())
        );
    }

    // ── Integer Literals ───────────────────────────────────────────────

    #[test]
    fn tokenize_decimal_integer() {
        assert_tokens!("42", TokenKind::IntLit(42));
    }

    #[test]
    fn tokenize_zero() {
        assert_tokens!("0", TokenKind::IntLit(0));
    }

    #[test]
    fn tokenize_hex_integer() {
        assert_tokens!("0xFF", TokenKind::IntLit(255));
    }

    #[test]
    fn tokenize_binary_integer() {
        assert_tokens!("0b1010", TokenKind::IntLit(10));
    }

    #[test]
    fn tokenize_octal_integer() {
        assert_tokens!("0o17", TokenKind::IntLit(15));
    }

    #[test]
    fn tokenize_underscore_separator_in_number() {
        assert_tokens!("1_000_000", TokenKind::IntLit(1_000_000));
    }

    // ── Float Literals ─────────────────────────────────────────────────

    #[test]
    fn tokenize_float_literal() {
        assert_tokens!("3.14", TokenKind::FloatLit(3.14));
    }

    #[test]
    fn tokenize_scientific_notation() {
        assert_tokens!("1.0e4", TokenKind::FloatLit(1.0e4));
    }

    #[test]
    fn tokenize_scientific_notation_negative_exponent() {
        assert_tokens!("1.0e-4", TokenKind::FloatLit(1.0e-4));
    }

    #[test]
    fn tokenize_integer_scientific_notation() {
        assert_tokens!("1e10", TokenKind::FloatLit(1e10));
    }

    // ── String Literals ────────────────────────────────────────────────

    #[test]
    fn tokenize_simple_string() {
        assert_tokens!(r#""hello""#, TokenKind::StringLit("hello".into()));
    }

    #[test]
    fn tokenize_string_with_escapes() {
        assert_tokens!(
            r#""hello\nworld""#,
            TokenKind::StringLit("hello\nworld".into())
        );
    }

    #[test]
    fn tokenize_string_with_escaped_quote() {
        assert_tokens!(r#""say \"hi\"""#, TokenKind::StringLit("say \"hi\"".into()));
    }

    #[test]
    fn tokenize_empty_string() {
        assert_tokens!(r#""""#, TokenKind::StringLit("".into()));
    }

    #[test]
    fn tokenize_raw_string() {
        assert_tokens!(
            r#"r"raw \n string""#,
            TokenKind::RawStringLit(r"raw \n string".into())
        );
    }

    // ── Character Literals ─────────────────────────────────────────────

    #[test]
    fn tokenize_char_literal() {
        assert_tokens!("'a'", TokenKind::CharLit('a'));
    }

    #[test]
    fn tokenize_char_escape() {
        assert_tokens!(r"'\n'", TokenKind::CharLit('\n'));
    }

    // ── Annotations ────────────────────────────────────────────────────

    #[test]
    fn tokenize_annotations() {
        assert_tokens!(
            "@kernel @device @safe @unsafe @ffi",
            TokenKind::AtKernel,
            TokenKind::AtDevice,
            TokenKind::AtSafe,
            TokenKind::AtUnsafe,
            TokenKind::AtFfi
        );
    }

    #[test]
    fn tokenize_at_sign_alone_is_matmul() {
        assert_tokens!("@", TokenKind::At);
    }

    // ── Operators ──────────────────────────────────────────────────────

    #[test]
    fn tokenize_arithmetic_operators() {
        assert_tokens!(
            "+ - * / % **",
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::StarStar
        );
    }

    #[test]
    fn tokenize_comparison_operators() {
        assert_tokens!(
            "== != < > <= >=",
            TokenKind::EqEq,
            TokenKind::BangEq,
            TokenKind::Lt,
            TokenKind::Gt,
            TokenKind::LtEq,
            TokenKind::GtEq
        );
    }

    #[test]
    fn tokenize_logical_operators() {
        assert_tokens!(
            "&& || !",
            TokenKind::AmpAmp,
            TokenKind::PipePipe,
            TokenKind::Bang
        );
    }

    #[test]
    fn tokenize_bitwise_operators() {
        assert_tokens!(
            "& | ^ ~ << >>",
            TokenKind::Amp,
            TokenKind::Pipe,
            TokenKind::Caret,
            TokenKind::Tilde,
            TokenKind::LtLt,
            TokenKind::GtGt
        );
    }

    #[test]
    fn tokenize_assignment_operators() {
        assert_tokens!(
            "= += -= *= /= %= &= |= ^= <<= >>=",
            TokenKind::Eq,
            TokenKind::PlusEq,
            TokenKind::MinusEq,
            TokenKind::StarEq,
            TokenKind::SlashEq,
            TokenKind::PercentEq,
            TokenKind::AmpEq,
            TokenKind::PipeEq,
            TokenKind::CaretEq,
            TokenKind::LtLtEq,
            TokenKind::GtGtEq
        );
    }

    #[test]
    fn tokenize_range_operators() {
        assert_tokens!(".. ..=", TokenKind::DotDot, TokenKind::DotDotEq);
    }

    #[test]
    fn tokenize_pipeline_operator() {
        assert_tokens!("|>", TokenKind::PipeGt);
    }

    // ── Delimiters & Punctuation ───────────────────────────────────────

    #[test]
    fn tokenize_delimiters() {
        assert_tokens!(
            "( ) { } [ ]",
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::LBracket,
            TokenKind::RBracket
        );
    }

    #[test]
    fn tokenize_punctuation() {
        assert_tokens!(
            "; : :: , . -> => ?",
            TokenKind::Semi,
            TokenKind::Colon,
            TokenKind::ColonColon,
            TokenKind::Comma,
            TokenKind::Dot,
            TokenKind::Arrow,
            TokenKind::FatArrow,
            TokenKind::Question
        );
    }

    // ── Whitespace & Comments ──────────────────────────────────────────

    #[test]
    fn tokenize_skips_whitespace() {
        assert_tokens!("  let   x  ", TokenKind::Let, TokenKind::Ident("x".into()));
    }

    #[test]
    fn tokenize_skips_single_line_comment() {
        assert_tokens!(
            "let // this is a comment\nx",
            TokenKind::Let,
            TokenKind::Ident("x".into())
        );
    }

    #[test]
    fn tokenize_skips_multi_line_comment() {
        assert_tokens!(
            "let /* comment */ x",
            TokenKind::Let,
            TokenKind::Ident("x".into())
        );
    }

    #[test]
    fn tokenize_skips_nested_block_comment() {
        assert_tokens!(
            "let /* outer /* inner */ still outer */ x",
            TokenKind::Let,
            TokenKind::Ident("x".into())
        );
    }

    #[test]
    fn tokenize_emits_doc_comments() {
        assert_tokens!(
            "/// doc comment\nlet x",
            TokenKind::DocComment("doc comment".into()),
            TokenKind::Let,
            TokenKind::Ident("x".into())
        );
    }

    #[test]
    fn tokenize_skips_quad_slash_comments() {
        // //// is NOT a doc comment — it's a regular comment
        assert_tokens!(
            "//// not a doc\nlet x",
            TokenKind::Let,
            TokenKind::Ident("x".into())
        );
    }

    // ── EOF ────────────────────────────────────────────────────────────

    #[test]
    fn tokenize_empty_source_produces_eof() {
        let tokens = tokenize("").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn tokenize_eof_is_always_last() {
        let tokens = tokenize("42").unwrap();
        assert_eq!(tokens.last().unwrap().kind, TokenKind::Eof);
    }

    // ── Spans & Positions ──────────────────────────────────────────────

    #[test]
    fn tokenize_tracks_correct_spans() {
        let tokens = tokenize("let x = 42").unwrap();
        // "let" at 0..3
        assert_eq!(tokens[0].span, Span::new(0, 3));
        assert_eq!(tokens[0].line, 1);
        assert_eq!(tokens[0].col, 1);
        // "x" at 4..5
        assert_eq!(tokens[1].span, Span::new(4, 5));
        assert_eq!(tokens[1].line, 1);
        assert_eq!(tokens[1].col, 5);
        // "=" at 6..7
        assert_eq!(tokens[2].span, Span::new(6, 7));
        // "42" at 8..10
        assert_eq!(tokens[3].span, Span::new(8, 10));
    }

    #[test]
    fn tokenize_tracks_line_numbers() {
        let tokens = tokenize("let\nx").unwrap();
        assert_eq!(tokens[0].line, 1); // "let"
        assert_eq!(tokens[1].line, 2); // "x"
    }

    // ── Complex Expression ─────────────────────────────────────────────

    #[test]
    fn tokenize_let_binding() {
        assert_tokens!(
            "let x: i32 = 42",
            TokenKind::Let,
            TokenKind::Ident("x".into()),
            TokenKind::Colon,
            TokenKind::I32,
            TokenKind::Eq,
            TokenKind::IntLit(42)
        );
    }

    #[test]
    fn tokenize_function_definition() {
        assert_tokens!(
            "fn add(a: i32, b: i32) -> i32 { a + b }",
            TokenKind::Fn,
            TokenKind::Ident("add".into()),
            TokenKind::LParen,
            TokenKind::Ident("a".into()),
            TokenKind::Colon,
            TokenKind::I32,
            TokenKind::Comma,
            TokenKind::Ident("b".into()),
            TokenKind::Colon,
            TokenKind::I32,
            TokenKind::RParen,
            TokenKind::Arrow,
            TokenKind::I32,
            TokenKind::LBrace,
            TokenKind::Ident("a".into()),
            TokenKind::Plus,
            TokenKind::Ident("b".into()),
            TokenKind::RBrace
        );
    }

    #[test]
    fn tokenize_pipeline_expression() {
        assert_tokens!(
            "5 |> double |> add_one",
            TokenKind::IntLit(5),
            TokenKind::PipeGt,
            TokenKind::Ident("double".into()),
            TokenKind::PipeGt,
            TokenKind::Ident("add_one".into())
        );
    }

    #[test]
    fn tokenize_annotated_function() {
        assert_tokens!(
            "@kernel fn init() { }",
            TokenKind::AtKernel,
            TokenKind::Fn,
            TokenKind::Ident("init".into()),
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::RBrace
        );
    }

    // ── Error Cases ────────────────────────────────────────────────────

    #[test]
    fn tokenize_reports_unexpected_char() {
        let err = tokenize("let # x").unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(matches!(&err[0], LexError::UnexpectedChar { ch: '#', .. }));
    }

    #[test]
    fn tokenize_reports_unterminated_string() {
        let err = tokenize(r#""hello"#).unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(matches!(&err[0], LexError::UnterminatedString { .. }));
    }

    #[test]
    fn tokenize_reports_invalid_escape() {
        let err = tokenize(r#""\q""#).unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(matches!(&err[0], LexError::InvalidEscape { ch: 'q', .. }));
    }

    #[test]
    fn tokenize_reports_unterminated_block_comment() {
        let err = tokenize("/* unterminated").unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(matches!(&err[0], LexError::UnterminatedBlockComment { .. }));
    }

    #[test]
    fn tokenize_reports_unknown_annotation() {
        let err = tokenize("@unknown").unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(matches!(&err[0], LexError::UnknownAnnotation { .. }));
    }

    #[test]
    fn tokenize_reports_invalid_hex_no_digits() {
        let err = tokenize("0x").unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(matches!(&err[0], LexError::InvalidNumber { .. }));
    }

    #[test]
    fn tokenize_reports_empty_char_literal() {
        let err = tokenize("''").unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(matches!(&err[0], LexError::EmptyCharLiteral { .. }));
    }

    #[test]
    fn tokenize_reports_multi_char_literal() {
        let err = tokenize("'ab'").unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(matches!(&err[0], LexError::MultiCharLiteral { .. }));
    }

    #[test]
    fn tokenize_reports_number_overflow() {
        let err = tokenize("99999999999999999999").unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(matches!(&err[0], LexError::NumberOverflow { .. }));
    }

    #[test]
    fn tokenize_collects_multiple_errors() {
        // $ is now a valid token (macro capture), % is modulo operator
        // Use characters that are actually invalid
        let err = tokenize(r#"# ` #"#).unwrap_err();
        assert!(err.len() >= 2);
    }

    // ── Lifetime tokenization tests ─────────────────────────────────────

    #[test]
    fn tokenize_lifetime_simple() {
        assert_tokens!("'a", TokenKind::Lifetime("a".into()));
    }

    #[test]
    fn tokenize_lifetime_static() {
        assert_tokens!("'static", TokenKind::Lifetime("static".into()));
    }

    #[test]
    fn tokenize_lifetime_wildcard() {
        assert_tokens!("'_", TokenKind::Lifetime("_".into()));
    }

    #[test]
    fn tokenize_lifetime_multi_char_name() {
        assert_tokens!("'buf", TokenKind::Lifetime("buf".into()));
    }

    #[test]
    fn tokenize_char_literal_not_confused_with_lifetime() {
        assert_tokens!("'a'", TokenKind::CharLit('a'));
    }

    #[test]
    fn tokenize_underscore_char_literal_not_confused_with_lifetime() {
        assert_tokens!("'_'", TokenKind::CharLit('_'));
    }

    #[test]
    fn tokenize_lifetime_followed_by_comma() {
        assert_tokens!("'a,", TokenKind::Lifetime("a".into()), TokenKind::Comma);
    }

    #[test]
    fn tokenize_lifetime_followed_by_gt() {
        assert_tokens!("'a>", TokenKind::Lifetime("a".into()), TokenKind::Gt);
    }

    #[test]
    fn tokenize_multiple_lifetimes_in_angle_brackets() {
        assert_tokens!(
            "<'a, 'b>",
            TokenKind::Lt,
            TokenKind::Lifetime("a".into()),
            TokenKind::Comma,
            TokenKind::Lifetime("b".into()),
            TokenKind::Gt
        );
    }

    #[test]
    fn tokenize_lifetime_in_reference_type() {
        // &'a T
        assert_tokens!(
            "&'a T",
            TokenKind::Amp,
            TokenKind::Lifetime("a".into()),
            TokenKind::Ident("T".into())
        );
    }
}
