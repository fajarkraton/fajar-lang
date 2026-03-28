//! GUI widget toolkit — colors, geometry, canvas, events, and all standard widgets.
//!
//! This module provides a complete set of cross-platform GUI primitives
//! for building desktop applications in Fajar Lang, including:
//!
//! - **Color** — RGBA color with named constants and blending
//! - **Rect** — Axis-aligned bounding rectangle with hit-testing
//! - **Canvas** — Pixel buffer with draw primitives
//! - **Event** — Input and window events
//! - **Widget** — Trait implemented by all interactive controls
//! - **Widgets** — Button, Label, TextInput, TextArea, Checkbox, RadioButton,
//!   Slider, ProgressBar, Dropdown, ListView, TreeView, Table, Image,
//!   TabView, SplitView, ScrollView
//! - **Dialog** — Alert, confirm, and prompt dialogs
//! - **Menu** — Menu bar and menu items with shortcuts
//! - **Theme** — Light, dark, and high-contrast themes
//! - **StatusBar** — Segmented status display

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Color
// ═══════════════════════════════════════════════════════════════════════

/// An RGBA color with 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red channel (0-255).
    pub r: u8,
    /// Green channel (0-255).
    pub g: u8,
    /// Blue channel (0-255).
    pub b: u8,
    /// Alpha channel (0 = transparent, 255 = opaque).
    pub a: u8,
}

impl Color {
    /// Fully opaque black.
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    /// Fully opaque white.
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    /// Fully opaque red.
    pub const RED: Color = Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    /// Fully opaque green.
    pub const GREEN: Color = Color {
        r: 0,
        g: 128,
        b: 0,
        a: 255,
    };
    /// Fully opaque blue.
    pub const BLUE: Color = Color {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };
    /// Fully opaque yellow.
    pub const YELLOW: Color = Color {
        r: 255,
        g: 255,
        b: 0,
        a: 255,
    };
    /// Fully opaque cyan.
    pub const CYAN: Color = Color {
        r: 0,
        g: 255,
        b: 255,
        a: 255,
    };
    /// Fully opaque magenta.
    pub const MAGENTA: Color = Color {
        r: 255,
        g: 0,
        b: 255,
        a: 255,
    };
    /// Mid-gray.
    pub const GRAY: Color = Color {
        r: 128,
        g: 128,
        b: 128,
        a: 255,
    };
    /// Fully transparent.
    pub const TRANSPARENT: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    /// Fully opaque orange.
    pub const ORANGE: Color = Color {
        r: 255,
        g: 165,
        b: 0,
        a: 255,
    };
    /// Fully opaque purple.
    pub const PURPLE: Color = Color {
        r: 128,
        g: 0,
        b: 128,
        a: 255,
    };

    /// Creates a new opaque color from RGB values.
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Creates a color with an explicit alpha channel.
    pub fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parses a hex color string. Supports `#RRGGBB` and `#RRGGBBAA`.
    ///
    /// Returns `None` if the string is not a valid hex color.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
                let g = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
                let b = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
                Some(Self { r, g, b, a: 255 })
            }
            8 => {
                let r = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
                let g = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
                let b = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
                let a = u8::from_str_radix(hex.get(6..8)?, 16).ok()?;
                Some(Self { r, g, b, a })
            }
            _ => None,
        }
    }

    /// Converts this color to a `#RRGGBB` or `#RRGGBBAA` hex string.
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }

    /// Alpha-blends `other` on top of `self` using standard "source over" compositing.
    pub fn blend(&self, other: &Color) -> Color {
        if other.a == 255 {
            return *other;
        }
        if other.a == 0 {
            return *self;
        }
        let sa = other.a as f32 / 255.0;
        let da = self.a as f32 / 255.0;
        let out_a = sa + da * (1.0 - sa);
        if out_a < f32::EPSILON {
            return Color::TRANSPARENT;
        }
        let blend_ch = |sc: u8, dc: u8| -> u8 {
            let s = sc as f32 / 255.0;
            let d = dc as f32 / 255.0;
            let out = (s * sa + d * da * (1.0 - sa)) / out_a;
            (out * 255.0).round().clamp(0.0, 255.0) as u8
        };
        Color {
            r: blend_ch(other.r, self.r),
            g: blend_ch(other.g, self.g),
            b: blend_ch(other.b, self.b),
            a: (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Rect
// ═══════════════════════════════════════════════════════════════════════

/// An axis-aligned rectangle defined by its top-left corner and dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// X coordinate of the left edge.
    pub x: f32,
    /// Y coordinate of the top edge.
    pub y: f32,
    /// Width (non-negative).
    pub width: f32,
    /// Height (non-negative).
    pub height: f32,
}

impl Rect {
    /// Creates a new rectangle.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width: width.max(0.0),
            height: height.max(0.0),
        }
    }

    /// A zero-size rectangle at the origin.
    pub const ZERO: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    /// Right edge (`x + width`).
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge (`y + height`).
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Center point.
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Returns `true` if the point `(px, py)` lies inside this rectangle.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }

    /// Returns `true` if this rectangle overlaps with `other`.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Returns the smallest rectangle that contains both `self` and `other`.
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, right - x, bottom - y)
    }

    /// Returns a rectangle inset by `dx` horizontally and `dy` vertically.
    /// If the inset would collapse the rectangle, returns a zero-size rect at the center.
    pub fn inset(&self, dx: f32, dy: f32) -> Rect {
        let new_w = self.width - 2.0 * dx;
        let new_h = self.height - 2.0 * dy;
        if new_w <= 0.0 || new_h <= 0.0 {
            let (cx, cy) = self.center();
            return Rect::new(cx, cy, 0.0, 0.0);
        }
        Rect::new(self.x + dx, self.y + dy, new_w, new_h)
    }

    /// Returns the intersection of two rectangles, or `None` if they do not overlap.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        if !self.intersects(other) {
            return None;
        }
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Some(Rect::new(x, y, right - x, bottom - y))
    }

    /// Area of the rectangle.
    pub fn area(&self) -> f32 {
        self.width * self.height
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Rect({}, {}, {}x{})",
            self.x, self.y, self.width, self.height
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Event
// ═══════════════════════════════════════════════════════════════════════

/// Keyboard modifier flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Shift key held.
    pub shift: bool,
    /// Ctrl key held.
    pub ctrl: bool,
    /// Alt/Option key held.
    pub alt: bool,
    /// Super/Command/Win key held.
    pub super_key: bool,
}

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Left / primary button.
    Left,
    /// Right / secondary button.
    Right,
    /// Middle / wheel button.
    Middle,
}

/// A GUI event dispatched to widgets.
#[derive(Debug, Clone)]
pub enum Event {
    /// Mouse button pressed.
    MouseDown {
        /// X coordinate relative to the widget.
        x: f32,
        /// Y coordinate relative to the widget.
        y: f32,
        /// Which button was pressed.
        button: MouseButton,
        /// Modifier keys.
        modifiers: Modifiers,
    },
    /// Mouse button released.
    MouseUp {
        /// X coordinate.
        x: f32,
        /// Y coordinate.
        y: f32,
        /// Which button was released.
        button: MouseButton,
    },
    /// Mouse pointer moved.
    MouseMove {
        /// New X coordinate.
        x: f32,
        /// New Y coordinate.
        y: f32,
    },
    /// Keyboard key pressed.
    KeyDown {
        /// Key name (e.g. "a", "Enter", "Backspace").
        key: String,
        /// Modifier keys.
        modifiers: Modifiers,
    },
    /// Keyboard key released.
    KeyUp {
        /// Key name.
        key: String,
    },
    /// Window / widget resized.
    Resize {
        /// New width.
        width: f32,
        /// New height.
        height: f32,
    },
    /// Close request (e.g. window close button).
    Close,
    /// Mouse wheel or trackpad scroll.
    Scroll {
        /// Horizontal scroll delta.
        dx: f32,
        /// Vertical scroll delta.
        dy: f32,
    },
    /// Widget gained focus.
    Focus,
    /// Widget lost focus.
    Blur,
}

// ═══════════════════════════════════════════════════════════════════════
// Widget trait
// ═══════════════════════════════════════════════════════════════════════

/// The core trait that all GUI widgets implement.
pub trait Widget {
    /// Renders this widget onto the given canvas.
    fn render(&self, canvas: &mut Canvas);

    /// Handles an event. Returns `true` if the event was consumed.
    fn handle_event(&mut self, event: &Event) -> bool;

    /// Returns the bounding rectangle of this widget.
    fn bounds(&self) -> Rect;

    /// Sets the bounding rectangle of this widget.
    fn set_bounds(&mut self, rect: Rect);
}

// ═══════════════════════════════════════════════════════════════════════
// Canvas
// ═══════════════════════════════════════════════════════════════════════

/// A software pixel buffer that widgets render into.
#[derive(Debug, Clone)]
pub struct Canvas {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major RGBA pixel data (`width * height` entries).
    pub pixels: Vec<Color>,
}

impl Canvas {
    /// Creates a new canvas filled with the given background color.
    pub fn new(width: u32, height: u32, bg: Color) -> Self {
        let count = (width as usize) * (height as usize);
        Self {
            width,
            height,
            pixels: vec![bg; count],
        }
    }

    /// Clears the entire canvas to the given color.
    pub fn clear(&mut self, color: Color) {
        for px in &mut self.pixels {
            *px = color;
        }
    }

    /// Sets a single pixel if the coordinates are within bounds.
    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            let idx = (y as u32 * self.width + x as u32) as usize;
            self.pixels[idx] = self.pixels[idx].blend(&color);
        }
    }

    /// Returns the color at `(x, y)`, or `None` if out of bounds.
    pub fn get_pixel(&self, x: i32, y: i32) -> Option<Color> {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            let idx = (y as u32 * self.width + x as u32) as usize;
            Some(self.pixels[idx])
        } else {
            None
        }
    }

    /// Fills a rectangle with the given color using alpha blending.
    pub fn fill_rect(&mut self, rect: &Rect, color: Color) {
        let x0 = (rect.x as i32).max(0) as u32;
        let y0 = (rect.y as i32).max(0) as u32;
        let x1 = (rect.right() as u32).min(self.width);
        let y1 = (rect.bottom() as u32).min(self.height);
        for py in y0..y1 {
            for px in x0..x1 {
                let idx = (py * self.width + px) as usize;
                self.pixels[idx] = self.pixels[idx].blend(&color);
            }
        }
    }

    /// Draws a 1-pixel-wide rectangle outline.
    pub fn draw_rect(&mut self, rect: &Rect, color: Color) {
        let x0 = rect.x as i32;
        let y0 = rect.y as i32;
        let x1 = rect.right() as i32 - 1;
        let y1 = rect.bottom() as i32 - 1;
        // Top and bottom edges
        for x in x0..=x1 {
            self.set_pixel(x, y0, color);
            self.set_pixel(x, y1, color);
        }
        // Left and right edges (excluding corners already drawn)
        for y in (y0 + 1)..y1 {
            self.set_pixel(x0, y, color);
            self.set_pixel(x1, y, color);
        }
    }

    /// Draws a line from `(x0, y0)` to `(x1, y1)` using Bresenham's algorithm.
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let mut x = x0;
        let mut y = y0;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.set_pixel(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = err.saturating_mul(2);
            if e2 >= dy {
                if x == x1 {
                    break;
                }
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                if y == y1 {
                    break;
                }
                err += dx;
                y += sy;
            }
        }
    }

    /// Draws a circle outline using the midpoint circle algorithm.
    pub fn draw_circle(&mut self, cx: i32, cy: i32, radius: i32, color: Color) {
        if radius <= 0 {
            self.set_pixel(cx, cy, color);
            return;
        }
        let mut x = 0;
        let mut y = radius;
        let mut d = 1 - radius;
        while x <= y {
            self.set_pixel(cx + x, cy + y, color);
            self.set_pixel(cx - x, cy + y, color);
            self.set_pixel(cx + x, cy - y, color);
            self.set_pixel(cx - x, cy - y, color);
            self.set_pixel(cx + y, cy + x, color);
            self.set_pixel(cx - y, cy + x, color);
            self.set_pixel(cx + y, cy - x, color);
            self.set_pixel(cx - y, cy - x, color);
            x += 1;
            if d < 0 {
                d = d.saturating_add(x.saturating_mul(2).saturating_add(1));
            } else {
                y -= 1;
                d = d.saturating_add((x - y).saturating_mul(2).saturating_add(1));
            }
        }
    }

    /// Draws text at `(x, y)` using a minimal built-in 5x7 bitmap font.
    ///
    /// Only ASCII printable characters (32-126) are supported. Each glyph
    /// occupies a 6x8 cell (5 wide + 1px spacing, 7 tall + 1px spacing).
    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, color: Color) {
        let mut cx = x;
        for ch in text.chars() {
            if ch == ' ' {
                cx += 6;
                continue;
            }
            let glyph = bitmap_glyph(ch);
            for (row, &bits) in glyph.iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        self.set_pixel(cx + col, y + row as i32, color);
                    }
                }
            }
            cx += 6;
        }
    }
}

/// Returns a 7-row bitmap for a character (each row is 5 bits, MSB-first).
/// Unrecognized characters render as a filled block.
fn bitmap_glyph(ch: char) -> [u8; 7] {
    match ch {
        'A' | 'a' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' | 'b' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' | 'c' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' | 'd' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' | 'e' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' | 'f' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' | 'g' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' | 'h' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' | 'i' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' | 'j' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' | 'k' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' | 'l' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' | 'm' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' | 'n' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' | 'o' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' | 'p' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' | 'q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' | 'r' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' | 's' => [
            0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110,
        ],
        'T' | 't' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' | 'u' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' | 'v' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100,
        ],
        'W' | 'w' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' | 'x' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' | 'y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' | 'z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
        ],
        '3' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b01000,
        ],
        ':' => [
            0b00000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00100, 0b00000,
        ],
        ';' => [
            0b00000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00100, 0b01000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        '!' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
        '?' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b00100, 0b00000, 0b00100,
        ],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        _ => [
            0b11111, 0b11111, 0b11111, 0b11111, 0b11111, 0b11111, 0b11111,
        ],
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Text alignment
// ═══════════════════════════════════════════════════════════════════════

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Left-aligned (default).
    #[default]
    Left,
    /// Center-aligned.
    Center,
    /// Right-aligned.
    Right,
}

/// Orientation for sliders and split views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Horizontal layout.
    Horizontal,
    /// Vertical layout.
    Vertical,
}

/// Image scaling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    /// Scale to fit within bounds, preserving aspect ratio.
    Fit,
    /// Scale to fill bounds, cropping excess.
    Fill,
    /// Stretch to exactly match bounds.
    Stretch,
}

// ═══════════════════════════════════════════════════════════════════════
// Button
// ═══════════════════════════════════════════════════════════════════════

/// A clickable button widget.
#[derive(Debug, Clone)]
pub struct Button {
    /// Display text.
    pub text: String,
    /// Bounding rectangle.
    pub rect: Rect,
    /// Whether the button is currently pressed (mouse down inside).
    pub pressed: bool,
    /// Whether the mouse is hovering over the button.
    pub hovered: bool,
    /// Set to `true` when a click completes (mouse up while pressed).
    pub clicked: bool,
    /// Whether the button is enabled.
    pub enabled: bool,
}

impl Button {
    /// Creates a new button with the given text and bounds.
    pub fn new(text: &str, rect: Rect) -> Self {
        Self {
            text: text.to_string(),
            rect,
            pressed: false,
            hovered: false,
            clicked: false,
            enabled: true,
        }
    }
}

impl Widget for Button {
    fn render(&self, canvas: &mut Canvas) {
        let bg = if !self.enabled {
            Color::GRAY
        } else if self.pressed {
            Color::new(100, 100, 200)
        } else if self.hovered {
            Color::new(180, 180, 220)
        } else {
            Color::new(200, 200, 200)
        };
        canvas.fill_rect(&self.rect, bg);
        canvas.draw_rect(&self.rect, Color::BLACK);
        let tx = self.rect.x as i32 + 4;
        let ty = self.rect.y as i32 + (self.rect.height as i32 - 7) / 2;
        let fg = if self.enabled {
            Color::BLACK
        } else {
            Color::new(160, 160, 160)
        };
        canvas.draw_text(tx, ty, &self.text, fg);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.enabled {
            return false;
        }
        self.clicked = false;
        match event {
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if self.rect.contains(*x, *y) {
                    self.pressed = true;
                    return true;
                }
            }
            Event::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            } => {
                if self.pressed {
                    self.pressed = false;
                    if self.rect.contains(*x, *y) {
                        self.clicked = true;
                    }
                    return true;
                }
            }
            Event::MouseMove { x, y } => {
                let was = self.hovered;
                self.hovered = self.rect.contains(*x, *y);
                return was != self.hovered;
            }
            _ => {}
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Label
// ═══════════════════════════════════════════════════════════════════════

/// A non-interactive text label.
#[derive(Debug, Clone)]
pub struct Label {
    /// Display text.
    pub text: String,
    /// Text alignment within the bounding rectangle.
    pub align: TextAlign,
    /// Text color.
    pub color: Color,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl Label {
    /// Creates a new label with the given text and bounds.
    pub fn new(text: &str, rect: Rect) -> Self {
        Self {
            text: text.to_string(),
            align: TextAlign::Left,
            color: Color::BLACK,
            rect,
        }
    }

    /// Sets the text alignment. Returns `self` for chaining.
    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Sets the text color. Returns `self` for chaining.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl Widget for Label {
    fn render(&self, canvas: &mut Canvas) {
        let text_width = self.text.len() as i32 * 6;
        let tx = match self.align {
            TextAlign::Left => self.rect.x as i32 + 2,
            TextAlign::Center => self.rect.x as i32 + (self.rect.width as i32 - text_width) / 2,
            TextAlign::Right => self.rect.x as i32 + self.rect.width as i32 - text_width - 2,
        };
        let ty = self.rect.y as i32 + (self.rect.height as i32 - 7) / 2;
        canvas.draw_text(tx, ty, &self.text, self.color);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false // labels do not consume events
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TextInput
// ═══════════════════════════════════════════════════════════════════════

/// A single-line text input field.
#[derive(Debug, Clone)]
pub struct TextInput {
    /// Current text value.
    pub value: String,
    /// Cursor position (byte offset, clamped to `value.len()`).
    pub cursor_pos: usize,
    /// Placeholder text shown when value is empty.
    pub placeholder: String,
    /// Whether this input currently has keyboard focus.
    pub focused: bool,
    /// Selection range `(start, end)` as byte offsets, or `None` if no selection.
    pub selection: Option<(usize, usize)>,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl TextInput {
    /// Creates a new text input with the given bounds.
    pub fn new(rect: Rect) -> Self {
        Self {
            value: String::new(),
            cursor_pos: 0,
            placeholder: String::new(),
            focused: false,
            selection: None,
            rect,
        }
    }

    /// Sets the placeholder text. Returns `self` for chaining.
    pub fn with_placeholder(mut self, text: &str) -> Self {
        self.placeholder = text.to_string();
        self
    }

    /// Moves the cursor left by one character.
    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            let mut pos = self.cursor_pos - 1;
            while pos > 0 && !self.value.is_char_boundary(pos) {
                pos -= 1;
            }
            self.cursor_pos = pos;
        }
        self.selection = None;
    }

    /// Moves the cursor right by one character.
    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.value.len() {
            let mut pos = self.cursor_pos + 1;
            while pos < self.value.len() && !self.value.is_char_boundary(pos) {
                pos += 1;
            }
            self.cursor_pos = pos;
        }
        self.selection = None;
    }

    /// Inserts a character at the current cursor position.
    pub fn insert_char(&mut self, ch: char) {
        self.delete_selection();
        self.value.insert(self.cursor_pos, ch);
        self.cursor_pos += ch.len_utf8();
    }

    /// Deletes the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }
        if self.cursor_pos > 0 {
            let mut prev = self.cursor_pos - 1;
            while prev > 0 && !self.value.is_char_boundary(prev) {
                prev -= 1;
            }
            self.value.drain(prev..self.cursor_pos);
            self.cursor_pos = prev;
        }
    }

    /// Deletes the character at the cursor (delete key).
    pub fn delete_forward(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }
        if self.cursor_pos < self.value.len() {
            let mut next = self.cursor_pos + 1;
            while next < self.value.len() && !self.value.is_char_boundary(next) {
                next += 1;
            }
            self.value.drain(self.cursor_pos..next);
        }
    }

    /// Deletes the selected text, if any.
    fn delete_selection(&mut self) {
        if let Some((start, end)) = self.selection.take() {
            let lo = start.min(end);
            let hi = start.max(end);
            self.value.drain(lo..hi);
            self.cursor_pos = lo;
        }
    }

    /// Returns the currently selected text, or an empty string if nothing is selected.
    pub fn selected_text(&self) -> &str {
        match self.selection {
            Some((start, end)) => {
                let lo = start.min(end);
                let hi = start.max(end);
                &self.value[lo..hi]
            }
            None => "",
        }
    }
}

impl Widget for TextInput {
    fn render(&self, canvas: &mut Canvas) {
        let bg = if self.focused {
            Color::WHITE
        } else {
            Color::new(240, 240, 240)
        };
        canvas.fill_rect(&self.rect, bg);
        let border = if self.focused {
            Color::BLUE
        } else {
            Color::GRAY
        };
        canvas.draw_rect(&self.rect, border);
        let tx = self.rect.x as i32 + 4;
        let ty = self.rect.y as i32 + (self.rect.height as i32 - 7) / 2;
        if self.value.is_empty() && !self.placeholder.is_empty() {
            canvas.draw_text(tx, ty, &self.placeholder, Color::GRAY);
        } else {
            canvas.draw_text(tx, ty, &self.value, Color::BLACK);
        }
        if self.focused {
            let cursor_x = tx + (self.cursor_pos as i32) * 6;
            let cy0 = self.rect.y as i32 + 2;
            let cy1 = self.rect.bottom() as i32 - 2;
            canvas.draw_line(cursor_x, cy0, cursor_x, cy1, Color::BLACK);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                let was_focused = self.focused;
                self.focused = self.rect.contains(*x, *y);
                if self.focused {
                    let rel_x = (*x - self.rect.x - 4.0).max(0.0);
                    self.cursor_pos = ((rel_x / 6.0) as usize).min(self.value.len());
                    self.selection = None;
                }
                return was_focused != self.focused || self.focused;
            }
            Event::KeyDown { key, modifiers } if self.focused => {
                match key.as_str() {
                    "Backspace" => self.backspace(),
                    "Delete" => self.delete_forward(),
                    "Left" => self.move_cursor_left(),
                    "Right" => self.move_cursor_right(),
                    "Home" => {
                        self.cursor_pos = 0;
                        self.selection = None;
                    }
                    "End" => {
                        self.cursor_pos = self.value.len();
                        self.selection = None;
                    }
                    k if k.len() == 1 && !modifiers.ctrl => {
                        if let Some(ch) = k.chars().next() {
                            self.insert_char(ch);
                        }
                    }
                    _ => return false,
                }
                return true;
            }
            Event::Focus => {
                self.focused = true;
                return true;
            }
            Event::Blur => {
                self.focused = false;
                return true;
            }
            _ => {}
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TextArea
// ═══════════════════════════════════════════════════════════════════════

/// A multi-line text editing area.
#[derive(Debug, Clone)]
pub struct TextArea {
    /// Lines of text.
    pub lines: Vec<String>,
    /// Current cursor line index.
    pub cursor_line: usize,
    /// Current cursor column (byte offset within the line).
    pub cursor_col: usize,
    /// Vertical scroll offset in lines.
    pub scroll_offset: usize,
    /// Whether this widget has focus.
    pub focused: bool,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl TextArea {
    /// Creates a new text area with the given bounds.
    pub fn new(rect: Rect) -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            scroll_offset: 0,
            focused: false,
            rect,
        }
    }

    /// Returns the number of visible lines that fit in the viewport.
    pub fn visible_lines(&self) -> usize {
        (self.rect.height as usize) / 8
    }

    /// Inserts a character at the cursor.
    pub fn insert_char(&mut self, ch: char) {
        if ch == '\n' {
            let rest = self.lines[self.cursor_line].split_off(self.cursor_col);
            self.cursor_line += 1;
            self.lines.insert(self.cursor_line, rest);
            self.cursor_col = 0;
        } else {
            self.lines[self.cursor_line].insert(self.cursor_col, ch);
            self.cursor_col += ch.len_utf8();
        }
    }

    /// Deletes the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let mut prev = self.cursor_col - 1;
            while prev > 0 && !self.lines[self.cursor_line].is_char_boundary(prev) {
                prev -= 1;
            }
            self.lines[self.cursor_line].drain(prev..self.cursor_col);
            self.cursor_col = prev;
        } else if self.cursor_line > 0 {
            let removed = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
            self.lines[self.cursor_line].push_str(&removed);
        }
    }

    /// Returns the full text content joined by newlines.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Sets the text, splitting on newlines.
    pub fn set_text(&mut self, text: &str) {
        self.lines = text.split('\n').map(String::from).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.scroll_offset = 0;
    }
}

impl Widget for TextArea {
    fn render(&self, canvas: &mut Canvas) {
        let bg = if self.focused {
            Color::WHITE
        } else {
            Color::new(245, 245, 245)
        };
        canvas.fill_rect(&self.rect, bg);
        canvas.draw_rect(&self.rect, Color::GRAY);
        let visible = self.visible_lines();
        let end = (self.scroll_offset + visible).min(self.lines.len());
        let tx = self.rect.x as i32 + 4;
        for (i, line_idx) in (self.scroll_offset..end).enumerate() {
            let ty = self.rect.y as i32 + 2 + (i as i32) * 8;
            canvas.draw_text(tx, ty, &self.lines[line_idx], Color::BLACK);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                self.focused = self.rect.contains(*x, *y);
                return self.focused;
            }
            Event::KeyDown { key, modifiers } if self.focused => {
                match key.as_str() {
                    "Backspace" => self.backspace(),
                    "Enter" => self.insert_char('\n'),
                    "Up" => {
                        if self.cursor_line > 0 {
                            self.cursor_line -= 1;
                            self.cursor_col =
                                self.cursor_col.min(self.lines[self.cursor_line].len());
                        }
                    }
                    "Down" => {
                        if self.cursor_line + 1 < self.lines.len() {
                            self.cursor_line += 1;
                            self.cursor_col =
                                self.cursor_col.min(self.lines[self.cursor_line].len());
                        }
                    }
                    k if k.len() == 1 && !modifiers.ctrl => {
                        if let Some(ch) = k.chars().next() {
                            self.insert_char(ch);
                        }
                    }
                    _ => return false,
                }
                let visible = self.visible_lines();
                if self.cursor_line < self.scroll_offset {
                    self.scroll_offset = self.cursor_line;
                }
                if visible > 0 && self.cursor_line >= self.scroll_offset + visible {
                    self.scroll_offset = self.cursor_line - visible + 1;
                }
                return true;
            }
            Event::Scroll { dy, .. } if self.focused => {
                let max_scroll = self.lines.len().saturating_sub(self.visible_lines());
                if *dy < 0.0 && self.scroll_offset < max_scroll {
                    self.scroll_offset += 1;
                } else if *dy > 0.0 && self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
                return true;
            }
            _ => {}
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Checkbox
// ═══════════════════════════════════════════════════════════════════════

/// A toggle checkbox widget.
#[derive(Debug, Clone)]
pub struct Checkbox {
    /// Whether the checkbox is currently checked.
    pub checked: bool,
    /// Label displayed next to the checkbox.
    pub label: String,
    /// Set to `true` when the checked state changes.
    pub changed: bool,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl Checkbox {
    /// Creates a new unchecked checkbox with the given label and bounds.
    pub fn new(label: &str, rect: Rect) -> Self {
        Self {
            checked: false,
            label: label.to_string(),
            changed: false,
            rect,
        }
    }

    /// Toggles the checked state.
    pub fn toggle(&mut self) {
        self.checked = !self.checked;
        self.changed = true;
    }
}

impl Widget for Checkbox {
    fn render(&self, canvas: &mut Canvas) {
        let box_rect = Rect::new(
            self.rect.x,
            self.rect.y + (self.rect.height - 14.0) / 2.0,
            14.0,
            14.0,
        );
        canvas.fill_rect(&box_rect, Color::WHITE);
        canvas.draw_rect(&box_rect, Color::BLACK);
        if self.checked {
            let bx = box_rect.x as i32 + 2;
            let by = box_rect.y as i32 + 2;
            canvas.draw_line(bx, by, bx + 10, by + 10, Color::BLACK);
            canvas.draw_line(bx + 10, by, bx, by + 10, Color::BLACK);
        }
        let tx = self.rect.x as i32 + 18;
        let ty = self.rect.y as i32 + (self.rect.height as i32 - 7) / 2;
        canvas.draw_text(tx, ty, &self.label, Color::BLACK);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        self.changed = false;
        if let Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
        {
            if self.rect.contains(*x, *y) {
                self.toggle();
                return true;
            }
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RadioButton
// ═══════════════════════════════════════════════════════════════════════

/// A group of mutually-exclusive radio buttons.
#[derive(Debug, Clone)]
pub struct RadioButton {
    /// The index of the currently selected option.
    pub selected_index: usize,
    /// Option labels.
    pub options: Vec<String>,
    /// Group name (logical grouping identifier).
    pub group_name: String,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl RadioButton {
    /// Creates a new radio button group.
    pub fn new(group_name: &str, options: Vec<String>, rect: Rect) -> Self {
        Self {
            selected_index: 0,
            options,
            group_name: group_name.to_string(),
            rect,
        }
    }

    /// Returns the label of the currently selected option.
    pub fn selected_label(&self) -> Option<&str> {
        self.options.get(self.selected_index).map(|s| s.as_str())
    }

    /// Height of each option row.
    fn row_height(&self) -> f32 {
        if self.options.is_empty() {
            return self.rect.height;
        }
        self.rect.height / self.options.len() as f32
    }
}

impl Widget for RadioButton {
    fn render(&self, canvas: &mut Canvas) {
        let rh = self.row_height();
        for (i, option) in self.options.iter().enumerate() {
            let cy = self.rect.y + rh * i as f32 + rh / 2.0;
            let cx = self.rect.x + 7.0;
            canvas.draw_circle(cx as i32, cy as i32, 6, Color::BLACK);
            if i == self.selected_index {
                canvas.draw_circle(cx as i32, cy as i32, 3, Color::BLACK);
                for dy in -2..=2i32 {
                    for dx in -2..=2i32 {
                        if dx * dx + dy * dy <= 9 {
                            canvas.set_pixel(cx as i32 + dx, cy as i32 + dy, Color::BLACK);
                        }
                    }
                }
            }
            let tx = self.rect.x as i32 + 18;
            let ty = cy as i32 - 3;
            canvas.draw_text(tx, ty, option, Color::BLACK);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
        {
            if self.rect.contains(*x, *y) {
                let rh = self.row_height();
                let rel_y = y - self.rect.y;
                let idx = (rel_y / rh) as usize;
                if idx < self.options.len() {
                    self.selected_index = idx;
                    return true;
                }
            }
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Slider
// ═══════════════════════════════════════════════════════════════════════

/// A value slider with a draggable thumb.
#[derive(Debug, Clone)]
pub struct Slider {
    /// Minimum value.
    pub min: f64,
    /// Maximum value.
    pub max: f64,
    /// Current value (clamped to `[min, max]`).
    pub value: f64,
    /// Step increment (0 = continuous).
    pub step: f64,
    /// Layout orientation.
    pub orientation: Orientation,
    /// Whether the user is currently dragging the thumb.
    pub dragging: bool,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl Slider {
    /// Creates a new horizontal slider.
    pub fn new(min: f64, max: f64, value: f64, rect: Rect) -> Self {
        let clamped = value.clamp(min, max);
        Self {
            min,
            max,
            value: clamped,
            step: 0.0,
            orientation: Orientation::Horizontal,
            dragging: false,
            rect,
        }
    }

    /// Sets the step. Returns `self` for chaining.
    pub fn with_step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Sets the orientation. Returns `self` for chaining.
    pub fn with_orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Sets the value, clamping to `[min, max]` and snapping to step if nonzero.
    pub fn set_value(&mut self, val: f64) {
        let mut v = val.clamp(self.min, self.max);
        if self.step > 0.0 {
            v = ((v - self.min) / self.step).round() * self.step + self.min;
            v = v.clamp(self.min, self.max);
        }
        self.value = v;
    }

    /// Returns the normalized position of the thumb in `[0.0, 1.0]`.
    pub fn normalized(&self) -> f64 {
        if (self.max - self.min).abs() < f64::EPSILON {
            return 0.0;
        }
        (self.value - self.min) / (self.max - self.min)
    }

    /// Computes the value from a pixel coordinate along the slider axis.
    fn value_from_position(&self, pos: f32) -> f64 {
        let (start, length) = match self.orientation {
            Orientation::Horizontal => (self.rect.x, self.rect.width),
            Orientation::Vertical => (self.rect.y, self.rect.height),
        };
        let ratio = ((pos - start) / length).clamp(0.0, 1.0) as f64;
        self.min + ratio * (self.max - self.min)
    }
}

impl Widget for Slider {
    fn render(&self, canvas: &mut Canvas) {
        let track = match self.orientation {
            Orientation::Horizontal => {
                let ty = self.rect.y + self.rect.height / 2.0 - 2.0;
                Rect::new(self.rect.x, ty, self.rect.width, 4.0)
            }
            Orientation::Vertical => {
                let tx = self.rect.x + self.rect.width / 2.0 - 2.0;
                Rect::new(tx, self.rect.y, 4.0, self.rect.height)
            }
        };
        canvas.fill_rect(&track, Color::new(180, 180, 180));
        let norm = self.normalized() as f32;
        let (tx, ty) = match self.orientation {
            Orientation::Horizontal => {
                let tx = self.rect.x + norm * self.rect.width;
                let ty = self.rect.y + self.rect.height / 2.0;
                (tx as i32, ty as i32)
            }
            Orientation::Vertical => {
                let tx = self.rect.x + self.rect.width / 2.0;
                let ty = self.rect.y + norm * self.rect.height;
                (tx as i32, ty as i32)
            }
        };
        let thumb_color = if self.dragging {
            Color::BLUE
        } else {
            Color::new(100, 100, 100)
        };
        for dy in -6..=6i32 {
            for dx in -6..=6i32 {
                if dx * dx + dy * dy <= 36 {
                    canvas.set_pixel(tx + dx, ty + dy, thumb_color);
                }
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if self.rect.contains(*x, *y) {
                    self.dragging = true;
                    let pos = match self.orientation {
                        Orientation::Horizontal => *x,
                        Orientation::Vertical => *y,
                    };
                    self.set_value(self.value_from_position(pos));
                    return true;
                }
            }
            Event::MouseMove { x, y } if self.dragging => {
                let pos = match self.orientation {
                    Orientation::Horizontal => *x,
                    Orientation::Vertical => *y,
                };
                self.set_value(self.value_from_position(pos));
                return true;
            }
            Event::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if self.dragging {
                    self.dragging = false;
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ProgressBar
// ═══════════════════════════════════════════════════════════════════════

/// A progress indicator bar.
#[derive(Debug, Clone)]
pub struct ProgressBar {
    /// Progress value in `[0.0, 1.0]`.
    pub value: f64,
    /// Whether the progress is indeterminate (unknown completion).
    pub indeterminate: bool,
    /// Optional label displayed over the bar.
    pub label: Option<String>,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl ProgressBar {
    /// Creates a new progress bar at 0%.
    pub fn new(rect: Rect) -> Self {
        Self {
            value: 0.0,
            indeterminate: false,
            label: None,
            rect,
        }
    }

    /// Sets the progress value (clamped to `[0.0, 1.0]`).
    pub fn set_value(&mut self, val: f64) {
        self.value = val.clamp(0.0, 1.0);
    }
}

impl Widget for ProgressBar {
    fn render(&self, canvas: &mut Canvas) {
        canvas.fill_rect(&self.rect, Color::new(220, 220, 220));
        canvas.draw_rect(&self.rect, Color::GRAY);
        if !self.indeterminate {
            let fill_w = self.rect.width * self.value as f32;
            let fill = Rect::new(self.rect.x, self.rect.y, fill_w, self.rect.height);
            canvas.fill_rect(&fill, Color::new(76, 175, 80));
        }
        if let Some(ref text) = self.label {
            let text_w = text.len() as i32 * 6;
            let tx = self.rect.x as i32 + (self.rect.width as i32 - text_w) / 2;
            let ty = self.rect.y as i32 + (self.rect.height as i32 - 7) / 2;
            canvas.draw_text(tx, ty, text, Color::BLACK);
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Dropdown
// ═══════════════════════════════════════════════════════════════════════

/// A drop-down selector.
#[derive(Debug, Clone)]
pub struct Dropdown {
    /// Available options.
    pub options: Vec<String>,
    /// Index of the currently selected option.
    pub selected_index: usize,
    /// Whether the dropdown list is currently open.
    pub open: bool,
    /// Bounding rectangle (collapsed state).
    pub rect: Rect,
}

impl Dropdown {
    /// Creates a new dropdown with the given options.
    pub fn new(options: Vec<String>, rect: Rect) -> Self {
        Self {
            options,
            selected_index: 0,
            open: false,
            rect,
        }
    }

    /// Returns the currently selected option label, or `None` if empty.
    pub fn selected(&self) -> Option<&str> {
        self.options.get(self.selected_index).map(|s| s.as_str())
    }

    /// Returns the rectangle of the expanded options list.
    fn expanded_rect(&self) -> Rect {
        let list_h = self.options.len() as f32 * self.rect.height;
        Rect::new(self.rect.x, self.rect.bottom(), self.rect.width, list_h)
    }
}

impl Widget for Dropdown {
    fn render(&self, canvas: &mut Canvas) {
        canvas.fill_rect(&self.rect, Color::WHITE);
        canvas.draw_rect(&self.rect, Color::BLACK);
        let display = self.selected().unwrap_or("");
        let tx = self.rect.x as i32 + 4;
        let ty = self.rect.y as i32 + (self.rect.height as i32 - 7) / 2;
        canvas.draw_text(tx, ty, display, Color::BLACK);
        let ax = self.rect.right() as i32 - 12;
        let ay = self.rect.y as i32 + (self.rect.height as i32) / 2;
        canvas.draw_line(ax, ay - 2, ax + 4, ay + 2, Color::BLACK);
        canvas.draw_line(ax + 4, ay + 2, ax + 8, ay - 2, Color::BLACK);
        if self.open {
            let list_rect = self.expanded_rect();
            canvas.fill_rect(&list_rect, Color::WHITE);
            canvas.draw_rect(&list_rect, Color::BLACK);
            let rh = self.rect.height;
            for (i, opt) in self.options.iter().enumerate() {
                let oy = self.rect.bottom() + rh * i as f32;
                if i == self.selected_index {
                    let highlight = Rect::new(self.rect.x, oy, self.rect.width, rh);
                    canvas.fill_rect(&highlight, Color::new(200, 200, 255));
                }
                let otx = self.rect.x as i32 + 4;
                let oty = oy as i32 + (rh as i32 - 7) / 2;
                canvas.draw_text(otx, oty, opt, Color::BLACK);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
        {
            if self.open {
                let list_rect = self.expanded_rect();
                if list_rect.contains(*x, *y) {
                    let rel_y = y - list_rect.y;
                    let idx = (rel_y / self.rect.height) as usize;
                    if idx < self.options.len() {
                        self.selected_index = idx;
                    }
                }
                self.open = false;
                return true;
            } else if self.rect.contains(*x, *y) {
                self.open = true;
                return true;
            }
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ListView
// ═══════════════════════════════════════════════════════════════════════

/// A scrollable list of selectable items.
#[derive(Debug, Clone)]
pub struct ListView {
    /// Item labels.
    pub items: Vec<String>,
    /// Index of the selected item, or `None`.
    pub selected_index: Option<usize>,
    /// Vertical scroll offset in pixels.
    pub scroll_offset: f32,
    /// Height of each item row in pixels.
    pub item_height: f32,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl ListView {
    /// Creates a new list view.
    pub fn new(items: Vec<String>, rect: Rect) -> Self {
        Self {
            items,
            selected_index: None,
            scroll_offset: 0.0,
            item_height: 24.0,
            rect,
        }
    }

    /// Returns the total content height.
    pub fn content_height(&self) -> f32 {
        self.items.len() as f32 * self.item_height
    }

    /// Returns the maximum scroll offset.
    pub fn max_scroll(&self) -> f32 {
        (self.content_height() - self.rect.height).max(0.0)
    }
}

impl Widget for ListView {
    fn render(&self, canvas: &mut Canvas) {
        canvas.fill_rect(&self.rect, Color::WHITE);
        canvas.draw_rect(&self.rect, Color::GRAY);
        let first = (self.scroll_offset / self.item_height) as usize;
        let visible_count = (self.rect.height / self.item_height) as usize + 2;
        let end = (first + visible_count).min(self.items.len());
        for i in first..end {
            let iy = self.rect.y + (i as f32 * self.item_height) - self.scroll_offset;
            if iy + self.item_height < self.rect.y || iy > self.rect.bottom() {
                continue;
            }
            let row_rect = Rect::new(self.rect.x, iy, self.rect.width, self.item_height);
            if Some(i) == self.selected_index {
                canvas.fill_rect(&row_rect, Color::new(0, 120, 215));
                canvas.draw_text(
                    self.rect.x as i32 + 4,
                    iy as i32 + (self.item_height as i32 - 7) / 2,
                    &self.items[i],
                    Color::WHITE,
                );
            } else {
                canvas.draw_text(
                    self.rect.x as i32 + 4,
                    iy as i32 + (self.item_height as i32 - 7) / 2,
                    &self.items[i],
                    Color::BLACK,
                );
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                if self.rect.contains(*x, *y) {
                    let rel_y = *y - self.rect.y + self.scroll_offset;
                    let idx = (rel_y / self.item_height) as usize;
                    if idx < self.items.len() {
                        self.selected_index = Some(idx);
                    }
                    return true;
                }
            }
            Event::Scroll { dy, .. } => {
                self.scroll_offset = (self.scroll_offset - dy * 20.0).clamp(0.0, self.max_scroll());
                return true;
            }
            _ => {}
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TreeView
// ═══════════════════════════════════════════════════════════════════════

/// A node in a tree view hierarchy.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Display label.
    pub label: String,
    /// Child nodes.
    pub children: Vec<TreeNode>,
    /// Whether this node is expanded (children visible).
    pub expanded: bool,
    /// Whether this node is selected.
    pub selected: bool,
}

impl TreeNode {
    /// Creates a new leaf node.
    pub fn leaf(label: &str) -> Self {
        Self {
            label: label.to_string(),
            children: Vec::new(),
            expanded: false,
            selected: false,
        }
    }

    /// Creates a new branch node with children.
    pub fn branch(label: &str, children: Vec<TreeNode>) -> Self {
        Self {
            label: label.to_string(),
            children,
            expanded: false,
            selected: false,
        }
    }

    /// Toggles the expanded state.
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }

    /// Flattens the visible tree into a list of `(depth, label, is_branch, is_expanded)`.
    pub fn flatten(&self) -> Vec<(usize, String, bool, bool)> {
        let mut result = Vec::new();
        self.flatten_inner(0, &mut result);
        result
    }

    /// Recursive helper for flatten.
    fn flatten_inner(&self, depth: usize, out: &mut Vec<(usize, String, bool, bool)>) {
        let is_branch = !self.children.is_empty();
        out.push((depth, self.label.clone(), is_branch, self.expanded));
        if self.expanded {
            for child in &self.children {
                child.flatten_inner(depth + 1, out);
            }
        }
    }
}

/// A hierarchical tree view widget.
#[derive(Debug, Clone)]
pub struct TreeView {
    /// Root nodes.
    pub roots: Vec<TreeNode>,
    /// Index of the selected visible row, or `None`.
    pub selected_index: Option<usize>,
    /// Height of each row.
    pub row_height: f32,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl TreeView {
    /// Creates a new tree view with the given root nodes.
    pub fn new(roots: Vec<TreeNode>, rect: Rect) -> Self {
        Self {
            roots,
            selected_index: None,
            row_height: 20.0,
            rect,
        }
    }

    /// Returns the flattened visible rows.
    pub fn visible_rows(&self) -> Vec<(usize, String, bool, bool)> {
        let mut rows = Vec::new();
        for root in &self.roots {
            for item in root.flatten() {
                rows.push(item);
            }
        }
        rows
    }

    /// Toggles the expand state of the node at the given visible row index.
    pub fn toggle_at(&mut self, visible_index: usize) {
        let mut counter = 0usize;
        for root in &mut self.roots {
            if Self::toggle_inner(root, visible_index, &mut counter) {
                return;
            }
        }
    }

    /// Recursive helper to find and toggle a node by visible index.
    fn toggle_inner(node: &mut TreeNode, target: usize, counter: &mut usize) -> bool {
        if *counter == target {
            node.toggle();
            return true;
        }
        *counter += 1;
        if node.expanded {
            for child in &mut node.children {
                if Self::toggle_inner(child, target, counter) {
                    return true;
                }
            }
        }
        false
    }
}

impl Widget for TreeView {
    fn render(&self, canvas: &mut Canvas) {
        canvas.fill_rect(&self.rect, Color::WHITE);
        canvas.draw_rect(&self.rect, Color::GRAY);
        let rows = self.visible_rows();
        for (i, (depth, label, is_branch, expanded)) in rows.iter().enumerate() {
            let iy = self.rect.y + i as f32 * self.row_height;
            if iy > self.rect.bottom() {
                break;
            }
            let indent = *depth as i32 * 16;
            if Some(i) == self.selected_index {
                let row_rect = Rect::new(self.rect.x, iy, self.rect.width, self.row_height);
                canvas.fill_rect(&row_rect, Color::new(200, 220, 255));
            }
            let tx = self.rect.x as i32 + 4 + indent;
            let ty = iy as i32 + (self.row_height as i32 - 7) / 2;
            if *is_branch {
                let indicator = if *expanded { "-" } else { "+" };
                canvas.draw_text(tx, ty, indicator, Color::BLACK);
                canvas.draw_text(tx + 10, ty, label, Color::BLACK);
            } else {
                canvas.draw_text(tx + 10, ty, label, Color::BLACK);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
        {
            if self.rect.contains(*x, *y) {
                let rel_y = y - self.rect.y;
                let idx = (rel_y / self.row_height) as usize;
                let rows = self.visible_rows();
                if idx < rows.len() {
                    self.selected_index = Some(idx);
                    let (_, _, is_branch, _) = &rows[idx];
                    if *is_branch {
                        self.toggle_at(idx);
                    }
                    return true;
                }
            }
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Table
// ═══════════════════════════════════════════════════════════════════════

/// A tabular data widget with sortable columns.
#[derive(Debug, Clone)]
pub struct Table {
    /// Column headers.
    pub columns: Vec<String>,
    /// Row data — each row is a `Vec<String>` of cell values.
    pub rows: Vec<Vec<String>>,
    /// Index of the selected row, or `None`.
    pub selected_row: Option<usize>,
    /// Column index currently used for sorting, or `None`.
    pub sort_column: Option<usize>,
    /// Sort direction (`true` = ascending).
    pub sort_ascending: bool,
    /// Row height in pixels.
    pub row_height: f32,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl Table {
    /// Creates a new table.
    pub fn new(columns: Vec<String>, rect: Rect) -> Self {
        Self {
            columns,
            rows: Vec::new(),
            selected_row: None,
            sort_column: None,
            sort_ascending: true,
            row_height: 24.0,
            rect,
        }
    }

    /// Adds a row of cell values.
    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }

    /// Sorts the rows by the given column index.
    pub fn sort_by_column(&mut self, col: usize) {
        if Some(col) == self.sort_column {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_column = Some(col);
            self.sort_ascending = true;
        }
        let asc = self.sort_ascending;
        self.rows.sort_by(|a, b| {
            let va = a.get(col).map(|s| s.as_str()).unwrap_or("");
            let vb = b.get(col).map(|s| s.as_str()).unwrap_or("");
            if asc { va.cmp(vb) } else { vb.cmp(va) }
        });
    }

    /// Column width (evenly divided).
    fn col_width(&self) -> f32 {
        if self.columns.is_empty() {
            return self.rect.width;
        }
        self.rect.width / self.columns.len() as f32
    }
}

impl Widget for Table {
    fn render(&self, canvas: &mut Canvas) {
        canvas.fill_rect(&self.rect, Color::WHITE);
        canvas.draw_rect(&self.rect, Color::BLACK);
        let cw = self.col_width();
        let header_rect = Rect::new(self.rect.x, self.rect.y, self.rect.width, self.row_height);
        canvas.fill_rect(&header_rect, Color::new(220, 220, 220));
        for (i, col) in self.columns.iter().enumerate() {
            let cx = self.rect.x + cw * i as f32;
            let ty = self.rect.y as i32 + (self.row_height as i32 - 7) / 2;
            canvas.draw_text(cx as i32 + 4, ty, col, Color::BLACK);
        }
        for (ri, row) in self.rows.iter().enumerate() {
            let ry = self.rect.y + self.row_height * (ri + 1) as f32;
            if ry > self.rect.bottom() {
                break;
            }
            if Some(ri) == self.selected_row {
                let row_rect = Rect::new(self.rect.x, ry, self.rect.width, self.row_height);
                canvas.fill_rect(&row_rect, Color::new(200, 220, 255));
            }
            for (ci, cell) in row.iter().enumerate() {
                let cx = self.rect.x + cw * ci as f32;
                let ty = ry as i32 + (self.row_height as i32 - 7) / 2;
                canvas.draw_text(cx as i32 + 4, ty, cell, Color::BLACK);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
        {
            if self.rect.contains(*x, *y) {
                let rel_y = y - self.rect.y;
                if rel_y < self.row_height {
                    let col = ((x - self.rect.x) / self.col_width()) as usize;
                    if col < self.columns.len() {
                        self.sort_by_column(col);
                    }
                } else {
                    let row_idx = ((rel_y - self.row_height) / self.row_height) as usize;
                    if row_idx < self.rows.len() {
                        self.selected_row = Some(row_idx);
                    }
                }
                return true;
            }
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Image
// ═══════════════════════════════════════════════════════════════════════

/// An image widget that displays a pixel buffer.
#[derive(Debug, Clone)]
pub struct ImageWidget {
    /// Image width in pixels.
    pub img_width: u32,
    /// Image height in pixels.
    pub img_height: u32,
    /// Row-major pixel data.
    pub pixels: Vec<Color>,
    /// How the image is scaled into its bounds.
    pub scale_mode: ScaleMode,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl ImageWidget {
    /// Creates a new image widget from pixel data.
    pub fn new(width: u32, height: u32, pixels: Vec<Color>, rect: Rect) -> Self {
        Self {
            img_width: width,
            img_height: height,
            pixels,
            scale_mode: ScaleMode::Fit,
            rect,
        }
    }

    /// Sets the scale mode. Returns `self` for chaining.
    pub fn with_scale_mode(mut self, mode: ScaleMode) -> Self {
        self.scale_mode = mode;
        self
    }

    /// Samples the image at normalized coordinates `(u, v)` in `[0,1]`.
    fn sample(&self, u: f32, v: f32) -> Color {
        if self.pixels.is_empty() || self.img_width == 0 || self.img_height == 0 {
            return Color::TRANSPARENT;
        }
        let sx = (u * self.img_width as f32) as u32;
        let sy = (v * self.img_height as f32) as u32;
        let sx = sx.min(self.img_width - 1);
        let sy = sy.min(self.img_height - 1);
        let idx = (sy * self.img_width + sx) as usize;
        if idx < self.pixels.len() {
            self.pixels[idx]
        } else {
            Color::TRANSPARENT
        }
    }
}

impl Widget for ImageWidget {
    fn render(&self, canvas: &mut Canvas) {
        if self.img_width == 0 || self.img_height == 0 || self.pixels.is_empty() {
            canvas.fill_rect(&self.rect, Color::new(200, 200, 200));
            return;
        }
        let aspect = self.img_width as f32 / self.img_height as f32;
        let (draw_w, draw_h) = match self.scale_mode {
            ScaleMode::Stretch => (self.rect.width, self.rect.height),
            ScaleMode::Fit => {
                let by_w = (self.rect.width, self.rect.width / aspect);
                let by_h = (self.rect.height * aspect, self.rect.height);
                if by_w.1 <= self.rect.height {
                    by_w
                } else {
                    by_h
                }
            }
            ScaleMode::Fill => {
                let by_w = (self.rect.width, self.rect.width / aspect);
                let by_h = (self.rect.height * aspect, self.rect.height);
                if by_w.1 >= self.rect.height {
                    by_w
                } else {
                    by_h
                }
            }
        };
        let ox = self.rect.x + (self.rect.width - draw_w) / 2.0;
        let oy = self.rect.y + (self.rect.height - draw_h) / 2.0;
        for py in 0..(draw_h as u32) {
            for px in 0..(draw_w as u32) {
                let u = px as f32 / draw_w;
                let v = py as f32 / draw_h;
                let color = self.sample(u, v);
                canvas.set_pixel(ox as i32 + px as i32, oy as i32 + py as i32, color);
            }
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TabView
// ═══════════════════════════════════════════════════════════════════════

/// A tab in a `TabView`.
#[derive(Debug, Clone)]
pub struct Tab {
    /// Tab title displayed in the tab header.
    pub title: String,
    /// Arbitrary content identifier (index into an external widget list, etc.).
    pub content_id: usize,
}

/// A tabbed container widget.
#[derive(Debug, Clone)]
pub struct TabView {
    /// Tabs.
    pub tabs: Vec<Tab>,
    /// Index of the active (visible) tab.
    pub active_tab: usize,
    /// Height of the tab header strip.
    pub header_height: f32,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl TabView {
    /// Creates a new tab view.
    pub fn new(tabs: Vec<Tab>, rect: Rect) -> Self {
        Self {
            tabs,
            active_tab: 0,
            header_height: 28.0,
            rect,
        }
    }

    /// Returns the rectangle available for the active tab's content.
    pub fn content_rect(&self) -> Rect {
        Rect::new(
            self.rect.x,
            self.rect.y + self.header_height,
            self.rect.width,
            (self.rect.height - self.header_height).max(0.0),
        )
    }

    /// Width of each tab header.
    fn tab_width(&self) -> f32 {
        if self.tabs.is_empty() {
            return self.rect.width;
        }
        self.rect.width / self.tabs.len() as f32
    }
}

impl Widget for TabView {
    fn render(&self, canvas: &mut Canvas) {
        let header = Rect::new(
            self.rect.x,
            self.rect.y,
            self.rect.width,
            self.header_height,
        );
        canvas.fill_rect(&header, Color::new(230, 230, 230));
        let tw = self.tab_width();
        for (i, tab) in self.tabs.iter().enumerate() {
            let tx = self.rect.x + tw * i as f32;
            let tab_rect = Rect::new(tx, self.rect.y, tw, self.header_height);
            if i == self.active_tab {
                canvas.fill_rect(&tab_rect, Color::WHITE);
            }
            canvas.draw_rect(&tab_rect, Color::GRAY);
            let text_x = tx as i32 + 4;
            let text_y = self.rect.y as i32 + (self.header_height as i32 - 7) / 2;
            canvas.draw_text(text_x, text_y, &tab.title, Color::BLACK);
        }
        let content = self.content_rect();
        canvas.fill_rect(&content, Color::WHITE);
        canvas.draw_rect(&content, Color::GRAY);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            ..
        } = event
        {
            if *y >= self.rect.y
                && *y < self.rect.y + self.header_height
                && *x >= self.rect.x
                && *x < self.rect.right()
            {
                let tw = self.tab_width();
                let idx = ((*x - self.rect.x) / tw) as usize;
                if idx < self.tabs.len() {
                    self.active_tab = idx;
                    return true;
                }
            }
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SplitView
// ═══════════════════════════════════════════════════════════════════════

/// A split-pane view that divides its area into two panels.
#[derive(Debug, Clone)]
pub struct SplitView {
    /// Split orientation.
    pub orientation: Orientation,
    /// Position of the split divider as a ratio `[0.0, 1.0]`.
    pub split_ratio: f32,
    /// Whether the user is currently dragging the divider.
    pub dragging: bool,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl SplitView {
    /// Creates a new split view with a 50/50 split.
    pub fn new(orientation: Orientation, rect: Rect) -> Self {
        Self {
            orientation,
            split_ratio: 0.5,
            dragging: false,
            rect,
        }
    }

    /// Returns the bounding rectangle of the first (left/top) pane.
    pub fn first_pane(&self) -> Rect {
        match self.orientation {
            Orientation::Horizontal => {
                let w = self.rect.width * self.split_ratio;
                Rect::new(self.rect.x, self.rect.y, w, self.rect.height)
            }
            Orientation::Vertical => {
                let h = self.rect.height * self.split_ratio;
                Rect::new(self.rect.x, self.rect.y, self.rect.width, h)
            }
        }
    }

    /// Returns the bounding rectangle of the second (right/bottom) pane.
    pub fn second_pane(&self) -> Rect {
        match self.orientation {
            Orientation::Horizontal => {
                let w = self.rect.width * self.split_ratio;
                Rect::new(
                    self.rect.x + w + 4.0,
                    self.rect.y,
                    self.rect.width - w - 4.0,
                    self.rect.height,
                )
            }
            Orientation::Vertical => {
                let h = self.rect.height * self.split_ratio;
                Rect::new(
                    self.rect.x,
                    self.rect.y + h + 4.0,
                    self.rect.width,
                    self.rect.height - h - 4.0,
                )
            }
        }
    }

    /// Returns the rectangle of the divider handle.
    fn divider_rect(&self) -> Rect {
        match self.orientation {
            Orientation::Horizontal => {
                let x = self.rect.x + self.rect.width * self.split_ratio;
                Rect::new(x - 2.0, self.rect.y, 4.0, self.rect.height)
            }
            Orientation::Vertical => {
                let y = self.rect.y + self.rect.height * self.split_ratio;
                Rect::new(self.rect.x, y - 2.0, self.rect.width, 4.0)
            }
        }
    }
}

impl Widget for SplitView {
    fn render(&self, canvas: &mut Canvas) {
        let div = self.divider_rect();
        let div_color = if self.dragging {
            Color::BLUE
        } else {
            Color::new(180, 180, 180)
        };
        canvas.fill_rect(&div, div_color);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                ..
            } => {
                let div = self.divider_rect();
                if div.contains(*x, *y) {
                    self.dragging = true;
                    return true;
                }
            }
            Event::MouseMove { x, y } if self.dragging => {
                match self.orientation {
                    Orientation::Horizontal => {
                        self.split_ratio = ((*x - self.rect.x) / self.rect.width).clamp(0.1, 0.9);
                    }
                    Orientation::Vertical => {
                        self.split_ratio = ((*y - self.rect.y) / self.rect.height).clamp(0.1, 0.9);
                    }
                }
                return true;
            }
            Event::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if self.dragging {
                    self.dragging = false;
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ScrollView
// ═══════════════════════════════════════════════════════════════════════

/// A scrollable viewport container.
#[derive(Debug, Clone)]
pub struct ScrollView {
    /// Total height of the scrollable content.
    pub content_height: f32,
    /// Current vertical scroll offset.
    pub scroll_y: f32,
    /// Height of the visible viewport.
    pub viewport_height: f32,
    /// Bounding rectangle.
    pub rect: Rect,
}

impl ScrollView {
    /// Creates a new scroll view.
    pub fn new(content_height: f32, rect: Rect) -> Self {
        Self {
            content_height,
            scroll_y: 0.0,
            viewport_height: rect.height,
            rect,
        }
    }

    /// Maximum scroll offset.
    pub fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport_height).max(0.0)
    }

    /// Returns the normalized scrollbar thumb position in `[0.0, 1.0]`.
    pub fn thumb_position(&self) -> f32 {
        let max = self.max_scroll();
        if max < f32::EPSILON {
            return 0.0;
        }
        self.scroll_y / max
    }

    /// Returns the scrollbar thumb height as a fraction of the track.
    pub fn thumb_size(&self) -> f32 {
        if self.content_height < f32::EPSILON {
            return 1.0;
        }
        (self.viewport_height / self.content_height).min(1.0)
    }

    /// Scrolls to the given offset, clamped.
    pub fn scroll_to(&mut self, y: f32) {
        self.scroll_y = y.clamp(0.0, self.max_scroll());
    }
}

impl Widget for ScrollView {
    fn render(&self, canvas: &mut Canvas) {
        canvas.draw_rect(&self.rect, Color::GRAY);
        let bar_w = 12.0;
        let track = Rect::new(
            self.rect.right() - bar_w,
            self.rect.y,
            bar_w,
            self.rect.height,
        );
        canvas.fill_rect(&track, Color::new(230, 230, 230));
        let thumb_h = (self.thumb_size() * self.rect.height).max(20.0);
        let thumb_y = self.rect.y + self.thumb_position() * (self.rect.height - thumb_h);
        let thumb = Rect::new(self.rect.right() - bar_w, thumb_y, bar_w, thumb_h);
        canvas.fill_rect(&thumb, Color::new(160, 160, 160));
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::Scroll { dy, .. } = event {
            let new_y = self.scroll_y - dy * 30.0;
            self.scroll_to(new_y);
            return true;
        }
        false
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
        self.viewport_height = rect.height;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Dialog
// ═══════════════════════════════════════════════════════════════════════

/// A dialog type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogKind {
    /// Informational alert with a single OK button.
    Alert,
    /// Yes/No confirmation dialog.
    Confirm,
    /// Text prompt with input field.
    Prompt,
}

/// A modal dialog box.
#[derive(Debug, Clone)]
pub struct Dialog {
    /// Dialog title bar text.
    pub title: String,
    /// Message body text.
    pub message: String,
    /// Button labels.
    pub buttons: Vec<String>,
    /// Kind of dialog.
    pub kind: DialogKind,
    /// Index of the button that was clicked, or `None` if not yet dismissed.
    pub result: Option<usize>,
    /// Text input value (only used for `Prompt` dialogs).
    pub input_value: String,
}

impl Dialog {
    /// Creates an alert dialog with a single "OK" button.
    pub fn alert(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            buttons: vec!["OK".to_string()],
            kind: DialogKind::Alert,
            result: None,
            input_value: String::new(),
        }
    }

    /// Creates a confirmation dialog with "Yes" and "No" buttons.
    pub fn confirm(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            buttons: vec!["Yes".to_string(), "No".to_string()],
            kind: DialogKind::Confirm,
            result: None,
            input_value: String::new(),
        }
    }

    /// Creates a text prompt dialog with "OK" and "Cancel" buttons.
    pub fn prompt(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            buttons: vec!["OK".to_string(), "Cancel".to_string()],
            kind: DialogKind::Prompt,
            result: None,
            input_value: String::new(),
        }
    }

    /// Returns `true` if the dialog has been dismissed.
    pub fn is_dismissed(&self) -> bool {
        self.result.is_some()
    }

    /// Returns `true` if the dialog was confirmed (button 0 = "OK" or "Yes").
    pub fn is_confirmed(&self) -> bool {
        self.result == Some(0)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Menu
// ═══════════════════════════════════════════════════════════════════════

/// A single item in a menu hierarchy.
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Display label (use "-" for a separator).
    pub label: String,
    /// Optional keyboard shortcut (e.g. "Ctrl+S").
    pub shortcut: Option<String>,
    /// Sub-menu children.
    pub children: Vec<MenuItem>,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Whether this item shows a check mark (for toggle items).
    pub checked: bool,
}

impl MenuItem {
    /// Creates a new enabled menu item.
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            shortcut: None,
            children: Vec::new(),
            enabled: true,
            checked: false,
        }
    }

    /// Creates a separator item.
    pub fn separator() -> Self {
        Self {
            label: "-".to_string(),
            shortcut: None,
            children: Vec::new(),
            enabled: false,
            checked: false,
        }
    }

    /// Sets the keyboard shortcut. Returns `self` for chaining.
    pub fn with_shortcut(mut self, shortcut: &str) -> Self {
        self.shortcut = Some(shortcut.to_string());
        self
    }

    /// Adds a child item (creates a sub-menu). Returns `self` for chaining.
    pub fn with_child(mut self, child: MenuItem) -> Self {
        self.children.push(child);
        self
    }

    /// Returns `true` if this item is a separator.
    pub fn is_separator(&self) -> bool {
        self.label == "-"
    }

    /// Returns `true` if this item has a sub-menu.
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

/// A menu bar containing top-level menus.
#[derive(Debug, Clone)]
pub struct MenuBar {
    /// Top-level menu items (each acts as a dropdown menu).
    pub menus: Vec<MenuItem>,
    /// Index of the currently open menu, or `None`.
    pub open_menu: Option<usize>,
}

impl MenuBar {
    /// Creates an empty menu bar.
    pub fn new() -> Self {
        Self {
            menus: Vec::new(),
            open_menu: None,
        }
    }

    /// Adds a top-level menu. Returns `self` for chaining.
    pub fn add_menu(mut self, menu: MenuItem) -> Self {
        self.menus.push(menu);
        self
    }

    /// Returns the number of top-level menus.
    pub fn menu_count(&self) -> usize {
        self.menus.len()
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Theme
// ═══════════════════════════════════════════════════════════════════════

/// Visual theme for all widgets.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Background color.
    pub bg: Color,
    /// Foreground (text) color.
    pub fg: Color,
    /// Primary accent color.
    pub primary: Color,
    /// Secondary accent color.
    pub secondary: Color,
    /// Highlight/accent color.
    pub accent: Color,
    /// Default font size in pixels.
    pub font_size: f32,
    /// Border radius for rounded corners (pixels).
    pub border_radius: f32,
}

impl Theme {
    /// Standard light theme.
    pub fn light() -> Self {
        Self {
            bg: Color::WHITE,
            fg: Color::BLACK,
            primary: Color::new(33, 150, 243),
            secondary: Color::new(156, 39, 176),
            accent: Color::new(255, 193, 7),
            font_size: 14.0,
            border_radius: 4.0,
        }
    }

    /// Standard dark theme.
    pub fn dark() -> Self {
        Self {
            bg: Color::new(30, 30, 30),
            fg: Color::new(230, 230, 230),
            primary: Color::new(100, 181, 246),
            secondary: Color::new(206, 147, 216),
            accent: Color::new(255, 213, 79),
            font_size: 14.0,
            border_radius: 4.0,
        }
    }

    /// High-contrast accessibility theme.
    pub fn high_contrast() -> Self {
        Self {
            bg: Color::BLACK,
            fg: Color::WHITE,
            primary: Color::YELLOW,
            secondary: Color::CYAN,
            accent: Color::new(255, 128, 0),
            font_size: 16.0,
            border_radius: 0.0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// StatusBar
// ═══════════════════════════════════════════════════════════════════════

/// A segment in a status bar.
#[derive(Debug, Clone)]
pub struct StatusSegment {
    /// Display text.
    pub text: String,
    /// Text alignment within this segment.
    pub align: TextAlign,
}

impl StatusSegment {
    /// Creates a new status segment.
    pub fn new(text: &str, align: TextAlign) -> Self {
        Self {
            text: text.to_string(),
            align,
        }
    }
}

/// A status bar displayed at the bottom of a window.
#[derive(Debug, Clone)]
pub struct StatusBar {
    /// Segments arranged left-to-right.
    pub segments: Vec<StatusSegment>,
    /// Height of the status bar.
    pub height: f32,
    /// Background color.
    pub bg: Color,
    /// Text color.
    pub fg: Color,
}

impl StatusBar {
    /// Creates a new status bar with default styling.
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            height: 22.0,
            bg: Color::new(240, 240, 240),
            fg: Color::BLACK,
        }
    }

    /// Adds a segment. Returns `self` for chaining.
    pub fn add_segment(mut self, segment: StatusSegment) -> Self {
        self.segments.push(segment);
        self
    }

    /// Renders the status bar at the bottom of the given canvas.
    pub fn render(&self, canvas: &mut Canvas) {
        let y = canvas.height as f32 - self.height;
        let bar_rect = Rect::new(0.0, y, canvas.width as f32, self.height);
        canvas.fill_rect(&bar_rect, self.bg);
        canvas.draw_line(0, y as i32, canvas.width as i32, y as i32, Color::GRAY);
        if self.segments.is_empty() {
            return;
        }
        let seg_w = canvas.width as f32 / self.segments.len() as f32;
        for (i, seg) in self.segments.iter().enumerate() {
            let sx = seg_w * i as f32;
            let text_w = seg.text.len() as f32 * 6.0;
            let tx = match seg.align {
                TextAlign::Left => sx as i32 + 4,
                TextAlign::Center => sx as i32 + ((seg_w - text_w) / 2.0) as i32,
                TextAlign::Right => sx as i32 + (seg_w - text_w - 4.0) as i32,
            };
            let ty = y as i32 + (self.height as i32 - 7) / 2;
            canvas.draw_text(tx, ty, &seg.text, self.fg);
        }
    }
}

impl Default for StatusBar {
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
    fn color_new_is_opaque() {
        let c = Color::new(10, 20, 30);
        assert_eq!(c.a, 255);
        assert_eq!(c.r, 10);
        assert_eq!(c.g, 20);
        assert_eq!(c.b, 30);
    }

    #[test]
    fn color_from_hex_rgb() {
        let c = Color::from_hex("#FF8000");
        assert!(c.is_some());
        let c = c.unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 0);
        assert_eq!(c.a, 255);
    }

    #[test]
    fn color_from_hex_rgba() {
        let c = Color::from_hex("#FF800080").unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 0);
        assert_eq!(c.a, 128);
    }

    #[test]
    fn color_from_hex_without_hash() {
        let c = Color::from_hex("00FF00").unwrap();
        assert_eq!(c, Color::new(0, 255, 0));
    }

    #[test]
    fn color_from_hex_invalid() {
        assert!(Color::from_hex("#ZZZ").is_none());
        assert!(Color::from_hex("12345").is_none());
        assert!(Color::from_hex("").is_none());
    }

    #[test]
    fn color_to_hex_opaque() {
        assert_eq!(Color::RED.to_hex(), "#FF0000");
        assert_eq!(Color::new(0, 128, 255).to_hex(), "#0080FF");
    }

    #[test]
    fn color_to_hex_with_alpha() {
        let c = Color::with_alpha(255, 0, 0, 128);
        assert_eq!(c.to_hex(), "#FF000080");
    }

    #[test]
    fn color_blend_opaque_over() {
        let bg = Color::RED;
        let fg = Color::BLUE;
        let result = bg.blend(&fg);
        assert_eq!(result, Color::BLUE);
    }

    #[test]
    fn color_blend_transparent_over() {
        let bg = Color::RED;
        let fg = Color::TRANSPARENT;
        let result = bg.blend(&fg);
        assert_eq!(result, Color::RED);
    }

    #[test]
    fn color_blend_semitransparent() {
        let bg = Color::new(0, 0, 0);
        let fg = Color::with_alpha(255, 255, 255, 128);
        let result = bg.blend(&fg);
        assert!(result.r > 100 && result.r < 160);
        assert!(result.g > 100 && result.g < 160);
    }

    #[test]
    fn color_named_constants() {
        assert_eq!(Color::WHITE, Color::new(255, 255, 255));
        assert_eq!(Color::BLACK, Color::new(0, 0, 0));
        assert_eq!(Color::TRANSPARENT.a, 0);
    }

    #[test]
    fn rect_contains_inside() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains(50.0, 40.0));
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(110.0, 70.0));
    }

    #[test]
    fn rect_contains_outside() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(!r.contains(5.0, 40.0));
        assert!(!r.contains(50.0, 75.0));
    }

    #[test]
    fn rect_intersects_overlap() {
        let a = Rect::new(0.0, 0.0, 50.0, 50.0);
        let b = Rect::new(25.0, 25.0, 50.0, 50.0);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));
    }

    #[test]
    fn rect_intersects_no_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(20.0, 20.0, 10.0, 10.0);
        assert!(!a.intersects(&b));
    }

    #[test]
    fn rect_union() {
        let a = Rect::new(0.0, 0.0, 50.0, 50.0);
        let b = Rect::new(30.0, 30.0, 50.0, 50.0);
        let u = a.union(&b);
        assert!((u.x - 0.0).abs() < f32::EPSILON);
        assert!((u.y - 0.0).abs() < f32::EPSILON);
        assert!((u.width - 80.0).abs() < f32::EPSILON);
        assert!((u.height - 80.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rect_inset_normal() {
        let r = Rect::new(10.0, 10.0, 100.0, 80.0);
        let inset = r.inset(5.0, 10.0);
        assert!((inset.x - 15.0).abs() < f32::EPSILON);
        assert!((inset.y - 20.0).abs() < f32::EPSILON);
        assert!((inset.width - 90.0).abs() < f32::EPSILON);
        assert!((inset.height - 60.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rect_inset_collapse() {
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);
        let inset = r.inset(15.0, 15.0);
        assert!((inset.width - 0.0).abs() < f32::EPSILON);
        assert!((inset.height - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rect_intersection_some() {
        let a = Rect::new(0.0, 0.0, 50.0, 50.0);
        let b = Rect::new(25.0, 25.0, 50.0, 50.0);
        let i = a.intersection(&b).unwrap();
        assert!((i.x - 25.0).abs() < f32::EPSILON);
        assert!((i.y - 25.0).abs() < f32::EPSILON);
        assert!((i.width - 25.0).abs() < f32::EPSILON);
        assert!((i.height - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rect_area() {
        let r = Rect::new(0.0, 0.0, 10.0, 20.0);
        assert!((r.area() - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn canvas_clear() {
        let mut c = Canvas::new(4, 4, Color::WHITE);
        c.clear(Color::RED);
        assert_eq!(c.get_pixel(0, 0), Some(Color::RED));
        assert_eq!(c.get_pixel(3, 3), Some(Color::RED));
    }

    #[test]
    fn canvas_set_get_pixel() {
        let mut c = Canvas::new(10, 10, Color::BLACK);
        c.set_pixel(5, 5, Color::GREEN);
        assert_eq!(c.get_pixel(5, 5), Some(Color::GREEN));
    }

    #[test]
    fn canvas_pixel_out_of_bounds() {
        let mut c = Canvas::new(4, 4, Color::BLACK);
        c.set_pixel(-1, 0, Color::RED);
        c.set_pixel(0, 100, Color::RED);
        assert_eq!(c.get_pixel(-1, 0), None);
        assert_eq!(c.get_pixel(5, 0), None);
    }

    #[test]
    fn canvas_fill_rect() {
        let mut c = Canvas::new(10, 10, Color::BLACK);
        let r = Rect::new(2.0, 2.0, 3.0, 3.0);
        c.fill_rect(&r, Color::WHITE);
        assert_eq!(c.get_pixel(2, 2), Some(Color::WHITE));
        assert_eq!(c.get_pixel(4, 4), Some(Color::WHITE));
        assert_eq!(c.get_pixel(1, 1), Some(Color::BLACK));
        assert_eq!(c.get_pixel(5, 5), Some(Color::BLACK));
    }

    #[test]
    fn canvas_draw_line_horizontal() {
        let mut c = Canvas::new(10, 10, Color::BLACK);
        c.draw_line(0, 5, 9, 5, Color::RED);
        for x in 0..10 {
            assert_eq!(c.get_pixel(x, 5), Some(Color::RED));
        }
    }

    #[test]
    fn canvas_draw_circle() {
        let mut c = Canvas::new(20, 20, Color::BLACK);
        c.draw_circle(10, 10, 5, Color::WHITE);
        assert_eq!(c.get_pixel(10, 5), Some(Color::WHITE));
        assert_eq!(c.get_pixel(10, 10), Some(Color::BLACK));
    }

    #[test]
    fn canvas_draw_text() {
        let mut c = Canvas::new(100, 20, Color::BLACK);
        c.draw_text(0, 0, "HI", Color::WHITE);
        assert_eq!(c.get_pixel(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn button_click_cycle() {
        let mut btn = Button::new("Click", Rect::new(0.0, 0.0, 80.0, 30.0));
        assert!(!btn.pressed);
        assert!(!btn.clicked);
        let down = Event::MouseDown {
            x: 40.0,
            y: 15.0,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        };
        assert!(btn.handle_event(&down));
        assert!(btn.pressed);
        let up = Event::MouseUp {
            x: 40.0,
            y: 15.0,
            button: MouseButton::Left,
        };
        assert!(btn.handle_event(&up));
        assert!(!btn.pressed);
        assert!(btn.clicked);
    }

    #[test]
    fn button_click_outside_no_click() {
        let mut btn = Button::new("X", Rect::new(0.0, 0.0, 50.0, 30.0));
        let down = Event::MouseDown {
            x: 25.0,
            y: 15.0,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        };
        btn.handle_event(&down);
        assert!(btn.pressed);
        let up = Event::MouseUp {
            x: 200.0,
            y: 200.0,
            button: MouseButton::Left,
        };
        btn.handle_event(&up);
        assert!(!btn.pressed);
        assert!(!btn.clicked);
    }

    #[test]
    fn button_disabled() {
        let mut btn = Button::new("Disabled", Rect::new(0.0, 0.0, 80.0, 30.0));
        btn.enabled = false;
        let down = Event::MouseDown {
            x: 40.0,
            y: 15.0,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        };
        assert!(!btn.handle_event(&down));
        assert!(!btn.pressed);
    }

    #[test]
    fn text_input_insert_and_backspace() {
        let mut ti = TextInput::new(Rect::new(0.0, 0.0, 200.0, 30.0));
        ti.focused = true;
        ti.insert_char('H');
        ti.insert_char('i');
        assert_eq!(ti.value, "Hi");
        assert_eq!(ti.cursor_pos, 2);
        ti.backspace();
        assert_eq!(ti.value, "H");
        assert_eq!(ti.cursor_pos, 1);
    }

    #[test]
    fn text_input_cursor_movement() {
        let mut ti = TextInput::new(Rect::new(0.0, 0.0, 200.0, 30.0));
        ti.focused = true;
        ti.insert_char('A');
        ti.insert_char('B');
        ti.insert_char('C');
        assert_eq!(ti.cursor_pos, 3);
        ti.move_cursor_left();
        assert_eq!(ti.cursor_pos, 2);
        ti.move_cursor_left();
        assert_eq!(ti.cursor_pos, 1);
        ti.move_cursor_right();
        assert_eq!(ti.cursor_pos, 2);
        ti.cursor_pos = 0;
        ti.move_cursor_left();
        assert_eq!(ti.cursor_pos, 0);
        ti.cursor_pos = 3;
        ti.move_cursor_right();
        assert_eq!(ti.cursor_pos, 3);
    }

    #[test]
    fn text_input_delete_forward() {
        let mut ti = TextInput::new(Rect::new(0.0, 0.0, 200.0, 30.0));
        ti.value = "Hello".to_string();
        ti.cursor_pos = 0;
        ti.delete_forward();
        assert_eq!(ti.value, "ello");
    }

    #[test]
    fn text_input_keydown_events() {
        let mut ti = TextInput::new(Rect::new(0.0, 0.0, 200.0, 30.0));
        ti.focused = true;
        let key_a = Event::KeyDown {
            key: "a".to_string(),
            modifiers: Modifiers::default(),
        };
        assert!(ti.handle_event(&key_a));
        assert_eq!(ti.value, "a");
        let backspace = Event::KeyDown {
            key: "Backspace".to_string(),
            modifiers: Modifiers::default(),
        };
        ti.handle_event(&backspace);
        assert_eq!(ti.value, "");
    }

    #[test]
    fn checkbox_toggle() {
        let mut cb = Checkbox::new("Option", Rect::new(0.0, 0.0, 120.0, 24.0));
        assert!(!cb.checked);
        cb.toggle();
        assert!(cb.checked);
        assert!(cb.changed);
        cb.toggle();
        assert!(!cb.checked);
    }

    #[test]
    fn checkbox_click_event() {
        let mut cb = Checkbox::new("Test", Rect::new(0.0, 0.0, 100.0, 24.0));
        let click = Event::MouseDown {
            x: 10.0,
            y: 12.0,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        };
        assert!(cb.handle_event(&click));
        assert!(cb.checked);
        assert!(cb.changed);
    }

    #[test]
    fn slider_value_clamping() {
        let mut s = Slider::new(0.0, 100.0, 50.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        s.set_value(150.0);
        assert!((s.value - 100.0).abs() < f64::EPSILON);
        s.set_value(-10.0);
        assert!((s.value - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn slider_step_snapping() {
        let mut s = Slider::new(0.0, 100.0, 0.0, Rect::new(0.0, 0.0, 200.0, 20.0)).with_step(10.0);
        s.set_value(33.0);
        assert!((s.value - 30.0).abs() < f64::EPSILON);
        s.set_value(47.0);
        assert!((s.value - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn slider_normalized() {
        let s = Slider::new(0.0, 100.0, 25.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        assert!((s.normalized() - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn listview_selection_via_click() {
        let items = vec!["Alpha".into(), "Beta".into(), "Gamma".into()];
        let mut lv = ListView::new(items, Rect::new(0.0, 0.0, 200.0, 100.0));
        lv.item_height = 24.0;
        let click = Event::MouseDown {
            x: 50.0,
            y: 30.0,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        };
        assert!(lv.handle_event(&click));
        assert_eq!(lv.selected_index, Some(1));
    }

    #[test]
    fn listview_scroll() {
        let items = (0..20).map(|i| format!("Item {i}")).collect();
        let mut lv = ListView::new(items, Rect::new(0.0, 0.0, 200.0, 100.0));
        lv.item_height = 24.0;
        assert!((lv.scroll_offset - 0.0).abs() < f32::EPSILON);
        let scroll = Event::Scroll { dx: 0.0, dy: -1.0 };
        lv.handle_event(&scroll);
        assert!(lv.scroll_offset > 0.0);
    }

    #[test]
    fn treeview_expand_collapse() {
        let child1 = TreeNode::leaf("Child 1");
        let child2 = TreeNode::leaf("Child 2");
        let root = TreeNode::branch("Root", vec![child1, child2]);
        let mut tv = TreeView::new(vec![root], Rect::new(0.0, 0.0, 200.0, 200.0));
        assert_eq!(tv.visible_rows().len(), 1);
        tv.toggle_at(0);
        assert_eq!(tv.visible_rows().len(), 3);
        tv.toggle_at(0);
        assert_eq!(tv.visible_rows().len(), 1);
    }

    #[test]
    fn treenode_flatten_depth() {
        let grandchild = TreeNode::leaf("GC");
        let mut child = TreeNode::branch("Child", vec![grandchild]);
        child.expanded = true;
        let mut root = TreeNode::branch("Root", vec![child]);
        root.expanded = true;
        let flat = root.flatten();
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].0, 0);
        assert_eq!(flat[1].0, 1);
        assert_eq!(flat[2].0, 2);
    }

    #[test]
    fn table_sort_ascending_descending() {
        let mut t = Table::new(
            vec!["Name".into(), "Age".into()],
            Rect::new(0.0, 0.0, 300.0, 200.0),
        );
        t.add_row(vec!["Charlie".into(), "30".into()]);
        t.add_row(vec!["Alice".into(), "25".into()]);
        t.add_row(vec!["Bob".into(), "35".into()]);
        t.sort_by_column(0);
        assert_eq!(t.rows[0][0], "Alice");
        assert_eq!(t.rows[1][0], "Bob");
        assert_eq!(t.rows[2][0], "Charlie");
        t.sort_by_column(0);
        assert_eq!(t.rows[0][0], "Charlie");
        assert_eq!(t.rows[2][0], "Alice");
    }

    #[test]
    fn theme_light_dark() {
        let light = Theme::light();
        let dark = Theme::dark();
        assert_eq!(light.bg, Color::WHITE);
        assert_eq!(dark.bg, Color::new(30, 30, 30));
        assert_eq!(light.fg, Color::BLACK);
        assert_eq!(dark.fg, Color::new(230, 230, 230));
    }

    #[test]
    fn theme_high_contrast() {
        let hc = Theme::high_contrast();
        assert_eq!(hc.bg, Color::BLACK);
        assert_eq!(hc.fg, Color::WHITE);
        assert_eq!(hc.primary, Color::YELLOW);
        assert!((hc.border_radius - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn dialog_alert() {
        let d = Dialog::alert("Warning", "Something happened");
        assert_eq!(d.kind, DialogKind::Alert);
        assert_eq!(d.buttons.len(), 1);
        assert_eq!(d.buttons[0], "OK");
        assert!(!d.is_dismissed());
    }

    #[test]
    fn dialog_confirm() {
        let mut d = Dialog::confirm("Delete?", "Are you sure?");
        assert_eq!(d.kind, DialogKind::Confirm);
        assert_eq!(d.buttons.len(), 2);
        assert!(!d.is_confirmed());
        d.result = Some(0);
        assert!(d.is_confirmed());
        assert!(d.is_dismissed());
    }

    #[test]
    fn dialog_prompt() {
        let d = Dialog::prompt("Name", "Enter your name:");
        assert_eq!(d.kind, DialogKind::Prompt);
        assert_eq!(d.buttons.len(), 2);
        assert!(d.input_value.is_empty());
    }

    #[test]
    fn menu_hierarchy() {
        let file_menu = MenuItem::new("File")
            .with_child(MenuItem::new("New").with_shortcut("Ctrl+N"))
            .with_child(MenuItem::new("Open").with_shortcut("Ctrl+O"))
            .with_child(MenuItem::separator())
            .with_child(MenuItem::new("Exit"));
        assert_eq!(file_menu.children.len(), 4);
        assert!(file_menu.has_children());
        assert!(file_menu.children[2].is_separator());
        assert_eq!(file_menu.children[0].shortcut.as_deref(), Some("Ctrl+N"));
    }

    #[test]
    fn menubar_add_menus() {
        let bar = MenuBar::new()
            .add_menu(MenuItem::new("File"))
            .add_menu(MenuItem::new("Edit"))
            .add_menu(MenuItem::new("View"));
        assert_eq!(bar.menu_count(), 3);
        assert!(bar.open_menu.is_none());
    }

    #[test]
    fn statusbar_segments() {
        let sb = StatusBar::new()
            .add_segment(StatusSegment::new("Ready", TextAlign::Left))
            .add_segment(StatusSegment::new("Ln 42, Col 8", TextAlign::Right));
        assert_eq!(sb.segments.len(), 2);
        assert_eq!(sb.segments[0].text, "Ready");
        assert_eq!(sb.segments[1].align, TextAlign::Right);
    }

    #[test]
    fn button_renders_without_panic() {
        let btn = Button::new("Test", Rect::new(5.0, 5.0, 60.0, 24.0));
        let mut canvas = Canvas::new(100, 50, Color::WHITE);
        btn.render(&mut canvas);
        let center = canvas.get_pixel(35, 17).unwrap();
        assert_ne!(center, Color::WHITE);
    }

    #[test]
    fn label_renders_text() {
        let label = Label::new("Hi", Rect::new(0.0, 0.0, 50.0, 20.0));
        let mut canvas = Canvas::new(50, 20, Color::WHITE);
        label.render(&mut canvas);
        let has_black = canvas.pixels.iter().any(|c| *c != Color::WHITE);
        assert!(has_black);
    }

    #[test]
    fn textarea_insert_newline() {
        let mut ta = TextArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        ta.insert_char('A');
        ta.insert_char('\n');
        ta.insert_char('B');
        assert_eq!(ta.lines.len(), 2);
        assert_eq!(ta.lines[0], "A");
        assert_eq!(ta.lines[1], "B");
        assert_eq!(ta.cursor_line, 1);
    }

    #[test]
    fn textarea_set_and_get_text() {
        let mut ta = TextArea::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        ta.set_text("Line 1\nLine 2\nLine 3");
        assert_eq!(ta.lines.len(), 3);
        assert_eq!(ta.text(), "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn scrollview_thumb_position() {
        let mut sv = ScrollView::new(1000.0, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert!((sv.thumb_position() - 0.0).abs() < f32::EPSILON);
        sv.scroll_to(400.0);
        assert!((sv.scroll_y - 400.0).abs() < f32::EPSILON);
        assert!(sv.thumb_position() > 0.0);
        assert!(sv.thumb_position() < 1.0);
    }

    #[test]
    fn scrollview_max_scroll() {
        let sv = ScrollView::new(500.0, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert!((sv.max_scroll() - 300.0).abs() < f32::EPSILON);
    }

    #[test]
    fn splitview_panes() {
        let sv = SplitView::new(Orientation::Horizontal, Rect::new(0.0, 0.0, 200.0, 100.0));
        let first = sv.first_pane();
        let second = sv.second_pane();
        assert!((first.width - 100.0).abs() < f32::EPSILON);
        assert!(second.x > 100.0);
    }

    #[test]
    fn dropdown_selection() {
        let mut dd = Dropdown::new(
            vec!["Option A".into(), "Option B".into(), "Option C".into()],
            Rect::new(0.0, 0.0, 150.0, 24.0),
        );
        assert_eq!(dd.selected(), Some("Option A"));
        let click_header = Event::MouseDown {
            x: 75.0,
            y: 12.0,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        };
        dd.handle_event(&click_header);
        assert!(dd.open);
        // Expanded list starts at y=24 (header bottom). Each option is 24px tall.
        // Option A: y=24..48, Option B: y=48..72. Click at y=60 selects Option B.
        let click_opt = Event::MouseDown {
            x: 75.0,
            y: 60.0,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        };
        dd.handle_event(&click_opt);
        assert!(!dd.open);
        assert_eq!(dd.selected(), Some("Option B"));
    }

    #[test]
    fn radio_button_selection() {
        let mut rb = RadioButton::new(
            "group1",
            vec!["One".into(), "Two".into(), "Three".into()],
            Rect::new(0.0, 0.0, 100.0, 75.0),
        );
        assert_eq!(rb.selected_index, 0);
        assert_eq!(rb.selected_label(), Some("One"));
        let click = Event::MouseDown {
            x: 50.0,
            y: 37.0,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        };
        rb.handle_event(&click);
        assert_eq!(rb.selected_index, 1);
        assert_eq!(rb.selected_label(), Some("Two"));
    }

    #[test]
    fn progress_bar_set_value() {
        let mut pb = ProgressBar::new(Rect::new(0.0, 0.0, 200.0, 20.0));
        pb.set_value(0.5);
        assert!((pb.value - 0.5).abs() < f64::EPSILON);
        pb.set_value(1.5);
        assert!((pb.value - 1.0).abs() < f64::EPSILON);
        pb.set_value(-0.5);
        assert!((pb.value - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tabview_switch_tabs() {
        let tabs = vec![
            Tab {
                title: "Tab 1".into(),
                content_id: 0,
            },
            Tab {
                title: "Tab 2".into(),
                content_id: 1,
            },
        ];
        let mut tv = TabView::new(tabs, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert_eq!(tv.active_tab, 0);
        let tw = tv.tab_width();
        let click = Event::MouseDown {
            x: tw + 10.0,
            y: 10.0,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        };
        tv.handle_event(&click);
        assert_eq!(tv.active_tab, 1);
    }

    #[test]
    fn image_widget_sample() {
        let pixels = vec![Color::RED, Color::GREEN, Color::BLUE, Color::WHITE];
        let img = ImageWidget::new(2, 2, pixels, Rect::new(0.0, 0.0, 20.0, 20.0));
        let tl = img.sample(0.0, 0.0);
        assert_eq!(tl, Color::RED);
        let tr = img.sample(0.9, 0.0);
        assert_eq!(tr, Color::GREEN);
    }
}
