//! Framebuffer display driver simulation for FajarOS.
//!
//! Provides a simulated framebuffer with pixel drawing, rectangle fill,
//! bitmap font rendering, and double buffering. No real hardware access.

/// Display errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DisplayError {
    /// Coordinates out of bounds.
    #[error("pixel ({x}, {y}) out of bounds ({width}x{height})")]
    OutOfBounds {
        /// X coordinate.
        x: u32,
        /// Y coordinate.
        y: u32,
        /// Framebuffer width.
        width: u32,
        /// Framebuffer height.
        height: u32,
    },
    /// Framebuffer not initialized.
    #[error("framebuffer not initialized")]
    NotInitialized,
}

/// RGBA color (packed 32-bit).
pub type Color = u32;

/// Common colors.
pub mod colors {
    use super::Color;
    /// Black.
    pub const BLACK: Color = 0x00000000;
    /// White.
    pub const WHITE: Color = 0x00FFFFFF;
    /// Red.
    pub const RED: Color = 0x00FF0000;
    /// Green.
    pub const GREEN: Color = 0x0000FF00;
    /// Blue.
    pub const BLUE: Color = 0x000000FF;
    /// Yellow.
    pub const YELLOW: Color = 0x00FFFF00;
    /// Cyan.
    pub const CYAN: Color = 0x0000FFFF;
    /// Magenta.
    pub const MAGENTA: Color = 0x00FF00FF;
}

/// 8x16 bitmap font (ASCII 32-126, simplified).
/// Each character is 16 bytes (16 rows of 8 pixels).
const FONT_WIDTH: u32 = 8;
const FONT_HEIGHT: u32 = 16;

/// Returns a simple bitmap for a character (8x16).
fn char_bitmap(ch: char) -> [u8; 16] {
    // Minimal bitmap font for printable ASCII
    let c = ch as u32;
    if !(32..=126).contains(&c) {
        return [0; 16]; // unprintable
    }
    // Simple block patterns for a few common characters
    match ch {
        'A' => [
            0x00, 0x18, 0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
        'B' => [
            0x00, 0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x66, 0x7C, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
        'F' => [
            0x00, 0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x60, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
        '0' => [
            0x00, 0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
        ' ' => [0; 16],
        // Default: filled block for any other printable char
        _ => [
            0x00, 0x00, 0x7E, 0x7E, 0x7E, 0x7E, 0x7E, 0x7E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
    }
}

/// Simulated framebuffer.
#[derive(Debug)]
pub struct Framebuffer {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Bits per pixel.
    pub bpp: u32,
    /// Pitch (bytes per row).
    pub pitch: u32,
    /// Front buffer (displayed).
    front: Vec<u32>,
    /// Back buffer (drawing target).
    back: Vec<u32>,
    /// Whether initialized.
    pub initialized: bool,
    /// Whether double buffering is enabled.
    pub double_buffered: bool,
    /// Text cursor position (column, row) for text console.
    pub cursor_col: u32,
    /// Text cursor row.
    pub cursor_row: u32,
}

impl Framebuffer {
    /// Creates a new framebuffer with given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            bpp: 32,
            pitch: width * 4,
            front: vec![0; size],
            back: vec![0; size],
            initialized: true,
            double_buffered: false,
            cursor_col: 0,
            cursor_row: 0,
        }
    }

    /// Returns a reference to the active buffer (front or back depending on mode).
    fn active_buf(&self) -> &[u32] {
        if self.double_buffered {
            &self.back
        } else {
            &self.front
        }
    }

    /// Returns a mutable reference to the drawing buffer.
    fn draw_buf(&mut self) -> &mut Vec<u32> {
        if self.double_buffered {
            &mut self.back
        } else {
            &mut self.front
        }
    }

    /// Sets a pixel at (x, y) to the given color.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) -> Result<(), DisplayError> {
        if !self.initialized {
            return Err(DisplayError::NotInitialized);
        }
        if x >= self.width || y >= self.height {
            return Err(DisplayError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        }
        let idx = (y * self.width + x) as usize;
        self.draw_buf()[idx] = color;
        Ok(())
    }

    /// Gets the pixel color at (x, y).
    pub fn get_pixel(&self, x: u32, y: u32) -> Result<Color, DisplayError> {
        if !self.initialized {
            return Err(DisplayError::NotInitialized);
        }
        if x >= self.width || y >= self.height {
            return Err(DisplayError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        }
        let idx = (y * self.width + x) as usize;
        Ok(self.active_buf()[idx])
    }

    /// Fills a rectangle with the given color.
    pub fn fill_rect(
        &mut self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        color: Color,
    ) -> Result<(), DisplayError> {
        if !self.initialized {
            return Err(DisplayError::NotInitialized);
        }
        let x_end = (x + w).min(self.width);
        let y_end = (y + h).min(self.height);
        for py in y..y_end {
            for px in x..x_end {
                let idx = (py * self.width + px) as usize;
                self.draw_buf()[idx] = color;
            }
        }
        Ok(())
    }

    /// Draws a character at pixel position (x, y) using bitmap font.
    pub fn draw_char(
        &mut self,
        x: u32,
        y: u32,
        ch: char,
        fg: Color,
        bg: Color,
    ) -> Result<(), DisplayError> {
        if !self.initialized {
            return Err(DisplayError::NotInitialized);
        }
        let bitmap = char_bitmap(ch);
        for row in 0..FONT_HEIGHT {
            if y + row >= self.height {
                break;
            }
            let bits = bitmap[row as usize];
            for col in 0..FONT_WIDTH {
                if x + col >= self.width {
                    break;
                }
                let pixel = if bits & (0x80 >> col) != 0 { fg } else { bg };
                let idx = ((y + row) * self.width + (x + col)) as usize;
                self.draw_buf()[idx] = pixel;
            }
        }
        Ok(())
    }

    /// Draws a string at pixel position (x, y).
    pub fn draw_text(
        &mut self,
        x: u32,
        y: u32,
        text: &str,
        fg: Color,
        bg: Color,
    ) -> Result<(), DisplayError> {
        let mut cx = x;
        for ch in text.chars() {
            if cx + FONT_WIDTH > self.width {
                break;
            }
            self.draw_char(cx, y, ch, fg, bg)?;
            cx += FONT_WIDTH;
        }
        Ok(())
    }

    /// Clears the framebuffer to a color.
    pub fn clear(&mut self, color: Color) {
        self.draw_buf().fill(color);
        self.cursor_col = 0;
        self.cursor_row = 0;
    }

    /// Enables double buffering.
    pub fn enable_double_buffer(&mut self) {
        self.double_buffered = true;
    }

    /// Swaps front and back buffers (for double buffering).
    pub fn swap_buffers(&mut self) {
        if self.double_buffered {
            std::mem::swap(&mut self.front, &mut self.back);
        }
    }

    /// Returns the number of text columns.
    pub fn text_cols(&self) -> u32 {
        self.width / FONT_WIDTH
    }

    /// Returns the number of text rows.
    pub fn text_rows(&self) -> u32 {
        self.height / FONT_HEIGHT
    }
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new(320, 200)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s28_1_framebuffer_creation() {
        let fb = Framebuffer::new(640, 480);
        assert_eq!(fb.width, 640);
        assert_eq!(fb.height, 480);
        assert_eq!(fb.bpp, 32);
        assert!(fb.initialized);
    }

    #[test]
    fn s28_2_set_and_get_pixel() {
        let mut fb = Framebuffer::new(320, 200);
        fb.set_pixel(10, 20, colors::RED).unwrap();
        assert_eq!(fb.get_pixel(10, 20).unwrap(), colors::RED);
        assert_eq!(fb.get_pixel(0, 0).unwrap(), colors::BLACK);
    }

    #[test]
    fn s28_3_pixel_out_of_bounds() {
        let mut fb = Framebuffer::new(100, 100);
        assert!(matches!(
            fb.set_pixel(100, 0, colors::WHITE),
            Err(DisplayError::OutOfBounds { .. })
        ));
        assert!(matches!(
            fb.set_pixel(0, 100, colors::WHITE),
            Err(DisplayError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn s28_4_fill_rect() {
        let mut fb = Framebuffer::new(100, 100);
        fb.fill_rect(10, 10, 5, 5, colors::BLUE).unwrap();
        assert_eq!(fb.get_pixel(12, 12).unwrap(), colors::BLUE);
        assert_eq!(fb.get_pixel(9, 9).unwrap(), colors::BLACK);
        assert_eq!(fb.get_pixel(15, 15).unwrap(), colors::BLACK);
    }

    #[test]
    fn s28_5_draw_char() {
        let mut fb = Framebuffer::new(100, 100);
        fb.draw_char(0, 0, 'A', colors::WHITE, colors::BLACK)
            .unwrap();
        // 'A' has pixels set in its bitmap (row 1 = 0x18 = bits at cols 3,4)
        assert_eq!(fb.get_pixel(3, 1).unwrap(), colors::WHITE);
        assert_eq!(fb.get_pixel(4, 1).unwrap(), colors::WHITE);
    }

    #[test]
    fn s28_6_draw_text() {
        let mut fb = Framebuffer::new(200, 100);
        fb.draw_text(0, 0, "AB", colors::GREEN, colors::BLACK)
            .unwrap();
        // First char at x=0, second at x=8
        // 'A' row 1 col 3 should be green
        assert_eq!(fb.get_pixel(3, 1).unwrap(), colors::GREEN);
    }

    #[test]
    fn s28_7_clear() {
        let mut fb = Framebuffer::new(100, 100);
        fb.set_pixel(50, 50, colors::RED).unwrap();
        fb.clear(colors::BLUE);
        assert_eq!(fb.get_pixel(50, 50).unwrap(), colors::BLUE);
        assert_eq!(fb.get_pixel(0, 0).unwrap(), colors::BLUE);
    }

    #[test]
    fn s28_8_double_buffering() {
        let mut fb = Framebuffer::new(100, 100);
        fb.enable_double_buffer();

        // Draw to back buffer
        fb.set_pixel(10, 10, colors::RED).unwrap();
        // Front buffer still black
        assert_eq!(fb.front[10 * 100 + 10], colors::BLACK);

        // Swap
        fb.swap_buffers();
        // Now front has the pixel
        assert_eq!(fb.front[10 * 100 + 10], colors::RED);
    }

    #[test]
    fn s28_9_text_dimensions() {
        let fb = Framebuffer::new(640, 480);
        assert_eq!(fb.text_cols(), 80);
        assert_eq!(fb.text_rows(), 30);
    }

    #[test]
    fn s28_10_fill_rect_clipping() {
        let mut fb = Framebuffer::new(100, 100);
        // Fill rect that extends beyond bounds — should clip, not panic
        fb.fill_rect(90, 90, 20, 20, colors::YELLOW).unwrap();
        assert_eq!(fb.get_pixel(95, 95).unwrap(), colors::YELLOW);
        assert_eq!(fb.get_pixel(99, 99).unwrap(), colors::YELLOW);
    }
}
