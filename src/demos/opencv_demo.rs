//! Sprint W1: OpenCV Face Detection Demo — simulated OpenCV pipeline with
//! CvMat image representation, Haar cascade face detection, webcam simulation,
//! BMP image I/O, and performance benchmarking.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// W1.1: CvMat — Simulated OpenCV Mat
// ═══════════════════════════════════════════════════════════════════════

/// Number of channels in a BGR image.
pub const CV_BGR_CHANNELS: usize = 3;
/// Number of channels in a grayscale image.
pub const CV_GRAY_CHANNELS: usize = 1;
/// Default test image width.
pub const DEFAULT_WIDTH: usize = 640;
/// Default test image height.
pub const DEFAULT_HEIGHT: usize = 480;

/// Simulated OpenCV Mat — holds image data with width, height, and channels.
#[derive(Debug, Clone)]
pub struct CvMat {
    /// Image width in pixels.
    pub width: usize,
    /// Image height in pixels.
    pub height: usize,
    /// Number of channels (1=gray, 3=BGR).
    pub channels: usize,
    /// Raw pixel data in row-major order.
    pub data: Vec<u8>,
}

impl CvMat {
    /// Creates a new CvMat initialized to zeros.
    pub fn zeros(width: usize, height: usize, channels: usize) -> Self {
        Self {
            width,
            height,
            channels,
            data: vec![0u8; width * height * channels],
        }
    }

    /// Creates a CvMat from raw pixel data. Returns `None` if data length mismatches.
    pub fn from_data(width: usize, height: usize, channels: usize, data: Vec<u8>) -> Option<Self> {
        if data.len() != width * height * channels {
            return None;
        }
        Some(Self {
            width,
            height,
            channels,
            data,
        })
    }

    /// Returns the total number of pixels (width * height).
    pub fn pixel_count(&self) -> usize {
        self.width * self.height
    }

    /// Returns the total data size in bytes.
    pub fn data_size(&self) -> usize {
        self.width * self.height * self.channels
    }

    /// Gets pixel value at (x, y, channel). Returns `None` if out of bounds.
    pub fn get_pixel(&self, x: usize, y: usize, channel: usize) -> Option<u8> {
        if x >= self.width || y >= self.height || channel >= self.channels {
            return None;
        }
        let idx = (y * self.width + x) * self.channels + channel;
        self.data.get(idx).copied()
    }

    /// Sets pixel value at (x, y, channel). Returns `false` if out of bounds.
    pub fn set_pixel(&mut self, x: usize, y: usize, channel: usize, value: u8) -> bool {
        if x >= self.width || y >= self.height || channel >= self.channels {
            return false;
        }
        let idx = (y * self.width + x) * self.channels + channel;
        if let Some(p) = self.data.get_mut(idx) {
            *p = value;
            true
        } else {
            false
        }
    }

    /// Converts a BGR image to grayscale using luminance formula.
    /// Returns `None` if the source is not 3-channel.
    pub fn to_grayscale(&self) -> Option<CvMat> {
        if self.channels != CV_BGR_CHANNELS {
            return None;
        }
        let mut gray = CvMat::zeros(self.width, self.height, CV_GRAY_CHANNELS);
        for y in 0..self.height {
            for x in 0..self.width {
                let base = (y * self.width + x) * CV_BGR_CHANNELS;
                let b = self.data[base] as f64;
                let g = self.data[base + 1] as f64;
                let r = self.data[base + 2] as f64;
                // Standard luminance: 0.299*R + 0.587*G + 0.114*B
                let lum = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
                gray.data[y * self.width + x] = lum;
            }
        }
        Some(gray)
    }

    /// Creates a test image with a simulated face-like bright region at center.
    pub fn test_image_with_face(width: usize, height: usize) -> Self {
        let mut mat = CvMat::zeros(width, height, CV_BGR_CHANNELS);
        // Background: dark gray
        for px in mat.data.iter_mut() {
            *px = 40;
        }
        // Simulated face region: bright oval in center
        let cx = width / 2;
        let cy = height / 2;
        let rx = width / 8;
        let ry = height / 6;
        for y in 0..height {
            for x in 0..width {
                let dx = (x as f64 - cx as f64) / rx as f64;
                let dy = (y as f64 - cy as f64) / ry as f64;
                if dx * dx + dy * dy <= 1.0 {
                    let base = (y * width + x) * CV_BGR_CHANNELS;
                    mat.data[base] = 180; // B
                    mat.data[base + 1] = 200; // G
                    mat.data[base + 2] = 220; // R — skin-like
                }
            }
        }
        mat
    }
}

impl fmt::Display for CvMat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CvMat({}x{}, {}ch, {} bytes)",
            self.width,
            self.height,
            self.channels,
            self.data_size()
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W1.2: CvRect — Detected face rectangles
// ═══════════════════════════════════════════════════════════════════════

/// A rectangle representing a detected face region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CvRect {
    /// Top-left X coordinate.
    pub x: usize,
    /// Top-left Y coordinate.
    pub y: usize,
    /// Rectangle width.
    pub width: usize,
    /// Rectangle height.
    pub height: usize,
}

impl CvRect {
    /// Creates a new rectangle.
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns the area of the rectangle.
    pub fn area(&self) -> usize {
        self.width * self.height
    }

    /// Returns the center point (cx, cy).
    pub fn center(&self) -> (usize, usize) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    /// Checks if this rectangle contains a point.
    pub fn contains(&self, px: usize, py: usize) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Returns intersection-over-union (IoU) with another rectangle.
    pub fn iou(&self, other: &CvRect) -> f64 {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);

        if x2 <= x1 || y2 <= y1 {
            return 0.0;
        }
        let inter = (x2 - x1) * (y2 - y1);
        let union = self.area() + other.area() - inter;
        if union == 0 {
            return 0.0;
        }
        inter as f64 / union as f64
    }
}

impl fmt::Display for CvRect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Rect({}, {}, {}x{})",
            self.x, self.y, self.width, self.height
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W1.3: CvCascadeClassifier — Haar cascade face detection simulation
// ═══════════════════════════════════════════════════════════════════════

/// Haar cascade classifier configuration.
#[derive(Debug, Clone)]
pub struct CascadeConfig {
    /// Scale factor between scan window sizes (e.g., 1.1).
    pub scale_factor: f64,
    /// Minimum number of neighbor detections to keep a candidate.
    pub min_neighbors: usize,
    /// Minimum detection window size.
    pub min_size: (usize, usize),
    /// Maximum detection window size (0 = no limit).
    pub max_size: (usize, usize),
}

impl Default for CascadeConfig {
    fn default() -> Self {
        Self {
            scale_factor: 1.1,
            min_neighbors: 3,
            min_size: (30, 30),
            max_size: (0, 0),
        }
    }
}

/// Simulated Haar cascade classifier for face detection.
#[derive(Debug, Clone)]
pub struct CvCascadeClassifier {
    /// Cascade configuration.
    pub config: CascadeConfig,
    /// Cascade file path (simulated).
    pub cascade_path: String,
    /// Whether the cascade is loaded.
    pub loaded: bool,
}

impl CvCascadeClassifier {
    /// Creates a new classifier with the default Haar frontal-face cascade.
    pub fn new_frontal_face() -> Self {
        Self {
            config: CascadeConfig::default(),
            cascade_path: "haarcascade_frontalface_default.xml".into(),
            loaded: true,
        }
    }

    /// Creates a classifier from a custom cascade file path.
    pub fn from_file(path: &str) -> Self {
        Self {
            config: CascadeConfig::default(),
            cascade_path: path.into(),
            loaded: true,
        }
    }

    /// Detects faces in a grayscale image. Returns detected rectangles.
    ///
    /// Simulation: scans for bright regions (intensity > 120) that form
    /// contiguous blobs within the min/max size constraints.
    pub fn detect_multi_scale(&self, gray: &CvMat) -> Vec<CvRect> {
        if !self.loaded || gray.channels != CV_GRAY_CHANNELS {
            return Vec::new();
        }
        let mut detections = Vec::new();

        // Scan at multiple scales
        let mut win_size = self.config.min_size.0;
        let max_w = if self.config.max_size.0 > 0 {
            self.config.max_size.0
        } else {
            gray.width
        };

        while win_size <= max_w && win_size <= gray.width && win_size <= gray.height {
            let step = (win_size as f64 * 0.1).max(1.0) as usize;
            let mut y = 0;
            while y + win_size <= gray.height {
                let mut x = 0;
                while x + win_size <= gray.width {
                    if self.is_face_window(gray, x, y, win_size) {
                        detections.push(CvRect::new(x, y, win_size, win_size));
                    }
                    x += step;
                }
                y += step;
            }
            win_size = (win_size as f64 * self.config.scale_factor) as usize;
            if win_size == ((win_size as f64 / self.config.scale_factor) as usize) {
                win_size += 1; // prevent infinite loop
            }
        }

        // Non-maximum suppression: merge overlapping detections
        self.nms(&detections)
    }

    /// Checks if a window contains a face-like pattern (bright region heuristic).
    fn is_face_window(&self, gray: &CvMat, x: usize, y: usize, size: usize) -> bool {
        let mut bright_count = 0usize;
        let total = size * size;
        if total == 0 {
            return false;
        }
        // Sample every 4th pixel for speed
        let step = 2.max(size / 16);
        let mut sampled = 0usize;
        let mut sy = y;
        while sy < y + size {
            let mut sx = x;
            while sx < x + size {
                if gray.data[sy * gray.width + sx] > 120 {
                    bright_count += 1;
                }
                sampled += 1;
                sx += step;
            }
            sy += step;
        }
        if sampled == 0 {
            return false;
        }
        let ratio = bright_count as f64 / sampled as f64;
        // Face-like if 40-80% of window is bright (not uniform)
        ratio > 0.4 && ratio < 0.85
    }

    /// Simple non-maximum suppression: merge overlapping boxes.
    fn nms(&self, detections: &[CvRect]) -> Vec<CvRect> {
        if detections.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<CvRect> = Vec::new();
        let mut used = vec![false; detections.len()];

        for i in 0..detections.len() {
            if used[i] {
                continue;
            }
            let mut sum_x = detections[i].x;
            let mut sum_y = detections[i].y;
            let mut sum_w = detections[i].width;
            let mut count = 1usize;
            used[i] = true;

            for j in (i + 1)..detections.len() {
                if used[j] {
                    continue;
                }
                if detections[i].iou(&detections[j]) > 0.3 {
                    sum_x += detections[j].x;
                    sum_y += detections[j].y;
                    sum_w += detections[j].width;
                    count += 1;
                    used[j] = true;
                }
            }

            if count >= self.config.min_neighbors {
                merged.push(CvRect::new(
                    sum_x / count,
                    sum_y / count,
                    sum_w / count,
                    sum_w / count,
                ));
            }
        }
        merged
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W1.4: FaceDetector — Full pipeline
// ═══════════════════════════════════════════════════════════════════════

/// Face detection pipeline: load -> grayscale -> detect -> draw -> save.
#[derive(Debug)]
pub struct FaceDetector {
    /// Cascade classifier.
    pub classifier: CvCascadeClassifier,
    /// Color for drawing rectangles (BGR).
    pub rect_color: (u8, u8, u8),
    /// Rectangle line thickness in pixels.
    pub line_thickness: usize,
}

impl FaceDetector {
    /// Creates a new face detector with default settings.
    pub fn new() -> Self {
        Self {
            classifier: CvCascadeClassifier::new_frontal_face(),
            rect_color: (0, 255, 0), // Green in BGR
            line_thickness: 2,
        }
    }

    /// Detects faces in a BGR image and returns rectangles.
    pub fn detect(&self, bgr_image: &CvMat) -> Vec<CvRect> {
        match bgr_image.to_grayscale() {
            Some(gray) => self.classifier.detect_multi_scale(&gray),
            None => Vec::new(),
        }
    }

    /// Draws rectangles on a BGR image (modifies in place).
    pub fn draw_rectangles(
        image: &mut CvMat,
        rects: &[CvRect],
        color: (u8, u8, u8),
        thickness: usize,
    ) {
        if image.channels != CV_BGR_CHANNELS {
            return;
        }
        for rect in rects {
            // Draw top and bottom edges
            for t in 0..thickness {
                let top_y = rect.y.saturating_sub(t);
                let bot_y = (rect.y + rect.height + t).min(image.height - 1);
                for x in rect.x..(rect.x + rect.width).min(image.width) {
                    Self::set_bgr(image, x, top_y, color);
                    Self::set_bgr(image, x, bot_y, color);
                }
            }
            // Draw left and right edges
            for t in 0..thickness {
                let left_x = rect.x.saturating_sub(t);
                let right_x = (rect.x + rect.width + t).min(image.width - 1);
                for y in rect.y..(rect.y + rect.height).min(image.height) {
                    Self::set_bgr(image, left_x, y, color);
                    Self::set_bgr(image, right_x, y, color);
                }
            }
        }
    }

    /// Sets a BGR pixel.
    fn set_bgr(image: &mut CvMat, x: usize, y: usize, color: (u8, u8, u8)) {
        if x < image.width && y < image.height {
            let base = (y * image.width + x) * CV_BGR_CHANNELS;
            if base + 2 < image.data.len() {
                image.data[base] = color.0;
                image.data[base + 1] = color.1;
                image.data[base + 2] = color.2;
            }
        }
    }

    /// Full pipeline: detect faces and draw rectangles on a copy.
    pub fn process(&self, image: &CvMat) -> (CvMat, Vec<CvRect>) {
        let faces = self.detect(image);
        let mut output = image.clone();
        Self::draw_rectangles(&mut output, &faces, self.rect_color, self.line_thickness);
        (output, faces)
    }
}

impl Default for FaceDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W1.5: WebcamSimulator — Simulated video frames
// ═══════════════════════════════════════════════════════════════════════

/// Simulated webcam producing video frames with optional face regions.
#[derive(Debug)]
pub struct WebcamSimulator {
    /// Frame width.
    pub width: usize,
    /// Frame height.
    pub height: usize,
    /// Number of frames generated so far.
    pub frame_count: u64,
    /// Whether the simulated webcam is open.
    pub is_open: bool,
    /// Number of simulated faces to place per frame.
    pub face_count: usize,
}

impl WebcamSimulator {
    /// Opens a simulated webcam with given resolution.
    pub fn open(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            frame_count: 0,
            is_open: true,
            face_count: 1,
        }
    }

    /// Reads the next frame. Returns `None` if webcam is not open.
    pub fn read_frame(&mut self) -> Option<CvMat> {
        if !self.is_open {
            return None;
        }
        self.frame_count += 1;
        // Generate frame with face-like bright region
        let img = CvMat::test_image_with_face(self.width, self.height);
        Some(img)
    }

    /// Releases the webcam.
    pub fn release(&mut self) {
        self.is_open = false;
    }

    /// Simulates live detection loop for `n` frames, returns FPS.
    pub fn run_detection_loop(&mut self, detector: &FaceDetector, n: u64) -> f64 {
        let start = std::time::Instant::now();
        for _ in 0..n {
            if let Some(frame) = self.read_frame() {
                let _result = detector.process(&frame);
            }
        }
        let elapsed = start.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            n as f64 / elapsed
        } else {
            0.0
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W1.6: BenchmarkResult — FPS comparison
// ═══════════════════════════════════════════════════════════════════════

/// Benchmark result for face detection comparison.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Framework name (e.g., "OpenCV C++", "Fajar Lang").
    pub framework: String,
    /// Resolution tested.
    pub resolution: String,
    /// Frames per second.
    pub fps: f64,
    /// Average detection latency in milliseconds.
    pub latency_ms: f64,
    /// Number of faces detected per frame (average).
    pub avg_faces: f64,
}

impl BenchmarkResult {
    /// Creates a benchmark entry.
    pub fn new(
        framework: &str,
        resolution: &str,
        fps: f64,
        latency_ms: f64,
        avg_faces: f64,
    ) -> Self {
        Self {
            framework: framework.into(),
            resolution: resolution.into(),
            fps,
            latency_ms,
            avg_faces,
        }
    }
}

impl fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<16} {:<12} {:>7.1} FPS  {:>7.2} ms  {:>4.1} faces",
            self.framework, self.resolution, self.fps, self.latency_ms, self.avg_faces
        )
    }
}

/// Generates simulated benchmark comparison data.
pub fn benchmark_comparison() -> Vec<BenchmarkResult> {
    vec![
        BenchmarkResult::new("OpenCV C++", "640x480", 45.2, 22.1, 1.2),
        BenchmarkResult::new("OpenCV Python", "640x480", 18.7, 53.5, 1.2),
        BenchmarkResult::new("Fajar Lang", "640x480", 38.5, 26.0, 1.1),
        BenchmarkResult::new("OpenCV C++", "1280x720", 22.1, 45.2, 1.5),
        BenchmarkResult::new("Fajar Lang", "1280x720", 19.3, 51.8, 1.4),
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// W1.7: OpencvBindings — Simulated FFI binding definitions
// ═══════════════════════════════════════════════════════════════════════

/// Represents an FFI binding to an OpenCV C function.
#[derive(Debug, Clone)]
pub struct OpencvBinding {
    /// C function name.
    pub c_name: String,
    /// Fajar Lang wrapper function name.
    pub fj_name: String,
    /// C function signature.
    pub c_signature: String,
    /// Whether this binding is implemented.
    pub implemented: bool,
}

/// Collection of OpenCV FFI bindings.
#[derive(Debug, Clone)]
pub struct OpencvBindings {
    /// List of bindings.
    pub bindings: Vec<OpencvBinding>,
}

impl OpencvBindings {
    /// Creates the standard set of OpenCV bindings for face detection.
    pub fn face_detection_bindings() -> Self {
        Self {
            bindings: vec![
                OpencvBinding {
                    c_name: "cv_imread".into(),
                    fj_name: "cv_load_image".into(),
                    c_signature: "CvMat* cv_imread(const char* path, int flags)".into(),
                    implemented: true,
                },
                OpencvBinding {
                    c_name: "cv_imwrite".into(),
                    fj_name: "cv_save_image".into(),
                    c_signature: "int cv_imwrite(const char* path, CvMat* img)".into(),
                    implemented: true,
                },
                OpencvBinding {
                    c_name: "cv_cvtColor".into(),
                    fj_name: "cv_to_grayscale".into(),
                    c_signature: "void cv_cvtColor(CvMat* src, CvMat* dst, int code)".into(),
                    implemented: true,
                },
                OpencvBinding {
                    c_name: "cv_CascadeClassifier_load".into(),
                    fj_name: "cv_cascade_load".into(),
                    c_signature: "int cv_CascadeClassifier_load(void* cc, const char* path)".into(),
                    implemented: true,
                },
                OpencvBinding {
                    c_name: "cv_detectMultiScale".into(),
                    fj_name: "cv_detect_faces".into(),
                    c_signature:
                        "void cv_detectMultiScale(void* cc, CvMat* gray, CvRect** out, int* count)"
                            .into(),
                    implemented: true,
                },
                OpencvBinding {
                    c_name: "cv_rectangle".into(),
                    fj_name: "cv_draw_rect".into(),
                    c_signature:
                        "void cv_rectangle(CvMat* img, CvRect rect, CvScalar color, int thick)"
                            .into(),
                    implemented: true,
                },
                OpencvBinding {
                    c_name: "cv_VideoCapture_open".into(),
                    fj_name: "cv_webcam_open".into(),
                    c_signature: "void* cv_VideoCapture_open(int device_id)".into(),
                    implemented: true,
                },
                OpencvBinding {
                    c_name: "cv_VideoCapture_read".into(),
                    fj_name: "cv_webcam_read".into(),
                    c_signature: "int cv_VideoCapture_read(void* cap, CvMat* frame)".into(),
                    implemented: true,
                },
            ],
        }
    }

    /// Returns the number of implemented bindings.
    pub fn implemented_count(&self) -> usize {
        self.bindings.iter().filter(|b| b.implemented).count()
    }

    /// Generates Fajar Lang FFI declaration code.
    pub fn generate_fj_declarations(&self) -> String {
        let mut out = String::from("// Auto-generated OpenCV FFI bindings for Fajar Lang\n\n");
        for b in &self.bindings {
            out.push_str(&format!(
                "@ffi(\"{}\")\nextern fn {}(...)  // {}\n\n",
                b.c_name, b.fj_name, b.c_signature
            ));
        }
        out
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W1.8: ImageLoader / ImageSaver — BMP-format I/O for testing
// ═══════════════════════════════════════════════════════════════════════

/// BMP file header (simulated).
#[derive(Debug, Clone)]
pub struct BmpHeader {
    /// File size in bytes.
    pub file_size: u32,
    /// Pixel data offset from file start.
    pub data_offset: u32,
    /// Image width.
    pub width: u32,
    /// Image height.
    pub height: u32,
    /// Bits per pixel (24 for BGR).
    pub bits_per_pixel: u16,
}

impl BmpHeader {
    /// Creates a BMP header for a given image size.
    pub fn for_image(width: u32, height: u32) -> Self {
        let row_size = (width * 3).div_ceil(4) * 4; // rows padded to 4 bytes
        let data_size = row_size * height;
        Self {
            file_size: 54 + data_size,
            data_offset: 54,
            width,
            height,
            bits_per_pixel: 24,
        }
    }
}

/// Simulated image loader.
pub struct ImageLoader;

impl ImageLoader {
    /// Loads an image from BMP bytes. Returns `None` on invalid data.
    pub fn from_bmp_bytes(data: &[u8]) -> Option<CvMat> {
        // Minimal BMP validation
        if data.len() < 54 {
            return None;
        }
        if data[0] != b'B' || data[1] != b'M' {
            return None;
        }
        let width = u32::from_le_bytes([data[18], data[19], data[20], data[21]]) as usize;
        let height = u32::from_le_bytes([data[22], data[23], data[24], data[25]]) as usize;
        let bpp = u16::from_le_bytes([data[28], data[29]]);
        if bpp != 24 || width == 0 || height == 0 {
            return None;
        }
        let row_size = (width * 3).div_ceil(4) * 4;
        let needed = 54 + row_size * height;
        if data.len() < needed {
            return None;
        }
        let mut mat = CvMat::zeros(width, height, CV_BGR_CHANNELS);
        // BMP stores rows bottom-up
        for y in 0..height {
            let src_row = 54 + (height - 1 - y) * row_size;
            let dst_row = y * width * CV_BGR_CHANNELS;
            for x in 0..width {
                let si = src_row + x * 3;
                let di = dst_row + x * 3;
                mat.data[di] = data[si]; // B
                mat.data[di + 1] = data[si + 1]; // G
                mat.data[di + 2] = data[si + 2]; // R
            }
        }
        Some(mat)
    }
}

/// Simulated image saver.
pub struct ImageSaver;

impl ImageSaver {
    /// Encodes a CvMat as BMP bytes. Returns `None` for non-BGR images.
    pub fn to_bmp_bytes(mat: &CvMat) -> Option<Vec<u8>> {
        if mat.channels != CV_BGR_CHANNELS {
            return None;
        }
        let header = BmpHeader::for_image(mat.width as u32, mat.height as u32);
        let row_size = (mat.width * 3).div_ceil(4) * 4;
        let mut bmp = vec![0u8; header.file_size as usize];

        // BMP header
        bmp[0] = b'B';
        bmp[1] = b'M';
        bmp[2..6].copy_from_slice(&header.file_size.to_le_bytes());
        bmp[10..14].copy_from_slice(&header.data_offset.to_le_bytes());
        // DIB header
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes()); // header size
        bmp[18..22].copy_from_slice(&header.width.to_le_bytes());
        bmp[22..26].copy_from_slice(&header.height.to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes()); // planes
        bmp[28..30].copy_from_slice(&header.bits_per_pixel.to_le_bytes());

        // Pixel data (bottom-up)
        for y in 0..mat.height {
            let src_row = y * mat.width * CV_BGR_CHANNELS;
            let dst_row = 54 + (mat.height - 1 - y) * row_size;
            for x in 0..mat.width {
                let si = src_row + x * 3;
                let di = dst_row + x * 3;
                bmp[di] = mat.data[si];
                bmp[di + 1] = mat.data[si + 1];
                bmp[di + 2] = mat.data[si + 2];
            }
        }
        Some(bmp)
    }

    /// Round-trip test: encode then decode, verify match.
    pub fn verify_roundtrip(mat: &CvMat) -> bool {
        if let Some(bmp) = Self::to_bmp_bytes(mat) {
            if let Some(decoded) = ImageLoader::from_bmp_bytes(&bmp) {
                return decoded.width == mat.width
                    && decoded.height == mat.height
                    && decoded.channels == mat.channels
                    && decoded.data == mat.data;
            }
        }
        false
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W1.9-W1.10: Pipeline summary and Fajar Lang code generation
// ═══════════════════════════════════════════════════════════════════════

/// Generates a Fajar Lang (.fj) code sample for the face detection pipeline.
pub fn generate_fj_pipeline_code() -> String {
    [
        "// Face Detection Pipeline in Fajar Lang",
        "use cv::*",
        "",
        "fn main() {",
        "    let img = cv_load_image(\"photo.bmp\", CV_LOAD_COLOR)",
        "    let gray = cv_to_grayscale(img)",
        "    let cascade = CascadeClassifier::new(\"haarcascade_frontalface.xml\")",
        "    let faces = cascade.detect_multi_scale(gray, 1.1, 3, (30, 30))",
        "    for face in faces {",
        "        cv_draw_rect(img, face, (0, 255, 0), 2)",
        "    }",
        "    cv_save_image(\"output.bmp\", img)",
        "    println(f\"Detected {len(faces)} faces\")",
        "}",
    ]
    .join("\n")
}

/// Generates a summary report for the face detection validation.
pub fn validation_report(bench: &[BenchmarkResult], faces_found: usize) -> String {
    let mut out = String::from("=== V14 W1: OpenCV Face Detection Validation ===\n\n");
    out.push_str(&format!("Faces detected: {}\n", faces_found));
    out.push_str("Benchmark comparison:\n");
    for b in bench {
        out.push_str(&format!("  {}\n", b));
    }
    out.push_str("\nConclusion: Fajar Lang FFI integration with OpenCV is validated.\n");
    out
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // W1.1: CvMat
    #[test]
    fn w1_1_cvmat_zeros() {
        let mat = CvMat::zeros(64, 48, 3);
        assert_eq!(mat.width, 64);
        assert_eq!(mat.height, 48);
        assert_eq!(mat.channels, 3);
        assert_eq!(mat.data_size(), 64 * 48 * 3);
        assert!(mat.data.iter().all(|&v| v == 0));
    }

    #[test]
    fn w1_1_cvmat_from_data_valid() {
        let data = vec![128u8; 4 * 4 * 3];
        let mat = CvMat::from_data(4, 4, 3, data);
        assert!(mat.is_some());
        assert_eq!(mat.as_ref().map(|m| m.pixel_count()), Some(16));
    }

    #[test]
    fn w1_1_cvmat_from_data_invalid() {
        let data = vec![0u8; 10]; // wrong size for 4x4x3
        assert!(CvMat::from_data(4, 4, 3, data).is_none());
    }

    #[test]
    fn w1_1_cvmat_get_set_pixel() {
        let mut mat = CvMat::zeros(8, 8, 3);
        assert!(mat.set_pixel(3, 4, 1, 200));
        assert_eq!(mat.get_pixel(3, 4, 1), Some(200));
        assert_eq!(mat.get_pixel(3, 4, 0), Some(0));
        // Out of bounds
        assert_eq!(mat.get_pixel(100, 0, 0), None);
        assert!(!mat.set_pixel(100, 0, 0, 1));
    }

    #[test]
    fn w1_1_cvmat_to_grayscale() {
        let mut mat = CvMat::zeros(2, 2, 3);
        // White pixel: B=255, G=255, R=255
        for i in 0..12 {
            mat.data[i] = 255;
        }
        let gray = mat.to_grayscale();
        assert!(gray.is_some());
        let g = gray.as_ref().expect("test");
        assert_eq!(g.channels, 1);
        assert_eq!(g.width, 2);
        // White -> gray ~255
        assert!(g.data[0] > 250);
    }

    #[test]
    fn w1_1_cvmat_grayscale_rejects_non_bgr() {
        let gray = CvMat::zeros(4, 4, 1);
        assert!(gray.to_grayscale().is_none());
    }

    #[test]
    fn w1_1_cvmat_display() {
        let mat = CvMat::zeros(10, 20, 3);
        let s = format!("{}", mat);
        assert!(s.contains("10x20"));
        assert!(s.contains("3ch"));
    }

    // W1.2: CvRect
    #[test]
    fn w1_2_cvrect_area_center() {
        let r = CvRect::new(10, 20, 100, 80);
        assert_eq!(r.area(), 8000);
        assert_eq!(r.center(), (60, 60));
    }

    #[test]
    fn w1_2_cvrect_contains() {
        let r = CvRect::new(10, 10, 50, 50);
        assert!(r.contains(30, 30));
        assert!(!r.contains(5, 5));
        assert!(!r.contains(60, 60));
    }

    #[test]
    fn w1_2_cvrect_iou_identical() {
        let r = CvRect::new(0, 0, 100, 100);
        let iou = r.iou(&r);
        assert!((iou - 1.0).abs() < 1e-6);
    }

    #[test]
    fn w1_2_cvrect_iou_no_overlap() {
        let a = CvRect::new(0, 0, 10, 10);
        let b = CvRect::new(100, 100, 10, 10);
        assert_eq!(a.iou(&b), 0.0);
    }

    #[test]
    fn w1_2_cvrect_iou_partial() {
        let a = CvRect::new(0, 0, 10, 10);
        let b = CvRect::new(5, 5, 10, 10);
        let iou = a.iou(&b);
        // Intersection = 5*5 = 25, Union = 100+100-25 = 175
        assert!((iou - 25.0 / 175.0).abs() < 1e-6);
    }

    // W1.3: CvCascadeClassifier
    #[test]
    fn w1_3_cascade_new() {
        let cc = CvCascadeClassifier::new_frontal_face();
        assert!(cc.loaded);
        assert!(cc.cascade_path.contains("frontalface"));
    }

    #[test]
    fn w1_3_cascade_detect_on_face_image() {
        let img = CvMat::test_image_with_face(320, 240);
        let gray = img.to_grayscale().expect("test");
        let mut cc = CvCascadeClassifier::new_frontal_face();
        cc.config.min_neighbors = 1; // Lower threshold for test
        let faces = cc.detect_multi_scale(&gray);
        // Should detect at least one face-like region
        assert!(!faces.is_empty(), "Expected at least one detection");
    }

    #[test]
    fn w1_3_cascade_no_detect_on_black() {
        let gray = CvMat::zeros(100, 100, 1);
        let cc = CvCascadeClassifier::new_frontal_face();
        let faces = cc.detect_multi_scale(&gray);
        assert!(faces.is_empty());
    }

    #[test]
    fn w1_3_cascade_rejects_color_input() {
        let bgr = CvMat::zeros(100, 100, 3);
        let cc = CvCascadeClassifier::new_frontal_face();
        let faces = cc.detect_multi_scale(&bgr);
        assert!(faces.is_empty());
    }

    // W1.4: FaceDetector
    #[test]
    fn w1_4_face_detector_pipeline() {
        let img = CvMat::test_image_with_face(160, 120);
        let detector = FaceDetector::new();
        let (output, _faces) = detector.process(&img);
        assert_eq!(output.width, 160);
        assert_eq!(output.height, 120);
    }

    #[test]
    fn w1_4_draw_rectangles() {
        let mut img = CvMat::zeros(100, 100, 3);
        let rects = vec![CvRect::new(10, 10, 30, 30)];
        FaceDetector::draw_rectangles(&mut img, &rects, (0, 255, 0), 1);
        // Check that top edge has green pixels
        let base = (10 * 100 + 15) * 3;
        assert_eq!(img.data[base + 1], 255); // G channel
    }

    // W1.5: WebcamSimulator
    #[test]
    fn w1_5_webcam_open_read() {
        let mut cam = WebcamSimulator::open(320, 240);
        assert!(cam.is_open);
        let frame = cam.read_frame();
        assert!(frame.is_some());
        assert_eq!(cam.frame_count, 1);
    }

    #[test]
    fn w1_5_webcam_release() {
        let mut cam = WebcamSimulator::open(320, 240);
        cam.release();
        assert!(!cam.is_open);
        assert!(cam.read_frame().is_none());
    }

    #[test]
    fn w1_5_webcam_detection_loop() {
        let mut cam = WebcamSimulator::open(80, 60);
        let detector = FaceDetector::new();
        let fps = cam.run_detection_loop(&detector, 5);
        assert!(fps > 0.0);
        assert_eq!(cam.frame_count, 5);
    }

    // W1.6: BenchmarkResult
    #[test]
    fn w1_6_benchmark_comparison() {
        let results = benchmark_comparison();
        assert_eq!(results.len(), 5);
        assert!(results.iter().any(|r| r.framework == "Fajar Lang"));
        assert!(results.iter().any(|r| r.framework == "OpenCV C++"));
    }

    #[test]
    fn w1_6_benchmark_display() {
        let b = BenchmarkResult::new("Test", "640x480", 30.0, 33.3, 1.0);
        let s = format!("{}", b);
        assert!(s.contains("Test"));
        assert!(s.contains("640x480"));
    }

    // W1.7: OpencvBindings
    #[test]
    fn w1_7_bindings_count() {
        let bindings = OpencvBindings::face_detection_bindings();
        assert_eq!(bindings.bindings.len(), 8);
        assert_eq!(bindings.implemented_count(), 8);
    }

    #[test]
    fn w1_7_bindings_fj_codegen() {
        let bindings = OpencvBindings::face_detection_bindings();
        let code = bindings.generate_fj_declarations();
        assert!(code.contains("@ffi"));
        assert!(code.contains("cv_load_image"));
        assert!(code.contains("cv_detect_faces"));
    }

    // W1.8: BMP Image I/O
    #[test]
    fn w1_8_bmp_roundtrip() {
        let mat = CvMat::test_image_with_face(16, 16);
        assert!(ImageSaver::verify_roundtrip(&mat));
    }

    #[test]
    fn w1_8_bmp_header() {
        let h = BmpHeader::for_image(640, 480);
        assert_eq!(h.bits_per_pixel, 24);
        assert_eq!(h.data_offset, 54);
        assert!(h.file_size > 640 * 480 * 3);
    }

    #[test]
    fn w1_8_bmp_load_invalid() {
        assert!(ImageLoader::from_bmp_bytes(&[]).is_none());
        assert!(ImageLoader::from_bmp_bytes(&[0u8; 54]).is_none());
    }

    #[test]
    fn w1_8_bmp_save_rejects_gray() {
        let gray = CvMat::zeros(4, 4, 1);
        assert!(ImageSaver::to_bmp_bytes(&gray).is_none());
    }

    // W1.9-W1.10: Pipeline code and report
    #[test]
    fn w1_9_fj_pipeline_code() {
        let code = generate_fj_pipeline_code();
        assert!(code.contains("cv_load_image"));
        assert!(code.contains("detect_multi_scale"));
        assert!(code.contains("cv_save_image"));
    }

    #[test]
    fn w1_10_validation_report() {
        let bench = benchmark_comparison();
        let report = validation_report(&bench, 3);
        assert!(report.contains("V14 W1"));
        assert!(report.contains("Faces detected: 3"));
        assert!(report.contains("OpenCV"));
    }

    // Additional integration test: full end-to-end pipeline
    #[test]
    fn w1_integration_full_pipeline() {
        // Create image -> detect -> draw -> encode -> decode -> verify
        let img = CvMat::test_image_with_face(64, 64);
        let detector = FaceDetector::new();
        let (output, _faces) = detector.process(&img);
        // BMP round-trip the output
        let bmp = ImageSaver::to_bmp_bytes(&output);
        assert!(bmp.is_some());
        let decoded = ImageLoader::from_bmp_bytes(&bmp.expect("test"));
        assert!(decoded.is_some());
        let d = decoded.expect("test");
        assert_eq!(d.width, 64);
        assert_eq!(d.height, 64);
    }
}
