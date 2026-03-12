//! Performance Profiling — CPU profiler, flame graphs, hot path
//! detection, memory profiler, async profiler, GPU profiler,
//! lock contention, I/O profiler, profile-guided hints.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S28.1: CPU Profiler
// ═══════════════════════════════════════════════════════════════════════

/// CPU profiler configuration.
#[derive(Debug, Clone)]
pub struct ProfilerConfig {
    /// Sampling frequency in Hz.
    pub frequency_hz: u32,
    /// Whether to include kernel frames.
    pub include_kernel: bool,
    /// Maximum stack depth to capture.
    pub max_depth: usize,
    /// Duration limit (0 = unlimited).
    pub duration_ms: u64,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 1000,
            include_kernel: false,
            max_depth: 128,
            duration_ms: 0,
        }
    }
}

/// A CPU sample.
#[derive(Debug, Clone)]
pub struct CpuSample {
    /// Timestamp (ns).
    pub timestamp_ns: u64,
    /// Thread ID.
    pub thread_id: u32,
    /// Stack trace (bottom to top).
    pub stack: Vec<String>,
}

/// CPU profile data.
#[derive(Debug, Clone)]
pub struct CpuProfile {
    /// All samples.
    pub samples: Vec<CpuSample>,
    /// Sample count per function.
    pub function_counts: HashMap<String, u64>,
    /// Total sample count.
    pub total_samples: u64,
}

impl CpuProfile {
    /// Creates a new empty profile.
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            function_counts: HashMap::new(),
            total_samples: 0,
        }
    }

    /// Records a sample.
    pub fn record_sample(&mut self, sample: CpuSample) {
        for fn_name in &sample.stack {
            *self.function_counts.entry(fn_name.clone()).or_insert(0) += 1;
        }
        self.total_samples += 1;
        self.samples.push(sample);
    }

    /// Returns top N functions by sample count (exclusive time approximation).
    pub fn top_functions(&self, n: usize) -> Vec<(&str, u64, f64)> {
        let mut sorted: Vec<(&String, &u64)> = self.function_counts.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        sorted
            .into_iter()
            .take(n)
            .map(|(name, &count)| {
                let pct = if self.total_samples > 0 {
                    count as f64 / self.total_samples as f64 * 100.0
                } else {
                    0.0
                };
                (name.as_str(), count, pct)
            })
            .collect()
    }
}

impl Default for CpuProfile {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S28.2: Flame Graph
// ═══════════════════════════════════════════════════════════════════════

/// A flame graph node.
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

/// Builds a flame graph from CPU samples.
pub fn build_flame_graph(samples: &[CpuSample]) -> FlameNode {
    let mut root = FlameNode {
        name: "root".into(),
        self_count: 0,
        total_count: samples.len() as u64,
        children: Vec::new(),
    };

    for sample in samples {
        insert_stack(&mut root, &sample.stack, 0);
    }

    root
}

fn insert_stack(node: &mut FlameNode, stack: &[String], depth: usize) {
    if depth >= stack.len() {
        node.self_count += 1;
        return;
    }

    let name = &stack[depth];
    let child = node.children.iter_mut().find(|c| c.name == *name);

    match child {
        Some(child) => {
            child.total_count += 1;
            insert_stack(child, stack, depth + 1);
        }
        None => {
            let mut new_child = FlameNode {
                name: name.clone(),
                self_count: 0,
                total_count: 1,
                children: Vec::new(),
            };
            insert_stack(&mut new_child, stack, depth + 1);
            node.children.push(new_child);
        }
    }
}

/// Generates a folded stack format (for flamegraph.pl).
pub fn to_folded_stacks(samples: &[CpuSample]) -> Vec<String> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    for sample in samples {
        let key = sample.stack.join(";");
        *counts.entry(key).or_insert(0) += 1;
    }

    let mut lines: Vec<String> = counts
        .into_iter()
        .map(|(stack, count)| format!("{stack} {count}"))
        .collect();
    lines.sort();
    lines
}

// ═══════════════════════════════════════════════════════════════════════
// S28.3: Hot Path Detection
// ═══════════════════════════════════════════════════════════════════════

/// A hot function report.
#[derive(Debug, Clone)]
pub struct HotFunction {
    /// Function name.
    pub name: String,
    /// Inclusive time (samples including callees).
    pub inclusive: u64,
    /// Exclusive time (self-time only).
    pub exclusive: u64,
    /// Percentage of total time.
    pub pct_total: f64,
}

/// Computes hot functions from a profile.
pub fn find_hot_paths(profile: &CpuProfile, min_pct: f64) -> Vec<HotFunction> {
    profile
        .function_counts
        .iter()
        .filter_map(|(name, &count)| {
            let pct = count as f64 / profile.total_samples.max(1) as f64 * 100.0;
            if pct >= min_pct {
                Some(HotFunction {
                    name: name.clone(),
                    inclusive: count,
                    exclusive: count, // simplified
                    pct_total: pct,
                })
            } else {
                None
            }
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S28.4: Memory Profiler
// ═══════════════════════════════════════════════════════════════════════

/// Memory profiler data.
#[derive(Debug, Clone)]
pub struct MemoryProfile {
    /// Allocation rate (bytes/sec).
    pub alloc_rate: f64,
    /// Peak usage.
    pub peak_bytes: usize,
    /// Current usage.
    pub current_bytes: usize,
    /// Allocation hot spots (function → total bytes allocated).
    pub hot_spots: HashMap<String, usize>,
}

impl MemoryProfile {
    /// Creates new memory profile.
    pub fn new() -> Self {
        Self {
            alloc_rate: 0.0,
            peak_bytes: 0,
            current_bytes: 0,
            hot_spots: HashMap::new(),
        }
    }

    /// Records an allocation.
    pub fn record_alloc(&mut self, size: usize, source: &str) {
        self.current_bytes += size;
        self.peak_bytes = self.peak_bytes.max(self.current_bytes);
        *self.hot_spots.entry(source.to_string()).or_insert(0) += size;
    }

    /// Records a free.
    pub fn record_free(&mut self, size: usize) {
        self.current_bytes = self.current_bytes.saturating_sub(size);
    }

    /// Top allocation hot spots.
    pub fn top_allocators(&self, n: usize) -> Vec<(&str, usize)> {
        let mut sorted: Vec<(&String, &usize)> = self.hot_spots.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        sorted
            .into_iter()
            .take(n)
            .map(|(name, &bytes)| (name.as_str(), bytes))
            .collect()
    }
}

impl Default for MemoryProfile {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S28.5: Async Profiler
// ═══════════════════════════════════════════════════════════════════════

/// Async task profiling data.
#[derive(Debug, Clone)]
pub struct AsyncTaskProfile {
    /// Task ID.
    pub task_id: u64,
    /// Task name.
    pub name: String,
    /// Time spent in poll (ns).
    pub poll_time_ns: u64,
    /// Time spent waiting (ns).
    pub wait_time_ns: u64,
    /// Number of polls.
    pub poll_count: u32,
    /// Wakeup latency (ns, time from wake to poll).
    pub wakeup_latency_ns: u64,
}

impl AsyncTaskProfile {
    /// Total time.
    pub fn total_time_ns(&self) -> u64 {
        self.poll_time_ns + self.wait_time_ns
    }

    /// Efficiency (fraction of time doing useful work).
    pub fn efficiency(&self) -> f64 {
        let total = self.total_time_ns();
        if total == 0 {
            return 0.0;
        }
        self.poll_time_ns as f64 / total as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S28.6: GPU Profiler
// ═══════════════════════════════════════════════════════════════════════

/// GPU kernel profiling data.
#[derive(Debug, Clone)]
pub struct GpuKernelProfile {
    /// Kernel name.
    pub name: String,
    /// Launch count.
    pub launch_count: u32,
    /// Total execution time (us).
    pub total_time_us: f64,
    /// Average occupancy (0.0-1.0).
    pub avg_occupancy: f64,
    /// Memory transferred (bytes).
    pub memory_transferred: usize,
}

impl GpuKernelProfile {
    /// Average time per launch.
    pub fn avg_time_us(&self) -> f64 {
        if self.launch_count == 0 {
            return 0.0;
        }
        self.total_time_us / self.launch_count as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S28.7: Lock Contention
// ═══════════════════════════════════════════════════════════════════════

/// Lock contention data.
#[derive(Debug, Clone)]
pub struct LockContention {
    /// Lock ID or name.
    pub lock_name: String,
    /// Total time waiting to acquire (ns).
    pub wait_time_ns: u64,
    /// Total time holding (ns).
    pub hold_time_ns: u64,
    /// Number of acquisitions.
    pub acquire_count: u32,
    /// Number of contentions (had to wait).
    pub contention_count: u32,
}

impl LockContention {
    /// Contention ratio.
    pub fn contention_ratio(&self) -> f64 {
        if self.acquire_count == 0 {
            return 0.0;
        }
        self.contention_count as f64 / self.acquire_count as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S28.8: I/O Profiler
// ═══════════════════════════════════════════════════════════════════════

/// I/O profiling data.
#[derive(Debug, Clone)]
pub struct IoProfile {
    /// I/O category.
    pub category: IoCategory,
    /// Total wait time (ns).
    pub wait_time_ns: u64,
    /// Total bytes transferred.
    pub bytes_transferred: usize,
    /// Operation count.
    pub op_count: u32,
}

/// I/O category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoCategory {
    /// Disk I/O.
    Disk,
    /// Network I/O.
    Network,
    /// IPC.
    Ipc,
}

impl fmt::Display for IoCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IoCategory::Disk => write!(f, "Disk"),
            IoCategory::Network => write!(f, "Network"),
            IoCategory::Ipc => write!(f, "IPC"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S28.9: Profile-Guided Hints
// ═══════════════════════════════════════════════════════════════════════

/// An optimization hint.
#[derive(Debug, Clone)]
pub struct OptHint {
    /// Function name.
    pub function: String,
    /// Hint kind.
    pub kind: OptHintKind,
    /// Description.
    pub description: String,
    /// Estimated impact (1-10).
    pub impact: u8,
}

/// Kind of optimization hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptHintKind {
    /// Should be inlined.
    Inline,
    /// Can be vectorized.
    Vectorize,
    /// Cache-unfriendly access pattern.
    CacheOptimize,
    /// Lock contention — consider lock-free.
    ReduceContention,
    /// Excessive allocation — consider pooling.
    ReduceAlloc,
}

impl fmt::Display for OptHintKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptHintKind::Inline => write!(f, "inline"),
            OptHintKind::Vectorize => write!(f, "vectorize"),
            OptHintKind::CacheOptimize => write!(f, "cache-optimize"),
            OptHintKind::ReduceContention => write!(f, "reduce-contention"),
            OptHintKind::ReduceAlloc => write!(f, "reduce-alloc"),
        }
    }
}

/// Generates optimization hints from profiling data.
pub fn generate_hints(cpu: &CpuProfile, mem: &MemoryProfile, min_pct: f64) -> Vec<OptHint> {
    let mut hints = Vec::new();

    // Hot functions → inline hint
    for (name, &count) in &cpu.function_counts {
        let pct = count as f64 / cpu.total_samples.max(1) as f64 * 100.0;
        if pct >= min_pct {
            hints.push(OptHint {
                function: name.clone(),
                kind: OptHintKind::Inline,
                description: format!("Hot function ({pct:.1}% of samples) — consider inlining"),
                impact: (pct / 10.0).min(10.0) as u8,
            });
        }
    }

    // Allocation hot spots → reduce alloc hint
    for (source, &bytes) in &mem.hot_spots {
        if bytes > 1024 * 1024 {
            hints.push(OptHint {
                function: source.clone(),
                kind: OptHintKind::ReduceAlloc,
                description: format!(
                    "Allocates {} MB — consider object pooling",
                    bytes / (1024 * 1024)
                ),
                impact: 5,
            });
        }
    }

    hints
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S28.1 — CPU Profiler
    #[test]
    fn s28_1_cpu_profile() {
        let mut profile = CpuProfile::new();
        profile.record_sample(CpuSample {
            timestamp_ns: 100,
            thread_id: 0,
            stack: vec!["main".into(), "compute".into()],
        });
        profile.record_sample(CpuSample {
            timestamp_ns: 200,
            thread_id: 0,
            stack: vec!["main".into(), "compute".into()],
        });
        profile.record_sample(CpuSample {
            timestamp_ns: 300,
            thread_id: 0,
            stack: vec!["main".into(), "io_wait".into()],
        });
        assert_eq!(profile.total_samples, 3);
        assert_eq!(*profile.function_counts.get("compute").unwrap(), 2);
    }

    #[test]
    fn s28_1_top_functions() {
        let mut profile = CpuProfile::new();
        for _ in 0..10 {
            profile.record_sample(CpuSample {
                timestamp_ns: 0,
                thread_id: 0,
                stack: vec!["hot".into()],
            });
        }
        for _ in 0..2 {
            profile.record_sample(CpuSample {
                timestamp_ns: 0,
                thread_id: 0,
                stack: vec!["cold".into()],
            });
        }
        let top = profile.top_functions(1);
        assert_eq!(top[0].0, "hot");
    }

    // S28.2 — Flame Graph
    #[test]
    fn s28_2_flame_graph() {
        let samples = vec![
            CpuSample {
                timestamp_ns: 0,
                thread_id: 0,
                stack: vec!["main".into(), "a".into()],
            },
            CpuSample {
                timestamp_ns: 0,
                thread_id: 0,
                stack: vec!["main".into(), "b".into()],
            },
        ];
        let root = build_flame_graph(&samples);
        assert_eq!(root.total_count, 2);
        assert_eq!(root.children.len(), 1); // "main"
    }

    #[test]
    fn s28_2_folded_stacks() {
        let samples = vec![
            CpuSample {
                timestamp_ns: 0,
                thread_id: 0,
                stack: vec!["main".into(), "a".into()],
            },
            CpuSample {
                timestamp_ns: 0,
                thread_id: 0,
                stack: vec!["main".into(), "a".into()],
            },
        ];
        let folded = to_folded_stacks(&samples);
        assert_eq!(folded.len(), 1);
        assert!(folded[0].contains("main;a 2"));
    }

    // S28.3 — Hot Path Detection
    #[test]
    fn s28_3_find_hot_paths() {
        let mut profile = CpuProfile::new();
        for _ in 0..90 {
            profile.record_sample(CpuSample {
                timestamp_ns: 0,
                thread_id: 0,
                stack: vec!["hot".into()],
            });
        }
        for _ in 0..10 {
            profile.record_sample(CpuSample {
                timestamp_ns: 0,
                thread_id: 0,
                stack: vec!["cold".into()],
            });
        }
        let hot = find_hot_paths(&profile, 50.0);
        assert_eq!(hot.len(), 1);
        assert_eq!(hot[0].name, "hot");
    }

    // S28.4 — Memory Profiler
    #[test]
    fn s28_4_memory_profile() {
        let mut mp = MemoryProfile::new();
        mp.record_alloc(1024, "main:5");
        mp.record_alloc(2048, "main:10");
        mp.record_free(1024);
        assert_eq!(mp.peak_bytes, 3072);
        assert_eq!(mp.current_bytes, 2048);
    }

    #[test]
    fn s28_4_top_allocators() {
        let mut mp = MemoryProfile::new();
        mp.record_alloc(1000, "hot_fn");
        mp.record_alloc(2000, "hot_fn");
        mp.record_alloc(100, "cold_fn");
        let top = mp.top_allocators(1);
        assert_eq!(top[0].0, "hot_fn");
        assert_eq!(top[0].1, 3000);
    }

    // S28.5 — Async Profiler
    #[test]
    fn s28_5_async_profile() {
        let ap = AsyncTaskProfile {
            task_id: 1,
            name: "fetch".into(),
            poll_time_ns: 1_000_000,
            wait_time_ns: 9_000_000,
            poll_count: 5,
            wakeup_latency_ns: 50_000,
        };
        assert_eq!(ap.total_time_ns(), 10_000_000);
        assert!((ap.efficiency() - 0.1).abs() < 1e-10);
    }

    // S28.6 — GPU Profiler
    #[test]
    fn s28_6_gpu_profile() {
        let gp = GpuKernelProfile {
            name: "matmul".into(),
            launch_count: 100,
            total_time_us: 5000.0,
            avg_occupancy: 0.85,
            memory_transferred: 1024 * 1024,
        };
        assert!((gp.avg_time_us() - 50.0).abs() < 1e-10);
    }

    // S28.7 — Lock Contention
    #[test]
    fn s28_7_lock_contention() {
        let lc = LockContention {
            lock_name: "db_mutex".into(),
            wait_time_ns: 5_000_000,
            hold_time_ns: 1_000_000,
            acquire_count: 100,
            contention_count: 25,
        };
        assert!((lc.contention_ratio() - 0.25).abs() < 1e-10);
    }

    // S28.8 — I/O Profiler
    #[test]
    fn s28_8_io_category() {
        assert_eq!(IoCategory::Disk.to_string(), "Disk");
        assert_eq!(IoCategory::Network.to_string(), "Network");
        assert_eq!(IoCategory::Ipc.to_string(), "IPC");
    }

    // S28.9 — Profile-Guided Hints
    #[test]
    fn s28_9_generate_hints() {
        let mut cpu = CpuProfile::new();
        for _ in 0..100 {
            cpu.record_sample(CpuSample {
                timestamp_ns: 0,
                thread_id: 0,
                stack: vec!["hot_fn".into()],
            });
        }
        let mut mem = MemoryProfile::new();
        mem.record_alloc(2 * 1024 * 1024, "alloc_fn");

        let hints = generate_hints(&cpu, &mem, 50.0);
        assert!(hints.iter().any(|h| h.kind == OptHintKind::Inline));
        assert!(hints.iter().any(|h| h.kind == OptHintKind::ReduceAlloc));
    }

    #[test]
    fn s28_9_hint_kind_display() {
        assert_eq!(OptHintKind::Inline.to_string(), "inline");
        assert_eq!(OptHintKind::Vectorize.to_string(), "vectorize");
        assert_eq!(OptHintKind::CacheOptimize.to_string(), "cache-optimize");
    }

    // S28.10 — Profiler config default
    #[test]
    fn s28_10_profiler_config() {
        let cfg = ProfilerConfig::default();
        assert_eq!(cfg.frequency_hz, 1000);
        assert_eq!(cfg.max_depth, 128);
    }
}
