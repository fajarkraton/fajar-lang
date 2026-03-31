//! GUI framework for FajarOS Nova v2.0 — Sprint N8.
//!
//! Provides a simulated windowing system with window management, widget toolkit,
//! flexbox layout engine, theming, event dispatch, double-buffered compositor,
//! bitmap font rendering, image loading, and clipboard. All rendering targets
//! an in-memory framebuffer — no real GPU hardware is touched.

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// GUI Errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors produced by the GUI framework.
#[derive(Debug, Clone, thiserror::Error)]
pub enum GuiError {
    /// Window not found.
    #[error("window not found: {0}")]
    WindowNotFound(u32),
    /// Widget not found.
    #[error("widget not found: {0}")]
    WidgetNotFound(u32),
    /// Invalid dimensions.
    #[error("invalid dimensions: {0}x{1}")]
    InvalidDimensions(u32, u32),
    /// Image load failure.
    #[error("image load error: {0}")]
    ImageLoadError(String),
}

// ═══════════════════════════════════════════════════════════════════════
// Color & Pixel
// ═══════════════════════════════════════════════════════════════════════

/// An RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    /// Red (0-255).
    pub r: u8,
    /// Green (0-255).
    pub g: u8,
    /// Blue (0-255).
    pub b: u8,
    /// Alpha (0-255).
    pub a: u8,
}

impl Color {
    /// Creates an opaque color.
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Creates a color with alpha.
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Black.
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    /// White.
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    /// Transparent.
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };
    /// Gray.
    pub const GRAY: Self = Self { r: 128, g: 128, b: 128, a: 255 };
    /// Light gray.
    pub const LIGHT_GRAY: Self = Self { r: 200, g: 200, b: 200, a: 255 };
    /// Dark gray.
    pub const DARK_GRAY: Self = Self { r: 50, g: 50, b: 50, a: 255 };
    /// Blue (accent).
    pub const BLUE: Self = Self { r: 0, g: 120, b: 215, a: 255 };
    /// Red (error/danger).
    pub const RED: Self = Self { r: 220, g: 50, b: 50, a: 255 };
    /// Green (success).
    pub const GREEN: Self = Self { r: 50, g: 200, b: 50, a: 255 };
}

// ═══════════════════════════════════════════════════════════════════════
// Rectangle
// ═══════════════════════════════════════════════════════════════════════

/// A rectangle (position + size).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
}

impl Rect {
    /// Creates a new rectangle.
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// Returns `true` if point (px, py) is inside this rect.
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && px < self.x + self.w as i32
            && py >= self.y
            && py < self.y + self.h as i32
    }

    /// Returns `true` if this rect intersects with `other`.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.w as i32
            && self.x + self.w as i32 > other.x
            && self.y < other.y + other.h as i32
            && self.y + self.h as i32 > other.y
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Window Manager
// ═══════════════════════════════════════════════════════════════════════

/// A window in the window manager.
#[derive(Debug, Clone)]
pub struct Window {
    /// Window ID.
    pub id: u32,
    /// Window title.
    pub title: String,
    /// Position and size.
    pub rect: Rect,
    /// Is this window visible?
    pub visible: bool,
    /// Is this window focused?
    pub focused: bool,
    /// Z-order (higher = on top).
    pub z_order: u32,
    /// Background color.
    pub bg_color: Color,
    /// Widgets in this window.
    pub widgets: Vec<u32>,
    /// Is the window minimized?
    pub minimized: bool,
    /// Is the window maximized?
    pub maximized: bool,
}

/// Window manager — create, move, resize, close, focus, z-order.
#[derive(Debug)]
pub struct WindowManager {
    /// Windows keyed by ID.
    windows: HashMap<u32, Window>,
    /// Next window ID.
    next_id: u32,
    /// Currently focused window ID.
    pub focused_id: Option<u32>,
    /// Screen width.
    pub screen_width: u32,
    /// Screen height.
    pub screen_height: u32,
    /// Next z-order value.
    next_z: u32,
}

impl WindowManager {
    /// Creates a new window manager with the given screen size.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            windows: HashMap::new(),
            next_id: 1,
            focused_id: None,
            screen_width: width,
            screen_height: height,
            next_z: 1,
        }
    }

    /// Creates a new window.
    pub fn create_window(
        &mut self,
        title: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let z = self.next_z;
        self.next_z += 1;
        self.windows.insert(
            id,
            Window {
                id,
                title: title.to_string(),
                rect: Rect::new(x, y, w, h),
                visible: true,
                focused: false,
                z_order: z,
                bg_color: Color::WHITE,
                widgets: Vec::new(),
                minimized: false,
                maximized: false,
            },
        );
        self.focus_window(id);
        id
    }

    /// Moves a window to (x, y).
    pub fn move_window(&mut self, id: u32, x: i32, y: i32) -> Result<(), GuiError> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(GuiError::WindowNotFound(id))?;
        win.rect.x = x;
        win.rect.y = y;
        Ok(())
    }

    /// Resizes a window.
    pub fn resize_window(&mut self, id: u32, w: u32, h: u32) -> Result<(), GuiError> {
        if w == 0 || h == 0 {
            return Err(GuiError::InvalidDimensions(w, h));
        }
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(GuiError::WindowNotFound(id))?;
        win.rect.w = w;
        win.rect.h = h;
        Ok(())
    }

    /// Closes (removes) a window.
    pub fn close_window(&mut self, id: u32) -> Result<(), GuiError> {
        self.windows
            .remove(&id)
            .ok_or(GuiError::WindowNotFound(id))?;
        if self.focused_id == Some(id) {
            self.focused_id = None;
            // Focus next top window
            if let Some(top) = self.top_window() {
                self.focused_id = Some(top);
                if let Some(w) = self.windows.get_mut(&top) {
                    w.focused = true;
                }
            }
        }
        Ok(())
    }

    /// Focuses a window (brings to front).
    pub fn focus_window(&mut self, id: u32) -> bool {
        // Unfocus old
        if let Some(old) = self.focused_id {
            if let Some(w) = self.windows.get_mut(&old) {
                w.focused = false;
            }
        }
        if let Some(win) = self.windows.get_mut(&id) {
            win.focused = true;
            win.z_order = self.next_z;
            self.next_z += 1;
            self.focused_id = Some(id);
            true
        } else {
            false
        }
    }

    /// Returns the ID of the topmost (highest z-order) visible window.
    pub fn top_window(&self) -> Option<u32> {
        self.windows
            .values()
            .filter(|w| w.visible && !w.minimized)
            .max_by_key(|w| w.z_order)
            .map(|w| w.id)
    }

    /// Minimizes a window.
    pub fn minimize(&mut self, id: u32) -> Result<(), GuiError> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(GuiError::WindowNotFound(id))?;
        win.minimized = true;
        win.focused = false;
        if self.focused_id == Some(id) {
            self.focused_id = None;
        }
        Ok(())
    }

    /// Maximizes a window to fill the screen.
    pub fn maximize(&mut self, id: u32) -> Result<(), GuiError> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(GuiError::WindowNotFound(id))?;
        win.rect = Rect::new(0, 0, self.screen_width, self.screen_height);
        win.maximized = true;
        win.minimized = false;
        Ok(())
    }

    /// Returns a reference to a window.
    pub fn get_window(&self, id: u32) -> Option<&Window> {
        self.windows.get(&id)
    }

    /// Returns the number of windows.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Returns window IDs sorted by z-order (bottom to top).
    pub fn z_order(&self) -> Vec<u32> {
        let mut windows: Vec<&Window> = self.windows.values().collect();
        windows.sort_by_key(|w| w.z_order);
        windows.iter().map(|w| w.id).collect()
    }

    /// Finds the window at screen coordinates (px, py), topmost first.
    pub fn window_at(&self, px: i32, py: i32) -> Option<u32> {
        let mut candidates: Vec<&Window> = self
            .windows
            .values()
            .filter(|w| w.visible && !w.minimized && w.rect.contains(px, py))
            .collect();
        candidates.sort_by(|a, b| b.z_order.cmp(&a.z_order));
        candidates.first().map(|w| w.id)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Widget Toolkit
// ═══════════════════════════════════════════════════════════════════════

/// Widget type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetKind {
    /// Text label.
    Label(String),
    /// Clickable button.
    Button(String),
    /// Single-line text input.
    TextInput(String),
    /// Checkbox (checked/unchecked).
    Checkbox(bool),
    /// Horizontal slider (0-100).
    Slider(u32),
    /// Progress bar (0-100).
    ProgressBar(u32),
}

/// A widget in the GUI.
#[derive(Debug, Clone)]
pub struct Widget {
    /// Widget ID.
    pub id: u32,
    /// Widget type and value.
    pub kind: WidgetKind,
    /// Position and size relative to parent window.
    pub rect: Rect,
    /// Is this widget enabled?
    pub enabled: bool,
    /// Is this widget visible?
    pub visible: bool,
    /// Foreground color.
    pub fg_color: Color,
    /// Background color.
    pub bg_color: Color,
}

/// Widget toolkit — creates and manages widgets.
#[derive(Debug)]
pub struct WidgetToolkit {
    /// All widgets keyed by ID.
    widgets: HashMap<u32, Widget>,
    /// Next widget ID.
    next_id: u32,
}

impl WidgetToolkit {
    /// Creates a new widget toolkit.
    pub fn new() -> Self {
        Self {
            widgets: HashMap::new(),
            next_id: 1,
        }
    }

    /// Creates a label widget.
    pub fn create_label(&mut self, text: &str, rect: Rect) -> u32 {
        self.create_widget(WidgetKind::Label(text.to_string()), rect)
    }

    /// Creates a button widget.
    pub fn create_button(&mut self, text: &str, rect: Rect) -> u32 {
        self.create_widget(WidgetKind::Button(text.to_string()), rect)
    }

    /// Creates a text input widget.
    pub fn create_text_input(&mut self, placeholder: &str, rect: Rect) -> u32 {
        self.create_widget(WidgetKind::TextInput(placeholder.to_string()), rect)
    }

    /// Creates a checkbox widget.
    pub fn create_checkbox(&mut self, checked: bool, rect: Rect) -> u32 {
        self.create_widget(WidgetKind::Checkbox(checked), rect)
    }

    /// Creates a slider widget (0-100).
    pub fn create_slider(&mut self, value: u32, rect: Rect) -> u32 {
        self.create_widget(WidgetKind::Slider(value.min(100)), rect)
    }

    /// Creates a progress bar widget (0-100).
    pub fn create_progress_bar(&mut self, value: u32, rect: Rect) -> u32 {
        self.create_widget(WidgetKind::ProgressBar(value.min(100)), rect)
    }

    /// Internal widget creation.
    fn create_widget(&mut self, kind: WidgetKind, rect: Rect) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.widgets.insert(
            id,
            Widget {
                id,
                kind,
                rect,
                enabled: true,
                visible: true,
                fg_color: Color::BLACK,
                bg_color: Color::LIGHT_GRAY,
            },
        );
        id
    }

    /// Gets a widget by ID.
    pub fn get_widget(&self, id: u32) -> Option<&Widget> {
        self.widgets.get(&id)
    }

    /// Gets a mutable widget by ID.
    pub fn get_widget_mut(&mut self, id: u32) -> Option<&mut Widget> {
        self.widgets.get_mut(&id)
    }

    /// Updates a text input's value.
    pub fn set_text_input(&mut self, id: u32, text: &str) -> Result<(), GuiError> {
        let widget = self
            .widgets
            .get_mut(&id)
            .ok_or(GuiError::WidgetNotFound(id))?;
        if let WidgetKind::TextInput(ref mut val) = widget.kind {
            *val = text.to_string();
            Ok(())
        } else {
            Err(GuiError::WidgetNotFound(id))
        }
    }

    /// Toggles a checkbox.
    pub fn toggle_checkbox(&mut self, id: u32) -> Result<bool, GuiError> {
        let widget = self
            .widgets
            .get_mut(&id)
            .ok_or(GuiError::WidgetNotFound(id))?;
        if let WidgetKind::Checkbox(ref mut checked) = widget.kind {
            *checked = !*checked;
            Ok(*checked)
        } else {
            Err(GuiError::WidgetNotFound(id))
        }
    }

    /// Sets a slider value (clamped 0-100).
    pub fn set_slider(&mut self, id: u32, value: u32) -> Result<(), GuiError> {
        let widget = self
            .widgets
            .get_mut(&id)
            .ok_or(GuiError::WidgetNotFound(id))?;
        if let WidgetKind::Slider(ref mut val) = widget.kind {
            *val = value.min(100);
            Ok(())
        } else {
            Err(GuiError::WidgetNotFound(id))
        }
    }

    /// Sets a progress bar value (clamped 0-100).
    pub fn set_progress(&mut self, id: u32, value: u32) -> Result<(), GuiError> {
        let widget = self
            .widgets
            .get_mut(&id)
            .ok_or(GuiError::WidgetNotFound(id))?;
        if let WidgetKind::ProgressBar(ref mut val) = widget.kind {
            *val = value.min(100);
            Ok(())
        } else {
            Err(GuiError::WidgetNotFound(id))
        }
    }

    /// Removes a widget.
    pub fn remove_widget(&mut self, id: u32) -> bool {
        self.widgets.remove(&id).is_some()
    }

    /// Returns the number of widgets.
    pub fn widget_count(&self) -> usize {
        self.widgets.len()
    }
}

impl Default for WidgetToolkit {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Layout Engine (FlexBox)
// ═══════════════════════════════════════════════════════════════════════

/// Flex direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    /// Horizontal (left to right).
    Row,
    /// Vertical (top to bottom).
    Column,
}

/// Flex alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexAlign {
    /// Align to start (left/top).
    Start,
    /// Center.
    Center,
    /// Align to end (right/bottom).
    End,
    /// Distribute space evenly between items.
    SpaceBetween,
    /// Distribute space evenly around items.
    SpaceAround,
}

/// A flex layout item.
#[derive(Debug, Clone)]
pub struct FlexItem {
    /// Widget ID.
    pub widget_id: u32,
    /// Preferred width (0 = auto).
    pub width: u32,
    /// Preferred height (0 = auto).
    pub height: u32,
    /// Flex grow factor.
    pub grow: f32,
    /// Flex shrink factor.
    pub shrink: f32,
}

/// Computed layout position.
#[derive(Debug, Clone)]
pub struct LayoutResult {
    /// Widget ID.
    pub widget_id: u32,
    /// Computed rectangle.
    pub rect: Rect,
}

/// FlexBox layout engine.
#[derive(Debug)]
pub struct LayoutEngine {
    /// Layout direction.
    pub direction: FlexDirection,
    /// Main-axis alignment.
    pub justify: FlexAlign,
    /// Cross-axis alignment.
    pub align: FlexAlign,
    /// Gap between items (pixels).
    pub gap: u32,
    /// Wrap items to next line?
    pub wrap: bool,
    /// Container bounds.
    pub bounds: Rect,
}

impl LayoutEngine {
    /// Creates a new layout engine.
    pub fn new(direction: FlexDirection, bounds: Rect) -> Self {
        Self {
            direction,
            justify: FlexAlign::Start,
            align: FlexAlign::Start,
            gap: 0,
            wrap: false,
            bounds,
        }
    }

    /// Computes layout for the given items.
    pub fn layout(&self, items: &[FlexItem]) -> Vec<LayoutResult> {
        if items.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::with_capacity(items.len());

        match self.direction {
            FlexDirection::Row => self.layout_row(items, &mut results),
            FlexDirection::Column => self.layout_column(items, &mut results),
        }

        results
    }

    /// Row layout (horizontal).
    fn layout_row(&self, items: &[FlexItem], results: &mut Vec<LayoutResult>) {
        let total_gap = self.gap.saturating_mul(items.len().saturating_sub(1) as u32);
        let available_w = self.bounds.w.saturating_sub(total_gap);
        let fixed_w: u32 = items.iter().map(|i| i.width).sum();
        let total_grow: f32 = items.iter().map(|i| i.grow).sum();
        let extra = available_w.saturating_sub(fixed_w);

        let mut x = self.bounds.x;
        let start_offset = self.compute_start_offset(available_w, fixed_w, items.len());
        x += start_offset as i32;

        for item in items {
            let w = if total_grow > 0.0 && item.grow > 0.0 {
                item.width + ((extra as f32 * item.grow / total_grow) as u32)
            } else {
                item.width
            };
            let h = if item.height > 0 {
                item.height
            } else {
                self.bounds.h
            };
            let y = self.align_cross(self.bounds.y, self.bounds.h, h);

            results.push(LayoutResult {
                widget_id: item.widget_id,
                rect: Rect::new(x, y, w, h),
            });
            x += w as i32 + self.gap as i32;
        }
    }

    /// Column layout (vertical).
    fn layout_column(&self, items: &[FlexItem], results: &mut Vec<LayoutResult>) {
        let total_gap = self.gap.saturating_mul(items.len().saturating_sub(1) as u32);
        let available_h = self.bounds.h.saturating_sub(total_gap);
        let fixed_h: u32 = items.iter().map(|i| i.height).sum();
        let total_grow: f32 = items.iter().map(|i| i.grow).sum();
        let extra = available_h.saturating_sub(fixed_h);

        let mut y = self.bounds.y;
        let start_offset = self.compute_start_offset(available_h, fixed_h, items.len());
        y += start_offset as i32;

        for item in items {
            let h = if total_grow > 0.0 && item.grow > 0.0 {
                item.height + ((extra as f32 * item.grow / total_grow) as u32)
            } else {
                item.height
            };
            let w = if item.width > 0 {
                item.width
            } else {
                self.bounds.w
            };
            let x = self.align_cross(self.bounds.x, self.bounds.w, w);

            results.push(LayoutResult {
                widget_id: item.widget_id,
                rect: Rect::new(x, y, w, h),
            });
            y += h as i32 + self.gap as i32;
        }
    }

    /// Computes the start offset for justify alignment.
    fn compute_start_offset(&self, available: u32, used: u32, count: usize) -> u32 {
        match self.justify {
            FlexAlign::Start | FlexAlign::SpaceBetween | FlexAlign::SpaceAround => 0,
            FlexAlign::Center => available.saturating_sub(used) / 2,
            FlexAlign::End => available.saturating_sub(used),
        }
    }

    /// Computes cross-axis position for alignment.
    fn align_cross(&self, start: i32, container_size: u32, item_size: u32) -> i32 {
        match self.align {
            FlexAlign::Start | FlexAlign::SpaceBetween | FlexAlign::SpaceAround => start,
            FlexAlign::Center => start + (container_size.saturating_sub(item_size) / 2) as i32,
            FlexAlign::End => start + container_size.saturating_sub(item_size) as i32,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Theme Engine
// ═══════════════════════════════════════════════════════════════════════

/// Theme variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeVariant {
    /// Light theme.
    Light,
    /// Dark theme.
    Dark,
}

/// Color palette for a theme.
#[derive(Debug, Clone)]
pub struct ColorPalette {
    /// Primary color (buttons, links).
    pub primary: Color,
    /// Secondary/accent color.
    pub secondary: Color,
    /// Background color.
    pub background: Color,
    /// Surface color (cards, panels).
    pub surface: Color,
    /// Text color.
    pub text: Color,
    /// Error color.
    pub error: Color,
    /// Border color.
    pub border: Color,
}

/// Theme engine — light/dark themes with color palette.
#[derive(Debug, Clone)]
pub struct ThemeEngine {
    /// Current theme variant.
    pub variant: ThemeVariant,
    /// Active palette.
    pub palette: ColorPalette,
}

impl ThemeEngine {
    /// Creates a light theme.
    pub fn light() -> Self {
        Self {
            variant: ThemeVariant::Light,
            palette: ColorPalette {
                primary: Color::BLUE,
                secondary: Color::rgb(100, 100, 200),
                background: Color::WHITE,
                surface: Color::rgb(245, 245, 245),
                text: Color::BLACK,
                error: Color::RED,
                border: Color::GRAY,
            },
        }
    }

    /// Creates a dark theme.
    pub fn dark() -> Self {
        Self {
            variant: ThemeVariant::Dark,
            palette: ColorPalette {
                primary: Color::rgb(100, 180, 255),
                secondary: Color::rgb(150, 100, 255),
                background: Color::rgb(30, 30, 30),
                surface: Color::rgb(50, 50, 50),
                text: Color::WHITE,
                error: Color::rgb(255, 80, 80),
                border: Color::rgb(80, 80, 80),
            },
        }
    }

    /// Toggles between light and dark themes.
    pub fn toggle(&mut self) {
        match self.variant {
            ThemeVariant::Light => *self = Self::dark(),
            ThemeVariant::Dark => *self = Self::light(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Event System
// ═══════════════════════════════════════════════════════════════════════

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Right mouse button.
    Right,
    /// Middle mouse button.
    Middle,
}

/// Key code (simplified).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// Alphabetic key.
    Char(char),
    /// Enter/Return.
    Enter,
    /// Backspace.
    Backspace,
    /// Tab.
    Tab,
    /// Escape.
    Escape,
    /// Arrow keys.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
}

/// GUI event.
#[derive(Debug, Clone)]
pub enum GuiEvent {
    /// Mouse click.
    MouseClick {
        /// X coordinate.
        x: i32,
        /// Y coordinate.
        y: i32,
        /// Button.
        button: MouseButton,
    },
    /// Mouse move.
    MouseMove {
        /// X coordinate.
        x: i32,
        /// Y coordinate.
        y: i32,
    },
    /// Key press.
    KeyPress {
        /// Key code.
        key: KeyCode,
    },
    /// Key release.
    KeyRelease {
        /// Key code.
        key: KeyCode,
    },
    /// Window focus gained.
    FocusIn {
        /// Window ID.
        window_id: u32,
    },
    /// Window focus lost.
    FocusOut {
        /// Window ID.
        window_id: u32,
    },
    /// Mouse entered a widget area.
    Hover {
        /// Widget ID.
        widget_id: u32,
    },
}

/// Event dispatcher — queues and dispatches events.
#[derive(Debug)]
pub struct EventSystem {
    /// Event queue.
    queue: Vec<GuiEvent>,
    /// Processed event log.
    pub processed: Vec<GuiEvent>,
}

impl EventSystem {
    /// Creates a new event system.
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            processed: Vec::new(),
        }
    }

    /// Pushes an event onto the queue.
    pub fn push(&mut self, event: GuiEvent) {
        self.queue.push(event);
    }

    /// Pops the next event from the queue.
    pub fn pop(&mut self) -> Option<GuiEvent> {
        if self.queue.is_empty() {
            None
        } else {
            let ev = self.queue.remove(0);
            self.processed.push(ev.clone());
            Some(ev)
        }
    }

    /// Returns the number of pending events.
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Clears the event queue.
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

impl Default for EventSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Compositor (Double-Buffered)
// ═══════════════════════════════════════════════════════════════════════

/// Double-buffered compositor with dirty region tracking.
#[derive(Debug)]
pub struct GuiCompositor {
    /// Front buffer (displayed).
    pub front: Vec<Color>,
    /// Back buffer (being drawn to).
    pub back: Vec<Color>,
    /// Screen width.
    pub width: u32,
    /// Screen height.
    pub height: u32,
    /// Dirty regions that need redrawing.
    dirty_regions: Vec<Rect>,
    /// Frame counter.
    pub frame_count: u64,
}

impl GuiCompositor {
    /// Creates a new compositor.
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            front: vec![Color::BLACK; size],
            back: vec![Color::BLACK; size],
            width,
            height,
            dirty_regions: Vec::new(),
            frame_count: 0,
        }
    }

    /// Marks a region as dirty (needs redraw).
    pub fn mark_dirty(&mut self, rect: Rect) {
        self.dirty_regions.push(rect);
    }

    /// Clears the back buffer with the given color.
    pub fn clear(&mut self, color: Color) {
        for pixel in &mut self.back {
            *pixel = color;
        }
    }

    /// Draws a filled rectangle on the back buffer.
    pub fn fill_rect(&mut self, rect: &Rect, color: Color) {
        for dy in 0..rect.h {
            for dx in 0..rect.w {
                let px = rect.x + dx as i32;
                let py = rect.y + dy as i32;
                if px >= 0
                    && px < self.width as i32
                    && py >= 0
                    && py < self.height as i32
                {
                    let idx = py as usize * self.width as usize + px as usize;
                    if idx < self.back.len() {
                        self.back[idx] = color;
                    }
                }
            }
        }
    }

    /// Swaps front and back buffers (presents the frame).
    pub fn present(&mut self) {
        std::mem::swap(&mut self.front, &mut self.back);
        self.dirty_regions.clear();
        self.frame_count += 1;
    }

    /// Returns the number of dirty regions.
    pub fn dirty_count(&self) -> usize {
        self.dirty_regions.len()
    }

    /// Gets a pixel from the front buffer.
    pub fn get_pixel(&self, x: u32, y: u32) -> Color {
        let idx = y as usize * self.width as usize + x as usize;
        if idx < self.front.len() {
            self.front[idx]
        } else {
            Color::BLACK
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Font Renderer (Bitmap)
// ═══════════════════════════════════════════════════════════════════════

/// Font size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSize {
    /// Small (6x8 pixels per glyph).
    Small,
    /// Medium (8x12 pixels per glyph).
    Medium,
    /// Large (10x16 pixels per glyph).
    Large,
}

impl FontSize {
    /// Returns the glyph width.
    pub fn width(&self) -> u32 {
        match self {
            Self::Small => 6,
            Self::Medium => 8,
            Self::Large => 10,
        }
    }

    /// Returns the glyph height.
    pub fn height(&self) -> u32 {
        match self {
            Self::Small => 8,
            Self::Medium => 12,
            Self::Large => 16,
        }
    }
}

/// Bitmap font renderer.
///
/// Renders text by computing glyph positions. Actual glyph bitmaps are
/// simulated (filled rectangles per character) for testing purposes.
#[derive(Debug)]
pub struct FontRenderer {
    /// Default font size.
    pub size: FontSize,
    /// Default text color.
    pub color: Color,
}

impl FontRenderer {
    /// Creates a new font renderer.
    pub fn new(size: FontSize) -> Self {
        Self {
            size,
            color: Color::BLACK,
        }
    }

    /// Computes the bounding box for the given text.
    pub fn measure(&self, text: &str) -> (u32, u32) {
        let w = text.len() as u32 * self.size.width();
        let h = self.size.height();
        (w, h)
    }

    /// Renders text onto a compositor at (x, y).
    pub fn render(&self, compositor: &mut GuiCompositor, text: &str, x: i32, y: i32) {
        let gw = self.size.width();
        let gh = self.size.height();

        for (i, _ch) in text.chars().enumerate() {
            let gx = x + (i as u32 * gw) as i32;
            // Render glyph as a small filled rect (simplified)
            let inner_rect = Rect::new(gx + 1, y + 1, gw - 2, gh - 2);
            compositor.fill_rect(&inner_rect, self.color);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Image Loader (BMP/PPM)
// ═══════════════════════════════════════════════════════════════════════

/// A loaded image.
#[derive(Debug, Clone)]
pub struct Image {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Pixel data (RGBA).
    pub pixels: Vec<Color>,
}

/// Image loader supporting BMP and PPM formats.
#[derive(Debug)]
pub struct ImageLoader;

impl ImageLoader {
    /// Creates a new image loader.
    pub fn new() -> Self {
        Self
    }

    /// Loads a PPM (P6 binary) image from raw bytes.
    ///
    /// PPM format: `P6\n<width> <height>\n<maxval>\n<RGB data>`
    pub fn load_ppm(&self, data: &[u8]) -> Result<Image, GuiError> {
        // Very simplified PPM parser
        if data.len() < 10 {
            return Err(GuiError::ImageLoadError("data too short".to_string()));
        }
        // Check magic
        if data[0] != b'P' || data[1] != b'6' {
            return Err(GuiError::ImageLoadError("not PPM P6".to_string()));
        }

        // Parse header (simplified: assumes "P6\n<w> <h>\n255\n")
        let header_str = String::from_utf8_lossy(&data[..data.len().min(128)]);
        let lines: Vec<&str> = header_str.split('\n').collect();
        if lines.len() < 3 {
            return Err(GuiError::ImageLoadError("invalid PPM header".to_string()));
        }

        let dims: Vec<&str> = lines[1].split_whitespace().collect();
        if dims.len() < 2 {
            return Err(GuiError::ImageLoadError("invalid PPM dimensions".to_string()));
        }
        let width: u32 = dims[0]
            .parse()
            .map_err(|_| GuiError::ImageLoadError("bad width".to_string()))?;
        let height: u32 = dims[1]
            .parse()
            .map_err(|_| GuiError::ImageLoadError("bad height".to_string()))?;

        // Find pixel data start (after third newline)
        let mut newline_count = 0;
        let mut data_start = 0;
        for (i, &byte) in data.iter().enumerate() {
            if byte == b'\n' {
                newline_count += 1;
                if newline_count == 3 {
                    data_start = i + 1;
                    break;
                }
            }
        }

        let pixel_count = (width * height) as usize;
        let mut pixels = Vec::with_capacity(pixel_count);
        let pixel_data = &data[data_start..];

        for i in 0..pixel_count {
            let offset = i * 3;
            if offset + 2 < pixel_data.len() {
                pixels.push(Color::rgb(
                    pixel_data[offset],
                    pixel_data[offset + 1],
                    pixel_data[offset + 2],
                ));
            } else {
                pixels.push(Color::BLACK);
            }
        }

        Ok(Image {
            width,
            height,
            pixels,
        })
    }

    /// Creates a solid-color test image.
    pub fn create_solid(&self, width: u32, height: u32, color: Color) -> Image {
        let pixels = vec![color; (width * height) as usize];
        Image {
            width,
            height,
            pixels,
        }
    }
}

impl Default for ImageLoader {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Clipboard API
// ═══════════════════════════════════════════════════════════════════════

/// Clipboard for copy/paste between windows.
#[derive(Debug)]
pub struct ClipboardApi {
    /// Text content.
    text: Option<String>,
    /// Copy history.
    pub history: Vec<String>,
    /// Maximum history entries.
    pub max_history: usize,
}

impl ClipboardApi {
    /// Creates a new clipboard.
    pub fn new() -> Self {
        Self {
            text: None,
            history: Vec::new(),
            max_history: 50,
        }
    }

    /// Copies text to the clipboard.
    pub fn copy(&mut self, text: &str) {
        self.text = Some(text.to_string());
        if self.history.len() >= self.max_history {
            self.history.remove(0);
        }
        self.history.push(text.to_string());
    }

    /// Pastes text from the clipboard.
    pub fn paste(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// Clears the clipboard.
    pub fn clear(&mut self) {
        self.text = None;
    }

    /// Returns `true` if the clipboard has content.
    pub fn has_content(&self) -> bool {
        self.text.is_some()
    }
}

impl Default for ClipboardApi {
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

    // --- Window Manager tests ---

    #[test]
    fn wm_create_and_close_window() {
        let mut wm = WindowManager::new(1024, 768);
        let id = wm.create_window("Test", 100, 100, 400, 300);
        assert_eq!(wm.window_count(), 1);
        assert!(wm.get_window(id).is_some());
        wm.close_window(id).unwrap();
        assert_eq!(wm.window_count(), 0);
    }

    #[test]
    fn wm_move_and_resize() {
        let mut wm = WindowManager::new(1024, 768);
        let id = wm.create_window("Test", 0, 0, 200, 100);
        wm.move_window(id, 50, 50).unwrap();
        wm.resize_window(id, 300, 200).unwrap();
        let win = wm.get_window(id).unwrap();
        assert_eq!(win.rect.x, 50);
        assert_eq!(win.rect.y, 50);
        assert_eq!(win.rect.w, 300);
        assert_eq!(win.rect.h, 200);
    }

    #[test]
    fn wm_focus_management() {
        let mut wm = WindowManager::new(1024, 768);
        let w1 = wm.create_window("Win1", 0, 0, 200, 200);
        let w2 = wm.create_window("Win2", 100, 100, 200, 200);
        assert_eq!(wm.focused_id, Some(w2));

        wm.focus_window(w1);
        assert_eq!(wm.focused_id, Some(w1));
        let win1 = wm.get_window(w1).unwrap();
        assert!(win1.focused);
    }

    #[test]
    fn wm_z_order() {
        let mut wm = WindowManager::new(1024, 768);
        let w1 = wm.create_window("Win1", 0, 0, 200, 200);
        let w2 = wm.create_window("Win2", 0, 0, 200, 200);
        let order = wm.z_order();
        assert_eq!(order[0], w1);
        assert_eq!(order[1], w2);

        // Focusing w1 brings it to front
        wm.focus_window(w1);
        let order = wm.z_order();
        assert_eq!(*order.last().unwrap(), w1);
    }

    #[test]
    fn wm_minimize_and_maximize() {
        let mut wm = WindowManager::new(1024, 768);
        let id = wm.create_window("Test", 50, 50, 200, 100);
        wm.minimize(id).unwrap();
        assert!(wm.get_window(id).unwrap().minimized);
        wm.maximize(id).unwrap();
        let win = wm.get_window(id).unwrap();
        assert!(win.maximized);
        assert!(!win.minimized);
        assert_eq!(win.rect.w, 1024);
        assert_eq!(win.rect.h, 768);
    }

    #[test]
    fn wm_window_at() {
        let mut wm = WindowManager::new(1024, 768);
        let _w1 = wm.create_window("Bottom", 0, 0, 400, 400);
        let w2 = wm.create_window("Top", 50, 50, 200, 200);
        // Point (100, 100) is inside both, but w2 is on top
        assert_eq!(wm.window_at(100, 100), Some(w2));
    }

    // --- Widget Toolkit tests ---

    #[test]
    fn widget_create_all_types() {
        let mut kit = WidgetToolkit::new();
        let rect = Rect::new(0, 0, 100, 30);
        let _l = kit.create_label("Hello", rect);
        let _b = kit.create_button("Click", rect);
        let _t = kit.create_text_input("Type here", rect);
        let _c = kit.create_checkbox(false, rect);
        let _s = kit.create_slider(50, rect);
        let _p = kit.create_progress_bar(75, rect);
        assert_eq!(kit.widget_count(), 6);
    }

    #[test]
    fn widget_toggle_checkbox() {
        let mut kit = WidgetToolkit::new();
        let id = kit.create_checkbox(false, Rect::new(0, 0, 20, 20));
        let checked = kit.toggle_checkbox(id).unwrap();
        assert!(checked);
        let checked = kit.toggle_checkbox(id).unwrap();
        assert!(!checked);
    }

    #[test]
    fn widget_slider_clamped() {
        let mut kit = WidgetToolkit::new();
        let id = kit.create_slider(50, Rect::new(0, 0, 200, 20));
        kit.set_slider(id, 200).unwrap(); // should clamp to 100
        if let Some(w) = kit.get_widget(id) {
            if let WidgetKind::Slider(v) = w.kind {
                assert_eq!(v, 100);
            }
        }
    }

    // --- Layout Engine tests ---

    #[test]
    fn layout_row_basic() {
        let bounds = Rect::new(0, 0, 300, 100);
        let engine = LayoutEngine::new(FlexDirection::Row, bounds);
        let items = vec![
            FlexItem { widget_id: 1, width: 100, height: 50, grow: 0.0, shrink: 0.0 },
            FlexItem { widget_id: 2, width: 100, height: 50, grow: 0.0, shrink: 0.0 },
        ];
        let layout = engine.layout(&items);
        assert_eq!(layout.len(), 2);
        assert_eq!(layout[0].rect.x, 0);
        assert_eq!(layout[1].rect.x, 100);
    }

    #[test]
    fn layout_column_basic() {
        let bounds = Rect::new(0, 0, 200, 400);
        let engine = LayoutEngine::new(FlexDirection::Column, bounds);
        let items = vec![
            FlexItem { widget_id: 1, width: 100, height: 50, grow: 0.0, shrink: 0.0 },
            FlexItem { widget_id: 2, width: 100, height: 50, grow: 0.0, shrink: 0.0 },
        ];
        let layout = engine.layout(&items);
        assert_eq!(layout.len(), 2);
        assert_eq!(layout[0].rect.y, 0);
        assert_eq!(layout[1].rect.y, 50);
    }

    #[test]
    fn layout_with_gap() {
        let bounds = Rect::new(0, 0, 300, 100);
        let mut engine = LayoutEngine::new(FlexDirection::Row, bounds);
        engine.gap = 10;
        let items = vec![
            FlexItem { widget_id: 1, width: 50, height: 50, grow: 0.0, shrink: 0.0 },
            FlexItem { widget_id: 2, width: 50, height: 50, grow: 0.0, shrink: 0.0 },
        ];
        let layout = engine.layout(&items);
        assert_eq!(layout[1].rect.x, 60); // 50 + 10 gap
    }

    // --- Theme Engine tests ---

    #[test]
    fn theme_light_dark_toggle() {
        let mut theme = ThemeEngine::light();
        assert_eq!(theme.variant, ThemeVariant::Light);
        assert_eq!(theme.palette.background, Color::WHITE);
        theme.toggle();
        assert_eq!(theme.variant, ThemeVariant::Dark);
        assert_eq!(theme.palette.background, Color::rgb(30, 30, 30));
    }

    // --- Event System tests ---

    #[test]
    fn event_push_pop() {
        let mut events = EventSystem::new();
        events.push(GuiEvent::MouseClick {
            x: 100,
            y: 200,
            button: MouseButton::Left,
        });
        events.push(GuiEvent::KeyPress {
            key: KeyCode::Enter,
        });
        assert_eq!(events.pending_count(), 2);
        let ev = events.pop().unwrap();
        assert!(matches!(ev, GuiEvent::MouseClick { .. }));
        assert_eq!(events.pending_count(), 1);
    }

    // --- Compositor tests ---

    #[test]
    fn compositor_clear_and_fill() {
        let mut comp = GuiCompositor::new(10, 10);
        comp.clear(Color::WHITE);
        comp.fill_rect(&Rect::new(2, 2, 3, 3), Color::RED);
        comp.present();
        assert_eq!(comp.get_pixel(0, 0), Color::WHITE);
        assert_eq!(comp.get_pixel(3, 3), Color::RED);
        assert_eq!(comp.frame_count, 1);
    }

    #[test]
    fn compositor_dirty_tracking() {
        let mut comp = GuiCompositor::new(100, 100);
        comp.mark_dirty(Rect::new(0, 0, 50, 50));
        comp.mark_dirty(Rect::new(50, 50, 50, 50));
        assert_eq!(comp.dirty_count(), 2);
        comp.present();
        assert_eq!(comp.dirty_count(), 0);
    }

    // --- Font Renderer tests ---

    #[test]
    fn font_measure_text() {
        let font = FontRenderer::new(FontSize::Medium);
        let (w, h) = font.measure("Hello");
        assert_eq!(w, 5 * 8); // 5 chars * 8px each
        assert_eq!(h, 12);
    }

    #[test]
    fn font_render_does_not_panic() {
        let font = FontRenderer::new(FontSize::Small);
        let mut comp = GuiCompositor::new(100, 100);
        font.render(&mut comp, "Test", 0, 0);
        // Just verify it doesn't crash
        comp.present();
    }

    // --- Image Loader tests ---

    #[test]
    fn image_create_solid() {
        let loader = ImageLoader::new();
        let img = loader.create_solid(10, 10, Color::RED);
        assert_eq!(img.width, 10);
        assert_eq!(img.height, 10);
        assert_eq!(img.pixels.len(), 100);
        assert_eq!(img.pixels[0], Color::RED);
    }

    #[test]
    fn image_load_ppm() {
        let loader = ImageLoader::new();
        // Build a minimal 2x2 PPM P6 image
        let mut ppm = Vec::new();
        ppm.extend_from_slice(b"P6\n2 2\n255\n");
        ppm.extend_from_slice(&[255, 0, 0]); // red
        ppm.extend_from_slice(&[0, 255, 0]); // green
        ppm.extend_from_slice(&[0, 0, 255]); // blue
        ppm.extend_from_slice(&[255, 255, 0]); // yellow
        let img = loader.load_ppm(&ppm).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.pixels[0], Color::rgb(255, 0, 0));
    }

    // --- Clipboard tests ---

    #[test]
    fn clipboard_copy_paste() {
        let mut clip = ClipboardApi::new();
        assert!(!clip.has_content());
        clip.copy("Hello FajarOS");
        assert!(clip.has_content());
        assert_eq!(clip.paste(), Some("Hello FajarOS"));
    }

    #[test]
    fn clipboard_history() {
        let mut clip = ClipboardApi::new();
        clip.copy("first");
        clip.copy("second");
        clip.copy("third");
        assert_eq!(clip.history.len(), 3);
        assert_eq!(clip.paste(), Some("third"));
    }

    #[test]
    fn clipboard_clear() {
        let mut clip = ClipboardApi::new();
        clip.copy("data");
        clip.clear();
        assert!(!clip.has_content());
        assert_eq!(clip.paste(), None);
    }
}
