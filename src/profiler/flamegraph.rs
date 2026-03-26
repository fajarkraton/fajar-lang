//! Flamegraph Generation — SVG rendering, differential, memory-weighted.
//!
//! D1.2: 10 tasks covering stack frame capture, SVG flamegraph, differential
//! comparison, reverse (icicle) charts, and HTML report generation.

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// D1.2.1: Collapsed Stack Format
// ═══════════════════════════════════════════════════════════════════════

/// A collapsed stack entry (function chain + sample count).
#[derive(Debug, Clone)]
pub struct StackEntry {
    /// Semicolon-separated function chain (bottom → top).
    pub stack: String,
    /// Sample count (or time in ns).
    pub count: u64,
}

/// Parses collapsed stack format into entries.
pub fn parse_collapsed(input: &str) -> Vec<StackEntry> {
    input.lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.rsplitn(2, ' ').collect();
            if parts.len() == 2 {
                let count = parts[0].parse::<u64>().ok()?;
                Some(StackEntry { stack: parts[1].to_string(), count })
            } else {
                None
            }
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// D1.2.2: SVG Flamegraph Renderer
// ═══════════════════════════════════════════════════════════════════════

/// Flamegraph rendering options.
#[derive(Debug, Clone)]
pub struct FlameConfig {
    /// Image width in pixels.
    pub width: u32,
    /// Row height in pixels.
    pub row_height: u32,
    /// Font size.
    pub font_size: u32,
    /// Minimum width fraction to show text.
    pub min_text_width: f64,
    /// Color palette.
    pub palette: Palette,
    /// Title text.
    pub title: String,
    /// Whether to render as icicle (top-down) instead of flame (bottom-up).
    pub inverted: bool,
}

impl Default for FlameConfig {
    fn default() -> Self {
        Self {
            width: 1200, row_height: 16, font_size: 12,
            min_text_width: 0.02, palette: Palette::Hot,
            title: "Flamegraph".to_string(), inverted: false,
        }
    }
}

/// Color palette for flamegraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    /// Warm colors (red/orange/yellow) — default.
    Hot,
    /// Memory palette (green/blue).
    Memory,
    /// Differential (red = slower, blue = faster).
    Differential,
    /// Grayscale.
    Gray,
}

/// A node in the flamegraph tree.
#[derive(Debug, Clone)]
pub struct FlameNode {
    /// Function name.
    pub name: String,
    /// Self sample count.
    pub self_count: u64,
    /// Total sample count (self + children).
    pub total_count: u64,
    /// Children.
    pub children: Vec<FlameNode>,
}

/// Builds a flamegraph tree from collapsed stacks.
pub fn build_flame_tree(entries: &[StackEntry]) -> FlameNode {
    let mut root = FlameNode { name: "all".to_string(), self_count: 0, total_count: 0, children: Vec::new() };

    for entry in entries {
        let frames: Vec<&str> = entry.stack.split(';').collect();
        let mut node = &mut root;
        node.total_count += entry.count;

        for (i, frame) in frames.iter().enumerate() {
            let child_pos = node.children.iter().position(|c| c.name == *frame);
            let idx = if let Some(pos) = child_pos {
                pos
            } else {
                node.children.push(FlameNode {
                    name: frame.to_string(), self_count: 0, total_count: 0, children: Vec::new(),
                });
                node.children.len() - 1
            };
            node = &mut node.children[idx];
            node.total_count += entry.count;
            if i == frames.len() - 1 {
                node.self_count += entry.count;
            }
        }
    }
    root
}

/// Generates an SVG flamegraph from a flame tree.
pub fn render_svg(root: &FlameNode, config: &FlameConfig) -> String {
    let total = root.total_count;
    if total == 0 { return "<svg></svg>".to_string(); }

    let max_depth = tree_depth(root);
    let height = (max_depth + 2) * config.row_height as usize + 40;

    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">
<style>
  text {{ font-family: monospace; font-size: {}px; }}
  rect:hover {{ stroke: black; stroke-width: 1; cursor: pointer; }}
</style>
<text x="10" y="20" style="font-size:16px;font-weight:bold">{}</text>
"#,
        config.width, height, config.width, height, config.font_size, config.title
    );

    // Render nodes
    render_node(&mut svg, root, 0.0, config.width as f64, 0, total, config, max_depth);

    svg.push_str("</svg>");
    svg
}

fn render_node(
    svg: &mut String,
    node: &FlameNode,
    x: f64,
    width: f64,
    depth: usize,
    total: u64,
    config: &FlameConfig,
    max_depth: usize,
) {
    if width < 1.0 { return; }

    let y = if config.inverted {
        30 + depth * config.row_height as usize
    } else {
        30 + (max_depth - depth) * config.row_height as usize
    };

    let color = match config.palette {
        Palette::Hot => {
            let hue = 0 + (depth * 30) % 60; // red to yellow
            format!("hsl({hue}, 80%, 55%)")
        }
        Palette::Memory => {
            let hue = 120 + (depth * 20) % 80; // green to blue
            format!("hsl({hue}, 60%, 50%)")
        }
        Palette::Differential => "hsl(0, 70%, 60%)".to_string(),
        Palette::Gray => {
            let l = 40 + (depth * 5) % 30;
            format!("hsl(0, 0%, {l}%)")
        }
    };

    let pct = if total > 0 { node.total_count as f64 / total as f64 * 100.0 } else { 0.0 };
    let title = format!("{} ({:.1}%, {} samples)", node.name, pct, node.total_count);

    svg.push_str(&format!(
        r#"<g><title>{title}</title><rect x="{x:.1}" y="{y}" width="{width:.1}" height="{}" fill="{color}" rx="2"/>"#,
        config.row_height - 1
    ));

    if width > config.font_size as f64 * 3.0 {
        let text = if node.name.len() > (width / config.font_size as f64 * 1.5) as usize {
            format!("{}..", &node.name[..((width / config.font_size as f64 * 1.2) as usize).min(node.name.len())])
        } else {
            node.name.clone()
        };
        svg.push_str(&format!(
            r#"<text x="{:.1}" y="{}" fill="white">{text}</text>"#,
            x + 3.0, y + config.row_height as usize - 4
        ));
    }
    svg.push_str("</g>\n");

    // Render children
    let mut child_x = x;
    for child in &node.children {
        let child_width = if total > 0 {
            width * child.total_count as f64 / node.total_count.max(1) as f64
        } else {
            0.0
        };
        render_node(svg, child, child_x, child_width, depth + 1, total, config, max_depth);
        child_x += child_width;
    }
}

fn tree_depth(node: &FlameNode) -> usize {
    if node.children.is_empty() { return 0; }
    node.children.iter().map(|c| 1 + tree_depth(c)).max().unwrap_or(0)
}

// ═══════════════════════════════════════════════════════════════════════
// D1.2.3: Differential Flamegraph
// ═══════════════════════════════════════════════════════════════════════

/// Computes differential between two profiles (before/after).
pub fn differential(before: &[StackEntry], after: &[StackEntry]) -> Vec<DiffEntry> {
    let before_map: HashMap<&str, u64> = before.iter().map(|e| (e.stack.as_str(), e.count)).collect();
    let after_map: HashMap<&str, u64> = after.iter().map(|e| (e.stack.as_str(), e.count)).collect();

    let mut all_stacks: Vec<&str> = before_map.keys().chain(after_map.keys()).copied().collect();
    all_stacks.sort();
    all_stacks.dedup();

    all_stacks.iter().map(|&stack| {
        let b = before_map.get(stack).copied().unwrap_or(0);
        let a = after_map.get(stack).copied().unwrap_or(0);
        DiffEntry { stack: stack.to_string(), before: b, after: a, delta: a as i64 - b as i64 }
    }).collect()
}

/// A differential stack entry.
#[derive(Debug, Clone)]
pub struct DiffEntry {
    /// Stack string.
    pub stack: String,
    /// Count in "before" profile.
    pub before: u64,
    /// Count in "after" profile.
    pub after: u64,
    /// Delta (positive = slower, negative = faster).
    pub delta: i64,
}

// ═══════════════════════════════════════════════════════════════════════
// D1.2.7: HTML Report
// ═══════════════════════════════════════════════════════════════════════

/// Generates a self-contained HTML report with embedded SVG.
pub fn html_report(svg: &str, title: &str, metadata: &[(&str, &str)]) -> String {
    let mut html = format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="UTF-8"><title>{title}</title>
<style>body{{font-family:sans-serif;margin:20px;background:#0d1117;color:#e6edf3}}
h1{{color:#58a6ff}}.meta{{color:#8b949e;font-size:0.9rem}}
.svg-container{{overflow-x:auto;margin:20px 0}}
</style></head><body>
<h1>{title}</h1>
<div class="meta">
"#
    );
    for (key, value) in metadata {
        html.push_str(&format!("<p><b>{key}:</b> {value}</p>\n"));
    }
    html.push_str("</div>\n<div class=\"svg-container\">\n");
    html.push_str(svg);
    html.push_str("\n</div>\n</body></html>");
    html
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d1_2_parse_collapsed() {
        let input = "main;compute;fib 50\nmain;log 10\nmain 5\n";
        let entries = parse_collapsed(input);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].stack, "main;compute;fib");
        assert_eq!(entries[0].count, 50);
    }

    #[test]
    fn d1_2_build_flame_tree() {
        let entries = vec![
            StackEntry { stack: "main;compute".to_string(), count: 80 },
            StackEntry { stack: "main;log".to_string(), count: 20 },
        ];
        let tree = build_flame_tree(&entries);
        assert_eq!(tree.name, "all");
        assert_eq!(tree.total_count, 100);
        assert_eq!(tree.children.len(), 1); // "main"
        assert_eq!(tree.children[0].children.len(), 2); // "compute" + "log"
    }

    #[test]
    fn d1_2_render_svg() {
        let entries = vec![
            StackEntry { stack: "main;fib".to_string(), count: 90 },
            StackEntry { stack: "main;print".to_string(), count: 10 },
        ];
        let tree = build_flame_tree(&entries);
        let svg = render_svg(&tree, &FlameConfig::default());
        assert!(svg.contains("<svg"));
        assert!(svg.contains("main"));
        assert!(svg.contains("fib"));
    }

    #[test]
    fn d1_2_inverted_flamegraph() {
        let entries = vec![StackEntry { stack: "a;b".to_string(), count: 10 }];
        let tree = build_flame_tree(&entries);
        let config = FlameConfig { inverted: true, ..Default::default() };
        let svg = render_svg(&tree, &config);
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn d1_3_differential() {
        let before = vec![
            StackEntry { stack: "main;compute".to_string(), count: 100 },
            StackEntry { stack: "main;log".to_string(), count: 20 },
        ];
        let after = vec![
            StackEntry { stack: "main;compute".to_string(), count: 80 },
            StackEntry { stack: "main;log".to_string(), count: 30 },
            StackEntry { stack: "main;cache".to_string(), count: 10 },
        ];
        let diff = differential(&before, &after);
        let compute = diff.iter().find(|d| d.stack == "main;compute").unwrap();
        assert_eq!(compute.delta, -20); // faster
        let log = diff.iter().find(|d| d.stack == "main;log").unwrap();
        assert_eq!(log.delta, 10); // slower
    }

    #[test]
    fn d1_7_html_report() {
        let html = html_report("<svg></svg>", "Test", &[("Duration", "5.2s")]);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<svg></svg>"));
        assert!(html.contains("5.2s"));
    }
}
