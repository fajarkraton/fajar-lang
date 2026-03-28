//! GPU compositor and desktop environment for FajarOS Nova v2.0.
//!
//! Provides a simulated windowing system with compositing, window management,
//! taskbar, cursor, clipboard, theming, and screen capture. All rendering
//! targets an in-memory framebuffer — no real GPU hardware is touched.
//!
//! **Note on damage tracking:** The compositor currently redraws the full screen
//! each frame. Damage region tracking (dirty rectangles) is a planned
//! optimization that would allow partial redraws by tracking which screen
//! regions have changed between frames.

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

    /// Alpha-blends `src` (self) over `dst` using the Porter-Duff "over" operator.
    ///
    /// Uses integer arithmetic with u32 intermediates to avoid overflow:
    /// ```text
    /// out_a = src_a + dst_a * (255 - src_a) / 255
    /// out_c = (src_c * src_a + dst_c * dst_a * (255 - src_a) / 255) / out_a
    /// ```
    pub fn blend(self, dst: Pixel) -> Pixel {
        if self.a == 255 {
            return self;
        }
        if self.a == 0 {
            return dst;
        }
        let sa = self.a as u32;
        let da = dst.a as u32;
        let out_a = sa + da * (255 - sa) / 255;
        if out_a == 0 {
            return Pixel {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            };
        }
        let blend = |s: u8, d: u8| -> u8 {
            ((s as u32 * sa + d as u32 * da * (255 - sa) / 255) / out_a.max(1)) as u8
        };
        Pixel {
            r: blend(self.r, dst.r),
            g: blend(self.g, dst.g),
            b: blend(self.b, dst.b),
            a: out_a as u8,
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
            let e2 = err.saturating_mul(2);
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

/// Wraps a unique `u32` window identifier assigned by the [`WindowManager`].
///
/// Each window created via [`WindowManager::create_window`] receives a
/// monotonically increasing `WindowId`. The inner value is never reused
/// within a single `WindowManager` instance.
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
    /// to the window manager (focus/raise the clicked window). When the
    /// click lands on a window's title bar, the close, minimize, and
    /// maximize buttons are checked first.
    pub fn handle_mouse(&mut self, x: i32, y: i32, _button: MouseButton) {
        let taskbar_y = self.screen_height.saturating_sub(self.taskbar.height) as i32;

        if y >= taskbar_y {
            // Click in taskbar
            if let Some(wid) = self.taskbar.click(x as u32) {
                self.window_manager.focus_window(wid);
            }
        } else if let Some(wid) = self.window_manager.find_window_at(x, y) {
            // Check if click is in the title bar area
            if let Some(window) = self.window_manager.get_window(wid) {
                let tb_y = window.y - TITLE_BAR_HEIGHT as i32;
                if y >= tb_y && y < window.y {
                    // Click is within the title bar — check decoration buttons.
                    // Button positions mirror Decoration::render layout:
                    //   close:    x = win.x + win.width - 18, width 14
                    //   minimize: x = close_x - 20,           width 14
                    //   maximize: x = min_x - 20,             width 14
                    let btn_y_top = tb_y + 4;
                    let btn_y_bot = btn_y_top + 14;
                    let close_x = window.x + window.width as i32 - 18;
                    let min_x = close_x - 20;
                    let max_x = min_x - 20;
                    let screen_w = self.screen_width;
                    let screen_h = self.screen_height;

                    if y >= btn_y_top && y < btn_y_bot {
                        if x >= close_x && x < close_x + 14 {
                            self.window_manager.close_window(wid);
                            return;
                        } else if x >= min_x && x < min_x + 14 {
                            self.window_manager.minimize(wid);
                            return;
                        } else if x >= max_x && x < max_x + 14 {
                            self.window_manager.maximize(wid, screen_w, screen_h);
                            return;
                        }
                    }
                }
            }
            self.window_manager.focus_window(wid);
        }
    }

    /// Handles a key press event, dispatching to the focused window.
    ///
    /// Returns a [`KeyEvent`] carrying the focused [`WindowId`] and the raw
    /// key code, or `None` if no window currently has focus.
    pub fn handle_key(&self, key: u8) -> Option<KeyEvent> {
        self.window_manager
            .focused()
            .map(|window| KeyEvent { window, key })
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
// BitmapFont (OS3.5)
// ═══════════════════════════════════════════════════════════════════════

/// An 8×8 monochrome bitmap font covering ASCII 0x20–0x7E.
///
/// Each glyph is stored as a `u64` where bit `(row*8 + col)` (MSB = top-left)
/// indicates whether that pixel is set.
pub struct BitmapFont {
    /// Glyph bitmaps indexed by ASCII code (only 0x20–0x7E are valid).
    glyphs: [u64; 128],
}

/// Encode an 8-row glyph from 8 `u8` rows (each row is 8 horizontal bits,
/// MSB = leftmost pixel).
const fn encode_glyph(rows: [u8; 8]) -> u64 {
    ((rows[0] as u64) << 56)
        | ((rows[1] as u64) << 48)
        | ((rows[2] as u64) << 40)
        | ((rows[3] as u64) << 32)
        | ((rows[4] as u64) << 24)
        | ((rows[5] as u64) << 16)
        | ((rows[6] as u64) << 8)
        | (rows[7] as u64)
}

impl BitmapFont {
    /// Creates a `BitmapFont` with built-in glyph data for ASCII 0x20–0x7E.
    pub fn new() -> Self {
        let mut glyphs = [0u64; 128];
        // ── space (0x20) ──────────────────────────────────────────────
        glyphs[0x20] = 0;
        // ── '!' (0x21) ────────────────────────────────────────────────
        glyphs[0x21] = encode_glyph([0x08, 0x08, 0x08, 0x08, 0x08, 0x00, 0x08, 0x00]);
        // ── '"' (0x22) ────────────────────────────────────────────────
        glyphs[0x22] = encode_glyph([0x14, 0x14, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00]);
        // ── '#' (0x23) ────────────────────────────────────────────────
        glyphs[0x23] = encode_glyph([0x14, 0x14, 0x7E, 0x14, 0x7E, 0x14, 0x14, 0x00]);
        // ── '$' (0x24) ────────────────────────────────────────────────
        glyphs[0x24] = encode_glyph([0x08, 0x3E, 0x28, 0x1C, 0x0A, 0x3C, 0x08, 0x00]);
        // ── '%' (0x25) ────────────────────────────────────────────────
        glyphs[0x25] = encode_glyph([0x22, 0x52, 0x24, 0x08, 0x12, 0x25, 0x22, 0x00]);
        // ── '&' (0x26) ────────────────────────────────────────────────
        glyphs[0x26] = encode_glyph([0x1C, 0x22, 0x14, 0x0C, 0x2A, 0x22, 0x1D, 0x00]);
        // ── ''' (0x27) ────────────────────────────────────────────────
        glyphs[0x27] = encode_glyph([0x08, 0x08, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]);
        // ── '(' (0x28) ────────────────────────────────────────────────
        glyphs[0x28] = encode_glyph([0x04, 0x08, 0x10, 0x10, 0x10, 0x08, 0x04, 0x00]);
        // ── ')' (0x29) ────────────────────────────────────────────────
        glyphs[0x29] = encode_glyph([0x10, 0x08, 0x04, 0x04, 0x04, 0x08, 0x10, 0x00]);
        // ── '*' (0x2A) ────────────────────────────────────────────────
        glyphs[0x2A] = encode_glyph([0x00, 0x08, 0x2A, 0x1C, 0x2A, 0x08, 0x00, 0x00]);
        // ── '+' (0x2B) ────────────────────────────────────────────────
        glyphs[0x2B] = encode_glyph([0x00, 0x08, 0x08, 0x3E, 0x08, 0x08, 0x00, 0x00]);
        // ── ',' (0x2C) ────────────────────────────────────────────────
        glyphs[0x2C] = encode_glyph([0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x08, 0x10]);
        // ── '-' (0x2D) ────────────────────────────────────────────────
        glyphs[0x2D] = encode_glyph([0x00, 0x00, 0x00, 0x3E, 0x00, 0x00, 0x00, 0x00]);
        // ── '.' (0x2E) ────────────────────────────────────────────────
        glyphs[0x2E] = encode_glyph([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00]);
        // ── '/' (0x2F) ────────────────────────────────────────────────
        glyphs[0x2F] = encode_glyph([0x02, 0x04, 0x04, 0x08, 0x10, 0x10, 0x20, 0x00]);
        // ── digits ────────────────────────────────────────────────────
        glyphs[0x30] = encode_glyph([0x1C, 0x22, 0x26, 0x2A, 0x32, 0x22, 0x1C, 0x00]);
        glyphs[0x31] = encode_glyph([0x08, 0x18, 0x08, 0x08, 0x08, 0x08, 0x1C, 0x00]);
        glyphs[0x32] = encode_glyph([0x1C, 0x22, 0x02, 0x0C, 0x10, 0x20, 0x3E, 0x00]);
        glyphs[0x33] = encode_glyph([0x3E, 0x02, 0x04, 0x0C, 0x02, 0x22, 0x1C, 0x00]);
        glyphs[0x34] = encode_glyph([0x04, 0x0C, 0x14, 0x24, 0x3E, 0x04, 0x04, 0x00]);
        glyphs[0x35] = encode_glyph([0x3E, 0x20, 0x3C, 0x02, 0x02, 0x22, 0x1C, 0x00]);
        glyphs[0x36] = encode_glyph([0x0C, 0x10, 0x20, 0x3C, 0x22, 0x22, 0x1C, 0x00]);
        glyphs[0x37] = encode_glyph([0x3E, 0x02, 0x04, 0x08, 0x10, 0x10, 0x10, 0x00]);
        glyphs[0x38] = encode_glyph([0x1C, 0x22, 0x22, 0x1C, 0x22, 0x22, 0x1C, 0x00]);
        glyphs[0x39] = encode_glyph([0x1C, 0x22, 0x22, 0x1E, 0x02, 0x04, 0x18, 0x00]);
        // ── ':' ';' '<' '=' '>' '?' '@' ──────────────────────────────
        glyphs[0x3A] = encode_glyph([0x00, 0x08, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00]);
        glyphs[0x3B] = encode_glyph([0x00, 0x08, 0x00, 0x00, 0x08, 0x08, 0x10, 0x00]);
        glyphs[0x3C] = encode_glyph([0x04, 0x08, 0x10, 0x20, 0x10, 0x08, 0x04, 0x00]);
        glyphs[0x3D] = encode_glyph([0x00, 0x00, 0x3E, 0x00, 0x3E, 0x00, 0x00, 0x00]);
        glyphs[0x3E] = encode_glyph([0x20, 0x10, 0x08, 0x04, 0x08, 0x10, 0x20, 0x00]);
        glyphs[0x3F] = encode_glyph([0x1C, 0x22, 0x02, 0x04, 0x08, 0x00, 0x08, 0x00]);
        glyphs[0x40] = encode_glyph([0x1C, 0x22, 0x2A, 0x2E, 0x2C, 0x20, 0x1C, 0x00]);
        // ── A-Z ───────────────────────────────────────────────────────
        glyphs[0x41] = encode_glyph([0x08, 0x14, 0x22, 0x22, 0x3E, 0x22, 0x22, 0x00]);
        glyphs[0x42] = encode_glyph([0x3C, 0x22, 0x22, 0x3C, 0x22, 0x22, 0x3C, 0x00]);
        glyphs[0x43] = encode_glyph([0x1C, 0x22, 0x20, 0x20, 0x20, 0x22, 0x1C, 0x00]);
        glyphs[0x44] = encode_glyph([0x38, 0x24, 0x22, 0x22, 0x22, 0x24, 0x38, 0x00]);
        glyphs[0x45] = encode_glyph([0x3E, 0x20, 0x20, 0x3C, 0x20, 0x20, 0x3E, 0x00]);
        glyphs[0x46] = encode_glyph([0x3E, 0x20, 0x20, 0x3C, 0x20, 0x20, 0x20, 0x00]);
        glyphs[0x47] = encode_glyph([0x1C, 0x22, 0x20, 0x2E, 0x22, 0x22, 0x1C, 0x00]);
        glyphs[0x48] = encode_glyph([0x22, 0x22, 0x22, 0x3E, 0x22, 0x22, 0x22, 0x00]);
        glyphs[0x49] = encode_glyph([0x1C, 0x08, 0x08, 0x08, 0x08, 0x08, 0x1C, 0x00]);
        glyphs[0x4A] = encode_glyph([0x0E, 0x04, 0x04, 0x04, 0x04, 0x24, 0x18, 0x00]);
        glyphs[0x4B] = encode_glyph([0x22, 0x24, 0x28, 0x30, 0x28, 0x24, 0x22, 0x00]);
        glyphs[0x4C] = encode_glyph([0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x3E, 0x00]);
        glyphs[0x4D] = encode_glyph([0x22, 0x36, 0x2A, 0x2A, 0x22, 0x22, 0x22, 0x00]);
        glyphs[0x4E] = encode_glyph([0x22, 0x32, 0x2A, 0x26, 0x22, 0x22, 0x22, 0x00]);
        glyphs[0x4F] = encode_glyph([0x1C, 0x22, 0x22, 0x22, 0x22, 0x22, 0x1C, 0x00]);
        glyphs[0x50] = encode_glyph([0x3C, 0x22, 0x22, 0x3C, 0x20, 0x20, 0x20, 0x00]);
        glyphs[0x51] = encode_glyph([0x1C, 0x22, 0x22, 0x22, 0x2A, 0x24, 0x1A, 0x00]);
        glyphs[0x52] = encode_glyph([0x3C, 0x22, 0x22, 0x3C, 0x28, 0x24, 0x22, 0x00]);
        glyphs[0x53] = encode_glyph([0x1C, 0x22, 0x20, 0x1C, 0x02, 0x22, 0x1C, 0x00]);
        glyphs[0x54] = encode_glyph([0x3E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x00]);
        glyphs[0x55] = encode_glyph([0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x1C, 0x00]);
        glyphs[0x56] = encode_glyph([0x22, 0x22, 0x22, 0x14, 0x14, 0x08, 0x08, 0x00]);
        glyphs[0x57] = encode_glyph([0x22, 0x22, 0x22, 0x2A, 0x2A, 0x36, 0x22, 0x00]);
        glyphs[0x58] = encode_glyph([0x22, 0x22, 0x14, 0x08, 0x14, 0x22, 0x22, 0x00]);
        glyphs[0x59] = encode_glyph([0x22, 0x22, 0x14, 0x08, 0x08, 0x08, 0x08, 0x00]);
        glyphs[0x5A] = encode_glyph([0x3E, 0x02, 0x04, 0x08, 0x10, 0x20, 0x3E, 0x00]);
        // ── '[' '\' ']' '^' '_' '`' ──────────────────────────────────
        glyphs[0x5B] = encode_glyph([0x1C, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1C, 0x00]);
        glyphs[0x5C] = encode_glyph([0x20, 0x10, 0x10, 0x08, 0x04, 0x04, 0x02, 0x00]);
        glyphs[0x5D] = encode_glyph([0x1C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x1C, 0x00]);
        glyphs[0x5E] = encode_glyph([0x08, 0x14, 0x22, 0x00, 0x00, 0x00, 0x00, 0x00]);
        glyphs[0x5F] = encode_glyph([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3E, 0x00]);
        glyphs[0x60] = encode_glyph([0x10, 0x08, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00]);
        // ── a-z ───────────────────────────────────────────────────────
        glyphs[0x61] = encode_glyph([0x00, 0x00, 0x1C, 0x02, 0x1E, 0x22, 0x1E, 0x00]);
        glyphs[0x62] = encode_glyph([0x20, 0x20, 0x3C, 0x22, 0x22, 0x22, 0x3C, 0x00]);
        glyphs[0x63] = encode_glyph([0x00, 0x00, 0x1C, 0x20, 0x20, 0x20, 0x1C, 0x00]);
        glyphs[0x64] = encode_glyph([0x02, 0x02, 0x1E, 0x22, 0x22, 0x22, 0x1E, 0x00]);
        glyphs[0x65] = encode_glyph([0x00, 0x00, 0x1C, 0x22, 0x3E, 0x20, 0x1C, 0x00]);
        glyphs[0x66] = encode_glyph([0x0C, 0x10, 0x3C, 0x10, 0x10, 0x10, 0x10, 0x00]);
        glyphs[0x67] = encode_glyph([0x00, 0x00, 0x1E, 0x22, 0x22, 0x1E, 0x02, 0x1C]);
        glyphs[0x68] = encode_glyph([0x20, 0x20, 0x3C, 0x22, 0x22, 0x22, 0x22, 0x00]);
        glyphs[0x69] = encode_glyph([0x08, 0x00, 0x18, 0x08, 0x08, 0x08, 0x1C, 0x00]);
        glyphs[0x6A] = encode_glyph([0x04, 0x00, 0x0C, 0x04, 0x04, 0x04, 0x24, 0x18]);
        glyphs[0x6B] = encode_glyph([0x20, 0x24, 0x28, 0x30, 0x28, 0x24, 0x22, 0x00]);
        glyphs[0x6C] = encode_glyph([0x18, 0x08, 0x08, 0x08, 0x08, 0x08, 0x1C, 0x00]);
        glyphs[0x6D] = encode_glyph([0x00, 0x00, 0x36, 0x2A, 0x2A, 0x2A, 0x22, 0x00]);
        glyphs[0x6E] = encode_glyph([0x00, 0x00, 0x3C, 0x22, 0x22, 0x22, 0x22, 0x00]);
        glyphs[0x6F] = encode_glyph([0x00, 0x00, 0x1C, 0x22, 0x22, 0x22, 0x1C, 0x00]);
        glyphs[0x70] = encode_glyph([0x00, 0x00, 0x3C, 0x22, 0x22, 0x3C, 0x20, 0x20]);
        glyphs[0x71] = encode_glyph([0x00, 0x00, 0x1E, 0x22, 0x22, 0x1E, 0x02, 0x02]);
        glyphs[0x72] = encode_glyph([0x00, 0x00, 0x2C, 0x30, 0x20, 0x20, 0x20, 0x00]);
        glyphs[0x73] = encode_glyph([0x00, 0x00, 0x1C, 0x20, 0x1C, 0x02, 0x3C, 0x00]);
        glyphs[0x74] = encode_glyph([0x10, 0x10, 0x3C, 0x10, 0x10, 0x10, 0x0C, 0x00]);
        glyphs[0x75] = encode_glyph([0x00, 0x00, 0x22, 0x22, 0x22, 0x22, 0x1E, 0x00]);
        glyphs[0x76] = encode_glyph([0x00, 0x00, 0x22, 0x22, 0x14, 0x14, 0x08, 0x00]);
        glyphs[0x77] = encode_glyph([0x00, 0x00, 0x22, 0x2A, 0x2A, 0x2A, 0x14, 0x00]);
        glyphs[0x78] = encode_glyph([0x00, 0x00, 0x22, 0x14, 0x08, 0x14, 0x22, 0x00]);
        glyphs[0x79] = encode_glyph([0x00, 0x00, 0x22, 0x22, 0x1E, 0x02, 0x1C, 0x00]);
        glyphs[0x7A] = encode_glyph([0x00, 0x00, 0x3E, 0x04, 0x08, 0x10, 0x3E, 0x00]);
        // ── '{' '|' '}' '~' ──────────────────────────────────────────
        glyphs[0x7B] = encode_glyph([0x06, 0x08, 0x08, 0x10, 0x08, 0x08, 0x06, 0x00]);
        glyphs[0x7C] = encode_glyph([0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x00]);
        glyphs[0x7D] = encode_glyph([0x30, 0x08, 0x08, 0x04, 0x08, 0x08, 0x30, 0x00]);
        glyphs[0x7E] = encode_glyph([0x00, 0x10, 0x2A, 0x04, 0x00, 0x00, 0x00, 0x00]);
        Self { glyphs }
    }

    /// Returns the 64-bit bitmap for the given ASCII character.
    ///
    /// Returns the `?` glyph for characters outside the printable ASCII range.
    pub fn glyph(&self, ch: char) -> u64 {
        let code = ch as usize;
        if (0x20..0x80).contains(&code) {
            self.glyphs[code]
        } else {
            self.glyphs[0x3F] // '?'
        }
    }

    /// Draws a single 8×8 character at `(x, y)` on the framebuffer.
    ///
    /// Pixels that are set in the glyph bitmap are written with `color`;
    /// pixels that are clear are left unchanged.
    pub fn draw_char(&self, fb: &mut Framebuffer, x: u32, y: u32, ch: char, color: Pixel) {
        let bits = self.glyph(ch);
        for row in 0u32..8 {
            let row_bits = ((bits >> (56 - row * 8)) & 0xFF) as u8;
            for col in 0u32..8 {
                if row_bits & (0x80 >> col) != 0 {
                    fb.set_pixel(x.saturating_add(col), y.saturating_add(row), color);
                }
            }
        }
    }

    /// Draws a string of text starting at `(x, y)`.
    ///
    /// Each character advances 8 pixels to the right. Newlines (`\n`) advance
    /// one row (8 pixels) and reset the column.
    pub fn draw_text(&self, fb: &mut Framebuffer, x: u32, y: u32, text: &str, color: Pixel) {
        let mut cx = x;
        let mut cy = y;
        for ch in text.chars() {
            if ch == '\n' {
                cx = x;
                cy = cy.saturating_add(8);
            } else {
                self.draw_char(fb, cx, cy, ch, color);
                cx = cx.saturating_add(8);
            }
        }
    }
}

impl Default for BitmapFont {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TerminalEmulator (OS3.11)
// ═══════════════════════════════════════════════════════════════════════

/// A single cell in the terminal grid.
#[derive(Debug, Clone, Copy)]
pub struct TermCell {
    /// Character stored in this cell.
    pub ch: char,
    /// Foreground color.
    pub fg: Pixel,
    /// Background color.
    pub bg: Pixel,
}

impl TermCell {
    fn blank(fg: Pixel, bg: Pixel) -> Self {
        Self { ch: ' ', fg, bg }
    }
}

/// A software terminal emulator with basic ANSI support.
///
/// Renders onto a [`Framebuffer`] using a [`BitmapFont`].
#[derive(Debug)]
pub struct TerminalEmulator {
    /// Grid width in columns.
    pub cols: u32,
    /// Grid height in rows.
    pub rows: u32,
    /// Current cursor column.
    pub cursor_col: u32,
    /// Current cursor row.
    pub cursor_row: u32,
    /// Cell grid (row-major).
    buffer: Vec<Vec<TermCell>>,
    /// Current foreground color.
    pub fg_color: Pixel,
    /// Current background color.
    pub bg_color: Pixel,
    /// Scroll offset (number of rows scrolled from top).
    pub scroll_offset: u32,
    /// Whether the cursor is visible.
    pub cursor_visible: bool,
}

impl TerminalEmulator {
    /// Creates a new terminal emulator with `cols` columns and `rows` rows.
    pub fn new(cols: u32, rows: u32) -> Self {
        let fg = Pixel::rgb(200, 200, 200);
        let bg = Pixel::BLACK;
        let buffer = (0..rows as usize)
            .map(|_| vec![TermCell::blank(fg, bg); cols as usize])
            .collect();
        Self {
            cols,
            rows,
            cursor_col: 0,
            cursor_row: 0,
            buffer,
            fg_color: fg,
            bg_color: bg,
            scroll_offset: 0,
            cursor_visible: true,
        }
    }

    /// Writes a single character at the current cursor position.
    ///
    /// Advances the cursor; wraps to the next row at end of line;
    /// scrolls when the cursor moves past the last row.
    pub fn write_char(&mut self, ch: char) {
        match ch {
            '\n' => self.newline(),
            '\r' => self.cursor_col = 0,
            '\t' => {
                let next_tab = (self.cursor_col / 8 + 1) * 8;
                self.cursor_col = next_tab.min(self.cols.saturating_sub(1));
            }
            _ => {
                let col = self.cursor_col as usize;
                let row = self.cursor_row as usize;
                if row < self.buffer.len() && col < self.buffer[row].len() {
                    self.buffer[row][col] = TermCell {
                        ch,
                        fg: self.fg_color,
                        bg: self.bg_color,
                    };
                }
                self.cursor_col = self.cursor_col.saturating_add(1);
                if self.cursor_col >= self.cols {
                    self.newline();
                }
            }
        }
    }

    /// Writes a string, processing each character in order.
    ///
    /// Recognizes the ANSI escape sequence prefix `\x1b[` for basic color
    /// and clear commands.
    pub fn write_str(&mut self, s: &str) {
        let mut chars = s.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                if chars.peek() == Some(&'[') {
                    chars.next(); // consume '['
                    let mut seq = String::new();
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() {
                            seq.push(c);
                            break;
                        }
                        seq.push(c);
                    }
                    self.handle_escape(&seq);
                }
            } else {
                self.write_char(ch);
            }
        }
    }

    /// Advances the cursor to the beginning of the next row.
    ///
    /// Scrolls the buffer up by one row when the bottom is reached.
    pub fn newline(&mut self) {
        self.cursor_col = 0;
        self.cursor_row = self.cursor_row.saturating_add(1);
        if self.cursor_row >= self.rows {
            self.scroll_up();
            self.cursor_row = self.rows.saturating_sub(1);
        }
    }

    /// Clears the terminal to the current background color.
    pub fn clear(&mut self) {
        let fg = self.fg_color;
        let bg = self.bg_color;
        for row in &mut self.buffer {
            for cell in row.iter_mut() {
                *cell = TermCell::blank(fg, bg);
            }
        }
        self.cursor_col = 0;
        self.cursor_row = 0;
    }

    /// Scrolls the buffer up by one row, discarding the first row and
    /// adding a blank row at the bottom.
    pub fn scroll_up(&mut self) {
        if !self.buffer.is_empty() {
            self.buffer.remove(0);
            let fg = self.fg_color;
            let bg = self.bg_color;
            self.buffer
                .push(vec![TermCell::blank(fg, bg); self.cols as usize]);
            self.scroll_offset = self.scroll_offset.saturating_add(1);
        }
    }

    /// Moves the cursor to `(col, row)`, clamped to grid bounds.
    pub fn set_cursor(&mut self, col: u32, row: u32) {
        self.cursor_col = col.min(self.cols.saturating_sub(1));
        self.cursor_row = row.min(self.rows.saturating_sub(1));
    }

    /// Handles a parsed ANSI escape sequence body (the part after `ESC [`).
    ///
    /// Supported sequences:
    /// - `2J` — clear screen
    /// - `H` or `;H` — move cursor to home (0, 0)
    /// - `<n>m` — set color (30–37 = foreground, 0 = reset)
    pub fn handle_escape(&mut self, seq: &str) {
        if seq == "2J" {
            self.clear();
        } else if seq == "H" || seq.ends_with(';') {
            self.set_cursor(0, 0);
        } else if let Some(n_str) = seq.strip_suffix('m') {
            if let Ok(n) = n_str.parse::<u8>() {
                match n {
                    0 => {
                        self.fg_color = Pixel::rgb(200, 200, 200);
                        self.bg_color = Pixel::BLACK;
                    }
                    30 => self.fg_color = Pixel::BLACK,
                    31 => self.fg_color = Pixel::rgb(200, 0, 0),
                    32 => self.fg_color = Pixel::rgb(0, 200, 0),
                    33 => self.fg_color = Pixel::rgb(200, 200, 0),
                    34 => self.fg_color = Pixel::rgb(0, 0, 200),
                    35 => self.fg_color = Pixel::rgb(200, 0, 200),
                    36 => self.fg_color = Pixel::rgb(0, 200, 200),
                    37 => self.fg_color = Pixel::WHITE,
                    _ => {}
                }
            }
        }
    }

    /// Renders the terminal grid onto the framebuffer at pixel position `(x, y)`.
    ///
    /// Each cell is rendered as an 8×8 character block.
    pub fn render(&self, fb: &mut Framebuffer, font: &BitmapFont, x: u32, y: u32) {
        for (row_idx, row) in self.buffer.iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                let px = x.saturating_add(col_idx as u32 * 8);
                let py = y.saturating_add(row_idx as u32 * 8);
                fb.fill_rect(px, py, 8, 8, cell.bg);
                font.draw_char(fb, px, py, cell.ch, cell.fg);
            }
        }
        // Cursor block
        if self.cursor_visible {
            let cx = x.saturating_add(self.cursor_col * 8);
            let cy = y.saturating_add(self.cursor_row * 8);
            fb.fill_rect(cx, cy, 8, 8, self.fg_color);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// LockScreen (OS3.27)
// ═══════════════════════════════════════════════════════════════════════

/// Computes a simple FNV-1a 64-bit hash of the given string.
fn fnv1a_hash(s: &str) -> u64 {
    const OFFSET: u64 = 14_695_981_039_346_656_037;
    const PRIME: u64 = 1_099_511_628_211;
    let mut hash = OFFSET;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// A password-protected lock screen.
///
/// Stores only the FNV-1a hash of the password, never the plaintext.
#[derive(Debug)]
pub struct LockScreen {
    /// Whether the screen is currently locked.
    locked: bool,
    /// FNV-1a hash of the configured password.
    password_hash: u64,
    /// Current typed input (not yet submitted).
    input_buffer: String,
    /// Number of consecutive failed unlock attempts.
    failed_attempts: u32,
    /// Maximum allowed attempts before lockout.
    max_attempts: u32,
    /// Timestamp (ms) after which unlock is allowed again; 0 = not locked out.
    lockout_until: u64,
    /// Status / hint message shown to the user.
    pub message: String,
}

impl LockScreen {
    /// Creates a new lock screen with the given password.
    ///
    /// The screen starts in the **locked** state.
    pub fn new(password: &str) -> Self {
        Self {
            locked: true,
            password_hash: fnv1a_hash(password),
            input_buffer: String::new(),
            failed_attempts: 0,
            max_attempts: 5,
            lockout_until: 0,
            message: String::from("Screen locked. Enter password to unlock."),
        }
    }

    /// Locks the screen.
    pub fn lock(&mut self) {
        self.locked = true;
        self.input_buffer.clear();
        self.message = String::from("Screen locked.");
    }

    /// Attempts to unlock the screen with the given password at timestamp `now` (ms).
    ///
    /// Returns `true` if the password was correct and the screen is now unlocked.
    /// During lockout, always returns `false` regardless of password.
    pub fn attempt_unlock(&mut self, password: &str, now: u64) -> bool {
        if !self.locked {
            return true;
        }
        if self.lockout_until > 0 && now < self.lockout_until {
            self.message = format!(
                "Locked out. Try again in {} seconds.",
                self.lockout_until.saturating_sub(now).div_ceil(1000)
            );
            return false;
        }
        if fnv1a_hash(password) == self.password_hash {
            self.locked = false;
            self.failed_attempts = 0;
            self.lockout_until = 0;
            self.input_buffer.clear();
            self.message = String::from("Unlocked.");
            true
        } else {
            self.failed_attempts = self.failed_attempts.saturating_add(1);
            if self.failed_attempts >= self.max_attempts {
                self.lockout_until = now.saturating_add(30_000);
                self.message = "Too many attempts. Locked out for 30 seconds.".to_string();
            } else {
                self.message = format!(
                    "Wrong password. {} attempt(s) remaining.",
                    self.max_attempts.saturating_sub(self.failed_attempts)
                );
            }
            false
        }
    }

    /// Returns `true` if the screen is currently locked.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Appends a character to the password input buffer.
    pub fn add_char(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    /// Clears the password input buffer.
    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
    }

    /// Returns the number of failed unlock attempts since the last success.
    pub fn failed_attempts(&self) -> u32 {
        self.failed_attempts
    }

    /// Renders the lock screen onto the framebuffer using the given font.
    pub fn render(&self, fb: &mut Framebuffer, font: &BitmapFont) {
        fb.fill_rect(0, 0, fb.width, fb.height, Pixel::rgb(20, 20, 40));
        let text_color = Pixel::WHITE;
        let cx = fb.width / 4;
        let cy = fb.height / 3;
        font.draw_text(fb, cx, cy, &self.message, text_color);
        // Draw masked password dots
        let dot_y = cy.saturating_add(24);
        for i in 0..self.input_buffer.len() as u32 {
            let dx = cx.saturating_add(i * 10);
            fb.fill_rect(dx, dot_y, 6, 6, Pixel::WHITE);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AppLauncher (OS3.10)
// ═══════════════════════════════════════════════════════════════════════

/// A registered application entry.
#[derive(Debug, Clone)]
pub struct AppEntry {
    /// Display name.
    pub name: String,
    /// Icon color used for placeholder rendering.
    pub icon_color: Pixel,
    /// Command string to execute.
    pub command: String,
}

/// Application launcher widget with search and keyboard navigation.
#[derive(Debug)]
pub struct AppLauncher {
    /// All registered applications.
    apps: Vec<AppEntry>,
    /// Whether the launcher is visible.
    visible: bool,
    /// Index of the currently selected (highlighted) application.
    selected: usize,
    /// Current search filter query.
    search_query: String,
}

impl AppLauncher {
    /// Creates a new, empty application launcher (initially hidden).
    pub fn new() -> Self {
        Self {
            apps: Vec::new(),
            visible: false,
            selected: 0,
            search_query: String::new(),
        }
    }

    /// Registers an application entry.
    pub fn add_app(&mut self, name: &str, icon_color: Pixel, command: &str) {
        self.apps.push(AppEntry {
            name: name.to_string(),
            icon_color,
            command: command.to_string(),
        });
    }

    /// Toggles launcher visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.search_query.clear();
            self.selected = 0;
        }
    }

    /// Returns `true` if the launcher is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Moves the selection to the next filtered result, wrapping around.
    pub fn select_next(&mut self) {
        let count = self.filtered_apps().len();
        if count > 0 {
            self.selected = (self.selected.saturating_add(1)) % count;
        }
    }

    /// Moves the selection to the previous filtered result, wrapping around.
    pub fn select_prev(&mut self) {
        let count = self.filtered_apps().len();
        if count > 0 {
            self.selected = self.selected.checked_sub(1).unwrap_or(count - 1);
        }
    }

    /// Sets the search query and resets the selection.
    pub fn search(&mut self, query: &str) {
        self.search_query = query.to_string();
        self.selected = 0;
    }

    /// Appends a character to the search query.
    pub fn append_search_char(&mut self, c: char) {
        self.search_query.push(c);
        self.selected = 0;
    }

    /// Returns the command of the currently selected application, if any.
    ///
    /// Returns `None` if the launcher is hidden or no apps match the query.
    pub fn launch_selected(&self) -> Option<String> {
        if !self.visible {
            return None;
        }
        let filtered = self.filtered_apps();
        filtered.get(self.selected).map(|e| e.command.clone())
    }

    /// Returns the apps that match the current search query (case-insensitive).
    pub fn filtered_apps(&self) -> Vec<AppEntry> {
        let q = self.search_query.to_lowercase();
        self.apps
            .iter()
            .filter(|a| q.is_empty() || a.name.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    /// Renders the launcher at `(x, y)` onto the framebuffer using the given font.
    pub fn render(&self, fb: &mut Framebuffer, font: &BitmapFont, x: u32, y: u32) {
        if !self.visible {
            return;
        }
        let bg = Pixel::rgb(30, 30, 30);
        fb.fill_rect(x, y, 200, 300, bg);
        // Search box
        font.draw_text(
            fb,
            x.saturating_add(4),
            y.saturating_add(4),
            &self.search_query,
            Pixel::WHITE,
        );
        let filtered = self.filtered_apps();
        for (i, app) in filtered.iter().enumerate() {
            let item_y = y.saturating_add(20 + i as u32 * 20);
            let row_bg = if i == self.selected {
                Pixel::rgb(60, 60, 120)
            } else {
                bg
            };
            fb.fill_rect(x, item_y, 200, 18, row_bg);
            fb.fill_rect(
                x.saturating_add(2),
                item_y.saturating_add(2),
                14,
                14,
                app.icon_color,
            );
            font.draw_text(
                fb,
                x.saturating_add(20),
                item_y.saturating_add(4),
                &app.name,
                Pixel::WHITE,
            );
        }
    }
}

impl Default for AppLauncher {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TextEditor (OS3.12)
// ═══════════════════════════════════════════════════════════════════════

/// Cursor movement directions for the text editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorDir {
    /// Move one column left.
    Left,
    /// Move one column right.
    Right,
    /// Move one row up.
    Up,
    /// Move one row down.
    Down,
}

/// A simple multi-line text editor widget.
#[derive(Debug)]
pub struct TextEditor {
    /// Lines of text.
    lines: Vec<String>,
    /// Row index of the cursor.
    cursor_line: usize,
    /// Column index (byte offset) of the cursor within the current line.
    cursor_col: usize,
    /// Number of lines scrolled (top visible line index).
    scroll_y: usize,
    /// Whether the content has been modified since last save.
    pub modified: bool,
    /// Optional associated filename.
    pub filename: Option<String>,
}

impl TextEditor {
    /// Creates a new, empty text editor.
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            scroll_y: 0,
            modified: false,
            filename: None,
        }
    }

    /// Inserts a character at the current cursor position.
    pub fn insert_char(&mut self, ch: char) {
        let line = &mut self.lines[self.cursor_line];
        let col = self.cursor_col.min(line.len());
        line.insert(col, ch);
        self.cursor_col = col.saturating_add(1);
        self.modified = true;
    }

    /// Deletes the character immediately before the cursor (backspace behaviour).
    ///
    /// If at the beginning of a line (and not the first line), the line is
    /// merged with the previous one.
    pub fn delete_char(&mut self) {
        if self.cursor_col > 0 {
            let col = self.cursor_col;
            let line = &mut self.lines[self.cursor_line];
            if col <= line.len() {
                line.remove(col - 1);
                self.cursor_col -= 1;
            }
            self.modified = true;
        } else if self.cursor_line > 0 {
            let current = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_len = self.lines[self.cursor_line].len();
            self.lines[self.cursor_line].push_str(&current);
            self.cursor_col = prev_len;
            self.modified = true;
        }
    }

    /// Inserts a newline at the cursor, splitting the current line.
    pub fn newline(&mut self) {
        let col = self.cursor_col.min(self.lines[self.cursor_line].len());
        let rest = self.lines[self.cursor_line].split_off(col);
        self.cursor_line += 1;
        self.lines.insert(self.cursor_line, rest);
        self.cursor_col = 0;
        self.modified = true;
    }

    /// Moves the cursor one step in the given direction.
    pub fn move_cursor(&mut self, dir: CursorDir) {
        match dir {
            CursorDir::Left => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                } else if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    self.cursor_col = self.lines[self.cursor_line].len();
                }
            }
            CursorDir::Right => {
                let len = self.lines[self.cursor_line].len();
                if self.cursor_col < len {
                    self.cursor_col += 1;
                } else if self.cursor_line + 1 < self.lines.len() {
                    self.cursor_line += 1;
                    self.cursor_col = 0;
                }
            }
            CursorDir::Up => {
                if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
                }
            }
            CursorDir::Down => {
                if self.cursor_line + 1 < self.lines.len() {
                    self.cursor_line += 1;
                    self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
                }
            }
        }
    }

    /// Returns the total number of lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns the content of the current cursor line.
    pub fn current_line(&self) -> &str {
        &self.lines[self.cursor_line]
    }

    /// Replaces all text with the given content.
    ///
    /// Splits on `\n` to populate the line buffer.
    pub fn set_text(&mut self, text: &str) {
        self.lines = text.split('\n').map(|l| l.to_string()).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.scroll_y = 0;
        self.modified = false;
    }

    /// Returns the full text content joined with `\n`.
    pub fn get_text(&self) -> String {
        self.lines.join("\n")
    }

    /// Renders the visible portion of the editor at `(x, y)` with dimensions `(w, h)`.
    ///
    /// Renders up to `h / 8` lines of text using the given font.
    pub fn render(
        &mut self,
        fb: &mut Framebuffer,
        font: &BitmapFont,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) {
        fb.fill_rect(x, y, w, h, Pixel::rgb(20, 20, 20));
        let visible_rows = (h / 8) as usize;
        // Scroll so cursor is visible
        if self.cursor_line < self.scroll_y {
            self.scroll_y = self.cursor_line;
        } else if self.cursor_line >= self.scroll_y + visible_rows {
            self.scroll_y = self.cursor_line.saturating_sub(visible_rows - 1);
        }
        for (i, line) in self
            .lines
            .iter()
            .skip(self.scroll_y)
            .take(visible_rows)
            .enumerate()
        {
            let ly = y.saturating_add(i as u32 * 8);
            font.draw_text(fb, x.saturating_add(4), ly, line, Pixel::rgb(220, 220, 220));
        }
        // Cursor bar
        let cur_screen_row = self.cursor_line.saturating_sub(self.scroll_y);
        let cur_px = x.saturating_add(4 + self.cursor_col as u32 * 8);
        let cur_py = y.saturating_add(cur_screen_row as u32 * 8);
        fb.fill_rect(cur_px, cur_py, 2, 8, Pixel::WHITE);
    }
}

impl Default for TextEditor {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FileManager (OS3.13)
// ═══════════════════════════════════════════════════════════════════════

/// A directory entry displayed by the file manager.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// File or directory name.
    pub name: String,
    /// `true` if this entry is a directory.
    pub is_dir: bool,
    /// File size in bytes (`0` for directories).
    pub size: u64,
    /// Last-modified timestamp (Unix seconds).
    pub modified: u64,
}

/// A file manager widget for browsing directory listings.
#[derive(Debug)]
pub struct FileManager {
    /// Currently displayed path string.
    current_path: String,
    /// Entries in the current directory.
    entries: Vec<FileEntry>,
    /// Index of the currently selected entry.
    selected: usize,
    /// Number of entries scrolled from the top.
    scroll_offset: usize,
}

impl FileManager {
    /// Creates a new file manager starting at the given path with no entries.
    pub fn new(path: &str) -> Self {
        Self {
            current_path: path.to_string(),
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Replaces the current directory listing with the given entries.
    pub fn set_entries(&mut self, entries: Vec<FileEntry>) {
        self.entries = entries;
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Navigates into the selected directory entry.
    ///
    /// Appends the directory name to the current path.
    /// Does nothing if the selected entry is not a directory.
    pub fn navigate_into(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            if entry.is_dir {
                if self.current_path.ends_with('/') {
                    self.current_path.push_str(&entry.name);
                } else {
                    self.current_path.push('/');
                    self.current_path.push_str(&entry.name);
                }
                self.entries.clear();
                self.selected = 0;
                self.scroll_offset = 0;
            }
        }
    }

    /// Navigates to the parent directory.
    ///
    /// Removes the last path component from `current_path`.
    pub fn navigate_up(&mut self) {
        if let Some(idx) = self.current_path.rfind('/') {
            if idx == 0 {
                self.current_path = "/".to_string();
            } else {
                self.current_path.truncate(idx);
            }
        }
        self.entries.clear();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Returns the currently selected entry, if any.
    pub fn selected_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.selected)
    }

    /// Returns the current path string.
    pub fn current_path(&self) -> &str {
        &self.current_path
    }

    /// Sorts the entry list by name (case-insensitive, directories first).
    pub fn sort_by_name(&mut self) {
        self.entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }

    /// Sorts the entry list by size (largest first; directories sort before files).
    pub fn sort_by_size(&mut self) {
        self.entries
            .sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(b.size.cmp(&a.size)));
    }

    /// Moves the selection down by one entry.
    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected.saturating_add(1)).min(self.entries.len() - 1);
        }
    }

    /// Moves the selection up by one entry.
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Renders the file manager at `(x, y)` with dimensions `(w, h)`.
    pub fn render(&self, fb: &mut Framebuffer, font: &BitmapFont, x: u32, y: u32, w: u32, h: u32) {
        fb.fill_rect(x, y, w, h, Pixel::rgb(25, 25, 35));
        // Path bar
        font.draw_text(
            fb,
            x.saturating_add(4),
            y.saturating_add(2),
            &self.current_path,
            Pixel::rgb(180, 180, 255),
        );
        let visible = ((h.saturating_sub(14)) / 10) as usize;
        for (i, entry) in self
            .entries
            .iter()
            .skip(self.scroll_offset)
            .take(visible)
            .enumerate()
        {
            let ey = y.saturating_add(14 + i as u32 * 10);
            let row_bg = if self.scroll_offset + i == self.selected {
                Pixel::rgb(50, 50, 100)
            } else {
                Pixel::rgb(25, 25, 35)
            };
            fb.fill_rect(x, ey, w, 9, row_bg);
            let icon = if entry.is_dir {
                Pixel::rgb(255, 200, 50)
            } else {
                Pixel::rgb(150, 200, 255)
            };
            fb.fill_rect(x.saturating_add(2), ey.saturating_add(1), 6, 7, icon);
            font.draw_text(fb, x.saturating_add(12), ey, &entry.name, Pixel::WHITE);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ImageWidget (OS3.15)
// ═══════════════════════════════════════════════════════════════════════

/// An image viewer widget supporting zoom and pan.
#[derive(Debug)]
pub struct ImageWidget {
    /// Image width in pixels.
    width: u32,
    /// Image height in pixels.
    height: u32,
    /// Raw pixel data (length = width * height).
    pixels: Vec<Pixel>,
    /// Zoom factor (1.0 = 100%).
    pub zoom: f32,
    /// Horizontal pan offset in image pixels.
    pub offset_x: i32,
    /// Vertical pan offset in image pixels.
    pub offset_y: i32,
}

impl ImageWidget {
    /// Creates a new black image of the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            pixels: vec![Pixel::BLACK; size],
            zoom: 1.0,
            offset_x: 0,
            offset_y: 0,
        }
    }

    /// Creates an `ImageWidget` from raw RGBA byte data.
    ///
    /// `data` must have length `width * height * 4`.  Extra bytes are ignored;
    /// missing bytes produce black pixels.
    pub fn from_rgba(data: &[u8], width: u32, height: u32) -> Self {
        let count = (width as usize).saturating_mul(height as usize);
        let mut pixels = Vec::with_capacity(count);
        for i in 0..count {
            let base = i * 4;
            let r = data.get(base).copied().unwrap_or(0);
            let g = data.get(base + 1).copied().unwrap_or(0);
            let b = data.get(base + 2).copied().unwrap_or(0);
            let a = data.get(base + 3).copied().unwrap_or(255);
            pixels.push(Pixel::rgba(r, g, b, a));
        }
        Self {
            width,
            height,
            pixels,
            zoom: 1.0,
            offset_x: 0,
            offset_y: 0,
        }
    }

    /// Sets a pixel at `(x, y)`. No-op if out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Pixel) {
        if x < self.width && y < self.height {
            let idx = y as usize * self.width as usize + x as usize;
            self.pixels[idx] = color;
        }
    }

    /// Returns the pixel at `(x, y)`, or `None` if out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Pixel> {
        if x < self.width && y < self.height {
            let idx = y as usize * self.width as usize + x as usize;
            Some(self.pixels[idx])
        } else {
            None
        }
    }

    /// Increases zoom by 25%, capped at 8×.
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).min(8.0);
    }

    /// Decreases zoom by 20%, capped at 0.125×.
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom * 0.8).max(0.125);
    }

    /// Pans the view by `(dx, dy)` image pixels.
    pub fn pan(&mut self, dx: i32, dy: i32) {
        self.offset_x = self.offset_x.saturating_add(dx);
        self.offset_y = self.offset_y.saturating_add(dy);
    }

    /// Renders the image (with current zoom and pan) onto the framebuffer at `(x, y)`.
    ///
    /// Uses nearest-neighbour scaling.
    pub fn render(&self, fb: &mut Framebuffer, x: u32, y: u32) {
        let zoom = if self.zoom <= 0.0 { 1.0 } else { self.zoom };
        let dst_w = (self.width as f32 * zoom) as u32;
        let dst_h = (self.height as f32 * zoom) as u32;
        for dy in 0..dst_h {
            for dx in 0..dst_w {
                let src_x = ((dx as f32 / zoom) as i32).saturating_add(self.offset_x);
                let src_y = ((dy as f32 / zoom) as i32).saturating_add(self.offset_y);
                if src_x >= 0 && src_y >= 0 {
                    if let Some(color) = self.get_pixel(src_x as u32, src_y as u32) {
                        fb.set_pixel(x.saturating_add(dx), y.saturating_add(dy), color);
                    }
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Calculator (OS3.16)
// ═══════════════════════════════════════════════════════════════════════

/// Arithmetic operations for the calculator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CalcOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
}

/// A simple four-function calculator widget.
#[derive(Debug)]
pub struct Calculator {
    /// Current display string.
    display: String,
    /// Accumulated value from the previous operation.
    accumulator: f64,
    /// Pending binary operation.
    pending_op: Option<CalcOp>,
    /// `true` when the next digit should start a new number.
    new_input: bool,
}

impl Calculator {
    /// Creates a new calculator with a zeroed display.
    pub fn new() -> Self {
        Self {
            display: String::from("0"),
            accumulator: 0.0,
            pending_op: None,
            new_input: true,
        }
    }

    /// Appends a digit `n` (0–9) to the display.
    pub fn digit(&mut self, n: u8) {
        let n = n.min(9);
        if self.new_input {
            self.display = n.to_string();
            self.new_input = false;
        } else if self.display.len() < 15 {
            if self.display == "0" {
                self.display = n.to_string();
            } else {
                self.display
                    .push(char::from_digit(n as u32, 10).unwrap_or('0'));
            }
        }
    }

    /// Appends a decimal point to the display, if none is already present.
    pub fn decimal(&mut self) {
        if self.new_input {
            self.display = String::from("0.");
            self.new_input = false;
        } else if !self.display.contains('.') {
            self.display.push('.');
        }
    }

    /// Records the given operation and saves the current display value as the accumulator.
    pub fn op(&mut self, op: CalcOp) {
        self.accumulator = self.display.parse().unwrap_or(0.0);
        self.pending_op = Some(op);
        self.new_input = true;
    }

    /// Computes the result of the pending operation and updates the display.
    ///
    /// Division by zero produces `"Error"`.
    pub fn equals(&mut self) {
        let rhs: f64 = self.display.parse().unwrap_or(0.0);
        let result = match self.pending_op {
            Some(CalcOp::Add) => self.accumulator + rhs,
            Some(CalcOp::Sub) => self.accumulator - rhs,
            Some(CalcOp::Mul) => self.accumulator * rhs,
            Some(CalcOp::Div) => {
                if rhs == 0.0 {
                    self.display = String::from("Error");
                    self.pending_op = None;
                    self.new_input = true;
                    return;
                }
                self.accumulator / rhs
            }
            None => rhs,
        };
        // Format: drop trailing ".0" for integer results
        if result.fract() == 0.0 && result.abs() < 1e14 {
            self.display = format!("{}", result as i64);
        } else {
            self.display = format!("{:.6}", result);
            // Trim trailing zeros after decimal
            if self.display.contains('.') {
                let trimmed = self.display.trim_end_matches('0');
                let trimmed = trimmed.trim_end_matches('.');
                self.display = trimmed.to_string();
            }
        }
        self.pending_op = None;
        self.new_input = true;
    }

    /// Resets the calculator to its initial state.
    pub fn clear(&mut self) {
        self.display = String::from("0");
        self.accumulator = 0.0;
        self.pending_op = None;
        self.new_input = true;
    }

    /// Returns the current display text.
    pub fn display_text(&self) -> &str {
        &self.display
    }
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DragState (OS3.20)
// ═══════════════════════════════════════════════════════════════════════

/// Tracks an in-progress window drag operation.
#[derive(Debug)]
pub struct DragState {
    /// Whether a drag is currently active.
    active: bool,
    /// The window being dragged.
    target: Option<WindowId>,
    /// Screen X where the drag started.
    start_x: i32,
    /// Screen Y where the drag started.
    start_y: i32,
    /// Current mouse X.
    current_x: i32,
    /// Current mouse Y.
    current_y: i32,
    /// X offset from window top-left to drag start point.
    offset_x: i32,
    /// Y offset from window top-left to drag start point.
    offset_y: i32,
}

impl DragState {
    /// Creates an idle `DragState`.
    pub fn new() -> Self {
        Self {
            active: false,
            target: None,
            start_x: 0,
            start_y: 0,
            current_x: 0,
            current_y: 0,
            offset_x: 0,
            offset_y: 0,
        }
    }

    /// Begins a drag of `window` from screen position `(x, y)`.
    ///
    /// `win_x` / `win_y` are the current top-left position of the window;
    /// the offset is computed so the window does not jump on first movement.
    pub fn begin(&mut self, window: WindowId, x: i32, y: i32, win_x: i32, win_y: i32) {
        self.active = true;
        self.target = Some(window);
        self.start_x = x;
        self.start_y = y;
        self.current_x = x;
        self.current_y = y;
        self.offset_x = x.saturating_sub(win_x);
        self.offset_y = y.saturating_sub(win_y);
    }

    /// Updates the current drag position.
    pub fn update(&mut self, x: i32, y: i32) {
        self.current_x = x;
        self.current_y = y;
    }

    /// Ends the drag and returns `Some((window_id, new_x, new_y))` if active.
    ///
    /// The returned position is where the window's top-left should be placed.
    pub fn end(&mut self) -> Option<(WindowId, i32, i32)> {
        if self.active {
            let wid = self.target?;
            let new_x = self.current_x.saturating_sub(self.offset_x);
            let new_y = self.current_y.saturating_sub(self.offset_y);
            self.active = false;
            self.target = None;
            Some((wid, new_x, new_y))
        } else {
            None
        }
    }

    /// Returns `true` if a drag is currently in progress.
    pub fn is_dragging(&self) -> bool {
        self.active
    }
}

impl Default for DragState {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DamageTracker (OS3.22)
// ═══════════════════════════════════════════════════════════════════════

/// A rectangle that marks a dirty (damaged) region of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRect {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
}

impl DamageRect {
    /// Returns `true` if this rect overlaps with `other`.
    pub fn overlaps(&self, other: &DamageRect) -> bool {
        self.x < other.x.saturating_add(other.w)
            && self.x.saturating_add(self.w) > other.x
            && self.y < other.y.saturating_add(other.h)
            && self.y.saturating_add(self.h) > other.y
    }

    /// Returns the axis-aligned bounding box that encloses both rects.
    pub fn union(&self, other: &DamageRect) -> DamageRect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let x2 = self
            .x
            .saturating_add(self.w)
            .max(other.x.saturating_add(other.w));
        let y2 = self
            .y
            .saturating_add(self.h)
            .max(other.y.saturating_add(other.h));
        DamageRect {
            x,
            y,
            w: x2.saturating_sub(x),
            h: y2.saturating_sub(y),
        }
    }
}

/// Tracks dirty screen regions to enable incremental redraws.
#[derive(Debug, Default)]
pub struct DamageTracker {
    /// Pending dirty rectangles.
    rects: Vec<DamageRect>,
    /// When `true`, the entire screen needs a redraw.
    full_redraw: bool,
}

impl DamageTracker {
    /// Creates a new, clean `DamageTracker`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a damage rectangle.
    pub fn add(&mut self, rect: DamageRect) {
        if !self.full_redraw {
            self.rects.push(rect);
        }
    }

    /// Marks the window's bounding rectangle as damaged.
    pub fn add_window_damage(&mut self, window: &Window) {
        let x = window.x.max(0) as u32;
        let y = (window.y - TITLE_BAR_HEIGHT as i32).max(0) as u32;
        self.add(DamageRect {
            x,
            y,
            w: window.width,
            h: window.height.saturating_add(TITLE_BAR_HEIGHT),
        });
    }

    /// Marks the entire screen as requiring a full redraw.
    pub fn mark_full_redraw(&mut self) {
        self.full_redraw = true;
        self.rects.clear();
    }

    /// Drains all pending damage rectangles and resets the tracker.
    ///
    /// If `full_redraw` was set, returns a single max-extent rect
    /// (`0, 0, u32::MAX, u32::MAX`).
    pub fn drain(&mut self) -> Vec<DamageRect> {
        if self.full_redraw {
            self.full_redraw = false;
            self.rects.clear();
            return vec![DamageRect {
                x: 0,
                y: 0,
                w: u32::MAX,
                h: u32::MAX,
            }];
        }
        self.merge_overlapping();
        let out = self.rects.clone();
        self.rects.clear();
        out
    }

    /// Returns `true` if there are pending damage regions or a full redraw is needed.
    pub fn needs_redraw(&self) -> bool {
        self.full_redraw || !self.rects.is_empty()
    }

    /// Merges overlapping rectangles into their union until no overlaps remain.
    pub fn merge_overlapping(&mut self) {
        let mut changed = true;
        while changed {
            changed = false;
            let mut merged: Vec<DamageRect> = Vec::new();
            'outer: for r in self.rects.drain(..) {
                for m in &mut merged {
                    if m.overlaps(&r) {
                        *m = m.union(&r);
                        changed = true;
                        continue 'outer;
                    }
                }
                merged.push(r);
            }
            self.rects = merged;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// MultiMonitor (OS3.23)
// ═══════════════════════════════════════════════════════════════════════

/// Describes a single physical or virtual monitor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// Unique monitor identifier.
    pub id: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Horizontal offset in the global desktop coordinate space.
    pub x_offset: u32,
    /// Vertical offset in the global desktop coordinate space.
    pub y_offset: u32,
    /// Whether this is the primary monitor.
    pub primary: bool,
    /// Human-readable name.
    pub name: String,
}

/// Multi-monitor manager: holds one framebuffer per monitor.
#[derive(Debug)]
pub struct MultiMonitor {
    /// Monitor descriptors (parallel with `framebuffers`).
    monitors: Vec<MonitorInfo>,
    /// One framebuffer per monitor.
    framebuffers: Vec<Framebuffer>,
}

impl MultiMonitor {
    /// Creates a new, empty multi-monitor manager.
    pub fn new() -> Self {
        Self {
            monitors: Vec::new(),
            framebuffers: Vec::new(),
        }
    }

    /// Adds a monitor and allocates its framebuffer.
    pub fn add_monitor(&mut self, info: MonitorInfo) {
        let fb = Framebuffer::new(info.width, info.height);
        self.monitors.push(info);
        self.framebuffers.push(fb);
    }

    /// Removes a monitor by ID, also releasing its framebuffer.
    ///
    /// Does nothing if the ID is not found.
    pub fn remove_monitor(&mut self, id: u32) {
        if let Some(pos) = self.monitors.iter().position(|m| m.id == id) {
            self.monitors.remove(pos);
            self.framebuffers.remove(pos);
        }
    }

    /// Returns a reference to the primary monitor, if one is registered.
    pub fn primary(&self) -> Option<&MonitorInfo> {
        self.monitors.iter().find(|m| m.primary)
    }

    /// Returns the monitor that contains global coordinates `(x, y)`, if any.
    pub fn monitor_at(&self, x: u32, y: u32) -> Option<&MonitorInfo> {
        self.monitors.iter().find(|m| {
            x >= m.x_offset
                && x < m.x_offset.saturating_add(m.width)
                && y >= m.y_offset
                && y < m.y_offset.saturating_add(m.height)
        })
    }

    /// Returns the total bounding box of all monitors as `(x, y, width, height)`.
    pub fn total_bounds(&self) -> (u32, u32, u32, u32) {
        if self.monitors.is_empty() {
            return (0, 0, 0, 0);
        }
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        for m in &self.monitors {
            max_x = max_x.max(m.x_offset.saturating_add(m.width));
            max_y = max_y.max(m.y_offset.saturating_add(m.height));
        }
        (0, 0, max_x, max_y)
    }

    /// Returns a mutable reference to the framebuffer for the given monitor ID.
    pub fn get_framebuffer(&mut self, id: u32) -> Option<&mut Framebuffer> {
        let pos = self.monitors.iter().position(|m| m.id == id)?;
        self.framebuffers.get_mut(pos)
    }

    /// Returns the number of registered monitors.
    pub fn monitor_count(&self) -> usize {
        self.monitors.len()
    }
}

impl Default for MultiMonitor {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Desktop extensions (OS3.18, OS3.24)
// ═══════════════════════════════════════════════════════════════════════

/// A key-press event carrying the receiving window ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// The window that should receive the key.
    pub window: WindowId,
    /// Raw key code.
    pub key: u8,
}

impl Desktop {
    /// Changes the virtual screen resolution.
    ///
    /// Updates `screen_width` and `screen_height`. Callers are responsible
    /// for re-creating any dependent framebuffers.
    pub fn set_resolution(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
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

    // ── BitmapFont ──

    #[test]
    fn font_draw_char_sets_pixels() {
        let font = BitmapFont::new();
        let mut fb = Framebuffer::new(16, 16);
        let red = Pixel::rgb(255, 0, 0);
        font.draw_char(&mut fb, 0, 0, 'A', red);
        // 'A' has pixels — at least one should be red
        let has_red = (0..8).any(|x| (0..8).any(|y| fb.get_pixel(x, y) == Some(red)));
        assert!(has_red, "'A' glyph should set at least one pixel");
    }

    #[test]
    fn font_draw_text_multiple_chars() {
        let font = BitmapFont::new();
        let mut fb = Framebuffer::new(64, 16);
        let white = Pixel::WHITE;
        font.draw_text(&mut fb, 0, 0, "Hi", white);
        // 'H' starts at x=0, 'i' starts at x=8 — both should have pixels
        let h_pixels = (0..8).any(|x| (0..8).any(|y| fb.get_pixel(x, y) == Some(white)));
        let i_pixels = (8..16).any(|x| (0..8).any(|y| fb.get_pixel(x, y) == Some(white)));
        assert!(h_pixels, "'H' should have drawn pixels");
        assert!(i_pixels, "'i' should have drawn pixels");
    }

    // ── TerminalEmulator ──

    #[test]
    fn terminal_write_and_scroll() {
        let mut term = TerminalEmulator::new(10, 3);
        // Fill all rows
        for _ in 0..3 {
            for _ in 0..10 {
                term.write_char('A');
            }
        }
        // Writing past the last row should have scrolled
        term.write_char('B');
        // Cursor should still be within bounds
        assert!(term.cursor_row < term.rows);
        // The scroll offset should have advanced
        assert!(term.scroll_offset > 0);
    }

    #[test]
    fn terminal_ansi_clear() {
        let mut term = TerminalEmulator::new(10, 4);
        term.write_str("Hello");
        term.write_str("\x1b[2J");
        // After clear, first cell should be blank
        let font = BitmapFont::new();
        let mut fb = Framebuffer::new(100, 50);
        term.render(&mut fb, &font, 0, 0);
        // Cursor should be at 0,0 after clear
        assert_eq!(term.cursor_col, 0);
        assert_eq!(term.cursor_row, 0);
    }

    // ── LockScreen ──

    #[test]
    fn lock_screen_correct_password() {
        let mut ls = LockScreen::new("secret");
        assert!(ls.is_locked());
        let ok = ls.attempt_unlock("secret", 1000);
        assert!(ok);
        assert!(!ls.is_locked());
    }

    #[test]
    fn lock_screen_wrong_password() {
        let mut ls = LockScreen::new("secret");
        let ok = ls.attempt_unlock("wrong", 1000);
        assert!(!ok);
        assert!(ls.is_locked());
        assert_eq!(ls.failed_attempts(), 1);
    }

    #[test]
    fn lock_screen_lockout() {
        let mut ls = LockScreen::new("secret");
        // Use up all attempts
        for _ in 0..5 {
            ls.attempt_unlock("wrong", 1000);
        }
        assert_eq!(ls.failed_attempts(), 5);
        // Even correct password should fail during lockout
        let ok = ls.attempt_unlock("secret", 2000);
        assert!(!ok);
        assert!(ls.is_locked());
    }

    // ── AppLauncher ──

    #[test]
    fn app_launcher_search() {
        let mut launcher = AppLauncher::new();
        launcher.add_app("Terminal", Pixel::rgb(0, 200, 0), "term");
        launcher.add_app("Files", Pixel::rgb(200, 200, 0), "fm");
        launcher.add_app("Text Editor", Pixel::rgb(0, 100, 200), "edit");
        launcher.toggle(); // make visible
        launcher.search("term");
        let filtered = launcher.filtered_apps();
        assert_eq!(filtered.len(), 1); // only "Terminal" contains "term"
        assert_eq!(filtered[0].name, "Terminal");
        launcher.search("t");
        let filtered2 = launcher.filtered_apps();
        assert_eq!(filtered2.len(), 2); // "Terminal" and "Text Editor"
        launcher.search("file");
        let filtered3 = launcher.filtered_apps();
        assert_eq!(filtered3.len(), 1);
        assert_eq!(filtered3[0].name, "Files");
    }

    // ── TextEditor ──

    #[test]
    fn text_editor_insert_delete() {
        let mut ed = TextEditor::new();
        ed.insert_char('H');
        ed.insert_char('i');
        assert_eq!(ed.current_line(), "Hi");
        ed.delete_char();
        assert_eq!(ed.current_line(), "H");
        assert!(ed.modified);
    }

    #[test]
    fn text_editor_multiline() {
        let mut ed = TextEditor::new();
        ed.insert_char('A');
        ed.newline();
        ed.insert_char('B');
        assert_eq!(ed.line_count(), 2);
        assert_eq!(ed.current_line(), "B");
        // Navigate up
        ed.move_cursor(CursorDir::Up);
        assert_eq!(ed.current_line(), "A");
    }

    // ── FileManager ──

    #[test]
    fn file_manager_navigation() {
        let mut fm = FileManager::new("/home");
        fm.set_entries(vec![
            FileEntry {
                name: "docs".to_string(),
                is_dir: true,
                size: 0,
                modified: 0,
            },
            FileEntry {
                name: "readme.txt".to_string(),
                is_dir: false,
                size: 42,
                modified: 0,
            },
        ]);
        assert_eq!(fm.selected_entry().unwrap().name, "docs");
        fm.navigate_into();
        assert_eq!(fm.current_path(), "/home/docs");
        fm.navigate_up();
        assert_eq!(fm.current_path(), "/home");
    }

    // ── ImageWidget ──

    #[test]
    fn image_widget_from_rgba() {
        // 2×2 image: top-left red, others black
        let data: Vec<u8> = vec![
            255, 0, 0, 255, // red
            0, 255, 0, 255, // green
            0, 0, 255, 255, // blue
            255, 255, 0, 255, // yellow
        ];
        let img = ImageWidget::from_rgba(&data, 2, 2);
        assert_eq!(img.get_pixel(0, 0), Some(Pixel::rgb(255, 0, 0)));
        assert_eq!(img.get_pixel(1, 0), Some(Pixel::rgb(0, 255, 0)));
        assert_eq!(img.get_pixel(0, 1), Some(Pixel::rgb(0, 0, 255)));
        assert_eq!(img.get_pixel(1, 1), Some(Pixel::rgb(255, 255, 0)));
    }

    // ── Calculator ──

    #[test]
    fn calculator_basic_ops() {
        let mut calc = Calculator::new();
        // 3 + 4 = 7
        calc.digit(3);
        calc.op(CalcOp::Add);
        calc.digit(4);
        calc.equals();
        assert_eq!(calc.display_text(), "7");

        // 10 - 3 = 7
        calc.clear();
        calc.digit(1);
        calc.digit(0);
        calc.op(CalcOp::Sub);
        calc.digit(3);
        calc.equals();
        assert_eq!(calc.display_text(), "7");

        // 6 * 7 = 42
        calc.clear();
        calc.digit(6);
        calc.op(CalcOp::Mul);
        calc.digit(7);
        calc.equals();
        assert_eq!(calc.display_text(), "42");

        // 8 / 4 = 2
        calc.clear();
        calc.digit(8);
        calc.op(CalcOp::Div);
        calc.digit(4);
        calc.equals();
        assert_eq!(calc.display_text(), "2");

        // Division by zero → Error
        calc.clear();
        calc.digit(5);
        calc.op(CalcOp::Div);
        calc.digit(0);
        calc.equals();
        assert_eq!(calc.display_text(), "Error");
    }

    // ── DragState ──

    #[test]
    fn drag_state_lifecycle() {
        let mut drag = DragState::new();
        assert!(!drag.is_dragging());
        let wid = WindowId(42);
        // Window is at (100, 50); user grabs at (110, 60) → offset (10, 10)
        drag.begin(wid, 110, 60, 100, 50);
        assert!(drag.is_dragging());
        drag.update(130, 80);
        let result = drag.end();
        assert!(result.is_some());
        let (id, nx, ny) = result.unwrap();
        assert_eq!(id, wid);
        // new_x = 130 - 10 = 120, new_y = 80 - 10 = 70
        assert_eq!(nx, 120);
        assert_eq!(ny, 70);
        assert!(!drag.is_dragging());
    }

    // ── DamageTracker ──

    #[test]
    fn damage_tracker_merge() {
        let mut dt = DamageTracker::new();
        dt.add(DamageRect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        });
        dt.add(DamageRect {
            x: 5,
            y: 5,
            w: 10,
            h: 10,
        }); // overlaps first
        dt.add(DamageRect {
            x: 100,
            y: 100,
            w: 5,
            h: 5,
        }); // isolated
        let drained = dt.drain();
        // Overlapping rects should merge → 2 rects total
        assert_eq!(drained.len(), 2);
        assert!(!dt.needs_redraw());
    }

    // ── MultiMonitor ──

    #[test]
    fn multi_monitor_layout() {
        let mut mm = MultiMonitor::new();
        mm.add_monitor(MonitorInfo {
            id: 1,
            width: 1920,
            height: 1080,
            x_offset: 0,
            y_offset: 0,
            primary: true,
            name: "DP-1".to_string(),
        });
        mm.add_monitor(MonitorInfo {
            id: 2,
            width: 1920,
            height: 1080,
            x_offset: 1920,
            y_offset: 0,
            primary: false,
            name: "DP-2".to_string(),
        });
        assert_eq!(mm.monitor_count(), 2);
        let primary = mm.primary().unwrap();
        assert_eq!(primary.id, 1);
        // Point in second monitor
        let at = mm.monitor_at(2000, 500);
        assert!(at.is_some());
        assert_eq!(at.unwrap().id, 2);
        // Total bounds = 3840 × 1080
        let (_, _, w, h) = mm.total_bounds();
        assert_eq!(w, 3840);
        assert_eq!(h, 1080);
        // Framebuffer access
        assert!(mm.get_framebuffer(1).is_some());
        mm.remove_monitor(2);
        assert_eq!(mm.monitor_count(), 1);
    }

    // ── Desktop extensions ──

    #[test]
    fn desktop_resolution_switch() {
        let mut desktop = Desktop::new(1920, 1080);
        assert_eq!(desktop.screen_width, 1920);
        assert_eq!(desktop.screen_height, 1080);
        desktop.set_resolution(2560, 1440);
        assert_eq!(desktop.screen_width, 2560);
        assert_eq!(desktop.screen_height, 1440);
    }

    #[test]
    fn desktop_handle_key_dispatches() {
        let mut desktop = Desktop::new(800, 600);
        let wid = desktop
            .window_manager
            .create_window("Term", 50, 50, 200, 150);
        let ev = desktop.handle_key(b'a');
        assert!(ev.is_some());
        let ev = ev.unwrap();
        assert_eq!(ev.window, wid);
        assert_eq!(ev.key, b'a');
    }
}
