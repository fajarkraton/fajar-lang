//! GC benchmarks — throughput, latency, pause time, memory overhead,
//! collection frequency, generational effectiveness, comparisons, reports.

use std::fmt;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// S24.1: Throughput Benchmark
// ═══════════════════════════════════════════════════════════════════════

/// Throughput measurement for a workload.
#[derive(Debug, Clone)]
pub struct ThroughputResult {
    /// Workload description.
    pub workload: String,
    /// Operations per second under GC mode.
    pub gc_ops_per_sec: f64,
    /// Operations per second under ownership mode.
    pub owned_ops_per_sec: f64,
}

impl ThroughputResult {
    /// Returns the GC overhead ratio (1.0 = no overhead).
    pub fn overhead_ratio(&self) -> f64 {
        if self.gc_ops_per_sec > 0.0 {
            self.owned_ops_per_sec / self.gc_ops_per_sec
        } else {
            f64::INFINITY
        }
    }
}

impl fmt::Display for ThroughputResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: GC {:.0} ops/s, Owned {:.0} ops/s (overhead {:.2}x)",
            self.workload,
            self.gc_ops_per_sec,
            self.owned_ops_per_sec,
            self.overhead_ratio()
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S24.2: Latency Benchmark
// ═══════════════════════════════════════════════════════════════════════

/// Latency percentile measurements.
#[derive(Debug, Clone)]
pub struct LatencyResult {
    /// Workload description.
    pub workload: String,
    /// p50 latency in microseconds.
    pub p50_us: f64,
    /// p99 latency in microseconds.
    pub p99_us: f64,
    /// Maximum latency in microseconds.
    pub max_us: f64,
    /// Memory mode used.
    pub mode: String,
}

impl fmt::Display for LatencyResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}): p50={:.1}us, p99={:.1}us, max={:.1}us",
            self.workload, self.mode, self.p50_us, self.p99_us, self.max_us
        )
    }
}

/// Computes percentiles from a sorted list of latency samples.
pub fn compute_percentile(sorted_samples: &[f64], percentile: f64) -> f64 {
    if sorted_samples.is_empty() {
        return 0.0;
    }
    let idx = ((percentile / 100.0) * (sorted_samples.len() as f64 - 1.0)).round() as usize;
    let idx = idx.min(sorted_samples.len() - 1);
    sorted_samples[idx]
}

// ═══════════════════════════════════════════════════════════════════════
// S24.3: Pause Time Benchmark
// ═══════════════════════════════════════════════════════════════════════

/// GC pause time distribution.
#[derive(Debug, Clone)]
pub struct PauseDistribution {
    /// Minimum pause time.
    pub min: Duration,
    /// Maximum pause time.
    pub max: Duration,
    /// p50 pause time.
    pub p50: Duration,
    /// p99 pause time.
    pub p99: Duration,
    /// Total number of pauses.
    pub count: usize,
}

impl fmt::Display for PauseDistribution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GC pauses ({}): min={:?}, p50={:?}, p99={:?}, max={:?}",
            self.count, self.min, self.p50, self.p99, self.max
        )
    }
}

/// Records a GC pause time.
#[derive(Debug, Clone, Default)]
pub struct PauseRecorder {
    /// All recorded pause durations in microseconds.
    pauses_us: Vec<f64>,
}

impl PauseRecorder {
    /// Creates a new pause recorder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a pause.
    pub fn record(&mut self, duration: Duration) {
        self.pauses_us.push(duration.as_micros() as f64);
    }

    /// Returns the number of recorded pauses.
    pub fn count(&self) -> usize {
        self.pauses_us.len()
    }

    /// Computes the pause distribution.
    pub fn distribution(&self) -> PauseDistribution {
        if self.pauses_us.is_empty() {
            return PauseDistribution {
                min: Duration::ZERO,
                max: Duration::ZERO,
                p50: Duration::ZERO,
                p99: Duration::ZERO,
                count: 0,
            };
        }

        let mut sorted = self.pauses_us.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let min_us = sorted[0];
        let max_us = sorted[sorted.len() - 1];
        let p50_us = compute_percentile(&sorted, 50.0);
        let p99_us = compute_percentile(&sorted, 99.0);

        PauseDistribution {
            min: Duration::from_micros(min_us as u64),
            max: Duration::from_micros(max_us as u64),
            p50: Duration::from_micros(p50_us as u64),
            p99: Duration::from_micros(p99_us as u64),
            count: sorted.len(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S24.4: Memory Overhead
// ═══════════════════════════════════════════════════════════════════════

/// Memory usage comparison.
#[derive(Debug, Clone)]
pub struct MemoryOverhead {
    /// Workload description.
    pub workload: String,
    /// Peak memory under GC (bytes).
    pub gc_peak_bytes: usize,
    /// Peak memory under ownership (bytes).
    pub owned_peak_bytes: usize,
}

impl MemoryOverhead {
    /// Returns the overhead ratio.
    pub fn ratio(&self) -> f64 {
        if self.owned_peak_bytes > 0 {
            self.gc_peak_bytes as f64 / self.owned_peak_bytes as f64
        } else {
            1.0
        }
    }
}

impl fmt::Display for MemoryOverhead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: GC {}KB, Owned {}KB (ratio {:.2}x)",
            self.workload,
            self.gc_peak_bytes / 1024,
            self.owned_peak_bytes / 1024,
            self.ratio()
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S24.5: Collection Frequency
// ═══════════════════════════════════════════════════════════════════════

/// Collection frequency metrics.
#[derive(Debug, Clone)]
pub struct CollectionFrequency {
    /// Total collections performed.
    pub total_collections: u64,
    /// Duration of measurement period.
    pub measurement_period: Duration,
    /// Allocation rate (bytes per second).
    pub alloc_rate_bytes_per_sec: f64,
}

impl CollectionFrequency {
    /// Returns collections per second.
    pub fn collections_per_sec(&self) -> f64 {
        let secs = self.measurement_period.as_secs_f64();
        if secs > 0.0 {
            self.total_collections as f64 / secs
        } else {
            0.0
        }
    }
}

impl fmt::Display for CollectionFrequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} collections in {:?} ({:.1}/s), alloc rate {:.0} bytes/s",
            self.total_collections,
            self.measurement_period,
            self.collections_per_sec(),
            self.alloc_rate_bytes_per_sec
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S24.6: Generational Effectiveness
// ═══════════════════════════════════════════════════════════════════════

/// Generational GC effectiveness metrics.
#[derive(Debug, Clone)]
pub struct GenerationalMetrics {
    /// Young-gen collections performed.
    pub young_collections: u64,
    /// Old-gen collections performed.
    pub old_collections: u64,
    /// Objects promoted from young to old.
    pub promotions: u64,
    /// Average young-gen collection time.
    pub avg_young_pause: Duration,
    /// Average old-gen collection time.
    pub avg_old_pause: Duration,
}

impl GenerationalMetrics {
    /// Returns the young-to-old collection ratio.
    pub fn collection_ratio(&self) -> f64 {
        if self.old_collections > 0 {
            self.young_collections as f64 / self.old_collections as f64
        } else {
            self.young_collections as f64
        }
    }
}

impl fmt::Display for GenerationalMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Young: {} ({:?} avg), Old: {} ({:?} avg), Promoted: {}, Ratio: {:.1}",
            self.young_collections,
            self.avg_young_pause,
            self.old_collections,
            self.avg_old_pause,
            self.promotions,
            self.collection_ratio()
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S24.7 / S24.8: Comparisons
// ═══════════════════════════════════════════════════════════════════════

/// A benchmark comparison entry.
#[derive(Debug, Clone)]
pub struct ComparisonEntry {
    /// Language/mode name.
    pub name: String,
    /// Throughput (ops/sec).
    pub throughput: f64,
    /// p99 latency (microseconds).
    pub p99_latency_us: f64,
    /// Peak memory (bytes).
    pub peak_memory: usize,
}

impl fmt::Display for ComparisonEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {:.0} ops/s, p99={:.1}us, mem={}KB",
            self.name,
            self.throughput,
            self.p99_latency_us,
            self.peak_memory / 1024
        )
    }
}

/// A benchmark comparison report.
#[derive(Debug, Clone)]
pub struct ComparisonReport {
    /// Benchmark name.
    pub benchmark: String,
    /// All entries.
    pub entries: Vec<ComparisonEntry>,
}

impl ComparisonReport {
    /// Returns the fastest entry by throughput.
    pub fn fastest(&self) -> Option<&ComparisonEntry> {
        self.entries.iter().max_by(|a, b| {
            a.throughput
                .partial_cmp(&b.throughput)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Returns the lowest-latency entry.
    pub fn lowest_latency(&self) -> Option<&ComparisonEntry> {
        self.entries.iter().min_by(|a, b| {
            a.p99_latency_us
                .partial_cmp(&b.p99_latency_us)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S24.9: Benchmark Report
// ═══════════════════════════════════════════════════════════════════════

/// A complete GC benchmark report.
#[derive(Debug, Clone)]
pub struct BenchmarkReport {
    /// Report title.
    pub title: String,
    /// Throughput results.
    pub throughput: Vec<ThroughputResult>,
    /// Latency results.
    pub latency: Vec<LatencyResult>,
    /// Memory overhead results.
    pub memory: Vec<MemoryOverhead>,
    /// Pause distribution.
    pub pauses: Option<PauseDistribution>,
}

impl BenchmarkReport {
    /// Creates a new empty report.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.into(),
            throughput: Vec::new(),
            latency: Vec::new(),
            memory: Vec::new(),
            pauses: None,
        }
    }

    /// Renders the report as a human-readable string.
    pub fn render(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("=== {} ===", self.title));
        lines.push(String::new());

        if !self.throughput.is_empty() {
            lines.push("## Throughput".into());
            for t in &self.throughput {
                lines.push(format!("  {t}"));
            }
            lines.push(String::new());
        }

        if !self.latency.is_empty() {
            lines.push("## Latency".into());
            for l in &self.latency {
                lines.push(format!("  {l}"));
            }
            lines.push(String::new());
        }

        if !self.memory.is_empty() {
            lines.push("## Memory".into());
            for m in &self.memory {
                lines.push(format!("  {m}"));
            }
            lines.push(String::new());
        }

        if let Some(pauses) = &self.pauses {
            lines.push("## GC Pauses".into());
            lines.push(format!("  {pauses}"));
            lines.push(String::new());
        }

        lines.join("\n")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S24.1 — Throughput Benchmark
    #[test]
    fn s24_1_throughput_overhead() {
        let result = ThroughputResult {
            workload: "sort".into(),
            gc_ops_per_sec: 800.0,
            owned_ops_per_sec: 1000.0,
        };
        assert!((result.overhead_ratio() - 1.25).abs() < 0.01);
    }

    #[test]
    fn s24_1_throughput_display() {
        let result = ThroughputResult {
            workload: "fib".into(),
            gc_ops_per_sec: 500.0,
            owned_ops_per_sec: 500.0,
        };
        assert!(result.to_string().contains("fib"));
        assert!((result.overhead_ratio() - 1.0).abs() < 0.01);
    }

    // S24.2 — Latency Benchmark
    #[test]
    fn s24_2_percentile_calculation() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let p50 = compute_percentile(&samples, 50.0);
        assert!((p50 - 5.0).abs() < 1.5);
        let p99 = compute_percentile(&samples, 99.0);
        assert!((p99 - 10.0).abs() < 1.0);
    }

    #[test]
    fn s24_2_latency_display() {
        let result = LatencyResult {
            workload: "request".into(),
            p50_us: 100.0,
            p99_us: 500.0,
            max_us: 1000.0,
            mode: "GC".into(),
        };
        assert!(result.to_string().contains("p50"));
        assert!(result.to_string().contains("p99"));
    }

    // S24.3 — Pause Time Benchmark
    #[test]
    fn s24_3_pause_recorder() {
        let mut recorder = PauseRecorder::new();
        recorder.record(Duration::from_micros(100));
        recorder.record(Duration::from_micros(200));
        recorder.record(Duration::from_micros(50));
        assert_eq!(recorder.count(), 3);
        let dist = recorder.distribution();
        assert_eq!(dist.count, 3);
        assert_eq!(dist.min, Duration::from_micros(50));
        assert_eq!(dist.max, Duration::from_micros(200));
    }

    #[test]
    fn s24_3_empty_distribution() {
        let recorder = PauseRecorder::new();
        let dist = recorder.distribution();
        assert_eq!(dist.count, 0);
        assert_eq!(dist.min, Duration::ZERO);
    }

    // S24.4 — Memory Overhead
    #[test]
    fn s24_4_memory_ratio() {
        let overhead = MemoryOverhead {
            workload: "tree".into(),
            gc_peak_bytes: 2048,
            owned_peak_bytes: 1024,
        };
        assert!((overhead.ratio() - 2.0).abs() < 0.01);
    }

    #[test]
    fn s24_4_memory_display() {
        let overhead = MemoryOverhead {
            workload: "list".into(),
            gc_peak_bytes: 1024 * 100,
            owned_peak_bytes: 1024 * 50,
        };
        assert!(overhead.to_string().contains("list"));
    }

    // S24.5 — Collection Frequency
    #[test]
    fn s24_5_collections_per_sec() {
        let freq = CollectionFrequency {
            total_collections: 100,
            measurement_period: Duration::from_secs(10),
            alloc_rate_bytes_per_sec: 1_000_000.0,
        };
        assert!((freq.collections_per_sec() - 10.0).abs() < 0.01);
    }

    // S24.6 — Generational Effectiveness
    #[test]
    fn s24_6_collection_ratio() {
        let metrics = GenerationalMetrics {
            young_collections: 100,
            old_collections: 5,
            promotions: 50,
            avg_young_pause: Duration::from_micros(100),
            avg_old_pause: Duration::from_micros(5000),
        };
        assert!((metrics.collection_ratio() - 20.0).abs() < 0.01);
    }

    #[test]
    fn s24_6_generational_display() {
        let metrics = GenerationalMetrics {
            young_collections: 50,
            old_collections: 2,
            promotions: 10,
            avg_young_pause: Duration::from_micros(50),
            avg_old_pause: Duration::from_micros(2000),
        };
        assert!(metrics.to_string().contains("Young: 50"));
    }

    // S24.7 — Comparison with Rust
    #[test]
    fn s24_7_comparison_fastest() {
        let report = ComparisonReport {
            benchmark: "sort".into(),
            entries: vec![
                ComparisonEntry {
                    name: "Fajar Owned".into(),
                    throughput: 1000.0,
                    p99_latency_us: 50.0,
                    peak_memory: 1024,
                },
                ComparisonEntry {
                    name: "Fajar GC".into(),
                    throughput: 800.0,
                    p99_latency_us: 100.0,
                    peak_memory: 2048,
                },
            ],
        };
        assert_eq!(report.fastest().unwrap().name, "Fajar Owned");
    }

    // S24.8 — Comparison with Go
    #[test]
    fn s24_8_lowest_latency() {
        let report = ComparisonReport {
            benchmark: "web".into(),
            entries: vec![
                ComparisonEntry {
                    name: "Fajar GC".into(),
                    throughput: 900.0,
                    p99_latency_us: 80.0,
                    peak_memory: 2048,
                },
                ComparisonEntry {
                    name: "Go".into(),
                    throughput: 850.0,
                    p99_latency_us: 120.0,
                    peak_memory: 3072,
                },
            ],
        };
        assert_eq!(report.lowest_latency().unwrap().name, "Fajar GC");
    }

    // S24.9 — Benchmark Report
    #[test]
    fn s24_9_report_render() {
        let mut report = BenchmarkReport::new("GC Benchmark Suite");
        report.throughput.push(ThroughputResult {
            workload: "sort".into(),
            gc_ops_per_sec: 800.0,
            owned_ops_per_sec: 1000.0,
        });
        let rendered = report.render();
        assert!(rendered.contains("GC Benchmark Suite"));
        assert!(rendered.contains("Throughput"));
        assert!(rendered.contains("sort"));
    }

    #[test]
    fn s24_9_empty_report() {
        let report = BenchmarkReport::new("Empty");
        let rendered = report.render();
        assert!(rendered.contains("Empty"));
    }

    // S24.10 — Additional
    #[test]
    fn s24_10_pause_distribution_display() {
        let dist = PauseDistribution {
            min: Duration::from_micros(10),
            max: Duration::from_micros(500),
            p50: Duration::from_micros(50),
            p99: Duration::from_micros(300),
            count: 100,
        };
        assert!(dist.to_string().contains("100"));
    }

    #[test]
    fn s24_10_comparison_entry_display() {
        let entry = ComparisonEntry {
            name: "Fajar".into(),
            throughput: 1000.0,
            p99_latency_us: 100.0,
            peak_memory: 1024 * 1024,
        };
        assert!(entry.to_string().contains("Fajar"));
        assert!(entry.to_string().contains("1000 ops/s"));
    }

    #[test]
    fn s24_10_percentile_empty() {
        assert_eq!(compute_percentile(&[], 50.0), 0.0);
    }

    #[test]
    fn s24_10_percentile_single() {
        assert_eq!(compute_percentile(&[42.0], 99.0), 42.0);
    }
}
