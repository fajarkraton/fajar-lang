//! PS/2 keyboard driver — scancode set 1 to ASCII translation.
//!
//! Handles scan codes from the 8042 keyboard controller (port 0x60).
//! Supports basic scancode set 1 (make/break codes) with shift state.

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Keyboard data port.
pub const KB_DATA_PORT: u16 = 0x60;
/// Keyboard status port.
pub const KB_STATUS_PORT: u16 = 0x64;

// ═══════════════════════════════════════════════════════════════════════
// Key events
// ═══════════════════════════════════════════════════════════════════════

/// A decoded key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    /// Key pressed with ASCII character.
    Press(char),
    /// Special key pressed.
    Special(SpecialKey),
    /// Key released (scancode).
    Release(u8),
    /// Unknown scancode.
    Unknown(u8),
}

/// Non-ASCII special keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    /// Enter / Return.
    Enter,
    /// Backspace.
    Backspace,
    /// Tab.
    Tab,
    /// Escape.
    Escape,
    /// Left Shift pressed.
    LShiftDown,
    /// Left Shift released.
    LShiftUp,
    /// Right Shift pressed.
    RShiftDown,
    /// Right Shift released.
    RShiftUp,
    /// Caps Lock.
    CapsLock,
    /// Arrow keys.
    ArrowUp,
    /// Arrow down.
    ArrowDown,
    /// Arrow left.
    ArrowLeft,
    /// Arrow right.
    ArrowRight,
}

// ═══════════════════════════════════════════════════════════════════════
// Keyboard driver
// ═══════════════════════════════════════════════════════════════════════

/// PS/2 keyboard driver with scancode-to-ASCII translation.
#[derive(Debug)]
pub struct Keyboard {
    /// Whether left shift is held.
    left_shift: bool,
    /// Whether right shift is held.
    right_shift: bool,
    /// Whether caps lock is active.
    caps_lock: bool,
    /// Input buffer for line editing.
    line_buffer: String,
    /// Completed lines (ready for consumption).
    completed_lines: Vec<String>,
}

impl Keyboard {
    /// Create a new keyboard driver.
    pub fn new() -> Self {
        Self {
            left_shift: false,
            right_shift: false,
            caps_lock: false,
            line_buffer: String::new(),
            completed_lines: Vec::new(),
        }
    }

    /// Whether shift is currently active.
    pub fn is_shift(&self) -> bool {
        self.left_shift || self.right_shift
    }

    /// Process a raw scancode and return the decoded event.
    pub fn process_scancode(&mut self, scancode: u8) -> KeyEvent {
        // Break code (key release) has bit 7 set
        if scancode & 0x80 != 0 {
            let make_code = scancode & 0x7F;
            match make_code {
                0x2A => {
                    self.left_shift = false;
                    return KeyEvent::Special(SpecialKey::LShiftUp);
                }
                0x36 => {
                    self.right_shift = false;
                    return KeyEvent::Special(SpecialKey::RShiftUp);
                }
                _ => return KeyEvent::Release(make_code),
            }
        }

        // Make code (key press)
        match scancode {
            0x01 => KeyEvent::Special(SpecialKey::Escape),
            0x0E => {
                // Backspace: remove last char from buffer
                self.line_buffer.pop();
                KeyEvent::Special(SpecialKey::Backspace)
            }
            0x0F => KeyEvent::Special(SpecialKey::Tab),
            0x1C => {
                // Enter: complete line
                let line = std::mem::take(&mut self.line_buffer);
                self.completed_lines.push(line);
                KeyEvent::Special(SpecialKey::Enter)
            }
            0x2A => {
                self.left_shift = true;
                KeyEvent::Special(SpecialKey::LShiftDown)
            }
            0x36 => {
                self.right_shift = true;
                KeyEvent::Special(SpecialKey::RShiftDown)
            }
            0x3A => {
                self.caps_lock = !self.caps_lock;
                KeyEvent::Special(SpecialKey::CapsLock)
            }
            // Arrow keys (extended scancodes simplified)
            0x48 => KeyEvent::Special(SpecialKey::ArrowUp),
            0x50 => KeyEvent::Special(SpecialKey::ArrowDown),
            0x4B => KeyEvent::Special(SpecialKey::ArrowLeft),
            0x4D => KeyEvent::Special(SpecialKey::ArrowRight),
            _ => {
                if let Some(ch) = self.scancode_to_ascii(scancode) {
                    self.line_buffer.push(ch);
                    KeyEvent::Press(ch)
                } else {
                    KeyEvent::Unknown(scancode)
                }
            }
        }
    }

    /// Get the current line buffer (in-progress input).
    pub fn line_buffer(&self) -> &str {
        &self.line_buffer
    }

    /// Pop a completed line (from Enter key).
    pub fn pop_line(&mut self) -> Option<String> {
        if self.completed_lines.is_empty() {
            None
        } else {
            Some(self.completed_lines.remove(0))
        }
    }

    /// Check if there are completed lines.
    pub fn has_line(&self) -> bool {
        !self.completed_lines.is_empty()
    }

    /// Translate scancode set 1 to ASCII.
    fn scancode_to_ascii(&self, scancode: u8) -> Option<char> {
        let shifted = self.is_shift() ^ self.caps_lock;

        // Scancode set 1 mapping (US QWERTY)
        let ch = match scancode {
            // Number row
            0x02 => {
                if self.is_shift() {
                    '!'
                } else {
                    '1'
                }
            }
            0x03 => {
                if self.is_shift() {
                    '@'
                } else {
                    '2'
                }
            }
            0x04 => {
                if self.is_shift() {
                    '#'
                } else {
                    '3'
                }
            }
            0x05 => {
                if self.is_shift() {
                    '$'
                } else {
                    '4'
                }
            }
            0x06 => {
                if self.is_shift() {
                    '%'
                } else {
                    '5'
                }
            }
            0x07 => {
                if self.is_shift() {
                    '^'
                } else {
                    '6'
                }
            }
            0x08 => {
                if self.is_shift() {
                    '&'
                } else {
                    '7'
                }
            }
            0x09 => {
                if self.is_shift() {
                    '*'
                } else {
                    '8'
                }
            }
            0x0A => {
                if self.is_shift() {
                    '('
                } else {
                    '9'
                }
            }
            0x0B => {
                if self.is_shift() {
                    ')'
                } else {
                    '0'
                }
            }
            0x0C => {
                if self.is_shift() {
                    '_'
                } else {
                    '-'
                }
            }
            0x0D => {
                if self.is_shift() {
                    '+'
                } else {
                    '='
                }
            }
            // QWERTY row
            0x10 => {
                if shifted {
                    'Q'
                } else {
                    'q'
                }
            }
            0x11 => {
                if shifted {
                    'W'
                } else {
                    'w'
                }
            }
            0x12 => {
                if shifted {
                    'E'
                } else {
                    'e'
                }
            }
            0x13 => {
                if shifted {
                    'R'
                } else {
                    'r'
                }
            }
            0x14 => {
                if shifted {
                    'T'
                } else {
                    't'
                }
            }
            0x15 => {
                if shifted {
                    'Y'
                } else {
                    'y'
                }
            }
            0x16 => {
                if shifted {
                    'U'
                } else {
                    'u'
                }
            }
            0x17 => {
                if shifted {
                    'I'
                } else {
                    'i'
                }
            }
            0x18 => {
                if shifted {
                    'O'
                } else {
                    'o'
                }
            }
            0x19 => {
                if shifted {
                    'P'
                } else {
                    'p'
                }
            }
            // ASDF row
            0x1E => {
                if shifted {
                    'A'
                } else {
                    'a'
                }
            }
            0x1F => {
                if shifted {
                    'S'
                } else {
                    's'
                }
            }
            0x20 => {
                if shifted {
                    'D'
                } else {
                    'd'
                }
            }
            0x21 => {
                if shifted {
                    'F'
                } else {
                    'f'
                }
            }
            0x22 => {
                if shifted {
                    'G'
                } else {
                    'g'
                }
            }
            0x23 => {
                if shifted {
                    'H'
                } else {
                    'h'
                }
            }
            0x24 => {
                if shifted {
                    'J'
                } else {
                    'j'
                }
            }
            0x25 => {
                if shifted {
                    'K'
                } else {
                    'k'
                }
            }
            0x26 => {
                if shifted {
                    'L'
                } else {
                    'l'
                }
            }
            0x27 => {
                if self.is_shift() {
                    ':'
                } else {
                    ';'
                }
            }
            0x28 => {
                if self.is_shift() {
                    '"'
                } else {
                    '\''
                }
            }
            0x29 => {
                if self.is_shift() {
                    '~'
                } else {
                    '`'
                }
            }
            // ZXCV row
            0x2C => {
                if shifted {
                    'Z'
                } else {
                    'z'
                }
            }
            0x2D => {
                if shifted {
                    'X'
                } else {
                    'x'
                }
            }
            0x2E => {
                if shifted {
                    'C'
                } else {
                    'c'
                }
            }
            0x2F => {
                if shifted {
                    'V'
                } else {
                    'v'
                }
            }
            0x30 => {
                if shifted {
                    'B'
                } else {
                    'b'
                }
            }
            0x31 => {
                if shifted {
                    'N'
                } else {
                    'n'
                }
            }
            0x32 => {
                if shifted {
                    'M'
                } else {
                    'm'
                }
            }
            0x33 => {
                if self.is_shift() {
                    '<'
                } else {
                    ','
                }
            }
            0x34 => {
                if self.is_shift() {
                    '>'
                } else {
                    '.'
                }
            }
            0x35 => {
                if self.is_shift() {
                    '?'
                } else {
                    '/'
                }
            }
            // Space
            0x39 => ' ',
            // Punctuation
            0x1A => {
                if self.is_shift() {
                    '{'
                } else {
                    '['
                }
            }
            0x1B => {
                if self.is_shift() {
                    '}'
                } else {
                    ']'
                }
            }
            0x2B => {
                if self.is_shift() {
                    '|'
                } else {
                    '\\'
                }
            }
            _ => return None,
        };

        Some(ch)
    }
}

impl Default for Keyboard {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_basic_letters() {
        let mut kb = Keyboard::new();
        // Scancode 0x1E = 'a'
        assert_eq!(kb.process_scancode(0x1E), KeyEvent::Press('a'));
        // Scancode 0x30 = 'b'
        assert_eq!(kb.process_scancode(0x30), KeyEvent::Press('b'));
        assert_eq!(kb.line_buffer(), "ab");
    }

    #[test]
    fn keyboard_shift_uppercase() {
        let mut kb = Keyboard::new();
        // Press left shift
        assert_eq!(
            kb.process_scancode(0x2A),
            KeyEvent::Special(SpecialKey::LShiftDown)
        );
        assert!(kb.is_shift());
        // 'a' -> 'A'
        assert_eq!(kb.process_scancode(0x1E), KeyEvent::Press('A'));
        // Release shift
        assert_eq!(
            kb.process_scancode(0xAA), // 0x2A | 0x80
            KeyEvent::Special(SpecialKey::LShiftUp)
        );
        assert!(!kb.is_shift());
        // 'a' -> 'a'
        assert_eq!(kb.process_scancode(0x1E), KeyEvent::Press('a'));
    }

    #[test]
    fn keyboard_enter_completes_line() {
        let mut kb = Keyboard::new();
        kb.process_scancode(0x23); // 'h'
        kb.process_scancode(0x17); // 'i'
        assert_eq!(kb.line_buffer(), "hi");
        assert!(!kb.has_line());

        kb.process_scancode(0x1C); // Enter
        assert!(kb.has_line());
        assert_eq!(kb.pop_line(), Some("hi".to_string()));
        assert_eq!(kb.line_buffer(), ""); // Buffer cleared
    }

    #[test]
    fn keyboard_backspace() {
        let mut kb = Keyboard::new();
        kb.process_scancode(0x1E); // 'a'
        kb.process_scancode(0x30); // 'b'
        assert_eq!(kb.line_buffer(), "ab");

        kb.process_scancode(0x0E); // Backspace
        assert_eq!(kb.line_buffer(), "a");
    }

    #[test]
    fn keyboard_numbers_and_symbols() {
        let mut kb = Keyboard::new();
        // '1' = scancode 0x02
        assert_eq!(kb.process_scancode(0x02), KeyEvent::Press('1'));
        // Space = scancode 0x39
        assert_eq!(kb.process_scancode(0x39), KeyEvent::Press(' '));
        // Shift + '1' = '!'
        kb.process_scancode(0x2A); // shift down
        assert_eq!(kb.process_scancode(0x02), KeyEvent::Press('!'));
    }

    #[test]
    fn keyboard_special_keys() {
        let mut kb = Keyboard::new();
        assert_eq!(
            kb.process_scancode(0x01),
            KeyEvent::Special(SpecialKey::Escape)
        );
        assert_eq!(
            kb.process_scancode(0x0F),
            KeyEvent::Special(SpecialKey::Tab)
        );
    }

    #[test]
    fn keyboard_unknown_scancode() {
        let mut kb = Keyboard::new();
        assert_eq!(kb.process_scancode(0x7F), KeyEvent::Unknown(0x7F));
    }
}
