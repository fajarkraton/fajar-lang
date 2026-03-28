//! Layout engine for the Fajar Lang GUI toolkit.
//!
//! Provides flexbox layout, grid layout, stack layout, hit testing,
//! focus management, and animation primitives with real computation.

/// Direction of the main axis in a flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    /// Items laid out left-to-right.
    Row,
    /// Items laid out top-to-bottom.
    Column,
    /// Items laid out right-to-left.
    RowReverse,
    /// Items laid out bottom-to-top.
    ColumnReverse,
}

/// Whether flex items wrap to new lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexWrap {
    /// All items on one line — may overflow.
    NoWrap,
    /// Items wrap to additional lines.
    Wrap,
    /// Items wrap in reverse order.
    WrapReverse,
}

/// Alignment along the main or cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    /// Pack items toward the start of the axis.
    Start,
    /// Center items along the axis.
    Center,
    /// Pack items toward the end of the axis.
    End,
    /// Stretch items to fill the axis.
    Stretch,
    /// Distribute items with equal space between them.
    SpaceBetween,
    /// Distribute items with equal space around them.
    SpaceAround,
    /// Distribute items with equal space between and at edges.
    SpaceEvenly,
}

/// A size dimension that can be fixed, flexible, percentage-based, or automatic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    /// Fixed size in logical pixels.
    Fixed(f32),
    /// Flex factor — proportional share of remaining space.
    Flex(f32),
    /// Percentage of the parent container size.
    Percent(f32),
    /// Automatically sized based on content.
    Auto,
}

/// A rectangle in 2D space with position and dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// X coordinate of the top-left corner.
    pub x: f32,
    /// Y coordinate of the top-left corner.
    pub y: f32,
    /// Width of the rectangle.
    pub width: f32,
    /// Height of the rectangle.
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check whether a point (px, py) falls inside this rectangle.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}

/// Padding values for all four sides of a box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Padding {
    /// Padding above the content.
    pub top: f32,
    /// Padding to the right of the content.
    pub right: f32,
    /// Padding below the content.
    pub bottom: f32,
    /// Padding to the left of the content.
    pub left: f32,
}

impl Padding {
    /// Create padding with the same value on all four sides.
    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create padding with vertical and horizontal values.
    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Create padding with explicit values for each side.
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Total horizontal padding (left + right).
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Total vertical padding (top + bottom).
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

impl Default for Padding {
    fn default() -> Self {
        Self::uniform(0.0)
    }
}

/// Constraints that limit how a layout box may be sized.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutConstraints {
    /// Minimum allowed width (0.0 means no minimum).
    pub min_width: f32,
    /// Maximum allowed width (`f32::INFINITY` means no maximum).
    pub max_width: f32,
    /// Minimum allowed height.
    pub min_height: f32,
    /// Maximum allowed height.
    pub max_height: f32,
    /// Optional aspect ratio (width / height). `None` means unconstrained.
    pub aspect_ratio: Option<f32>,
}

impl Default for LayoutConstraints {
    fn default() -> Self {
        Self {
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
            aspect_ratio: None,
        }
    }
}

impl LayoutConstraints {
    /// Clamp a width value to the allowed range.
    pub fn clamp_width(&self, width: f32) -> f32 {
        width.max(self.min_width).min(self.max_width)
    }

    /// Clamp a height value to the allowed range.
    pub fn clamp_height(&self, height: f32) -> f32 {
        height.max(self.min_height).min(self.max_height)
    }
}

/// A box participating in layout, with size preferences and margin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutBox {
    /// Preferred width of the box.
    pub preferred_width: Size,
    /// Preferred height of the box.
    pub preferred_height: Size,
    /// Constraints limiting the final size.
    pub constraints: LayoutConstraints,
    /// Margin around the box (top, right, bottom, left).
    pub margin: Padding,
}

impl Default for LayoutBox {
    fn default() -> Self {
        Self {
            preferred_width: Size::Auto,
            preferred_height: Size::Auto,
            constraints: LayoutConstraints::default(),
            margin: Padding::default(),
        }
    }
}

impl LayoutBox {
    /// Resolve the preferred width to a concrete pixel value given a container width.
    pub fn resolve_width(&self, container_width: f32) -> f32 {
        let raw = match self.preferred_width {
            Size::Fixed(px) => px,
            Size::Percent(pct) => container_width * pct / 100.0,
            Size::Flex(_) | Size::Auto => 0.0,
        };
        self.constraints.clamp_width(raw)
    }

    /// Resolve the preferred height to a concrete pixel value given a container height.
    pub fn resolve_height(&self, container_height: f32) -> f32 {
        let raw = match self.preferred_height {
            Size::Fixed(px) => px,
            Size::Percent(pct) => container_height * pct / 100.0,
            Size::Flex(_) | Size::Auto => 0.0,
        };
        self.constraints.clamp_height(raw)
    }

    /// Return the flex factor for width, or 0.0 if not a flex size.
    pub fn flex_factor_width(&self) -> f32 {
        match self.preferred_width {
            Size::Flex(f) => f,
            _ => 0.0,
        }
    }

    /// Return the flex factor for height, or 0.0 if not a flex size.
    pub fn flex_factor_height(&self) -> f32 {
        match self.preferred_height {
            Size::Flex(f) => f,
            _ => 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// FlexLayout
// ---------------------------------------------------------------------------

/// Flexbox-style layout engine.
///
/// Computes positions and sizes for a list of children inside a container
/// rectangle, following CSS Flexbox semantics (direction, wrapping,
/// justify-content, align-items, and gap).
#[derive(Debug, Clone)]
pub struct FlexLayout {
    /// Main axis direction.
    pub direction: FlexDirection,
    /// Wrapping behaviour.
    pub wrap: FlexWrap,
    /// Alignment along the main axis (justify-content).
    pub justify: Alignment,
    /// Alignment along the cross axis (align-items).
    pub align_items: Alignment,
    /// Gap between items in logical pixels.
    pub gap: f32,
    /// Padding inside the container.
    pub padding: Padding,
}

impl Default for FlexLayout {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            wrap: FlexWrap::NoWrap,
            justify: Alignment::Start,
            align_items: Alignment::Stretch,
            gap: 0.0,
            padding: Padding::default(),
        }
    }
}

impl FlexLayout {
    /// Compute the position and size of each child within `container`.
    ///
    /// Returns one `Rect` per child, in the same order as the input slice.
    pub fn compute(&self, children: &[LayoutBox], container: Rect) -> Vec<Rect> {
        if children.is_empty() {
            return Vec::new();
        }

        let is_row = matches!(
            self.direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
        let is_reversed = matches!(
            self.direction,
            FlexDirection::RowReverse | FlexDirection::ColumnReverse
        );

        // Inner container after padding
        let inner_x = container.x + self.padding.left;
        let inner_y = container.y + self.padding.top;
        let inner_w = (container.width - self.padding.horizontal()).max(0.0);
        let inner_h = (container.height - self.padding.vertical()).max(0.0);

        let main_size = if is_row { inner_w } else { inner_h };
        let cross_size = if is_row { inner_h } else { inner_w };

        // Resolve each child's base main-axis and cross-axis sizes
        let mut main_sizes: Vec<f32> = Vec::with_capacity(children.len());
        let mut cross_sizes: Vec<f32> = Vec::with_capacity(children.len());
        let mut flex_factors: Vec<f32> = Vec::with_capacity(children.len());

        for child in children {
            let (ms, cs, ff) = if is_row {
                (
                    child.resolve_width(inner_w),
                    child.resolve_height(inner_h),
                    child.flex_factor_width(),
                )
            } else {
                (
                    child.resolve_height(inner_h),
                    child.resolve_width(inner_w),
                    child.flex_factor_height(),
                )
            };
            main_sizes.push(ms);
            cross_sizes.push(cs);
            flex_factors.push(ff);
        }

        // Distribute flex space
        let total_gap = if children.len() > 1 {
            self.gap * (children.len() as f32 - 1.0)
        } else {
            0.0
        };
        let fixed_total: f32 = main_sizes
            .iter()
            .zip(flex_factors.iter())
            .map(|(&s, &f)| if f > 0.0 { 0.0 } else { s })
            .sum();
        let remaining = (main_size - fixed_total - total_gap).max(0.0);
        let total_flex: f32 = flex_factors.iter().sum();

        if total_flex > 0.0 {
            for i in 0..children.len() {
                if flex_factors[i] > 0.0 {
                    main_sizes[i] = remaining * flex_factors[i] / total_flex;
                    // Clamp flex-assigned sizes
                    if is_row {
                        main_sizes[i] = children[i].constraints.clamp_width(main_sizes[i]);
                    } else {
                        main_sizes[i] = children[i].constraints.clamp_height(main_sizes[i]);
                    }
                }
            }
        }

        // Cross-axis: if Stretch, fill the cross dimension
        for cs in &mut cross_sizes {
            if self.align_items == Alignment::Stretch && *cs <= 0.0 {
                *cs = cross_size;
            }
        }

        // Compute main-axis starting offset based on justify
        let used_main: f32 = main_sizes.iter().sum::<f32>() + total_gap;
        let free_main = (main_size - used_main).max(0.0);

        let (mut main_offset, item_spacing) = match self.justify {
            Alignment::Start | Alignment::Stretch => (0.0_f32, 0.0_f32),
            Alignment::End => (free_main, 0.0),
            Alignment::Center => (free_main / 2.0, 0.0),
            Alignment::SpaceBetween => {
                if children.len() > 1 {
                    (0.0, free_main / (children.len() as f32 - 1.0))
                } else {
                    (0.0, 0.0)
                }
            }
            Alignment::SpaceAround => {
                let s = free_main / children.len() as f32;
                (s / 2.0, s)
            }
            Alignment::SpaceEvenly => {
                let s = free_main / (children.len() as f32 + 1.0);
                (s, s)
            }
        };

        // Build result rects
        let mut results = Vec::with_capacity(children.len());

        let order: Vec<usize> = if is_reversed {
            (0..children.len()).rev().collect()
        } else {
            (0..children.len()).collect()
        };

        // We compute positions in forward order along the main axis,
        // but map them to the original child index for the result.
        let mut rects_in_order: Vec<(usize, Rect)> = Vec::with_capacity(children.len());

        for (seq, &idx) in order.iter().enumerate() {
            let ms = main_sizes[idx];
            let cs = cross_sizes[idx];
            let margin = &children[idx].margin;

            // Cross-axis alignment offset
            let cross_offset = match self.align_items {
                Alignment::Start => 0.0,
                Alignment::End => (cross_size - cs).max(0.0),
                Alignment::Center => ((cross_size - cs) / 2.0).max(0.0),
                _ => 0.0, // Stretch already handled, SpaceBetween/Around/Evenly less common on cross axis
            };

            let (x, y, w, h) = if is_row {
                (
                    inner_x + main_offset + margin.left,
                    inner_y + cross_offset + margin.top,
                    ms,
                    cs,
                )
            } else {
                (
                    inner_x + cross_offset + margin.left,
                    inner_y + main_offset + margin.top,
                    cs,
                    ms,
                )
            };

            rects_in_order.push((idx, Rect::new(x, y, w, h)));

            main_offset += ms + self.gap + item_spacing;
            if seq < order.len() - 1 {
                // gap already added above
            }
        }

        // Sort back to original child order
        rects_in_order.sort_by_key(|(idx, _)| *idx);
        for (_, rect) in rects_in_order {
            results.push(rect);
        }

        results
    }
}

// ---------------------------------------------------------------------------
// GridLayout
// ---------------------------------------------------------------------------

/// Grid-based layout engine.
///
/// Divides a container into rows and columns of configurable sizes,
/// then places children left-to-right, top-to-bottom into cells.
#[derive(Debug, Clone)]
pub struct GridLayout {
    /// Column size definitions.
    pub columns: Vec<Size>,
    /// Row size definitions.
    pub rows: Vec<Size>,
    /// Gap between cells in logical pixels.
    pub gap: f32,
    /// Padding inside the container.
    pub padding: Padding,
}

impl Default for GridLayout {
    fn default() -> Self {
        Self {
            columns: vec![Size::Flex(1.0)],
            rows: vec![Size::Flex(1.0)],
            gap: 0.0,
            padding: Padding::default(),
        }
    }
}

impl GridLayout {
    /// Resolve a list of `Size` values into concrete pixel widths/heights.
    fn resolve_track_sizes(tracks: &[Size], available: f32, gap: f32) -> Vec<f32> {
        if tracks.is_empty() {
            return Vec::new();
        }
        let total_gap = if tracks.len() > 1 {
            gap * (tracks.len() as f32 - 1.0)
        } else {
            0.0
        };
        let space = (available - total_gap).max(0.0);

        let mut sizes = vec![0.0_f32; tracks.len()];
        let mut fixed_total = 0.0_f32;
        let mut flex_total = 0.0_f32;

        for (i, track) in tracks.iter().enumerate() {
            match *track {
                Size::Fixed(px) => {
                    sizes[i] = px;
                    fixed_total += px;
                }
                Size::Percent(pct) => {
                    let px = space * pct / 100.0;
                    sizes[i] = px;
                    fixed_total += px;
                }
                Size::Flex(f) => {
                    flex_total += f;
                }
                Size::Auto => {
                    // Auto in grid defaults to equal share (treated as Flex(1))
                    flex_total += 1.0;
                }
            }
        }

        let remaining = (space - fixed_total).max(0.0);
        if flex_total > 0.0 {
            for (i, track) in tracks.iter().enumerate() {
                match *track {
                    Size::Flex(f) => sizes[i] = remaining * f / flex_total,
                    Size::Auto => sizes[i] = remaining / flex_total,
                    _ => {}
                }
            }
        }

        sizes
    }

    /// Compute the position and size of each child cell within `container`.
    ///
    /// Children are placed in row-major order (left-to-right, then next row).
    /// Returns one `Rect` per child, up to `children_count` cells.
    pub fn compute(&self, children_count: usize, container: Rect) -> Vec<Rect> {
        if children_count == 0 || self.columns.is_empty() || self.rows.is_empty() {
            return Vec::new();
        }

        let inner_x = container.x + self.padding.left;
        let inner_y = container.y + self.padding.top;
        let inner_w = (container.width - self.padding.horizontal()).max(0.0);
        let inner_h = (container.height - self.padding.vertical()).max(0.0);

        let col_sizes = Self::resolve_track_sizes(&self.columns, inner_w, self.gap);
        let row_sizes = Self::resolve_track_sizes(&self.rows, inner_h, self.gap);

        let num_cols = col_sizes.len();
        let num_rows = row_sizes.len();

        // Precompute column x-offsets
        let mut col_offsets = Vec::with_capacity(num_cols);
        let mut cx = 0.0_f32;
        for (i, &cw) in col_sizes.iter().enumerate() {
            col_offsets.push(cx);
            cx += cw;
            if i < num_cols - 1 {
                cx += self.gap;
            }
        }

        // Precompute row y-offsets
        let mut row_offsets = Vec::with_capacity(num_rows);
        let mut ry = 0.0_f32;
        for (i, &rh) in row_sizes.iter().enumerate() {
            row_offsets.push(ry);
            ry += rh;
            if i < num_rows - 1 {
                ry += self.gap;
            }
        }

        let mut results = Vec::with_capacity(children_count);

        for idx in 0..children_count {
            let col = idx % num_cols;
            let row = idx / num_cols;

            if row >= num_rows {
                // Exceeded defined rows — place at last row offset with zero height
                results.push(Rect::new(
                    inner_x + col_offsets.get(col).copied().unwrap_or(0.0),
                    inner_y + row_offsets.last().copied().unwrap_or(0.0),
                    col_sizes.get(col).copied().unwrap_or(0.0),
                    0.0,
                ));
            } else {
                results.push(Rect::new(
                    inner_x + col_offsets[col],
                    inner_y + row_offsets[row],
                    col_sizes[col],
                    row_sizes[row],
                ));
            }
        }

        results
    }
}

// ---------------------------------------------------------------------------
// StackLayout
// ---------------------------------------------------------------------------

/// Stack layout — all children occupy the full container (z-order stacking).
///
/// Each child receives the same rectangle as the container minus padding.
/// Later children are drawn on top of earlier ones.
#[derive(Debug, Clone, Default)]
pub struct StackLayout {
    /// Padding inside the container.
    pub padding: Padding,
}

impl StackLayout {
    /// Compute positions for stacked children. Every child fills the container.
    pub fn compute(&self, children_count: usize, container: Rect) -> Vec<Rect> {
        let inner = Rect::new(
            container.x + self.padding.left,
            container.y + self.padding.top,
            (container.width - self.padding.horizontal()).max(0.0),
            (container.height - self.padding.vertical()).max(0.0),
        );
        vec![inner; children_count]
    }
}

// ---------------------------------------------------------------------------
// HitTest
// ---------------------------------------------------------------------------

/// Perform a hit test against a list of widget rectangles.
///
/// Returns the **last** (topmost in z-order) widget whose rectangle contains
/// the point `(x, y)`, or `None` if no widget is hit.
pub fn hit_test(widgets: &[(Rect, usize)], x: f32, y: f32) -> Option<usize> {
    // Iterate in reverse to find the topmost (last-drawn) widget first.
    for &(ref rect, id) in widgets.iter().rev() {
        if rect.contains(x, y) {
            return Some(id);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// FocusManager
// ---------------------------------------------------------------------------

/// Manages keyboard focus across a set of focusable widgets.
///
/// Maintains a tab order and tracks which widget currently has focus.
#[derive(Debug, Clone)]
pub struct FocusManager {
    /// Index into `tab_order` of the currently focused widget, or `None`.
    pub focused_index: Option<usize>,
    /// Ordered list of widget IDs that can receive focus.
    pub tab_order: Vec<usize>,
}

impl FocusManager {
    /// Create a new focus manager with the given tab order.
    pub fn new(tab_order: Vec<usize>) -> Self {
        Self {
            focused_index: None,
            tab_order,
        }
    }

    /// Return the widget ID that currently has focus, if any.
    pub fn focused_widget(&self) -> Option<usize> {
        self.focused_index
            .and_then(|idx| self.tab_order.get(idx).copied())
    }

    /// Move focus to the next widget in the tab order.
    ///
    /// Wraps around to the first widget after the last.
    pub fn focus_next(&mut self) {
        if self.tab_order.is_empty() {
            self.focused_index = None;
            return;
        }
        self.focused_index = Some(match self.focused_index {
            Some(idx) => (idx + 1) % self.tab_order.len(),
            None => 0,
        });
    }

    /// Move focus to the previous widget in the tab order.
    ///
    /// Wraps around to the last widget before the first.
    pub fn focus_prev(&mut self) {
        if self.tab_order.is_empty() {
            self.focused_index = None;
            return;
        }
        self.focused_index = Some(match self.focused_index {
            Some(0) => self.tab_order.len() - 1,
            Some(idx) => idx - 1,
            None => self.tab_order.len() - 1,
        });
    }

    /// Set focus to a specific widget ID. Returns `true` if found in tab order.
    pub fn focus_widget(&mut self, widget_id: usize) -> bool {
        if let Some(pos) = self.tab_order.iter().position(|&id| id == widget_id) {
            self.focused_index = Some(pos);
            true
        } else {
            false
        }
    }

    /// Remove focus from the current widget.
    pub fn blur(&mut self) {
        self.focused_index = None;
    }
}

// ---------------------------------------------------------------------------
// Animation & Easing
// ---------------------------------------------------------------------------

/// Easing function for animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    /// Constant speed from start to end.
    Linear,
    /// Starts slow and accelerates (quadratic).
    EaseIn,
    /// Starts fast and decelerates (quadratic).
    EaseOut,
    /// Slow start, fast middle, slow end (quadratic).
    EaseInOut,
}

impl Easing {
    /// Apply the easing function to a normalized time `t` in `[0.0, 1.0]`.
    ///
    /// Returns an eased value in `[0.0, 1.0]`.
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => t * (2.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

/// A simple animation that interpolates between two values over time.
#[derive(Debug, Clone)]
pub struct Animation {
    /// Starting value.
    pub from: f32,
    /// Target value.
    pub to: f32,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Elapsed time in milliseconds.
    pub elapsed_ms: u64,
    /// Easing curve to use.
    pub easing: Easing,
}

impl Animation {
    /// Create a new animation.
    pub fn new(from: f32, to: f32, duration_ms: u64, easing: Easing) -> Self {
        Self {
            from,
            to,
            duration_ms,
            elapsed_ms: 0,
            easing,
        }
    }

    /// Compute the current interpolated value based on elapsed time and easing.
    pub fn progress(&self) -> f32 {
        if self.duration_ms == 0 {
            return self.to;
        }
        let t = (self.elapsed_ms as f32 / self.duration_ms as f32).clamp(0.0, 1.0);
        let eased = self.easing.apply(t);
        self.from + (self.to - self.from) * eased
    }

    /// Whether the animation has completed (elapsed >= duration).
    pub fn is_complete(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }

    /// Advance the animation by `delta_ms` milliseconds.
    pub fn advance(&mut self, delta_ms: u64) {
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        if self.elapsed_ms > self.duration_ms {
            self.elapsed_ms = self.duration_ms;
        }
    }

    /// Reset the animation to the beginning.
    pub fn reset(&mut self) {
        self.elapsed_ms = 0;
    }
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

    fn container() -> Rect {
        Rect::new(0.0, 0.0, 400.0, 300.0)
    }

    // -- FlexLayout Row ---------------------------------------------------

    #[test]
    fn flex_row_fixed_children() {
        let layout = FlexLayout {
            direction: FlexDirection::Row,
            ..FlexLayout::default()
        };
        let children = vec![
            LayoutBox {
                preferred_width: Size::Fixed(100.0),
                preferred_height: Size::Fixed(50.0),
                ..LayoutBox::default()
            },
            LayoutBox {
                preferred_width: Size::Fixed(150.0),
                preferred_height: Size::Fixed(50.0),
                ..LayoutBox::default()
            },
        ];
        let rects = layout.compute(&children, container());
        assert_eq!(rects.len(), 2);
        assert!(approx_eq(rects[0].x, 0.0));
        assert!(approx_eq(rects[0].width, 100.0));
        assert!(approx_eq(rects[1].x, 100.0));
        assert!(approx_eq(rects[1].width, 150.0));
    }

    #[test]
    fn flex_row_flex_children_distribute_space() {
        let layout = FlexLayout {
            direction: FlexDirection::Row,
            ..FlexLayout::default()
        };
        let children = vec![
            LayoutBox {
                preferred_width: Size::Flex(1.0),
                preferred_height: Size::Fixed(50.0),
                ..LayoutBox::default()
            },
            LayoutBox {
                preferred_width: Size::Flex(3.0),
                preferred_height: Size::Fixed(50.0),
                ..LayoutBox::default()
            },
        ];
        let rects = layout.compute(&children, container());
        assert!(approx_eq(rects[0].width, 100.0)); // 1/4 of 400
        assert!(approx_eq(rects[1].width, 300.0)); // 3/4 of 400
    }

    #[test]
    fn flex_row_gap() {
        let layout = FlexLayout {
            direction: FlexDirection::Row,
            gap: 10.0,
            ..FlexLayout::default()
        };
        let children = vec![
            LayoutBox {
                preferred_width: Size::Fixed(100.0),
                preferred_height: Size::Fixed(50.0),
                ..LayoutBox::default()
            },
            LayoutBox {
                preferred_width: Size::Fixed(100.0),
                preferred_height: Size::Fixed(50.0),
                ..LayoutBox::default()
            },
        ];
        let rects = layout.compute(&children, container());
        assert!(approx_eq(rects[1].x, 110.0)); // 100 + 10 gap
    }

    #[test]
    fn flex_row_justify_center() {
        let layout = FlexLayout {
            direction: FlexDirection::Row,
            justify: Alignment::Center,
            ..FlexLayout::default()
        };
        let children = vec![LayoutBox {
            preferred_width: Size::Fixed(100.0),
            preferred_height: Size::Fixed(50.0),
            ..LayoutBox::default()
        }];
        let rects = layout.compute(&children, container());
        // 400 - 100 = 300 free, centered => offset 150
        assert!(approx_eq(rects[0].x, 150.0));
    }

    #[test]
    fn flex_row_justify_space_between() {
        let layout = FlexLayout {
            direction: FlexDirection::Row,
            justify: Alignment::SpaceBetween,
            ..FlexLayout::default()
        };
        let children = vec![
            LayoutBox {
                preferred_width: Size::Fixed(50.0),
                preferred_height: Size::Fixed(50.0),
                ..LayoutBox::default()
            },
            LayoutBox {
                preferred_width: Size::Fixed(50.0),
                preferred_height: Size::Fixed(50.0),
                ..LayoutBox::default()
            },
            LayoutBox {
                preferred_width: Size::Fixed(50.0),
                preferred_height: Size::Fixed(50.0),
                ..LayoutBox::default()
            },
        ];
        let rects = layout.compute(&children, container());
        // 400 - 150 = 250 free, 2 gaps => 125 each
        assert!(approx_eq(rects[0].x, 0.0));
        assert!(approx_eq(rects[1].x, 175.0));
        assert!(approx_eq(rects[2].x, 350.0));
    }

    // -- FlexLayout Column ------------------------------------------------

    #[test]
    fn flex_column_fixed_children() {
        let layout = FlexLayout {
            direction: FlexDirection::Column,
            ..FlexLayout::default()
        };
        let children = vec![
            LayoutBox {
                preferred_width: Size::Fixed(100.0),
                preferred_height: Size::Fixed(80.0),
                ..LayoutBox::default()
            },
            LayoutBox {
                preferred_width: Size::Fixed(100.0),
                preferred_height: Size::Fixed(60.0),
                ..LayoutBox::default()
            },
        ];
        let rects = layout.compute(&children, container());
        assert!(approx_eq(rects[0].y, 0.0));
        assert!(approx_eq(rects[0].height, 80.0));
        assert!(approx_eq(rects[1].y, 80.0));
        assert!(approx_eq(rects[1].height, 60.0));
    }

    #[test]
    fn flex_column_flex_children() {
        let layout = FlexLayout {
            direction: FlexDirection::Column,
            ..FlexLayout::default()
        };
        let children = vec![
            LayoutBox {
                preferred_width: Size::Fixed(100.0),
                preferred_height: Size::Flex(1.0),
                ..LayoutBox::default()
            },
            LayoutBox {
                preferred_width: Size::Fixed(100.0),
                preferred_height: Size::Flex(2.0),
                ..LayoutBox::default()
            },
        ];
        let rects = layout.compute(&children, container());
        assert!(approx_eq(rects[0].height, 100.0)); // 1/3 of 300
        assert!(approx_eq(rects[1].height, 200.0)); // 2/3 of 300
    }

    // -- GridLayout -------------------------------------------------------

    #[test]
    fn grid_2x2_equal() {
        let grid = GridLayout {
            columns: vec![Size::Flex(1.0), Size::Flex(1.0)],
            rows: vec![Size::Flex(1.0), Size::Flex(1.0)],
            gap: 0.0,
            padding: Padding::default(),
        };
        let rects = grid.compute(4, container());
        assert_eq!(rects.len(), 4);
        assert!(approx_eq(rects[0].width, 200.0));
        assert!(approx_eq(rects[0].height, 150.0));
        assert!(approx_eq(rects[1].x, 200.0));
        assert!(approx_eq(rects[2].y, 150.0));
        assert!(approx_eq(rects[3].x, 200.0));
        assert!(approx_eq(rects[3].y, 150.0));
    }

    #[test]
    fn grid_with_gap() {
        let grid = GridLayout {
            columns: vec![Size::Flex(1.0), Size::Flex(1.0)],
            rows: vec![Size::Flex(1.0)],
            gap: 20.0,
            padding: Padding::default(),
        };
        let rects = grid.compute(2, container());
        // 400 - 20 gap = 380, half = 190 each
        assert!(approx_eq(rects[0].width, 190.0));
        assert!(approx_eq(rects[1].x, 210.0)); // 190 + 20 gap
        assert!(approx_eq(rects[1].width, 190.0));
    }

    #[test]
    fn grid_fixed_and_flex_columns() {
        let grid = GridLayout {
            columns: vec![Size::Fixed(100.0), Size::Flex(1.0)],
            rows: vec![Size::Flex(1.0)],
            gap: 0.0,
            padding: Padding::default(),
        };
        let rects = grid.compute(2, container());
        assert!(approx_eq(rects[0].width, 100.0));
        assert!(approx_eq(rects[1].width, 300.0));
        assert!(approx_eq(rects[1].x, 100.0));
    }

    // -- StackLayout ------------------------------------------------------

    #[test]
    fn stack_fills_container() {
        let stack = StackLayout::default();
        let rects = stack.compute(3, container());
        assert_eq!(rects.len(), 3);
        for r in &rects {
            assert!(approx_eq(r.x, 0.0));
            assert!(approx_eq(r.y, 0.0));
            assert!(approx_eq(r.width, 400.0));
            assert!(approx_eq(r.height, 300.0));
        }
    }

    #[test]
    fn stack_with_padding() {
        let stack = StackLayout {
            padding: Padding::uniform(10.0),
        };
        let rects = stack.compute(1, container());
        assert!(approx_eq(rects[0].x, 10.0));
        assert!(approx_eq(rects[0].y, 10.0));
        assert!(approx_eq(rects[0].width, 380.0));
        assert!(approx_eq(rects[0].height, 280.0));
    }

    // -- HitTest ----------------------------------------------------------

    #[test]
    fn hit_test_finds_correct_widget() {
        let widgets = vec![
            (Rect::new(0.0, 0.0, 100.0, 100.0), 0),
            (Rect::new(100.0, 0.0, 100.0, 100.0), 1),
            (Rect::new(200.0, 0.0, 100.0, 100.0), 2),
        ];
        assert_eq!(hit_test(&widgets, 50.0, 50.0), Some(0));
        assert_eq!(hit_test(&widgets, 150.0, 50.0), Some(1));
        assert_eq!(hit_test(&widgets, 250.0, 50.0), Some(2));
    }

    #[test]
    fn hit_test_returns_none_outside() {
        let widgets = vec![(Rect::new(0.0, 0.0, 100.0, 100.0), 0)];
        assert_eq!(hit_test(&widgets, 200.0, 200.0), None);
    }

    #[test]
    fn hit_test_topmost_wins() {
        // Overlapping widgets — last one (topmost) wins
        let widgets = vec![
            (Rect::new(0.0, 0.0, 100.0, 100.0), 10),
            (Rect::new(0.0, 0.0, 100.0, 100.0), 20),
        ];
        assert_eq!(hit_test(&widgets, 50.0, 50.0), Some(20));
    }

    // -- FocusManager -----------------------------------------------------

    #[test]
    fn focus_next_cycles() {
        let mut fm = FocusManager::new(vec![10, 20, 30]);
        assert_eq!(fm.focused_widget(), None);
        fm.focus_next();
        assert_eq!(fm.focused_widget(), Some(10));
        fm.focus_next();
        assert_eq!(fm.focused_widget(), Some(20));
        fm.focus_next();
        assert_eq!(fm.focused_widget(), Some(30));
        fm.focus_next(); // wrap
        assert_eq!(fm.focused_widget(), Some(10));
    }

    #[test]
    fn focus_prev_cycles() {
        let mut fm = FocusManager::new(vec![10, 20, 30]);
        fm.focus_prev(); // none -> last
        assert_eq!(fm.focused_widget(), Some(30));
        fm.focus_prev();
        assert_eq!(fm.focused_widget(), Some(20));
        fm.focus_prev();
        assert_eq!(fm.focused_widget(), Some(10));
        fm.focus_prev(); // wrap
        assert_eq!(fm.focused_widget(), Some(30));
    }

    #[test]
    fn focus_specific_widget() {
        let mut fm = FocusManager::new(vec![10, 20, 30]);
        assert!(fm.focus_widget(20));
        assert_eq!(fm.focused_widget(), Some(20));
        assert!(!fm.focus_widget(99)); // not in tab order
    }

    #[test]
    fn focus_empty_tab_order() {
        let mut fm = FocusManager::new(vec![]);
        fm.focus_next();
        assert_eq!(fm.focused_widget(), None);
        fm.focus_prev();
        assert_eq!(fm.focused_widget(), None);
    }

    // -- Animation & Easing -----------------------------------------------

    #[test]
    fn animation_progress_at_boundaries() {
        let anim = Animation::new(0.0, 100.0, 1000, Easing::Linear);
        assert!(approx_eq(anim.progress(), 0.0)); // 0% elapsed

        let mut anim_mid = Animation::new(0.0, 100.0, 1000, Easing::Linear);
        anim_mid.elapsed_ms = 500;
        assert!(approx_eq(anim_mid.progress(), 50.0)); // 50%

        let mut anim_end = Animation::new(0.0, 100.0, 1000, Easing::Linear);
        anim_end.elapsed_ms = 1000;
        assert!(approx_eq(anim_end.progress(), 100.0)); // 100%
    }

    #[test]
    fn animation_is_complete() {
        let mut anim = Animation::new(0.0, 100.0, 500, Easing::Linear);
        assert!(!anim.is_complete());
        anim.elapsed_ms = 499;
        assert!(!anim.is_complete());
        anim.elapsed_ms = 500;
        assert!(anim.is_complete());
    }

    #[test]
    fn animation_advance_and_reset() {
        let mut anim = Animation::new(0.0, 100.0, 1000, Easing::Linear);
        anim.advance(300);
        assert_eq!(anim.elapsed_ms, 300);
        anim.advance(900); // should cap at 1000
        assert_eq!(anim.elapsed_ms, 1000);
        anim.reset();
        assert_eq!(anim.elapsed_ms, 0);
    }

    #[test]
    fn easing_linear() {
        assert!(approx_eq(Easing::Linear.apply(0.0), 0.0));
        assert!(approx_eq(Easing::Linear.apply(0.5), 0.5));
        assert!(approx_eq(Easing::Linear.apply(1.0), 1.0));
    }

    #[test]
    fn easing_ease_in() {
        assert!(approx_eq(Easing::EaseIn.apply(0.0), 0.0));
        assert!(approx_eq(Easing::EaseIn.apply(0.5), 0.25)); // 0.5^2
        assert!(approx_eq(Easing::EaseIn.apply(1.0), 1.0));
    }

    #[test]
    fn easing_ease_out() {
        assert!(approx_eq(Easing::EaseOut.apply(0.0), 0.0));
        assert!(approx_eq(Easing::EaseOut.apply(0.5), 0.75)); // 0.5*(2-0.5)
        assert!(approx_eq(Easing::EaseOut.apply(1.0), 1.0));
    }

    #[test]
    fn easing_ease_in_out() {
        assert!(approx_eq(Easing::EaseInOut.apply(0.0), 0.0));
        assert!(approx_eq(Easing::EaseInOut.apply(0.5), 0.5));
        assert!(approx_eq(Easing::EaseInOut.apply(1.0), 1.0));
        // First half accelerates
        let q = Easing::EaseInOut.apply(0.25);
        assert!(q < 0.25); // slower start
    }

    #[test]
    fn animation_ease_in_progress() {
        let mut anim = Animation::new(0.0, 100.0, 1000, Easing::EaseIn);
        anim.elapsed_ms = 500;
        // EaseIn at t=0.5 => 0.25, so progress = 25.0
        assert!(approx_eq(anim.progress(), 25.0));
    }

    // -- Padding ----------------------------------------------------------

    #[test]
    fn padding_uniform() {
        let p = Padding::uniform(10.0);
        assert!(approx_eq(p.top, 10.0));
        assert!(approx_eq(p.right, 10.0));
        assert!(approx_eq(p.bottom, 10.0));
        assert!(approx_eq(p.left, 10.0));
        assert!(approx_eq(p.horizontal(), 20.0));
        assert!(approx_eq(p.vertical(), 20.0));
    }

    #[test]
    fn padding_symmetric() {
        let p = Padding::symmetric(5.0, 15.0);
        assert!(approx_eq(p.top, 5.0));
        assert!(approx_eq(p.bottom, 5.0));
        assert!(approx_eq(p.right, 15.0));
        assert!(approx_eq(p.left, 15.0));
    }

    // -- Rect -------------------------------------------------------------

    #[test]
    fn rect_contains_point() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains(50.0, 40.0)); // inside
        assert!(r.contains(10.0, 20.0)); // top-left edge
        assert!(r.contains(110.0, 70.0)); // bottom-right edge
        assert!(!r.contains(5.0, 40.0)); // outside left
        assert!(!r.contains(50.0, 75.0)); // outside bottom
    }

    // -- Empty cases ------------------------------------------------------

    #[test]
    fn flex_empty_children() {
        let layout = FlexLayout::default();
        let rects = layout.compute(&[], container());
        assert!(rects.is_empty());
    }

    #[test]
    fn grid_zero_children() {
        let grid = GridLayout::default();
        let rects = grid.compute(0, container());
        assert!(rects.is_empty());
    }
}
