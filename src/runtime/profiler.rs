//! Runtime profiler for Fajar Lang.
//!
//! Provides function-level profiling, memory tracking,
//! and flame graph data generation. All timing is based on
//! `std::time::Instant` and all operations are simulation-safe.

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from profiler operations.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum ProfilerError {
    /// Tried to stop a function that was not started.
    #[error("no active profile for function '{0}'")]
    NotStarted(String),

    /// Report generation failed.
    #[error("report error: {0}")]
    ReportError(String),
}

// ═══════════════════════════════════════════════════════════════════════
// FunctionProfile — per-function statistics
// ═══════════════════════════════════════════════════════════════════════

/// Accumulated profiling data for a single function.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionProfile {
    /// Fully-qualified function name.
    pub name: String,
    /// Total time spent in this function (microseconds).
    pub total_time_us: u64,
    /// Number of times this function was called.
    pub call_count: u64,
}

impl FunctionProfile {
    /// Creates a new empty profile for a function.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            total_time_us: 0,
            call_count: 0,
        }
    }

    /// Returns the average time per call (microseconds).
    ///
    /// Returns 0 if the function was never called.
    pub fn avg_time_us(&self) -> u64 {
        if self.call_count == 0 {
            0
        } else {
            self.total_time_us / self.call_count
        }
    }
}

impl fmt::Display for FunctionProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}us total, {} calls, {}us avg",
            self.name,
            self.total_time_us,
            self.call_count,
            self.avg_time_us()
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// MemorySnapshot — point-in-time memory state
// ═══════════════════════════════════════════════════════════════════════

/// A snapshot of memory usage at a point in time.
#[derive(Debug, Clone, PartialEq)]
pub struct MemorySnapshot {
    /// Estimated heap bytes in use.
    pub heap_bytes: usize,
    /// Estimated stack bytes in use.
    pub stack_bytes: usize,
    /// Monotonic timestamp (microseconds since profiler start).
    pub timestamp_us: u64,
}

impl MemorySnapshot {
    /// Creates a new memory snapshot.
    pub fn new(heap_bytes: usize, stack_bytes: usize, timestamp_us: u64) -> Self {
        Self {
            heap_bytes,
            stack_bytes,
            timestamp_us,
        }
    }

    /// Returns total memory in use (heap + stack).
    pub fn total_bytes(&self) -> usize {
        self.heap_bytes + self.stack_bytes
    }
}

impl fmt::Display for MemorySnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "@{}us: heap={}B, stack={}B, total={}B",
            self.timestamp_us,
            self.heap_bytes,
            self.stack_bytes,
            self.total_bytes()
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FlameGraphEntry — for flame graph visualisation
// ═══════════════════════════════════════════════════════════════════════

/// A single entry in collapsed-stack flame graph format.
#[derive(Debug, Clone, PartialEq)]
pub struct FlameGraphEntry {
    /// Function name.
    pub function_name: String,
    /// Call stack depth (0 = top-level).
    pub depth: usize,
    /// Duration in microseconds.
    pub duration_us: u64,
}

impl fmt::Display for FlameGraphEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{} {}",
            "  ".repeat(self.depth),
            self.function_name,
            self.duration_us
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AllocationTracker — alloc/free tracking
// ═══════════════════════════════════════════════════════════════════════

/// Tracks allocations and deallocations to detect leaks.
#[derive(Debug, Clone)]
pub struct AllocationTracker {
    /// Map of allocation address -> size in bytes.
    live: HashMap<u64, usize>,
    /// Total bytes allocated over the lifetime of the tracker.
    total_allocated: usize,
    /// Total bytes freed over the lifetime of the tracker.
    total_freed: usize,
    /// Number of alloc calls.
    alloc_count: u64,
    /// Number of free calls.
    free_count: u64,
}

impl AllocationTracker {
    /// Creates a new allocation tracker.
    pub fn new() -> Self {
        Self {
            live: HashMap::new(),
            total_allocated: 0,
            total_freed: 0,
            alloc_count: 0,
            free_count: 0,
        }
    }

    /// Records an allocation at `address` of `size` bytes.
    pub fn record_alloc(&mut self, address: u64, size: usize) {
        self.live.insert(address, size);
        self.total_allocated += size;
        self.alloc_count += 1;
    }

    /// Records a deallocation at `address`.
    ///
    /// Returns the size that was freed, or `None` if the address
    /// was not tracked (potential double-free or invalid free).
    pub fn record_free(&mut self, address: u64) -> Option<usize> {
        if let Some(size) = self.live.remove(&address) {
            self.total_freed += size;
            self.free_count += 1;
            Some(size)
        } else {
            None
        }
    }

    /// Returns the number of currently-live allocations.
    pub fn live_count(&self) -> usize {
        self.live.len()
    }

    /// Returns the total bytes currently live.
    pub fn live_bytes(&self) -> usize {
        self.live.values().sum()
    }

    /// Returns `true` if there are no leaks (all allocs freed).
    pub fn is_leak_free(&self) -> bool {
        self.live.is_empty()
    }

    /// Returns a summary of leaked allocations.
    pub fn leak_report(&self) -> Vec<(u64, usize)> {
        self.live
            .iter()
            .map(|(&addr, &size)| (addr, size))
            .collect()
    }

    /// Returns the total number of alloc calls.
    pub fn alloc_count(&self) -> u64 {
        self.alloc_count
    }

    /// Returns the total number of free calls.
    pub fn free_count(&self) -> u64 {
        self.free_count
    }
}

impl Default for AllocationTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Profiler — main profiler
// ═══════════════════════════════════════════════════════════════════════

/// Runtime profiler for Fajar Lang programs.
///
/// Tracks function call timing, memory snapshots, and flame graph
/// data for performance analysis.
#[derive(Debug)]
pub struct Profiler {
    /// Per-function accumulated profiles.
    profiles: HashMap<String, FunctionProfile>,
    /// Currently-active function timers.
    active: HashMap<String, Instant>,
    /// Memory snapshots taken over time.
    memory_snapshots: Vec<MemorySnapshot>,
    /// Flame graph entries.
    flame_entries: Vec<FlameGraphEntry>,
    /// Allocation tracker.
    alloc_tracker: AllocationTracker,
    /// Profiler start time.
    start_time: Instant,
    /// Current call stack depth (for flame graph).
    call_depth: usize,
}

impl Profiler {
    /// Creates a new profiler.
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            active: HashMap::new(),
            memory_snapshots: Vec::new(),
            flame_entries: Vec::new(),
            alloc_tracker: AllocationTracker::new(),
            start_time: Instant::now(),
            call_depth: 0,
        }
    }

    /// Starts profiling a function call.
    pub fn start_function(&mut self, name: &str) {
        self.active.insert(name.to_string(), Instant::now());
        self.call_depth += 1;
    }

    /// Stops profiling a function call and records the duration.
    ///
    /// # Errors
    ///
    /// Returns `ProfilerError::NotStarted` if the function was
    /// not previously started.
    pub fn stop_function(&mut self, name: &str) -> Result<Duration, ProfilerError> {
        let start = self
            .active
            .remove(name)
            .ok_or_else(|| ProfilerError::NotStarted(name.to_string()))?;

        let elapsed = start.elapsed();
        let us = elapsed.as_micros() as u64;

        let profile = self
            .profiles
            .entry(name.to_string())
            .or_insert_with(|| FunctionProfile::new(name));
        profile.total_time_us += us;
        profile.call_count += 1;

        // Record flame graph entry.
        if self.call_depth > 0 {
            self.call_depth -= 1;
        }
        self.flame_entries.push(FlameGraphEntry {
            function_name: name.to_string(),
            depth: self.call_depth,
            duration_us: us,
        });

        Ok(elapsed)
    }

    /// Takes a memory snapshot.
    pub fn snapshot_memory(&mut self, heap_bytes: usize, stack_bytes: usize) {
        let ts = self.start_time.elapsed().as_micros() as u64;
        self.memory_snapshots
            .push(MemorySnapshot::new(heap_bytes, stack_bytes, ts));
    }

    /// Returns a reference to the allocation tracker.
    pub fn alloc_tracker(&self) -> &AllocationTracker {
        &self.alloc_tracker
    }

    /// Returns a mutable reference to the allocation tracker.
    pub fn alloc_tracker_mut(&mut self) -> &mut AllocationTracker {
        &mut self.alloc_tracker
    }

    /// Returns all function profiles.
    pub fn function_profiles(&self) -> &HashMap<String, FunctionProfile> {
        &self.profiles
    }

    /// Returns the memory snapshots.
    pub fn memory_snapshots(&self) -> &[MemorySnapshot] {
        &self.memory_snapshots
    }

    /// Returns the flame graph entries.
    pub fn flame_entries(&self) -> &[FlameGraphEntry] {
        &self.flame_entries
    }

    /// Returns the top N functions by total time.
    pub fn hot_functions(&self, n: usize) -> Vec<&FunctionProfile> {
        let mut sorted: Vec<&FunctionProfile> = self.profiles.values().collect();
        sorted.sort_by_key(|x| std::cmp::Reverse(x.total_time_us));
        sorted.truncate(n);
        sorted
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ProfileReport — formatted output
// ═══════════════════════════════════════════════════════════════════════

/// A generated profiling report.
#[derive(Debug, Clone)]
pub struct ProfileReport {
    /// Text lines of the report.
    lines: Vec<String>,
    /// JSON representation.
    json: String,
}

impl ProfileReport {
    /// Generates a report from a profiler.
    pub fn from_profiler(profiler: &Profiler) -> Self {
        let lines = build_text_report(profiler);
        let json = build_json_report(profiler);
        Self { lines, json }
    }

    /// Returns the report as plain text.
    pub fn to_text(&self) -> String {
        self.lines.join("\n")
    }

    /// Returns the report as JSON.
    pub fn to_json(&self) -> &str {
        &self.json
    }

    /// Returns flamegraph-compatible collapsed-stack data.
    pub fn to_flamegraph_data(profiler: &Profiler) -> String {
        profiler
            .flame_entries
            .iter()
            .map(|e| format!("{} {}", e.function_name, e.duration_us))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Builds the text report lines.
fn build_text_report(profiler: &Profiler) -> Vec<String> {
    let mut lines = vec!["=== Profiler Report ===".to_string()];

    lines.push(String::new());
    lines.push("Functions:".to_string());

    let mut sorted: Vec<&FunctionProfile> = profiler.profiles.values().collect();
    sorted.sort_by_key(|x| std::cmp::Reverse(x.total_time_us));

    for p in &sorted {
        lines.push(format!("  {p}"));
    }

    if !profiler.memory_snapshots.is_empty() {
        lines.push(String::new());
        lines.push("Memory snapshots:".to_string());
        for snap in &profiler.memory_snapshots {
            lines.push(format!("  {snap}"));
        }
    }

    let tracker = profiler.alloc_tracker();
    lines.push(String::new());
    lines.push(format!(
        "Allocations: {} allocs, {} frees, {} live ({}B)",
        tracker.alloc_count(),
        tracker.free_count(),
        tracker.live_count(),
        tracker.live_bytes()
    ));

    lines
}

/// Builds the JSON report.
fn build_json_report(profiler: &Profiler) -> String {
    let functions: Vec<String> = profiler
        .profiles
        .values()
        .map(|p| {
            format!(
                "    {{\n      \"name\": \"{}\",\n      \"total_time_us\": {},\n      \"call_count\": {},\n      \"avg_time_us\": {}\n    }}",
                p.name, p.total_time_us, p.call_count, p.avg_time_us()
            )
        })
        .collect();

    let snapshots: Vec<String> = profiler
        .memory_snapshots
        .iter()
        .map(|s| {
            format!(
                "    {{\n      \"heap_bytes\": {},\n      \"stack_bytes\": {},\n      \"timestamp_us\": {}\n    }}",
                s.heap_bytes, s.stack_bytes, s.timestamp_us
            )
        })
        .collect();

    let tracker = profiler.alloc_tracker();
    format!(
        "{{\n  \"functions\": [\n{}\n  ],\n  \"memory_snapshots\": [\n{}\n  ],\n  \"allocations\": {{\n    \"alloc_count\": {},\n    \"free_count\": {},\n    \"live_count\": {},\n    \"live_bytes\": {}\n  }}\n}}",
        functions.join(",\n"),
        snapshots.join(",\n"),
        tracker.alloc_count(),
        tracker.free_count(),
        tracker.live_count(),
        tracker.live_bytes(),
    )
}

// ═══════════════════════════════════════════════════════════════════════
// SamplingProfiler — periodic stack trace capture (simulated)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated sampling profiler that records periodic stack traces.
///
/// In a real implementation, this would use OS signals (SIGPROF)
/// or hardware performance counters. Here it provides a simulation
/// API for testing profiler output formatting.
#[derive(Debug, Clone)]
pub struct SamplingProfiler {
    /// Recorded stack samples.
    samples: Vec<Vec<String>>,
    /// Sampling interval in microseconds.
    interval_us: u64,
}

impl SamplingProfiler {
    /// Creates a new sampling profiler with the given interval.
    pub fn new(interval_us: u64) -> Self {
        Self {
            samples: Vec::new(),
            interval_us,
        }
    }

    /// Records a simulated stack sample.
    pub fn record_sample(&mut self, stack: Vec<String>) {
        self.samples.push(stack);
    }

    /// Returns the number of recorded samples.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Returns the sampling interval.
    pub fn interval_us(&self) -> u64 {
        self.interval_us
    }

    /// Returns all recorded samples.
    pub fn samples(&self) -> &[Vec<String>] {
        &self.samples
    }

    /// Returns the most frequently-sampled function.
    pub fn hottest_function(&self) -> Option<String> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for sample in &self.samples {
            if let Some(top) = sample.last() {
                *counts.entry(top.as_str()).or_insert(0) += 1;
            }
        }
        counts
            .into_iter()
            .max_by_key(|&(_, c)| c)
            .map(|(name, _)| name.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s26_1_function_profile_avg() {
        let mut fp = FunctionProfile::new("main");
        fp.total_time_us = 100;
        fp.call_count = 4;
        assert_eq!(fp.avg_time_us(), 25);

        let empty = FunctionProfile::new("unused");
        assert_eq!(empty.avg_time_us(), 0);
    }

    #[test]
    fn s26_2_profiler_start_stop() {
        let mut p = Profiler::new();
        p.start_function("foo");
        let dur = p.stop_function("foo").unwrap();
        let _ = dur; // duration is always non-negative

        let profiles = p.function_profiles();
        assert!(profiles.contains_key("foo"));
        assert_eq!(profiles["foo"].call_count, 1);
    }

    #[test]
    fn s26_3_profiler_stop_not_started() {
        let mut p = Profiler::new();
        let result = p.stop_function("bar");
        assert!(result.is_err());
    }

    #[test]
    fn s26_4_profiler_multiple_calls() {
        let mut p = Profiler::new();
        for _ in 0..5 {
            p.start_function("compute");
            p.stop_function("compute").unwrap();
        }
        assert_eq!(p.function_profiles()["compute"].call_count, 5);
    }

    #[test]
    fn s26_5_memory_snapshot() {
        let mut p = Profiler::new();
        p.snapshot_memory(1024, 256);
        p.snapshot_memory(2048, 512);

        let snaps = p.memory_snapshots();
        assert_eq!(snaps.len(), 2);
        assert_eq!(snaps[0].heap_bytes, 1024);
        assert_eq!(snaps[0].stack_bytes, 256);
        assert_eq!(snaps[0].total_bytes(), 1280);
        assert_eq!(snaps[1].total_bytes(), 2560);
    }

    #[test]
    fn s26_6_allocation_tracker() {
        let mut at = AllocationTracker::new();
        at.record_alloc(0x1000, 64);
        at.record_alloc(0x2000, 128);
        assert_eq!(at.live_count(), 2);
        assert_eq!(at.live_bytes(), 192);
        assert!(!at.is_leak_free());

        let freed = at.record_free(0x1000);
        assert_eq!(freed, Some(64));
        assert_eq!(at.live_count(), 1);

        at.record_free(0x2000);
        assert!(at.is_leak_free());
    }

    #[test]
    fn s26_7_allocation_tracker_double_free() {
        let mut at = AllocationTracker::new();
        at.record_alloc(0x1000, 64);
        at.record_free(0x1000);
        // Double-free returns None.
        assert_eq!(at.record_free(0x1000), None);
    }

    #[test]
    fn s26_8_hot_functions() {
        let mut p = Profiler::new();

        // Simulate function profiles directly.
        p.profiles.insert(
            "fast".to_string(),
            FunctionProfile {
                name: "fast".to_string(),
                total_time_us: 10,
                call_count: 1,
            },
        );
        p.profiles.insert(
            "slow".to_string(),
            FunctionProfile {
                name: "slow".to_string(),
                total_time_us: 1000,
                call_count: 1,
            },
        );
        p.profiles.insert(
            "medium".to_string(),
            FunctionProfile {
                name: "medium".to_string(),
                total_time_us: 500,
                call_count: 1,
            },
        );

        let hot = p.hot_functions(2);
        assert_eq!(hot.len(), 2);
        assert_eq!(hot[0].name, "slow");
        assert_eq!(hot[1].name, "medium");
    }

    #[test]
    fn s26_9_report_text_and_json() {
        let mut p = Profiler::new();
        p.start_function("main");
        p.stop_function("main").unwrap();
        p.snapshot_memory(512, 128);

        let report = ProfileReport::from_profiler(&p);
        let text = report.to_text();
        assert!(text.contains("Profiler Report"));
        assert!(text.contains("main"));

        let json = report.to_json();
        assert!(json.contains("\"name\": \"main\""));
    }

    #[test]
    fn s26_10_sampling_profiler() {
        let mut sp = SamplingProfiler::new(1000);
        sp.record_sample(vec!["main".to_string(), "compute".to_string()]);
        sp.record_sample(vec!["main".to_string(), "compute".to_string()]);
        sp.record_sample(vec!["main".to_string(), "io".to_string()]);

        assert_eq!(sp.sample_count(), 3);
        assert_eq!(sp.interval_us(), 1000);

        let hottest = sp.hottest_function().unwrap();
        assert_eq!(hottest, "compute");
    }
}
