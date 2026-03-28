//! GPU compositor and desktop environment for FajarOS Nova v2.0.
//!
//! Provides a simulated windowing system with compositing, window management,
//! taskbar, cursor, clipboard, theming, and screen capture. All rendering
//! targets an in-memory framebuffer — no real GPU hardware is touched.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Pixel & Framebuffer
// ═══════════════════════════════════════════════════════════════════════

/// An RGBA pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixel {
    /// Red channel (0-255).
    pub r: u8,
    /// Green channel (0-255).
    pub g: u8,
    /// Blue channel (0-255).
    pub b: u8,
    /// Alpha channel (0 = transparent, 255 = opaque).
    pub a: u8,
}

impl Pixel {
    /// Creates an opaque pixel.
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Creates a pixel with the given alpha.
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Black (opaque).
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    /// White (opaque).
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    /// Fully transparent.
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    /// Alpha-blends `src` (self) over `dst` using the "over" operator.
    ///
    /// result = src * src_a + dst * (1 - src_a)
    pub fn blend(self, dst: Pixel) -> Pixel {
        if self.a == 255 {
            return self;
        }
        if self.a == 0 {
            return dst;
        }
        let sa = self.a as u16;
        let da = 255 - sa;
        Pixel {
            r: ((self.r as u16 * sa + dst.r as u16 * da) / 255) as u8,
            g: ((self.g as u16 * sa + dst.g as u16 * da) / 255) as u8,
            b: ((self.b as u16 * sa + dst.b as u16 * da) / 255) as u8,
            a: ((sa + dst.a as u16 * da / 255).min(255)) as u8,
        }
    }
}

impl Default for Pixel {
    fn default() -> Self {
        Self::BLACK
    }
}

/// A 2D pixel framebuffer.
///
/// Stores pixels in row-major order. Coordinates are `(x, y)` with origin
/// at the top-left corner.
#[derive(Debug, Clone)]
pub struct Framebuffer {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel data in row-major order.
    pixels: Vec<Pixel>,
}

impl Framebuffer {
    /// Creates a framebuffer filled with black.
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            pixels: vec![Pixel::BLACK; size],
        }
    }

    /// Creates a framebuffer filled with the given color.
    pub fn with_color(width: u32, height: u32, color: Pixel) -> Self {
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            pixels: vec![color; size],
        }
    }

    /// Returns the pixel index for `(x, y)`, or `None` if out of bounds.
    fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x < self.width && y < self.height {
            Some(y as usize * self.width as usize + x as usize)
        } else {
            None
        }
    }

    /// Returns the pixel at `(x, y)`, or `None` if out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Pixel> {
        self.index(x, y).map(|i| self.pixels[i])
    }

    /// Sets the pixel at `(x, y)`. No-op if out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Pixel) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i] = color;
        }
    }

    /// Fills a rectangle with the given color.
    ///
    /// Coordinates are clamped to the framebuffer bounds.
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: Pixel) {
        let x_end = (x.saturating_add(w)).min(self.width);
        let y_end = (y.saturating_add(h)).min(self.height);
        for py in y..y_end {
            for px in x..x_end {
                if let Some(i) = self.index(px, py) {
                    self.pixels[i] = color;
                }
            }
        }
    }

    /// Draws a line from `(x0, y0)` to `(x1, y1)` using Bresenham's algorithm.
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Pixel) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx: i32 = if x0 < x1 { 1 } else { -1 };
        let sy: i32 = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut cx = x0;
        let mut cy = y0;

        loop {
            if cx >= 0 && cy >= 0 {
                self.set_pixel(cx as u32, cy as u32, color);
            }
            if cx == x1 && cy == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                cx += sx;
            }
            if e2 <= dx {
                err += dx;
                cy += sy;
            }
        }
    }

    /// Clears the entire framebuffer to the given color.
    pub fn clear(&mut self, color: Pixel) {
        for p in &mut self.pixels {
            *p = color;
        }
    }

    /// Blits (copies) a source framebuffer onto this one at position `(dx, dy)`.
    ///
    /// Performs alpha blending if the source has semi-transparent pixels.
    pub fn blit(&mut self, src: &Framebuffer, dx: u32, dy: u32) {
        for sy in 0..src.height {
            for sx in 0..src.width {
                let tx = dx.saturating_add(sx);
                let ty = dy.saturating_add(sy);
                if let (Some(src_px), Some(dst_idx)) = (src.get_pixel(sx, sy), self.index(tx, ty)) {
                    self.pixels[dst_idx] = src_px.blend(self.pixels[dst_idx]);
                }
            }
        }
    }

    /// Returns the raw pixel data as a flat byte array (RGBA, 4 bytes per pixel).
    pub fn as_raw_rgba(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(self.pixels.len() * 4);
        for p in &self.pixels {
            data.push(p.r);
            data.push(p.g);
            data.push(p.b);
            data.push(p.a);
        }
        data
    }

    /// Returns the total number of pixels.
    pub fn pixel_count(&self) -> usize {
        self.pixels.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Window & WindowManager
// ═══════════════════════════════════════════════════════════════════════

/// Unique identifier for a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u32);

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Window({})", self.0)
    }
}

/// A single desktop window.
#[derive(Debug, Clone)]
pub struct Window {
    /// Unique identifier.
    pub id: WindowId,
    /// Window title text.
    pub title: String,
    /// X position (top-left corner).
    pub x: i32,
    /// Y position (top-left corner).
    pub y: i32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Whether the window is visible.
    pub visible: bool,
    /// Whether the window currently has input focus.
    pub focused: bool,
    /// Whether the window is minimized.
    pub minimized: bool,
    /// Z-order (higher = on top).
    pub z_order: u32,
    /// The window's content framebuffer.
    pub framebuffer: Framebuffer,
}

impl Window {
    /// Creates a new window with a white content area.
    pub fn new(id: WindowId, title: &str, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            title: title.to_string(),
            x,
            y,
            width,
            height,
            visible: true,
            focused: false,
            minimized: false,
            z_order: 0,
            framebuffer: Framebuffer::with_color(width, height, Pixel::WHITE),
        }
    }

    /// Returns `true` if the point `(px, py)` falls within this window's bounds.
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && py >= self.y
            && px < self.x + self.width as i32
            && py < self.y + self.height as i32
    }
}

/// Title bar height in pixels.
const TITLE_BAR_HEIGHT: u32 = 24;

/// Border width in pixels.
const BORDER_WIDTH: u32 = 1;

/// Desktop window manager.
///
/// Manages creation, focus, z-ordering, and compositing of windows.
#[derive(Debug)]
pub struct WindowManager {
    /// All managed windows.
    windows: Vec<Window>,
    /// ID of the currently focused window.
    focused_id: Option<WindowId>,
    /// Next window ID to assign.
    next_id: u32,
}

impl WindowManager {
    /// Creates an empty window manager.
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            focused_id: None,
            next_id: 1,
        }
    }

    /// Creates a new window and returns its ID.
    pub fn create_window(
        &mut self,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> WindowId {
        let id = WindowId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let z = self.windows.len() as u32;
        let mut win = Window::new(id, title, x, y, width, height);
        win.z_order = z;
        self.windows.push(win);
        self.focus_window(id);
        id
    }

    /// Closes (removes) a window by ID.
    pub fn close_window(&mut self, id: WindowId) {
        self.windows.retain(|w| w.id != id);
        if self.focused_id == Some(id) {
            self.focused_id = self.windows.last().map(|w| w.id);
            if let Some(fid) = self.focused_id {
                if let Some(w) = self.windows.iter_mut().find(|w| w.id == fid) {
                    w.focused = true;
                }
            }
        }
    }

    /// Gives focus to a window, raising it to the top of the z-order.
    pub fn focus_window(&mut self, id: WindowId) {
        // Unfocus previous
        for w in &mut self.windows {
            w.focused = false;
        }
        // Focus target and raise z-order
        let max_z = self.windows.len() as u32;
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.focused = true;
            w.z_order = max_z;
            w.minimized = false;
        }
        self.focused_id = Some(id);
    }

    /// Moves a window to a new position.
    pub fn move_window(&mut self, id: WindowId, x: i32, y: i32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.x = x;
            w.y = y;
        }
    }

    /// Resizes a window and reallocates its framebuffer.
    pub fn resize_window(&mut self, id: WindowId, width: u32, height: u32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.width = width;
            w.height = height;
            w.framebuffer = Framebuffer::with_color(width, height, Pixel::WHITE);
        }
    }

    /// Minimizes a window (hides it but keeps it in the window list).
    pub fn minimize(&mut self, id: WindowId) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.minimized = true;
            w.focused = false;
        }
        if self.focused_id == Some(id) {
            self.focused_id = None;
        }
    }

    /// Maximizes a window to fill the given screen dimensions.
    pub fn maximize(&mut self, id: WindowId, screen_w: u32, screen_h: u32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.x = 0;
            w.y = 0;
            w.width = screen_w;
            w.height = screen_h;
            w.minimized = false;
            w.framebuffer = Framebuffer::with_color(screen_w, screen_h, Pixel::WHITE);
        }
        self.focus_window(id);
    }

    /// Finds the topmost visible window at the given screen coordinates.
    pub fn find_window_at(&self, x: i32, y: i32) -> Option<WindowId> {
        // Check in reverse z-order (highest z first)
        let mut sorted: Vec<&Window> = self
            .windows
            .iter()
            .filter(|w| w.visible && !w.minimized)
            .collect();
        sorted.sort_by(|a, b| b.z_order.cmp(&a.z_order));

        for w in sorted {
            // Include title bar and border in hit area
            let total_x = w.x - BORDER_WIDTH as i32;
            let total_y = w.y - TITLE_BAR_HEIGHT as i32;
            let total_w = w.width + 2 * BORDER_WIDTH;
            let total_h = w.height + TITLE_BAR_HEIGHT + BORDER_WIDTH;
            if x >= total_x
                && y >= total_y
                && x < total_x + total_w as i32
                && y < total_y + total_h as i32
            {
                return Some(w.id);
            }
        }
        None
    }

    /// Composites all visible, non-minimized windows onto the target framebuffer.
    ///
    /// Windows are drawn in z-order (lowest first, highest on top).
    /// Each window is drawn with decorations (title bar, border).
    pub fn render_all(&self, target: &mut Framebuffer) {
        let mut sorted: Vec<&Window> = self
            .windows
            .iter()
            .filter(|w| w.visible && !w.minimized)
            .collect();
        sorted.sort_by(|a, b| a.z_order.cmp(&b.z_order));

        for w in sorted {
            Decoration::render(w, target);
            // Blit window content below the title bar
            if w.x >= 0 && w.y >= 0 {
                target.blit(&w.framebuffer, w.x as u32, w.y as u32);
            }
        }
    }

    /// Returns the currently focused window ID.
    pub fn focused(&self) -> Option<WindowId> {
        self.focused_id
    }

    /// Returns the number of managed windows.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Returns a reference to a window by ID.
    pub fn get_window(&self, id: WindowId) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    /// Returns a mutable reference to a window by ID.
    pub fn get_window_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    /// Returns all window IDs and titles (for taskbar rendering).
    pub fn window_list(&self) -> Vec<(WindowId, String)> {
        self.windows
            .iter()
            .map(|w| (w.id, w.title.clone()))
            .collect()
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Decoration
// ═══════════════════════════════════════════════════════════════════════

/// Window decoration renderer (title bar, buttons, border).
pub struct Decoration;

impl Decoration {
    /// Renders window decorations (title bar and border) onto the target.
    pub fn render(window: &Window, target: &mut Framebuffer) {
        let title_color = if window.focused {
            Pixel::rgb(50, 100, 200)
        } else {
            Pixel::rgb(120, 120, 120)
        };

        // Title bar
        let tb_x = window.x.max(0) as u32;
        let tb_y = (window.y - TITLE_BAR_HEIGHT as i32).max(0) as u32;
        target.fill_rect(tb_x, tb_y, window.width, TITLE_BAR_HEIGHT, title_color);

        // Close button (red square at top-right)
        let close_x = tb_x.saturating_add(window.width).saturating_sub(18);
        target.fill_rect(close_x, tb_y + 4, 14, 14, Pixel::rgb(220, 50, 50));

        // Minimize button (yellow square)
        let min_x = close_x.saturating_sub(20);
        target.fill_rect(min_x, tb_y + 4, 14, 14, Pixel::rgb(220, 200, 50));

        // Maximize button (green square)
        let max_x = min_x.saturating_sub(20);
        target.fill_rect(max_x, tb_y + 4, 14, 14, Pixel::rgb(50, 200, 50));

        // Border (1px gray)
        let border_color = Pixel::rgb(80, 80, 80);
        let bx = window.x.max(0) as u32;
        let by = tb_y;
        let bw = window.width + 2 * BORDER_WIDTH;
        let bh = window.height + TITLE_BAR_HEIGHT + BORDER_WIDTH;

        // Top border
        target.fill_rect(
            bx.saturating_sub(BORDER_WIDTH),
            by,
            bw,
            BORDER_WIDTH,
            border_color,
        );
        // Bottom border
        target.fill_rect(
            bx.saturating_sub(BORDER_WIDTH),
            by + bh - BORDER_WIDTH,
            bw,
            BORDER_WIDTH,
            border_color,
        );
        // Left border
        target.fill_rect(
            bx.saturating_sub(BORDER_WIDTH),
            by,
            BORDER_WIDTH,
            bh,
            border_color,
        );
        // Right border
        target.fill_rect(bx + window.width, by, BORDER_WIDTH, bh, border_color);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Taskbar
// ═══════════════════════════════════════════════════════════════════════

/// A single item in the taskbar.
#[derive(Debug, Clone)]
pub struct TaskbarItem {
    /// Window ID this item represents.
    pub window_id: WindowId,
    /// Display title (truncated).
    pub title: String,
    /// X position of this item in the taskbar.
    pub x: u32,
    /// Width of this item.
    pub width: u32,
}

/// Desktop taskbar rendered at the bottom of the screen.
#[derive(Debug)]
pub struct Taskbar {
    /// Height of the taskbar in pixels.
    pub height: u32,
    /// Items representing open windows.
    items: Vec<TaskbarItem>,
    /// Background color.
    pub bg_color: Pixel,
}

impl Taskbar {
    /// Default taskbar height.
    pub const DEFAULT_HEIGHT: u32 = 32;

    /// Creates a new taskbar.
    pub fn new() -> Self {
        Self {
            height: Self::DEFAULT_HEIGHT,
            items: Vec::new(),
            bg_color: Pixel::rgb(40, 40, 40),
        }
    }

    /// Updates the taskbar items from the window manager's window list.
    pub fn update(&mut self, windows: &[(WindowId, String)], screen_width: u32) {
        self.items.clear();
        if windows.is_empty() {
            return;
        }
        let item_width = (screen_width / windows.len().max(1) as u32).min(150);
        for (i, (id, title)) in windows.iter().enumerate() {
            let display_title = if title.len() > 15 {
                format!("{}...", &title[..12])
            } else {
                title.clone()
            };
            self.items.push(TaskbarItem {
                window_id: *id,
                title: display_title,
                x: i as u32 * item_width,
                width: item_width,
            });
        }
    }

    /// Renders the taskbar onto the framebuffer at the bottom of the screen.
    pub fn render(&self, fb: &mut Framebuffer) {
        let y = fb.height.saturating_sub(self.height);
        fb.fill_rect(0, y, fb.width, self.height, self.bg_color);

        // Draw item separators
        for item in &self.items {
            let sep_x = item.x + item.width;
            fb.fill_rect(sep_x, y + 2, 1, self.height - 4, Pixel::rgb(80, 80, 80));
        }
    }

    /// Determines which taskbar item was clicked at the given x coordinate.
    pub fn click(&self, x: u32) -> Option<WindowId> {
        for item in &self.items {
            if x >= item.x && x < item.x + item.width {
                return Some(item.window_id);
            }
        }
        None
    }

    /// Returns the number of taskbar items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

impl Default for Taskbar {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Desktop
// ═══════════════════════════════════════════════════════════════════════

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Right mouse button.
    Right,
    /// Middle mouse button.
    Middle,
}

/// The top-level desktop environment.
///
/// Combines the window manager, taskbar, cursor, and background into a
/// single composited desktop that renders to a framebuffer.
#[derive(Debug)]
pub struct Desktop {
    /// Background color.
    pub bg_color: Pixel,
    /// Optional wallpaper framebuffer.
    pub wallpaper: Option<Framebuffer>,
    /// Window manager.
    pub window_manager: WindowManager,
    /// Taskbar at the bottom.
    pub taskbar: Taskbar,
    /// Screen width.
    pub screen_width: u32,
    /// Screen height.
    pub screen_height: u32,
}

impl Desktop {
    /// Creates a new desktop environment with the given screen size.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            bg_color: Pixel::rgb(0, 120, 180),
            wallpaper: None,
            window_manager: WindowManager::new(),
            taskbar: Taskbar::new(),
            screen_width: width,
            screen_height: height,
        }
    }

    /// Renders the entire desktop (background + windows + taskbar) to the framebuffer.
    pub fn render(&mut self, fb: &mut Framebuffer) {
        // Background
        if let Some(wp) = &self.wallpaper {
            fb.blit(wp, 0, 0);
        } else {
            fb.clear(self.bg_color);
        }

        // Windows
        self.window_manager.render_all(fb);

        // Taskbar
        let win_list = self.window_manager.window_list();
        self.taskbar.update(&win_list, self.screen_width);
        self.taskbar.render(fb);
    }

    /// Handles a mouse click event.
    ///
    /// Dispatches to the taskbar (if at the bottom of the screen) or
    /// to the window manager (focus/raise the clicked window).
    pub fn handle_mouse(&mut self, x: i32, y: i32, _button: MouseButton) {
        let taskbar_y = self.screen_height.saturating_sub(self.taskbar.height) as i32;

        if y >= taskbar_y {
            // Click in taskbar
            if let Some(wid) = self.taskbar.click(x as u32) {
                self.window_manager.focus_window(wid);
            }
        } else if let Some(wid) = self.window_manager.find_window_at(x, y) {
            self.window_manager.focus_window(wid);
        }
    }

    /// Handles a key press event, dispatching to the focused window.
    ///
    /// Returns the focused window ID if there is one, `None` otherwise.
    pub fn handle_key(&self, _key: u8) -> Option<WindowId> {
        self.window_manager.focused()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Cursor
// ═══════════════════════════════════════════════════════════════════════

/// Cursor visual style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    /// Default arrow cursor.
    Arrow,
    /// Text editing I-beam.
    IBeam,
    /// Pointer/hand for clickable items.
    Pointer,
    /// Resize cursor.
    Resize,
    /// Busy/loading.
    Wait,
}

/// Mouse cursor with position and style.
#[derive(Debug)]
pub struct Cursor {
    /// X position.
    pub x: i32,
    /// Y position.
    pub y: i32,
    /// Visual style.
    pub style: CursorStyle,
    /// Cursor color.
    pub color: Pixel,
}

impl Cursor {
    /// Creates a new cursor at `(0, 0)`.
    pub fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            style: CursorStyle::Arrow,
            color: Pixel::WHITE,
        }
    }

    /// Moves the cursor to the given position.
    pub fn move_to(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    /// Renders the cursor onto the framebuffer as a small sprite.
    ///
    /// Draws a simple 8x8 arrow or block depending on style.
    pub fn render(&self, fb: &mut Framebuffer) {
        if self.x < 0 || self.y < 0 {
            return;
        }
        let px = self.x as u32;
        let py = self.y as u32;

        match self.style {
            CursorStyle::Arrow => {
                // Simple arrow: diagonal line + fill
                for i in 0u32..8 {
                    for j in 0..=i {
                        fb.set_pixel(px + j, py + i, self.color);
                    }
                }
            }
            CursorStyle::IBeam => {
                // Vertical bar
                for i in 0u32..12 {
                    fb.set_pixel(px, py + i, self.color);
                }
                // Top and bottom serifs
                for j in 0u32..4 {
                    fb.set_pixel(px.wrapping_sub(2) + j, py, self.color);
                    fb.set_pixel(px.wrapping_sub(2) + j, py + 11, self.color);
                }
            }
            CursorStyle::Pointer => {
                // Hand-like: small filled rectangle
                for i in 0u32..6 {
                    for j in 0u32..4 {
                        fb.set_pixel(px + j, py + i, self.color);
                    }
                }
            }
            CursorStyle::Resize => {
                // Double-headed arrow
                for i in 0u32..8 {
                    fb.set_pixel(px + 3, py + i, self.color);
                }
                // Top arrowhead
                fb.set_pixel(px + 2, py + 1, self.color);
                fb.set_pixel(px + 4, py + 1, self.color);
                // Bottom arrowhead
                fb.set_pixel(px + 2, py + 6, self.color);
                fb.set_pixel(px + 4, py + 6, self.color);
            }
            CursorStyle::Wait => {
                // Hourglass: two triangles
                for i in 0u32..4 {
                    for j in i..=(6 - i) {
                        fb.set_pixel(px + j, py + i, self.color);
                        fb.set_pixel(px + j, py + 7 - i, self.color);
                    }
                }
            }
        }
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Clipboard
// ═══════════════════════════════════════════════════════════════════════

/// A simple text clipboard.
#[derive(Debug, Clone, Default)]
pub struct Clipboard {
    /// Stored text content.
    text: Option<String>,
}

impl Clipboard {
    /// Creates an empty clipboard.
    pub fn new() -> Self {
        Self { text: None }
    }

    /// Copies text to the clipboard.
    pub fn copy(&mut self, text: &str) {
        self.text = Some(text.to_string());
    }

    /// Pastes (returns) the clipboard content, if any.
    pub fn paste(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// Returns `true` if the clipboard has content.
    pub fn has_content(&self) -> bool {
        self.text.is_some()
    }

    /// Clears the clipboard.
    pub fn clear(&mut self) {
        self.text = None;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Screen Capture
// ═══════════════════════════════════════════════════════════════════════

/// Screen capture utility.
pub struct ScreenCapture;

impl ScreenCapture {
    /// Captures the entire framebuffer as raw RGBA pixel data.
    ///
    /// Returns a `Vec<u8>` with 4 bytes per pixel (R, G, B, A) in row-major order.
    pub fn capture(fb: &Framebuffer) -> Vec<u8> {
        fb.as_raw_rgba()
    }

    /// Returns the size of the capture in bytes for the given dimensions.
    pub fn capture_size(width: u32, height: u32) -> usize {
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Theme
// ═══════════════════════════════════════════════════════════════════════

/// Visual theme for the desktop environment.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Desktop background color.
    pub bg_color: Pixel,
    /// Window title bar color.
    pub title_bar_color: Pixel,
    /// Text color.
    pub text_color: Pixel,
    /// Accent color (focused borders, selections).
    pub accent_color: Pixel,
    /// Base font size in pixels.
    pub font_size: u32,
}

impl Theme {
    /// A light color theme.
    pub fn default_light() -> Self {
        Self {
            bg_color: Pixel::rgb(220, 230, 240),
            title_bar_color: Pixel::rgb(200, 210, 220),
            text_color: Pixel::rgb(30, 30, 30),
            accent_color: Pixel::rgb(0, 120, 215),
            font_size: 14,
        }
    }

    /// A dark color theme.
    pub fn default_dark() -> Self {
        Self {
            bg_color: Pixel::rgb(30, 30, 30),
            title_bar_color: Pixel::rgb(50, 50, 50),
            text_color: Pixel::rgb(220, 220, 220),
            accent_color: Pixel::rgb(0, 150, 255),
            font_size: 14,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Compositor Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Performance metrics for the compositor.
#[derive(Debug, Clone, Default)]
pub struct CompositorMetrics {
    /// Current frames per second.
    pub fps: f64,
    /// Time to render the last frame in milliseconds.
    pub frame_time_ms: f64,
    /// Number of windows rendered in the last frame.
    pub windows_rendered: u32,
    /// Number of damage rectangles processed.
    pub damage_rects: u32,
}

impl CompositorMetrics {
    /// Creates zeroed metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates FPS from the given frame time (in milliseconds).
    pub fn update(&mut self, frame_time_ms: f64, windows: u32, damage: u32) {
        self.frame_time_ms = frame_time_ms;
        if frame_time_ms > 0.0 {
            self.fps = 1000.0 / frame_time_ms;
        }
        self.windows_rendered = windows;
        self.damage_rects = damage;
    }
}

impl fmt::Display for CompositorMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.1} FPS, {:.2}ms/frame, {} windows, {} damage rects",
            self.fps, self.frame_time_ms, self.windows_rendered, self.damage_rects,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pixel ──

    #[test]
    fn pixel_blend_opaque_over() {
        let src = Pixel::rgb(255, 0, 0);
        let dst = Pixel::rgb(0, 0, 255);
        let result = src.blend(dst);
        assert_eq!(result, Pixel::rgb(255, 0, 0)); // opaque src wins
    }

    #[test]
    fn pixel_blend_transparent() {
        let src = Pixel::TRANSPARENT;
        let dst = Pixel::rgb(0, 255, 0);
        let result = src.blend(dst);
        assert_eq!(result, dst); // transparent src → dst unchanged
    }

    #[test]
    fn pixel_blend_semi_transparent() {
        let src = Pixel::rgba(255, 0, 0, 128);
        let dst = Pixel::rgb(0, 0, 255);
        let result = src.blend(dst);
        // red ~128, blue ~127
        assert!(result.r > 100 && result.r < 160);
        assert!(result.b > 100 && result.b < 160);
    }

    // ── Framebuffer ──

    #[test]
    fn framebuffer_set_get_pixel() {
        let mut fb = Framebuffer::new(10, 10);
        let red = Pixel::rgb(255, 0, 0);
        fb.set_pixel(5, 5, red);
        assert_eq!(fb.get_pixel(5, 5), Some(red));
        assert_eq!(fb.get_pixel(100, 100), None); // out of bounds
    }

    #[test]
    fn framebuffer_fill_rect() {
        let mut fb = Framebuffer::new(20, 20);
        let green = Pixel::rgb(0, 255, 0);
        fb.fill_rect(5, 5, 3, 3, green);
        assert_eq!(fb.get_pixel(5, 5), Some(green));
        assert_eq!(fb.get_pixel(7, 7), Some(green));
        assert_eq!(fb.get_pixel(8, 8), Some(Pixel::BLACK)); // outside rect
    }

    #[test]
    fn framebuffer_clear() {
        let mut fb = Framebuffer::new(4, 4);
        fb.set_pixel(0, 0, Pixel::rgb(255, 0, 0));
        fb.clear(Pixel::WHITE);
        assert_eq!(fb.get_pixel(0, 0), Some(Pixel::WHITE));
        assert_eq!(fb.get_pixel(3, 3), Some(Pixel::WHITE));
    }

    #[test]
    fn framebuffer_blit() {
        let mut dst = Framebuffer::new(10, 10);
        let mut src = Framebuffer::new(3, 3);
        src.clear(Pixel::rgb(0, 255, 0));
        dst.blit(&src, 2, 2);
        assert_eq!(dst.get_pixel(2, 2), Some(Pixel::rgb(0, 255, 0)));
        assert_eq!(dst.get_pixel(4, 4), Some(Pixel::rgb(0, 255, 0)));
    }

    #[test]
    fn framebuffer_draw_line() {
        let mut fb = Framebuffer::new(10, 10);
        let white = Pixel::WHITE;
        fb.draw_line(0, 0, 9, 0, white);
        assert_eq!(fb.get_pixel(0, 0), Some(white));
        assert_eq!(fb.get_pixel(9, 0), Some(white));
        assert_eq!(fb.get_pixel(0, 1), Some(Pixel::BLACK)); // not on line
    }

    #[test]
    fn framebuffer_raw_rgba() {
        let fb = Framebuffer::new(2, 2);
        let raw = fb.as_raw_rgba();
        assert_eq!(raw.len(), 2 * 2 * 4);
    }

    // ── Window & WindowManager ──

    #[test]
    fn window_manager_create_and_focus() {
        let mut wm = WindowManager::new();
        let id1 = wm.create_window("App1", 10, 10, 100, 80);
        let id2 = wm.create_window("App2", 50, 50, 100, 80);
        assert_eq!(wm.window_count(), 2);
        assert_eq!(wm.focused(), Some(id2)); // last created gets focus
        wm.focus_window(id1);
        assert_eq!(wm.focused(), Some(id1));
    }

    #[test]
    fn window_manager_close() {
        let mut wm = WindowManager::new();
        let id1 = wm.create_window("A", 0, 0, 50, 50);
        let _id2 = wm.create_window("B", 0, 0, 50, 50);
        wm.close_window(id1);
        assert_eq!(wm.window_count(), 1);
    }

    #[test]
    fn window_manager_move_and_resize() {
        let mut wm = WindowManager::new();
        let id = wm.create_window("Test", 10, 10, 100, 80);
        wm.move_window(id, 200, 300);
        wm.resize_window(id, 400, 300);
        let w = wm.get_window(id);
        assert!(w.is_some());
        let w = w.unwrap();
        assert_eq!(w.x, 200);
        assert_eq!(w.y, 300);
        assert_eq!(w.width, 400);
        assert_eq!(w.height, 300);
    }

    #[test]
    fn window_manager_minimize() {
        let mut wm = WindowManager::new();
        let id = wm.create_window("Mini", 0, 0, 100, 100);
        wm.minimize(id);
        let w = wm.get_window(id);
        assert!(w.unwrap().minimized);
        assert_eq!(wm.focused(), None);
    }

    #[test]
    fn window_manager_render_composites() {
        let mut wm = WindowManager::new();
        let _id = wm.create_window("Win", 10, 30, 50, 50);
        let mut fb = Framebuffer::new(320, 200);
        wm.render_all(&mut fb);
        // Window content area should have white pixels (default window color)
        assert_eq!(fb.get_pixel(10, 30), Some(Pixel::WHITE));
    }

    // ── Taskbar ──

    #[test]
    fn taskbar_click_dispatch() {
        let mut tb = Taskbar::new();
        let windows = vec![
            (WindowId(1), "App1".to_string()),
            (WindowId(2), "App2".to_string()),
        ];
        tb.update(&windows, 300);
        assert_eq!(tb.item_count(), 2);
        // Click in first item area
        let clicked = tb.click(10);
        assert_eq!(clicked, Some(WindowId(1)));
    }

    // ── Clipboard ──

    #[test]
    fn clipboard_copy_paste() {
        let mut cb = Clipboard::new();
        assert!(!cb.has_content());
        cb.copy("Hello, FajarOS!");
        assert!(cb.has_content());
        assert_eq!(cb.paste(), Some("Hello, FajarOS!"));
        cb.clear();
        assert_eq!(cb.paste(), None);
    }

    // ── Theme ──

    #[test]
    fn theme_light_and_dark() {
        let light = Theme::default_light();
        let dark = Theme::default_dark();
        // Light theme has bright background
        assert!(light.bg_color.r > 200);
        // Dark theme has dark background
        assert!(dark.bg_color.r < 50);
    }

    // ── Compositor Metrics ──

    #[test]
    fn compositor_metrics_update() {
        let mut m = CompositorMetrics::new();
        m.update(16.67, 5, 3);
        assert!((m.fps - 60.0).abs() < 1.0);
        assert_eq!(m.windows_rendered, 5);
        assert_eq!(m.damage_rects, 3);
    }

    // ── Desktop ──

    #[test]
    fn desktop_render_and_mouse() {
        let mut desktop = Desktop::new(320, 200);
        let wid = desktop
            .window_manager
            .create_window("Test", 50, 50, 100, 80);
        let mut fb = Framebuffer::new(320, 200);
        desktop.render(&mut fb);
        // After render, the background should not be black (it's the desktop color)
        let bg = fb.get_pixel(0, 0);
        assert_ne!(bg, Some(Pixel::BLACK));
        // Click on window area to focus
        desktop.handle_mouse(60, 60, MouseButton::Left);
        assert_eq!(desktop.window_manager.focused(), Some(wid));
    }

    // ── Cursor ──

    #[test]
    fn cursor_render_arrow() {
        let mut fb = Framebuffer::new(100, 100);
        let mut cursor = Cursor::new();
        cursor.move_to(10, 10);
        cursor.render(&mut fb);
        // At least the origin pixel should be drawn
        assert_eq!(fb.get_pixel(10, 10), Some(Pixel::WHITE));
    }
}
