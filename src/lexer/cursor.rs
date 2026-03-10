//! Cursor for iterating over source characters.
//!
//! Provides peek, advance, and position tracking for the lexer.
//! The cursor operates on bytes but exposes characters, tracking line and column.

/// A cursor that walks through a source string character by character.
///
/// Tracks the current byte position, line number, and column number.
/// Used internally by the lexer to scan tokens.
pub struct Cursor<'src> {
    /// The full source string.
    source: &'src str,
    /// Remaining source as bytes (slice of source).
    chars: std::str::Chars<'src>,
    /// Current byte offset into `source`.
    pos: usize,
    /// 1-indexed line number.
    line: u32,
    /// 1-indexed column number (in characters).
    col: u32,
}

impl<'src> Cursor<'src> {
    /// Creates a new cursor at the beginning of the source string.
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            chars: source.chars(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    /// Returns the current byte offset.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Returns the current 1-indexed line number.
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Returns the current 1-indexed column number.
    pub fn col(&self) -> u32 {
        self.col
    }

    /// Returns the full source string.
    pub fn source(&self) -> &'src str {
        self.source
    }

    /// Returns `true` if the cursor has reached the end of source.
    pub fn is_eof(&self) -> bool {
        self.pos >= self.source.len()
    }

    /// Peeks at the next character without advancing.
    ///
    /// Returns `None` if at end of file.
    pub fn peek(&self) -> Option<char> {
        self.chars.clone().next()
    }

    /// Peeks at the character after next without advancing.
    ///
    /// Returns `None` if fewer than 2 characters remain.
    pub fn peek_second(&self) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.next();
        iter.next()
    }

    /// Peeks at the nth character ahead without advancing.
    ///
    /// `peek_nth(0)` = `peek()`, `peek_nth(1)` = `peek_second()`.
    pub fn peek_nth(&self, n: usize) -> Option<char> {
        let mut iter = self.chars.clone();
        for _ in 0..n {
            iter.next();
        }
        iter.next()
    }

    /// Advances the cursor by one character and returns it.
    ///
    /// Returns `None` if at end of file.
    pub fn advance(&mut self) -> Option<char> {
        let ch = self.chars.next()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    /// Advances if the next character matches `expected`.
    ///
    /// Returns `true` and advances if matched, `false` otherwise.
    pub fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Advances while `predicate` returns `true` for the peeked character.
    ///
    /// Returns the consumed substring.
    pub fn eat_while(&mut self, predicate: impl Fn(char) -> bool) -> &'src str {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if predicate(ch) {
                self.advance();
            } else {
                break;
            }
        }
        &self.source[start..self.pos]
    }

    /// Returns a slice of the source from `start` to the current position.
    pub fn slice_from(&self, start: usize) -> &'src str {
        &self.source[start..self.pos]
    }

    /// Returns a slice of the source between `start` and `end` byte offsets.
    pub fn slice(&self, start: usize, end: usize) -> &'src str {
        &self.source[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_new_starts_at_beginning() {
        let cursor = Cursor::new("hello");
        assert_eq!(cursor.pos(), 0);
        assert_eq!(cursor.line(), 1);
        assert_eq!(cursor.col(), 1);
        assert!(!cursor.is_eof());
    }

    #[test]
    fn cursor_empty_source_is_eof() {
        let cursor = Cursor::new("");
        assert!(cursor.is_eof());
        assert_eq!(cursor.peek(), None);
    }

    #[test]
    fn cursor_peek_does_not_advance() {
        let cursor = Cursor::new("ab");
        assert_eq!(cursor.peek(), Some('a'));
        assert_eq!(cursor.peek(), Some('a'));
        assert_eq!(cursor.pos(), 0);
    }

    #[test]
    fn cursor_peek_second_returns_char_after_next() {
        let cursor = Cursor::new("abc");
        assert_eq!(cursor.peek_second(), Some('b'));
    }

    #[test]
    fn cursor_advance_returns_current_and_moves() {
        let mut cursor = Cursor::new("ab");
        assert_eq!(cursor.advance(), Some('a'));
        assert_eq!(cursor.pos(), 1);
        assert_eq!(cursor.col(), 2);
        assert_eq!(cursor.advance(), Some('b'));
        assert_eq!(cursor.pos(), 2);
        assert!(cursor.is_eof());
        assert_eq!(cursor.advance(), None);
    }

    #[test]
    fn cursor_newline_tracking() {
        let mut cursor = Cursor::new("a\nb");
        cursor.advance(); // 'a'
        assert_eq!(cursor.line(), 1);
        assert_eq!(cursor.col(), 2);
        cursor.advance(); // '\n'
        assert_eq!(cursor.line(), 2);
        assert_eq!(cursor.col(), 1);
        cursor.advance(); // 'b'
        assert_eq!(cursor.line(), 2);
        assert_eq!(cursor.col(), 2);
    }

    #[test]
    fn cursor_eat_matches_and_advances() {
        let mut cursor = Cursor::new("=>");
        assert!(cursor.eat('='));
        assert_eq!(cursor.pos(), 1);
        assert!(!cursor.eat('='));
        assert!(cursor.eat('>'));
    }

    #[test]
    fn cursor_eat_while_consumes_matching_chars() {
        let mut cursor = Cursor::new("abc123");
        let word = cursor.eat_while(|c| c.is_alphabetic());
        assert_eq!(word, "abc");
        assert_eq!(cursor.pos(), 3);
    }

    #[test]
    fn cursor_slice_from_returns_substring() {
        let mut cursor = Cursor::new("hello world");
        cursor.advance(); // h
        cursor.advance(); // e
        cursor.advance(); // l
        cursor.advance(); // l
        cursor.advance(); // o
        assert_eq!(cursor.slice_from(0), "hello");
    }

    #[test]
    fn cursor_handles_multibyte_utf8() {
        let mut cursor = Cursor::new("apa");
        assert_eq!(cursor.advance(), Some('a'));
        assert_eq!(cursor.pos(), 1);
        assert_eq!(cursor.advance(), Some('p'));
        assert_eq!(cursor.advance(), Some('a'));
        assert!(cursor.is_eof());
    }

    #[test]
    fn cursor_source_returns_original() {
        let cursor = Cursor::new("test");
        assert_eq!(cursor.source(), "test");
    }
}
