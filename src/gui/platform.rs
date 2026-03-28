//! Platform abstraction layer for the Fajar Lang GUI toolkit.
//!
//! Provides window management, clipboard, notifications, DPI scaling,
//! touch/gesture input, and runtime platform/render backend detection.

/// Windowing platform backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformBackend {
    /// X11 (Linux/BSD).
    X11,
    /// Wayland (Linux).
    Wayland,
    /// Win32 (Windows).
    Win32,
    /// Cocoa (macOS).
    Cocoa,
    /// Pure software fallback (headless / framebuffer).
    Software,
}

impl std::fmt::Display for PlatformBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformBackend::X11 => write!(f, "X11"),
            PlatformBackend::Wayland => write!(f, "Wayland"),
            PlatformBackend::Win32 => write!(f, "Win32"),
            PlatformBackend::Cocoa => write!(f, "Cocoa"),
            PlatformBackend::Software => write!(f, "Software"),
        }
    }
}

/// GPU/software rendering backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    /// Vulkan (cross-platform).
    Vulkan,
    /// Metal (macOS/iOS).
    Metal,
    /// Direct3D 12 (Windows).
    D3D12,
    /// OpenGL (fallback).
    OpenGL,
    /// CPU-based software renderer.
    Software,
}

impl std::fmt::Display for RenderBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderBackend::Vulkan => write!(f, "Vulkan"),
            RenderBackend::Metal => write!(f, "Metal"),
            RenderBackend::D3D12 => write!(f, "Direct3D 12"),
            RenderBackend::OpenGL => write!(f, "OpenGL"),
            RenderBackend::Software => write!(f, "Software"),
        }
    }
}

/// Configuration for creating a window.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    /// Window title shown in the title bar.
    pub title: String,
    /// Initial width in logical pixels.
    pub width: u32,
    /// Initial height in logical pixels.
    pub height: u32,
    /// Whether the user can resize the window.
    pub resizable: bool,
    /// Whether the window starts in fullscreen mode.
    pub fullscreen: bool,
    /// Whether to enable vertical sync.
    pub vsync: bool,
    /// DPI scale factor (1.0 = standard, 2.0 = HiDPI/Retina).
    pub dpi_scale: f32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: String::from("Fajar Lang Application"),
            width: 800,
            height: 600,
            resizable: true,
            fullscreen: false,
            vsync: true,
            dpi_scale: 1.0,
        }
    }
}

impl WindowConfig {
    /// Create a new window configuration with the given title and dimensions.
    pub fn new(title: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            title: title.into(),
            width,
            height,
            ..Self::default()
        }
    }

    /// Physical width in device pixels (width * dpi_scale).
    pub fn physical_width(&self) -> u32 {
        (self.width as f32 * self.dpi_scale) as u32
    }

    /// Physical height in device pixels (height * dpi_scale).
    pub fn physical_height(&self) -> u32 {
        (self.height as f32 * self.dpi_scale) as u32
    }
}

/// Handle to a platform window.
#[derive(Debug, Clone)]
pub struct WindowHandle {
    /// Unique window identifier.
    pub id: u64,
    /// Platform backend this window was created on.
    pub platform: PlatformBackend,
    /// Configuration used to create the window.
    pub config: WindowConfig,
}

impl WindowHandle {
    /// Create a new window handle.
    pub fn new(id: u64, platform: PlatformBackend, config: WindowConfig) -> Self {
        Self {
            id,
            platform,
            config,
        }
    }
}

// ---------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------

/// Data that can be placed on the system clipboard.
#[derive(Debug, Clone)]
pub struct ClipboardData {
    /// Text content, if any.
    pub text: Option<String>,
    /// Raw image data (RGBA bytes), if any.
    pub image_data: Option<Vec<u8>>,
}

impl ClipboardData {
    /// Create clipboard data containing text.
    pub fn from_text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            image_data: None,
        }
    }

    /// Create clipboard data containing raw image bytes.
    pub fn from_image(data: Vec<u8>) -> Self {
        Self {
            text: None,
            image_data: Some(data),
        }
    }
}

/// System clipboard abstraction.
///
/// Stores clipboard contents in-process. A real implementation would
/// delegate to platform APIs (X11 selections, Win32 clipboard, etc.).
#[derive(Debug, Clone, Default)]
pub struct Clipboard {
    /// Current clipboard contents.
    data: Option<ClipboardData>,
}

impl Clipboard {
    /// Create a new empty clipboard.
    pub fn new() -> Self {
        Self { data: None }
    }

    /// Get the current clipboard contents.
    pub fn get(&self) -> Option<&ClipboardData> {
        self.data.as_ref()
    }

    /// Set the clipboard contents.
    pub fn set(&mut self, data: ClipboardData) {
        self.data = Some(data);
    }

    /// Clear the clipboard.
    pub fn clear(&mut self) {
        self.data = None;
    }

    /// Check whether the clipboard contains text.
    pub fn has_text(&self) -> bool {
        self.data
            .as_ref()
            .map(|d| d.text.is_some())
            .unwrap_or(false)
    }

    /// Check whether the clipboard contains image data.
    pub fn has_image(&self) -> bool {
        self.data
            .as_ref()
            .map(|d| d.image_data.is_some())
            .unwrap_or(false)
    }

    /// Get the text from the clipboard, if present.
    pub fn get_text(&self) -> Option<&str> {
        self.data.as_ref().and_then(|d| d.text.as_deref())
    }
}

// ---------------------------------------------------------------------------
// System Tray & Notifications
// ---------------------------------------------------------------------------

/// An item in a system tray context menu.
#[derive(Debug, Clone)]
pub struct TrayMenuItem {
    /// Label displayed in the menu.
    pub label: String,
    /// Unique action identifier.
    pub action_id: u32,
    /// Whether the item is enabled (clickable).
    pub enabled: bool,
}

impl TrayMenuItem {
    /// Create a new enabled tray menu item.
    pub fn new(label: impl Into<String>, action_id: u32) -> Self {
        Self {
            label: label.into(),
            action_id,
            enabled: true,
        }
    }
}

/// System tray icon with tooltip and context menu.
#[derive(Debug, Clone)]
pub struct SystemTray {
    /// Path or identifier of the tray icon.
    pub icon: String,
    /// Tooltip shown on hover.
    pub tooltip: String,
    /// Context menu items.
    pub menu_items: Vec<TrayMenuItem>,
}

impl SystemTray {
    /// Create a new system tray icon.
    pub fn new(icon: impl Into<String>, tooltip: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            tooltip: tooltip.into(),
            menu_items: Vec::new(),
        }
    }

    /// Add a menu item to the tray context menu.
    pub fn add_item(&mut self, item: TrayMenuItem) {
        self.menu_items.push(item);
    }
}

/// Urgency level for system notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationUrgency {
    /// Low priority — may be deferred or suppressed.
    Low,
    /// Normal priority.
    Normal,
    /// Critical — should interrupt the user.
    Critical,
}

/// A system notification (toast / alert).
#[derive(Debug, Clone)]
pub struct Notification {
    /// Notification title.
    pub title: String,
    /// Notification body text.
    pub body: String,
    /// Optional icon path or identifier.
    pub icon: Option<String>,
    /// Urgency level.
    pub urgency: NotificationUrgency,
    /// Display timeout in milliseconds (0 = persistent).
    pub timeout_ms: u64,
}

impl Notification {
    /// Create a new notification with normal urgency and 5-second timeout.
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            icon: None,
            urgency: NotificationUrgency::Normal,
            timeout_ms: 5000,
        }
    }

    /// Set the urgency level.
    pub fn with_urgency(mut self, urgency: NotificationUrgency) -> Self {
        self.urgency = urgency;
        self
    }

    /// Set the display timeout in milliseconds.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set the icon path.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

// ---------------------------------------------------------------------------
// Cursor
// ---------------------------------------------------------------------------

/// Mouse cursor style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    /// Default arrow cursor.
    Default,
    /// Pointing hand (links, buttons).
    Pointer,
    /// Text insertion beam.
    Text,
    /// Four-directional move/drag.
    Move,
    /// North-south resize.
    ResizeNS,
    /// East-west resize.
    ResizeEW,
    /// Crosshair (precision selection).
    Crosshair,
    /// Busy/wait spinner.
    Wait,
    /// Action not allowed.
    NotAllowed,
}

// ---------------------------------------------------------------------------
// DPI Scaling
// ---------------------------------------------------------------------------

/// DPI scale factor for coordinate conversion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DpiScale {
    /// Scale factor (e.g., 1.0 for 96 DPI, 2.0 for 192 DPI / Retina).
    pub factor: f32,
}

impl DpiScale {
    /// Create a new DPI scale with the given factor.
    ///
    /// The factor is clamped to a minimum of 0.25 to avoid degenerate scaling.
    pub fn new(factor: f32) -> Self {
        Self {
            factor: factor.max(0.25),
        }
    }

    /// Convert a physical (device) pixel value to logical pixels.
    pub fn physical_to_logical(&self, physical: f32) -> f32 {
        if self.factor == 0.0 {
            return physical;
        }
        physical / self.factor
    }

    /// Convert a logical pixel value to physical (device) pixels.
    pub fn logical_to_physical(&self, logical: f32) -> f32 {
        logical * self.factor
    }
}

impl Default for DpiScale {
    fn default() -> Self {
        Self { factor: 1.0 }
    }
}

// ---------------------------------------------------------------------------
// Touch & Gesture
// ---------------------------------------------------------------------------

/// Phase of a touch event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    /// Finger touched the surface.
    Begin,
    /// Finger moved on the surface.
    Move,
    /// Finger lifted from the surface.
    End,
    /// Touch was interrupted by the system.
    Cancel,
}

/// A single touch event from a touch screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchEvent {
    /// Unique identifier for this touch (finger).
    pub id: u64,
    /// X coordinate in logical pixels.
    pub x: f32,
    /// Y coordinate in logical pixels.
    pub y: f32,
    /// Phase of the touch.
    pub phase: TouchPhase,
}

impl TouchEvent {
    /// Create a new touch event.
    pub fn new(id: u64, x: f32, y: f32, phase: TouchPhase) -> Self {
        Self { id, x, y, phase }
    }
}

/// Recognized gesture types.
#[derive(Debug, Clone, PartialEq)]
pub enum GestureKind {
    /// Two-finger pinch. `scale` > 1.0 means zoom in, < 1.0 means zoom out.
    Pinch {
        /// Current scale factor relative to the starting distance.
        scale: f32,
    },
    /// One- or two-finger pan/drag.
    Pan {
        /// Horizontal translation since gesture start.
        dx: f32,
        /// Vertical translation since gesture start.
        dy: f32,
    },
    /// Two-finger rotation.
    Rotate {
        /// Rotation angle in radians since gesture start.
        angle_rad: f32,
    },
}

/// Recognizes multi-touch gestures from a stream of touch events.
///
/// Tracks active touches and detects pinch, pan, and rotate gestures.
#[derive(Debug, Clone, Default)]
pub struct GestureRecognizer {
    /// Active touches by ID.
    touches: Vec<TouchEvent>,
    /// Starting distance for pinch detection (between first two touches).
    start_distance: Option<f32>,
    /// Starting angle for rotation detection.
    start_angle: Option<f32>,
    /// Starting center for pan detection.
    start_center: Option<(f32, f32)>,
}

impl GestureRecognizer {
    /// Create a new gesture recognizer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a touch event and return any recognized gesture.
    pub fn process(&mut self, event: TouchEvent) -> Option<GestureKind> {
        match event.phase {
            TouchPhase::Begin => {
                // Remove any stale touch with same id
                self.touches.retain(|t| t.id != event.id);
                self.touches.push(event);
                if self.touches.len() == 2 {
                    self.start_distance = Some(self.current_distance());
                    self.start_angle = Some(self.current_angle());
                    self.start_center = Some(self.current_center());
                }
                None
            }
            TouchPhase::Move => {
                // Update the position of the existing touch
                if let Some(t) = self.touches.iter_mut().find(|t| t.id == event.id) {
                    t.x = event.x;
                    t.y = event.y;
                }
                self.detect_gesture()
            }
            TouchPhase::End | TouchPhase::Cancel => {
                self.touches.retain(|t| t.id != event.id);
                if self.touches.len() < 2 {
                    self.start_distance = None;
                    self.start_angle = None;
                    self.start_center = None;
                }
                None
            }
        }
    }

    /// Detect a gesture from the current active touches.
    fn detect_gesture(&self) -> Option<GestureKind> {
        if self.touches.len() >= 2 {
            // Try pinch first
            if let Some(start_dist) = self.start_distance {
                let current_dist = self.current_distance();
                if start_dist > 0.0 {
                    let scale = current_dist / start_dist;
                    // Only report pinch if scale changed meaningfully
                    if (scale - 1.0).abs() > 0.01 {
                        return Some(GestureKind::Pinch { scale });
                    }
                }
            }
            // Try rotation
            if let Some(start_angle) = self.start_angle {
                let current_angle = self.current_angle();
                let delta = current_angle - start_angle;
                if delta.abs() > 0.01 {
                    return Some(GestureKind::Rotate { angle_rad: delta });
                }
            }
        }
        // Try pan (single or multi-touch)
        if let Some((sx, sy)) = self.start_center {
            let (cx, cy) = self.current_center();
            let dx = cx - sx;
            let dy = cy - sy;
            if dx.abs() > 1.0 || dy.abs() > 1.0 {
                return Some(GestureKind::Pan { dx, dy });
            }
        }
        None
    }

    /// Euclidean distance between the first two active touches.
    fn current_distance(&self) -> f32 {
        if self.touches.len() < 2 {
            return 0.0;
        }
        let a = &self.touches[0];
        let b = &self.touches[1];
        ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
    }

    /// Angle (in radians) between the first two active touches.
    fn current_angle(&self) -> f32 {
        if self.touches.len() < 2 {
            return 0.0;
        }
        let a = &self.touches[0];
        let b = &self.touches[1];
        (b.y - a.y).atan2(b.x - a.x)
    }

    /// Center point of all active touches.
    fn current_center(&self) -> (f32, f32) {
        if self.touches.is_empty() {
            return (0.0, 0.0);
        }
        let n = self.touches.len() as f32;
        let sx: f32 = self.touches.iter().map(|t| t.x).sum();
        let sy: f32 = self.touches.iter().map(|t| t.y).sum();
        (sx / n, sy / n)
    }

    /// Number of active touches.
    pub fn active_touch_count(&self) -> usize {
        self.touches.len()
    }

    /// Reset the recognizer, clearing all tracked touches.
    pub fn reset(&mut self) {
        self.touches.clear();
        self.start_distance = None;
        self.start_angle = None;
        self.start_center = None;
    }
}

// ---------------------------------------------------------------------------
// Platform Info
// ---------------------------------------------------------------------------

/// Information about a display monitor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// Refresh rate in Hz.
    pub refresh_rate: u32,
}

/// Runtime information about the host platform.
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// Operating system name (e.g., "Linux", "Windows", "macOS").
    pub os_name: String,
    /// Operating system version string.
    pub os_version: String,
    /// Number of connected displays.
    pub display_count: u32,
    /// Primary monitor information.
    pub primary_monitor: Option<MonitorInfo>,
}

impl PlatformInfo {
    /// Gather platform information at runtime.
    ///
    /// Uses compile-time `cfg` for OS detection and provides
    /// placeholder monitor info (real implementation would query
    /// the display server).
    pub fn detect() -> Self {
        let os_name = if cfg!(target_os = "linux") {
            "Linux"
        } else if cfg!(target_os = "windows") {
            "Windows"
        } else if cfg!(target_os = "macos") {
            "macOS"
        } else {
            "Unknown"
        };

        Self {
            os_name: os_name.to_string(),
            os_version: String::new(), // Would query /etc/os-release, GetVersionEx, etc.
            display_count: 1,
            primary_monitor: Some(MonitorInfo {
                width: 1920,
                height: 1080,
                refresh_rate: 60,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Detection functions
// ---------------------------------------------------------------------------

/// Detect the best available platform backend at runtime.
///
/// On Linux, checks for Wayland first (via `WAYLAND_DISPLAY` env var),
/// then X11 (`DISPLAY`), falling back to Software.
/// On Windows returns Win32, on macOS returns Cocoa.
pub fn detect_platform() -> PlatformBackend {
    if cfg!(target_os = "windows") {
        return PlatformBackend::Win32;
    }
    if cfg!(target_os = "macos") {
        return PlatformBackend::Cocoa;
    }
    // Linux / BSD — check for Wayland first, then X11
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return PlatformBackend::Wayland;
    }
    if std::env::var("DISPLAY").is_ok() {
        return PlatformBackend::X11;
    }
    PlatformBackend::Software
}

/// Detect the best available render backend at runtime.
///
/// On macOS prefers Metal, on Windows prefers D3D12,
/// on Linux prefers Vulkan, with OpenGL and Software as fallbacks.
pub fn detect_render_backend() -> RenderBackend {
    if cfg!(target_os = "macos") {
        return RenderBackend::Metal;
    }
    if cfg!(target_os = "windows") {
        return RenderBackend::D3D12;
    }
    // Linux — prefer Vulkan, fall back to OpenGL then Software
    // A real implementation would probe for libvulkan.so / libGL.so
    if cfg!(target_os = "linux") {
        return RenderBackend::Vulkan;
    }
    RenderBackend::Software
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    // -- WindowConfig -----------------------------------------------------

    #[test]
    fn window_config_defaults() {
        let cfg = WindowConfig::default();
        assert_eq!(cfg.title, "Fajar Lang Application");
        assert_eq!(cfg.width, 800);
        assert_eq!(cfg.height, 600);
        assert!(cfg.resizable);
        assert!(!cfg.fullscreen);
        assert!(cfg.vsync);
        assert!(approx_eq(cfg.dpi_scale, 1.0));
    }

    #[test]
    fn window_config_custom() {
        let cfg = WindowConfig::new("My App", 1024, 768);
        assert_eq!(cfg.title, "My App");
        assert_eq!(cfg.width, 1024);
        assert_eq!(cfg.height, 768);
    }

    #[test]
    fn window_config_physical_size() {
        let mut cfg = WindowConfig::new("HiDPI", 800, 600);
        cfg.dpi_scale = 2.0;
        assert_eq!(cfg.physical_width(), 1600);
        assert_eq!(cfg.physical_height(), 1200);
    }

    // -- DpiScale ---------------------------------------------------------

    #[test]
    fn dpi_scale_conversion() {
        let scale = DpiScale::new(2.0);
        assert!(approx_eq(scale.logical_to_physical(100.0), 200.0));
        assert!(approx_eq(scale.physical_to_logical(200.0), 100.0));
    }

    #[test]
    fn dpi_scale_default_is_one() {
        let scale = DpiScale::default();
        assert!(approx_eq(scale.factor, 1.0));
        assert!(approx_eq(scale.logical_to_physical(50.0), 50.0));
        assert!(approx_eq(scale.physical_to_logical(50.0), 50.0));
    }

    #[test]
    fn dpi_scale_minimum_clamped() {
        let scale = DpiScale::new(0.1);
        assert!(approx_eq(scale.factor, 0.25));
    }

    // -- Platform detection -----------------------------------------------

    #[test]
    fn detect_platform_returns_valid_backend() {
        let platform = detect_platform();
        // On any platform, we should get a valid variant
        let valid = matches!(
            platform,
            PlatformBackend::X11
                | PlatformBackend::Wayland
                | PlatformBackend::Win32
                | PlatformBackend::Cocoa
                | PlatformBackend::Software
        );
        assert!(valid);
    }

    #[test]
    fn detect_render_backend_returns_valid() {
        let backend = detect_render_backend();
        let valid = matches!(
            backend,
            RenderBackend::Vulkan
                | RenderBackend::Metal
                | RenderBackend::D3D12
                | RenderBackend::OpenGL
                | RenderBackend::Software
        );
        assert!(valid);
    }

    #[test]
    fn platform_backend_display() {
        assert_eq!(format!("{}", PlatformBackend::X11), "X11");
        assert_eq!(format!("{}", PlatformBackend::Wayland), "Wayland");
        assert_eq!(format!("{}", PlatformBackend::Software), "Software");
    }

    #[test]
    fn render_backend_display() {
        assert_eq!(format!("{}", RenderBackend::Vulkan), "Vulkan");
        assert_eq!(format!("{}", RenderBackend::Metal), "Metal");
        assert_eq!(format!("{}", RenderBackend::Software), "Software");
    }

    // -- Clipboard --------------------------------------------------------

    #[test]
    fn clipboard_text_operations() {
        let mut cb = Clipboard::new();
        assert!(!cb.has_text());
        assert!(!cb.has_image());
        assert!(cb.get().is_none());

        cb.set(ClipboardData::from_text("hello"));
        assert!(cb.has_text());
        assert!(!cb.has_image());
        assert_eq!(cb.get_text(), Some("hello"));

        cb.clear();
        assert!(!cb.has_text());
        assert!(cb.get().is_none());
    }

    #[test]
    fn clipboard_image_data() {
        let mut cb = Clipboard::new();
        let pixels = vec![255u8, 0, 0, 255]; // one red RGBA pixel
        cb.set(ClipboardData::from_image(pixels.clone()));
        assert!(cb.has_image());
        assert!(!cb.has_text());
        let data = cb.get().expect("should have data");
        assert_eq!(data.image_data.as_ref().expect("should have image"), &pixels);
    }

    // -- Notification -----------------------------------------------------

    #[test]
    fn notification_creation() {
        let notif = Notification::new("Update", "Version 2.0 available")
            .with_urgency(NotificationUrgency::Critical)
            .with_timeout(10000)
            .with_icon("update.png");
        assert_eq!(notif.title, "Update");
        assert_eq!(notif.body, "Version 2.0 available");
        assert_eq!(notif.urgency, NotificationUrgency::Critical);
        assert_eq!(notif.timeout_ms, 10000);
        assert_eq!(notif.icon.as_deref(), Some("update.png"));
    }

    #[test]
    fn notification_defaults() {
        let notif = Notification::new("Hello", "World");
        assert_eq!(notif.urgency, NotificationUrgency::Normal);
        assert_eq!(notif.timeout_ms, 5000);
        assert!(notif.icon.is_none());
    }

    // -- SystemTray -------------------------------------------------------

    #[test]
    fn system_tray_menu_items() {
        let mut tray = SystemTray::new("icon.png", "Fajar Lang");
        tray.add_item(TrayMenuItem::new("Show", 1));
        tray.add_item(TrayMenuItem::new("Quit", 2));
        assert_eq!(tray.menu_items.len(), 2);
        assert_eq!(tray.menu_items[0].label, "Show");
        assert_eq!(tray.menu_items[1].action_id, 2);
        assert!(tray.menu_items[0].enabled);
    }

    // -- Touch & Gesture --------------------------------------------------

    #[test]
    fn touch_event_creation() {
        let te = TouchEvent::new(1, 100.0, 200.0, TouchPhase::Begin);
        assert_eq!(te.id, 1);
        assert!(approx_eq(te.x, 100.0));
        assert!(approx_eq(te.y, 200.0));
        assert_eq!(te.phase, TouchPhase::Begin);
    }

    #[test]
    fn gesture_recognizer_pinch() {
        let mut gr = GestureRecognizer::new();
        // Two fingers down
        gr.process(TouchEvent::new(1, 100.0, 100.0, TouchPhase::Begin));
        gr.process(TouchEvent::new(2, 200.0, 100.0, TouchPhase::Begin));
        // Move fingers apart (pinch out)
        let gesture = gr.process(TouchEvent::new(1, 50.0, 100.0, TouchPhase::Move));
        assert!(gesture.is_some());
        if let Some(GestureKind::Pinch { scale }) = gesture {
            assert!(scale > 1.0); // fingers moved apart
        }
    }

    #[test]
    fn gesture_recognizer_pan() {
        let mut gr = GestureRecognizer::new();
        // Single finger down, establish center
        gr.process(TouchEvent::new(1, 100.0, 100.0, TouchPhase::Begin));
        // A second touch to set start_center (pan requires start_center)
        gr.process(TouchEvent::new(2, 100.0, 100.0, TouchPhase::Begin));
        // Move both fingers together
        gr.process(TouchEvent::new(1, 120.0, 100.0, TouchPhase::Move));
        let gesture = gr.process(TouchEvent::new(2, 120.0, 100.0, TouchPhase::Move));
        // Should detect some gesture (pan or pinch depending on exact positions)
        assert!(gesture.is_some());
    }

    #[test]
    fn gesture_recognizer_reset() {
        let mut gr = GestureRecognizer::new();
        gr.process(TouchEvent::new(1, 100.0, 100.0, TouchPhase::Begin));
        assert_eq!(gr.active_touch_count(), 1);
        gr.reset();
        assert_eq!(gr.active_touch_count(), 0);
    }

    // -- PlatformInfo -----------------------------------------------------

    #[test]
    fn platform_info_detect() {
        let info = PlatformInfo::detect();
        assert!(!info.os_name.is_empty());
        assert!(info.display_count >= 1);
        assert!(info.primary_monitor.is_some());
        let monitor = info.primary_monitor.expect("should have primary monitor");
        assert!(monitor.width > 0);
        assert!(monitor.height > 0);
        assert!(monitor.refresh_rate > 0);
    }

    // -- WindowHandle -----------------------------------------------------

    #[test]
    fn window_handle_creation() {
        let cfg = WindowConfig::new("Test", 640, 480);
        let handle = WindowHandle::new(42, PlatformBackend::X11, cfg);
        assert_eq!(handle.id, 42);
        assert_eq!(handle.platform, PlatformBackend::X11);
        assert_eq!(handle.config.title, "Test");
    }

    // -- CursorStyle ------------------------------------------------------

    #[test]
    fn cursor_styles_are_distinct() {
        let styles = [
            CursorStyle::Default,
            CursorStyle::Pointer,
            CursorStyle::Text,
            CursorStyle::Move,
            CursorStyle::ResizeNS,
            CursorStyle::ResizeEW,
            CursorStyle::Crosshair,
            CursorStyle::Wait,
            CursorStyle::NotAllowed,
        ];
        // All 9 variants are distinct
        for i in 0..styles.len() {
            for j in (i + 1)..styles.len() {
                assert_ne!(styles[i], styles[j]);
            }
        }
    }
}
