//! Device profiler & visualizer — per-device timing, roofline model, flame graph SVG,
//! Chrome Trace export, throughput metrics, memory watermark, and comparison mode.

use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

use super::dispatch::DispatchDevice;

// ═══════════════════════════════════════════════════════════════════════
// S36.1: Per-Device Timer
// ═══════════════════════════════════════════════════════════════════════

/// A timing entry for a device execution.
#[derive(Debug, Clone)]
pub struct DeviceTimerEntry {
    /// Device that executed.
    pub device: DispatchDevice,
    /// Subgraph or operation name.
    pub label: String,
    /// Wall-clock duration in microseconds.
    pub duration_us: u64,
    /// Start offset from profiler start (microseconds).
    pub start_us: u64,
}

/// Per-device timer that records execution times.
#[derive(Debug)]
pub struct DeviceTimer {
    /// Profiler start time.
    start: Instant,
    /// Active timers (label -> start time).
    active: HashMap<String, Instant>,
    /// Completed entries.
    entries: Vec<DeviceTimerEntry>,
}

impl DeviceTimer {
    /// Creates a new device timer.
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            active: HashMap::new(),
            entries: Vec::new(),
        }
    }

    /// Starts timing an operation on a device.
    pub fn start(&mut self, label: &str) {
        self.active.insert(label.to_string(), Instant::now());
    }

    /// Stops timing an operation and records the result.
    pub fn stop(&mut self, label: &str, device: DispatchDevice) -> Option<u64> {
        let start_instant = self.active.remove(label)?;
        let elapsed = start_instant.elapsed();
        let duration_us = elapsed.as_micros() as u64;
        let start_us = (start_instant - self.start).as_micros() as u64;

        self.entries.push(DeviceTimerEntry {
            device,
            label: label.to_string(),
            duration_us,
            start_us,
        });

        Some(duration_us)
    }

    /// Records a pre-measured timing entry.
    pub fn record(&mut self, device: DispatchDevice, label: &str, duration_us: u64) {
        let start_us = self.start.elapsed().as_micros() as u64;
        self.entries.push(DeviceTimerEntry {
            device,
            label: label.to_string(),
            duration_us,
            start_us,
        });
    }

    /// Returns all timing entries.
    pub fn entries(&self) -> &[DeviceTimerEntry] {
        &self.entries
    }

    /// Returns entries grouped by device.
    pub fn by_device(&self) -> HashMap<String, Vec<&DeviceTimerEntry>> {
        let mut grouped: HashMap<String, Vec<&DeviceTimerEntry>> = HashMap::new();
        for entry in &self.entries {
            grouped
                .entry(format!("{}", entry.device))
                .or_default()
                .push(entry);
        }
        grouped
    }

    /// Returns total time per device.
    pub fn total_per_device(&self) -> HashMap<String, u64> {
        let mut totals: HashMap<String, u64> = HashMap::new();
        for entry in &self.entries {
            *totals.entry(format!("{}", entry.device)).or_insert(0) += entry.duration_us;
        }
        totals
    }

    /// Formats a per-device timing breakdown.
    pub fn format_breakdown(&self) -> String {
        let totals = self.total_per_device();
        let grand_total: u64 = totals.values().sum();

        let mut lines = vec!["=== Per-Device Timing Breakdown ===".to_string()];
        let mut sorted: Vec<_> = totals.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        for (device, total_us) in sorted {
            let pct = if grand_total > 0 {
                (*total_us as f64 / grand_total as f64) * 100.0
            } else {
                0.0
            };
            lines.push(format!("  {device}: {total_us}us ({pct:.1}%)"));
        }
        lines.push(format!("  Total: {grand_total}us"));
        lines.join("\n")
    }
}

impl Default for DeviceTimer {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S36.2: Memory Transfer Overhead
// ═══════════════════════════════════════════════════════════════════════

/// Memory transfer record.
#[derive(Debug, Clone)]
pub struct TransferRecord {
    /// Source device.
    pub src: String,
    /// Destination device.
    pub dst: String,
    /// Bytes transferred.
    pub bytes: u64,
    /// Transfer time in microseconds.
    pub duration_us: u64,
}

impl TransferRecord {
    /// Returns the effective bandwidth in GB/s.
    pub fn bandwidth_gbps(&self) -> f64 {
        if self.duration_us == 0 {
            return 0.0;
        }
        (self.bytes as f64 / 1e9) / (self.duration_us as f64 / 1e6)
    }
}

/// Tracks memory transfer overhead.
#[derive(Debug, Clone, Default)]
pub struct TransferTracker {
    /// All recorded transfers.
    records: Vec<TransferRecord>,
}

impl TransferTracker {
    /// Creates a new transfer tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a transfer.
    pub fn record(&mut self, src: &str, dst: &str, bytes: u64, duration_us: u64) {
        self.records.push(TransferRecord {
            src: src.to_string(),
            dst: dst.to_string(),
            bytes,
            duration_us,
        });
    }

    /// Returns total bytes transferred.
    pub fn total_bytes(&self) -> u64 {
        self.records.iter().map(|r| r.bytes).sum()
    }

    /// Returns total transfer time.
    pub fn total_time_us(&self) -> u64 {
        self.records.iter().map(|r| r.duration_us).sum()
    }

    /// Returns all records.
    pub fn records(&self) -> &[TransferRecord] {
        &self.records
    }

    /// Formats a transfer overhead report.
    pub fn format_report(&self) -> String {
        let mut lines = vec!["=== Memory Transfer Overhead ===".to_string()];
        for r in &self.records {
            lines.push(format!(
                "  {} -> {}: {}B in {}us ({:.2} GB/s)",
                r.src,
                r.dst,
                r.bytes,
                r.duration_us,
                r.bandwidth_gbps()
            ));
        }
        lines.push(format!(
            "  Total: {}B transferred in {}us",
            self.total_bytes(),
            self.total_time_us()
        ));
        lines.join("\n")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S36.3: Roofline Model
// ═══════════════════════════════════════════════════════════════════════

/// Roofline model for a device.
#[derive(Debug, Clone)]
pub struct RooflineModel {
    /// Device name.
    pub device: String,
    /// Peak compute (GFLOPS).
    pub peak_gflops: f64,
    /// Peak memory bandwidth (GB/s).
    pub peak_bw_gbps: f64,
}

impl RooflineModel {
    /// Creates a new roofline model.
    pub fn new(device: &str, peak_gflops: f64, peak_bw_gbps: f64) -> Self {
        Self {
            device: device.to_string(),
            peak_gflops,
            peak_bw_gbps,
        }
    }

    /// Returns the ridge point (operational intensity where compute meets bandwidth).
    pub fn ridge_point(&self) -> f64 {
        if self.peak_bw_gbps == 0.0 {
            return 0.0;
        }
        self.peak_gflops / self.peak_bw_gbps
    }

    /// Returns the achievable GFLOPS for a given operational intensity.
    pub fn achievable_gflops(&self, operational_intensity: f64) -> f64 {
        let bw_bound = operational_intensity * self.peak_bw_gbps;
        bw_bound.min(self.peak_gflops)
    }

    /// Determines the bottleneck for a given workload.
    pub fn bottleneck(&self, operational_intensity: f64) -> RooflineBottleneck {
        if operational_intensity < self.ridge_point() {
            RooflineBottleneck::MemoryBound
        } else {
            RooflineBottleneck::ComputeBound
        }
    }

    /// Returns the efficiency (actual / achievable).
    pub fn efficiency(&self, actual_gflops: f64, operational_intensity: f64) -> f64 {
        let achievable = self.achievable_gflops(operational_intensity);
        if achievable == 0.0 {
            return 0.0;
        }
        (actual_gflops / achievable).min(1.0)
    }
}

/// Roofline bottleneck type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RooflineBottleneck {
    /// Workload is limited by memory bandwidth.
    MemoryBound,
    /// Workload is limited by compute throughput.
    ComputeBound,
}

impl fmt::Display for RooflineBottleneck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MemoryBound => write!(f, "memory-bound"),
            Self::ComputeBound => write!(f, "compute-bound"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S36.4: Flame Graph SVG Output
// ═══════════════════════════════════════════════════════════════════════

/// A flame graph frame.
#[derive(Debug, Clone)]
pub struct FlameFrame {
    /// Function/operation name.
    pub name: String,
    /// Device.
    pub device: String,
    /// Start offset in microseconds.
    pub start_us: u64,
    /// Duration in microseconds.
    pub duration_us: u64,
    /// Stack depth.
    pub depth: u32,
}

/// Generates a flame graph SVG from profiling data.
pub fn generate_flame_svg(frames: &[FlameFrame], title: &str) -> String {
    let total_width = 1200.0_f64;
    let row_height = 20.0;
    let max_depth = frames.iter().map(|f| f.depth).max().unwrap_or(0) as f64;
    let total_height = (max_depth + 2.0) * row_height + 40.0;
    let total_us = frames
        .iter()
        .map(|f| f.start_us + f.duration_us)
        .max()
        .unwrap_or(1);

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{total_width}" height="{total_height}">"#
    ));
    svg.push_str(&format!(
        r#"<text x="600" y="20" text-anchor="middle" font-size="14" font-weight="bold">{title}</text>"#
    ));

    // Color palette by device
    for frame in frames {
        let x = (frame.start_us as f64 / total_us as f64) * total_width;
        let w = (frame.duration_us as f64 / total_us as f64) * total_width;
        let y = total_height - (frame.depth as f64 + 1.0) * row_height - 10.0;

        let color = match frame.device.as_str() {
            "CPU" => "#a0c4ff",
            d if d.starts_with("GPU") => "#ffc6ff",
            d if d.starts_with("NPU") => "#caffbf",
            _ => "#dddddd",
        };

        svg.push_str(&format!(
            "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{}\" fill=\"{color}\" stroke=\"#333\" stroke-width=\"0.5\">",
            row_height - 1.0
        ));
        svg.push_str(&format!(
            "<title>{} ({}) {}us</title>",
            frame.name, frame.device, frame.duration_us
        ));
        svg.push_str("</rect>");

        // Label if wide enough
        if w > 40.0 {
            let label = if frame.name.len() > 20 {
                format!("{}...", &frame.name[..17])
            } else {
                frame.name.clone()
            };
            svg.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"10\" fill=\"#333\">{label}</text>",
                x + 2.0,
                y + row_height - 5.0
            ));
        }
    }

    svg.push_str("</svg>");
    svg
}

// ═══════════════════════════════════════════════════════════════════════
// S36.5: CLI Profiler Output
// ═══════════════════════════════════════════════════════════════════════

/// Formats CLI profiler output for stdout.
pub fn format_cli_profile(
    timer: &DeviceTimer,
    transfers: &TransferTracker,
    throughput: &ThroughputMetrics,
) -> String {
    let mut output = String::new();
    output.push_str(&timer.format_breakdown());
    output.push('\n');
    output.push_str(&transfers.format_report());
    output.push('\n');
    output.push_str(&throughput.format_report());
    output
}

// ═══════════════════════════════════════════════════════════════════════
// S36.6: Chrome Trace Export
// ═══════════════════════════════════════════════════════════════════════

/// Generates Chrome Trace Event format JSON.
///
/// Compatible with chrome://tracing and Perfetto.
pub fn generate_chrome_trace(entries: &[DeviceTimerEntry]) -> String {
    let mut events: Vec<String> = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        let pid = match entry.device {
            DispatchDevice::Cpu => 0,
            DispatchDevice::Gpu(id) => 1 + id,
            DispatchDevice::Npu(id) => 100 + id,
        };
        let tid = 0;
        let device_str = format!("{}", entry.device);

        // Begin event
        events.push(format!(
            r#"  {{"name":"{}","cat":"{device_str}","ph":"X","ts":{},"dur":{},"pid":{},"tid":{}}}"#,
            entry.label, entry.start_us, entry.duration_us, pid, tid
        ));

        // Process name metadata
        if i == 0 || (i > 0 && entries[i - 1].device != entry.device) {
            events.push(format!(
                r#"  {{"name":"process_name","ph":"M","pid":{},"args":{{"name":"{}"}}}}"#,
                pid, entry.device
            ));
        }
    }

    format!("{{\n  \"traceEvents\": [\n{}\n  ]\n}}", events.join(",\n"))
}

// ═══════════════════════════════════════════════════════════════════════
// S36.7: Throughput Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Throughput measurements for common workloads.
#[derive(Debug, Clone, Default)]
pub struct ThroughputMetrics {
    /// Inferences per second.
    pub inferences_per_sec: f64,
    /// Tokens per second (for language models).
    pub tokens_per_sec: f64,
    /// Samples per second (for training).
    pub samples_per_sec: f64,
    /// Total wall-clock time in microseconds.
    pub total_time_us: u64,
    /// Number of iterations.
    pub iterations: u64,
}

impl ThroughputMetrics {
    /// Creates metrics from total time and count.
    pub fn from_timing(total_time_us: u64, count: u64) -> Self {
        let secs = total_time_us as f64 / 1_000_000.0;
        let per_sec = if secs > 0.0 { count as f64 / secs } else { 0.0 };

        Self {
            inferences_per_sec: per_sec,
            tokens_per_sec: 0.0,
            samples_per_sec: 0.0,
            total_time_us,
            iterations: count,
        }
    }

    /// Sets the tokens/second metric.
    pub fn with_tokens(mut self, tokens_per_sec: f64) -> Self {
        self.tokens_per_sec = tokens_per_sec;
        self
    }

    /// Sets the samples/second metric.
    pub fn with_samples(mut self, samples_per_sec: f64) -> Self {
        self.samples_per_sec = samples_per_sec;
        self
    }

    /// Returns the average latency per inference in microseconds.
    pub fn avg_latency_us(&self) -> u64 {
        if self.iterations == 0 {
            0
        } else {
            self.total_time_us / self.iterations
        }
    }

    /// Formats a throughput report.
    pub fn format_report(&self) -> String {
        let mut lines = vec!["=== Throughput Metrics ===".to_string()];
        lines.push(format!("  Inferences/sec: {:.1}", self.inferences_per_sec));
        if self.tokens_per_sec > 0.0 {
            lines.push(format!("  Tokens/sec: {:.1}", self.tokens_per_sec));
        }
        if self.samples_per_sec > 0.0 {
            lines.push(format!("  Samples/sec: {:.1}", self.samples_per_sec));
        }
        lines.push(format!("  Avg latency: {}us", self.avg_latency_us()));
        lines.push(format!(
            "  Total time: {:.3}ms",
            self.total_time_us as f64 / 1000.0
        ));
        lines.join("\n")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S36.8: Memory Watermark
// ═══════════════════════════════════════════════════════════════════════

/// Peak memory usage tracker per device.
#[derive(Debug, Clone, Default)]
pub struct MemoryWatermark {
    /// Peak memory per device (device_name -> peak_bytes).
    peaks: HashMap<String, u64>,
    /// Current memory per device.
    current: HashMap<String, u64>,
}

impl MemoryWatermark {
    /// Creates a new memory watermark tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a memory allocation on a device.
    pub fn record_alloc(&mut self, device: &str, bytes: u64) {
        let current = self.current.entry(device.to_string()).or_insert(0);
        *current += bytes;
        let peak = self.peaks.entry(device.to_string()).or_insert(0);
        *peak = (*peak).max(*current);
    }

    /// Records a memory free on a device.
    pub fn record_free(&mut self, device: &str, bytes: u64) {
        let current = self.current.entry(device.to_string()).or_insert(0);
        *current = current.saturating_sub(bytes);
    }

    /// Returns peak memory for a device.
    pub fn peak(&self, device: &str) -> u64 {
        self.peaks.get(device).copied().unwrap_or(0)
    }

    /// Returns current memory for a device.
    pub fn current(&self, device: &str) -> u64 {
        self.current.get(device).copied().unwrap_or(0)
    }

    /// Returns all peak measurements.
    pub fn all_peaks(&self) -> &HashMap<String, u64> {
        &self.peaks
    }

    /// Formats a memory watermark report.
    pub fn format_report(&self) -> String {
        let mut lines = vec!["=== Memory Watermark (Peak) ===".to_string()];
        let mut sorted: Vec<_> = self.peaks.iter().collect();
        sorted.sort_by_key(|(name, _)| (*name).clone());
        for (device, peak) in sorted {
            let current = self.current.get(device).copied().unwrap_or(0);
            lines.push(format!(
                "  {device}: peak={:.2}MB, current={:.2}MB",
                *peak as f64 / (1024.0 * 1024.0),
                current as f64 / (1024.0 * 1024.0)
            ));
        }
        lines.join("\n")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S36.9: Comparison Mode
// ═══════════════════════════════════════════════════════════════════════

/// Comparison result between two device runs.
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    /// Device A name.
    pub device_a: String,
    /// Device B name.
    pub device_b: String,
    /// Device A total time in microseconds.
    pub time_a_us: u64,
    /// Device B total time in microseconds.
    pub time_b_us: u64,
    /// Speedup of faster over slower.
    pub speedup: f64,
    /// Which device was faster.
    pub winner: String,
}

impl ComparisonResult {
    /// Creates a comparison between two device timings.
    pub fn compare(device_a: &str, time_a_us: u64, device_b: &str, time_b_us: u64) -> Self {
        let (speedup, winner) = if time_a_us <= time_b_us {
            if time_a_us == 0 {
                (0.0, device_a.to_string())
            } else {
                (time_b_us as f64 / time_a_us as f64, device_a.to_string())
            }
        } else if time_b_us == 0 {
            (0.0, device_b.to_string())
        } else {
            (time_a_us as f64 / time_b_us as f64, device_b.to_string())
        };

        Self {
            device_a: device_a.to_string(),
            device_b: device_b.to_string(),
            time_a_us,
            time_b_us,
            speedup,
            winner,
        }
    }

    /// Formats a side-by-side comparison.
    pub fn format_comparison(&self) -> String {
        let mut lines = vec![format!("=== {} vs {} ===", self.device_a, self.device_b)];
        lines.push(format!(
            "  {}: {:.3}ms",
            self.device_a,
            self.time_a_us as f64 / 1000.0
        ));
        lines.push(format!(
            "  {}: {:.3}ms",
            self.device_b,
            self.time_b_us as f64 / 1000.0
        ));
        lines.push(format!(
            "  Winner: {} ({:.2}x speedup)",
            self.winner, self.speedup
        ));
        lines.join("\n")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S36.1: Per-device timer
    #[test]
    fn s36_1_device_timer() {
        let mut timer = DeviceTimer::new();
        timer.record(DispatchDevice::Cpu, "matmul", 100);
        timer.record(DispatchDevice::Gpu(0), "conv2d", 50);
        timer.record(DispatchDevice::Cpu, "relu", 10);

        assert_eq!(timer.entries().len(), 3);
        let totals = timer.total_per_device();
        assert_eq!(totals.get("CPU"), Some(&110));
        assert_eq!(totals.get("GPU:0"), Some(&50));
    }

    #[test]
    fn s36_1_timer_breakdown() {
        let mut timer = DeviceTimer::new();
        timer.record(DispatchDevice::Cpu, "op1", 200);
        timer.record(DispatchDevice::Gpu(0), "op2", 100);
        let breakdown = timer.format_breakdown();
        assert!(breakdown.contains("CPU"));
        assert!(breakdown.contains("GPU:0"));
        assert!(breakdown.contains("Total"));
    }

    #[test]
    fn s36_1_timer_by_device() {
        let mut timer = DeviceTimer::new();
        timer.record(DispatchDevice::Cpu, "a", 10);
        timer.record(DispatchDevice::Gpu(0), "b", 20);
        timer.record(DispatchDevice::Cpu, "c", 30);

        let grouped = timer.by_device();
        assert_eq!(grouped.get("CPU").unwrap().len(), 2);
        assert_eq!(grouped.get("GPU:0").unwrap().len(), 1);
    }

    // S36.2: Transfer overhead
    #[test]
    fn s36_2_transfer_tracker() {
        let mut tt = TransferTracker::new();
        tt.record("CPU", "GPU:0", 1_000_000, 40);
        tt.record("GPU:0", "CPU", 500_000, 20);

        assert_eq!(tt.total_bytes(), 1_500_000);
        assert_eq!(tt.total_time_us(), 60);
        assert_eq!(tt.records().len(), 2);
    }

    #[test]
    fn s36_2_transfer_bandwidth() {
        let r = TransferRecord {
            src: "CPU".into(),
            dst: "GPU:0".into(),
            bytes: 1_000_000_000, // 1 GB
            duration_us: 40_000,  // 40ms
        };
        let bw = r.bandwidth_gbps();
        assert!(bw > 20.0 && bw < 30.0); // ~25 GB/s
    }

    // S36.3: Roofline model
    #[test]
    fn s36_3_roofline_model() {
        let rm = RooflineModel::new("GPU:0", 19500.0, 1000.0);

        // Ridge point
        let ridge = rm.ridge_point();
        assert!((ridge - 19.5).abs() < 0.1);

        // Memory-bound workload (low intensity)
        assert_eq!(rm.bottleneck(5.0), RooflineBottleneck::MemoryBound);

        // Compute-bound workload (high intensity)
        assert_eq!(rm.bottleneck(50.0), RooflineBottleneck::ComputeBound);
    }

    #[test]
    fn s36_3_roofline_efficiency() {
        let rm = RooflineModel::new("CPU", 100.0, 50.0);
        let eff = rm.efficiency(80.0, 100.0); // compute-bound, achieving 80/100
        assert!((eff - 0.8).abs() < 0.01);
    }

    #[test]
    fn s36_3_achievable_gflops() {
        let rm = RooflineModel::new("GPU", 1000.0, 100.0);
        // Low intensity: bandwidth-bound
        assert!((rm.achievable_gflops(5.0) - 500.0).abs() < 0.01);
        // High intensity: compute-bound (capped at peak)
        assert!((rm.achievable_gflops(50.0) - 1000.0).abs() < 0.01);
    }

    // S36.4: Flame graph SVG
    #[test]
    fn s36_4_flame_graph_svg() {
        let frames = vec![
            FlameFrame {
                name: "matmul".into(),
                device: "GPU:0".into(),
                start_us: 0,
                duration_us: 100,
                depth: 0,
            },
            FlameFrame {
                name: "relu".into(),
                device: "CPU".into(),
                start_us: 100,
                duration_us: 20,
                depth: 0,
            },
        ];
        let svg = generate_flame_svg(&frames, "Test Profile");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("matmul"));
        assert!(svg.contains("relu"));
        assert!(svg.contains("Test Profile"));
    }

    // S36.5: CLI profiler
    #[test]
    fn s36_5_cli_output() {
        let mut timer = DeviceTimer::new();
        timer.record(DispatchDevice::Cpu, "compute", 500);

        let transfers = TransferTracker::new();
        let throughput = ThroughputMetrics::from_timing(500, 10);

        let output = format_cli_profile(&timer, &transfers, &throughput);
        assert!(output.contains("Per-Device"));
        assert!(output.contains("Transfer"));
        assert!(output.contains("Throughput"));
    }

    // S36.6: Chrome Trace
    #[test]
    fn s36_6_chrome_trace() {
        let entries = vec![
            DeviceTimerEntry {
                device: DispatchDevice::Cpu,
                label: "matmul".into(),
                duration_us: 100,
                start_us: 0,
            },
            DeviceTimerEntry {
                device: DispatchDevice::Gpu(0),
                label: "conv2d".into(),
                duration_us: 50,
                start_us: 100,
            },
        ];
        let trace = generate_chrome_trace(&entries);
        assert!(trace.contains("traceEvents"));
        assert!(trace.contains("matmul"));
        assert!(trace.contains("conv2d"));
        assert!(trace.contains("\"ph\":\"X\""));
    }

    // S36.7: Throughput metrics
    #[test]
    fn s36_7_throughput() {
        let m = ThroughputMetrics::from_timing(1_000_000, 100); // 1 sec, 100 inferences
        assert!((m.inferences_per_sec - 100.0).abs() < 0.1);
        assert_eq!(m.avg_latency_us(), 10_000);
    }

    #[test]
    fn s36_7_throughput_with_tokens() {
        let m = ThroughputMetrics::from_timing(1_000_000, 50)
            .with_tokens(500.0)
            .with_samples(25.0);
        assert!((m.tokens_per_sec - 500.0).abs() < 0.1);
        assert!((m.samples_per_sec - 25.0).abs() < 0.1);
    }

    // S36.8: Memory watermark
    #[test]
    fn s36_8_memory_watermark() {
        let mut wm = MemoryWatermark::new();
        wm.record_alloc("GPU:0", 1000);
        wm.record_alloc("GPU:0", 2000);
        assert_eq!(wm.peak("GPU:0"), 3000);
        assert_eq!(wm.current("GPU:0"), 3000);

        wm.record_free("GPU:0", 1500);
        assert_eq!(wm.peak("GPU:0"), 3000); // Peak stays
        assert_eq!(wm.current("GPU:0"), 1500);
    }

    #[test]
    fn s36_8_watermark_report() {
        let mut wm = MemoryWatermark::new();
        wm.record_alloc("CPU", 1024 * 1024);
        wm.record_alloc("GPU:0", 4 * 1024 * 1024);

        let report = wm.format_report();
        assert!(report.contains("Memory Watermark"));
        assert!(report.contains("CPU"));
        assert!(report.contains("GPU:0"));
    }

    // S36.9: Comparison mode
    #[test]
    fn s36_9_comparison() {
        let cmp = ComparisonResult::compare("CPU", 1000, "GPU:0", 200);
        assert_eq!(cmp.winner, "GPU:0");
        assert!((cmp.speedup - 5.0).abs() < 0.01);
    }

    #[test]
    fn s36_9_comparison_format() {
        let cmp = ComparisonResult::compare("CPU", 500, "GPU:0", 100);
        let output = cmp.format_comparison();
        assert!(output.contains("CPU vs GPU:0"));
        assert!(output.contains("Winner: GPU:0"));
        assert!(output.contains("speedup"));
    }

    // S36.10: Integration
    #[test]
    fn s36_10_full_profiler_pipeline() {
        let mut timer = DeviceTimer::new();
        timer.record(DispatchDevice::Cpu, "preprocess", 50);
        timer.record(DispatchDevice::Gpu(0), "inference", 200);
        timer.record(DispatchDevice::Cpu, "postprocess", 30);

        let mut transfers = TransferTracker::new();
        transfers.record("CPU", "GPU:0", 4096, 10);
        transfers.record("GPU:0", "CPU", 1024, 5);

        let throughput = ThroughputMetrics::from_timing(280, 1);

        let mut watermark = MemoryWatermark::new();
        watermark.record_alloc("GPU:0", 1024 * 1024);

        let roofline = RooflineModel::new("GPU:0", 19500.0, 1000.0);

        // All components should work together
        assert!(!timer.format_breakdown().is_empty());
        assert!(!transfers.format_report().is_empty());
        assert!(!throughput.format_report().is_empty());
        assert!(!watermark.format_report().is_empty());
        assert!(roofline.ridge_point() > 0.0);

        let cmp = ComparisonResult::compare("CPU", 280, "GPU:0", 200);
        assert!(cmp.speedup > 1.0);
    }
}
