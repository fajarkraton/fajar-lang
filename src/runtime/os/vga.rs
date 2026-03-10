//! VGA text-mode buffer (0xB8000) for x86 kernel output.
//!
//! Simulates the standard 80×25 VGA text buffer used by x86 kernels
//! for early console output before framebuffer drivers are loaded.
//!
//! Each character cell is 2 bytes: `[ASCII char, attribute byte]`.
//! Attribute byte: `[blink:1][bg:3][fg:4]`.

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// VGA text buffer base address (physical).
pub const VGA_BUFFER_ADDR: u64 = 0xB8000;
/// Number of columns.
pub const VGA_WIDTH: usize = 80;
/// Number of rows.
pub const VGA_HEIGHT: usize = 25;
/// Total cells.
pub const VGA_CELLS: usize = VGA_WIDTH * VGA_HEIGHT;
/// Buffer size in bytes (2 bytes per cell).
pub const VGA_BUFFER_SIZE: usize = VGA_CELLS * 2;

// ═══════════════════════════════════════════════════════════════════════
// Colors
// ═══════════════════════════════════════════════════════════════════════

/// VGA text-mode color codes (4-bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VgaColor {
    /// Black.
    Black = 0,
    /// Blue.
    Blue = 1,
    /// Green.
    Green = 2,
    /// Cyan.
    Cyan = 3,
    /// Red.
    Red = 4,
    /// Magenta.
    Magenta = 5,
    /// Brown.
    Brown = 6,
    /// Light gray.
    LightGray = 7,
    /// Dark gray.
    DarkGray = 8,
    /// Light blue.
    LightBlue = 9,
    /// Light green.
    LightGreen = 10,
    /// Light cyan.
    LightCyan = 11,
    /// Light red.
    LightRed = 12,
    /// Pink.
    Pink = 13,
    /// Yellow.
    Yellow = 14,
    /// White.
    White = 15,
}

/// Combine foreground and background into an attribute byte.
pub fn color_code(fg: VgaColor, bg: VgaColor) -> u8 {
    (bg as u8) << 4 | (fg as u8)
}

// ═══════════════════════════════════════════════════════════════════════
// VGA errors
// ═══════════════════════════════════════════════════════════════════════

/// VGA buffer errors.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum VgaError {
    /// Cursor position out of bounds.
    #[error("VGA cursor out of bounds: ({col}, {row})")]
    OutOfBounds { col: usize, row: usize },
}

// ═══════════════════════════════════════════════════════════════════════
// VGA text buffer
// ═══════════════════════════════════════════════════════════════════════

/// Simulated VGA text-mode buffer.
///
/// Provides `write_byte`, `write_string`, `clear`, and scrolling.
/// In a real kernel, writes go to physical address 0xB8000 via MMIO.
#[derive(Debug)]
pub struct VgaBuffer {
    /// Buffer data: 80×25 cells, 2 bytes each.
    buffer: Vec<u8>,
    /// Current column position.
    col: usize,
    /// Current row position.
    row: usize,
    /// Current color attribute.
    color: u8,
    /// Output capture for testing.
    output_log: String,
}

impl VgaBuffer {
    /// Create a new VGA buffer with default colors (light gray on black).
    pub fn new() -> Self {
        Self {
            buffer: vec![0u8; VGA_BUFFER_SIZE],
            col: 0,
            row: 0,
            color: color_code(VgaColor::LightGray, VgaColor::Black),
            output_log: String::new(),
        }
    }

    /// Set the current text color.
    pub fn set_color(&mut self, fg: VgaColor, bg: VgaColor) {
        self.color = color_code(fg, bg);
    }

    /// Write a single byte at the current cursor position.
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            b'\r' => self.col = 0,
            byte => {
                if self.col >= VGA_WIDTH {
                    self.new_line();
                }
                let offset = (self.row * VGA_WIDTH + self.col) * 2;
                self.buffer[offset] = byte;
                self.buffer[offset + 1] = self.color;
                self.output_log.push(byte as char);
                self.col += 1;
            }
        }
    }

    /// Write a string to the buffer.
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // Printable ASCII or newline
                0x20..=0x7e | b'\n' | b'\r' => self.write_byte(byte),
                // Non-printable: show block character
                _ => self.write_byte(0xFE),
            }
        }
    }

    /// Write a line (string + newline).
    pub fn write_line(&mut self, s: &str) {
        self.write_string(s);
        self.write_byte(b'\n');
    }

    /// Clear the entire screen.
    pub fn clear(&mut self) {
        let blank_attr = color_code(VgaColor::LightGray, VgaColor::Black);
        for i in 0..VGA_CELLS {
            self.buffer[i * 2] = b' ';
            self.buffer[i * 2 + 1] = blank_attr;
        }
        self.col = 0;
        self.row = 0;
    }

    /// Get the character at (col, row).
    pub fn char_at(&self, col: usize, row: usize) -> Result<u8, VgaError> {
        if col >= VGA_WIDTH || row >= VGA_HEIGHT {
            return Err(VgaError::OutOfBounds { col, row });
        }
        let offset = (row * VGA_WIDTH + col) * 2;
        Ok(self.buffer[offset])
    }

    /// Get the attribute byte at (col, row).
    pub fn attr_at(&self, col: usize, row: usize) -> Result<u8, VgaError> {
        if col >= VGA_WIDTH || row >= VGA_HEIGHT {
            return Err(VgaError::OutOfBounds { col, row });
        }
        let offset = (row * VGA_WIDTH + col) * 2;
        Ok(self.buffer[offset + 1])
    }

    /// Get current cursor position.
    pub fn cursor(&self) -> (usize, usize) {
        (self.col, self.row)
    }

    /// Get the output log (all characters written).
    pub fn output_log(&self) -> &str {
        &self.output_log
    }

    /// Clear the output log.
    pub fn clear_log(&mut self) {
        self.output_log.clear();
    }

    /// Get raw buffer reference.
    pub fn raw_buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Advance to the next line, scrolling if needed.
    fn new_line(&mut self) {
        self.output_log.push('\n');
        self.col = 0;
        if self.row < VGA_HEIGHT - 1 {
            self.row += 1;
        } else {
            self.scroll_up();
        }
    }

    /// Scroll all rows up by one, clearing the bottom row.
    fn scroll_up(&mut self) {
        // Move rows 1..25 to 0..24
        let row_bytes = VGA_WIDTH * 2;
        for row in 1..VGA_HEIGHT {
            let src_start = row * row_bytes;
            let dst_start = (row - 1) * row_bytes;
            // Copy within same buffer
            for i in 0..row_bytes {
                self.buffer[dst_start + i] = self.buffer[src_start + i];
            }
        }
        // Clear last row
        let last_row_start = (VGA_HEIGHT - 1) * row_bytes;
        let blank_attr = color_code(VgaColor::LightGray, VgaColor::Black);
        for i in 0..VGA_WIDTH {
            self.buffer[last_row_start + i * 2] = b' ';
            self.buffer[last_row_start + i * 2 + 1] = blank_attr;
        }
    }
}

impl Default for VgaBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Write for VgaBuffer {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vga_write_byte() {
        let mut vga = VgaBuffer::new();
        vga.write_byte(b'A');
        assert_eq!(vga.char_at(0, 0).unwrap(), b'A');
        assert_eq!(vga.cursor(), (1, 0));
    }

    #[test]
    fn vga_write_string() {
        let mut vga = VgaBuffer::new();
        vga.write_string("Hello");
        assert_eq!(vga.char_at(0, 0).unwrap(), b'H');
        assert_eq!(vga.char_at(4, 0).unwrap(), b'o');
        assert_eq!(vga.cursor(), (5, 0));
        assert_eq!(vga.output_log(), "Hello");
    }

    #[test]
    fn vga_newline() {
        let mut vga = VgaBuffer::new();
        vga.write_string("Line1\nLine2");
        assert_eq!(vga.char_at(0, 0).unwrap(), b'L');
        assert_eq!(vga.char_at(0, 1).unwrap(), b'L');
        assert_eq!(vga.cursor(), (5, 1));
    }

    #[test]
    fn vga_clear() {
        let mut vga = VgaBuffer::new();
        vga.write_string("Test");
        vga.clear();
        assert_eq!(vga.char_at(0, 0).unwrap(), b' ');
        assert_eq!(vga.cursor(), (0, 0));
    }

    #[test]
    fn vga_color() {
        let mut vga = VgaBuffer::new();
        vga.set_color(VgaColor::Green, VgaColor::Black);
        vga.write_byte(b'X');
        let attr = vga.attr_at(0, 0).unwrap();
        assert_eq!(attr, color_code(VgaColor::Green, VgaColor::Black));
    }

    #[test]
    fn vga_scroll() {
        let mut vga = VgaBuffer::new();
        // Fill all 25 rows
        for i in 0..VGA_HEIGHT {
            vga.write_line(&format!("Row {i:02}"));
        }
        // After 25 newlines, row 0 should now contain what was row 1
        // (scroll happened when we wrote the 25th newline)
        assert_eq!(vga.char_at(0, 0).unwrap(), b'R');
    }

    #[test]
    fn vga_out_of_bounds() {
        let vga = VgaBuffer::new();
        assert!(vga.char_at(80, 0).is_err());
        assert!(vga.char_at(0, 25).is_err());
    }

    #[test]
    fn vga_line_wrap() {
        let mut vga = VgaBuffer::new();
        // Write exactly 80 chars to fill row 0
        for _ in 0..VGA_WIDTH {
            vga.write_byte(b'X');
        }
        // Next char should wrap to row 1
        vga.write_byte(b'Y');
        assert_eq!(vga.char_at(0, 1).unwrap(), b'Y');
        assert_eq!(vga.cursor(), (1, 1));
    }

    #[test]
    fn vga_buffer_size() {
        let vga = VgaBuffer::new();
        assert_eq!(vga.raw_buffer().len(), VGA_BUFFER_SIZE);
        assert_eq!(VGA_BUFFER_SIZE, 4000);
    }
}
