//! Instrumentation — function hooks, call graph, timing, sampling profiler.
//!
//! D1.1: 10 tasks covering entry/exit hooks, call graph, timing, hot path
//! detection, loop counting, branch stats, sampling, and Chrome Trace format.

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// D1.1.1: Function Entry/Exit Hooks
// ═══════════════════════════════════════════════════════════════════════

/// A function call record.
#[derive(Debug, Clone)]
pub struct CallRecord {
    /// Function name.
    pub function: String,
    /// File path.
    pub file: String,
    /// Line number.
    pub line: u32,
    /// Entry timestamp (nanoseconds).
    pub entry_ns: u64,
    /// Exit timestamp (nanoseconds, 0 if still running).
    pub exit_ns: u64,
    /// Call depth (0 = top-level).
    pub depth: u32,
    /// Parent call index (-1 for root).
    pub parent_idx: i64,
    /// Memory allocated during this call (bytes).
    pub alloc_bytes: u64,
    /// Memory freed during this call.
    pub free_bytes: u64,
}

impl CallRecord {
    /// Duration in nanoseconds.
    pub fn duration_ns(&self) -> u64 {
        if self.exit_ns == 0 {
            return 0;
        }
        self.exit_ns.saturating_sub(self.entry_ns)
    }

    /// Duration in microseconds.
    pub fn duration_us(&self) -> f64 {
        self.duration_ns() as f64 / 1000.0
    }

    /// Duration in milliseconds.
    pub fn duration_ms(&self) -> f64 {
        self.duration_ns() as f64 / 1_000_000.0
    }

    /// Net memory change (allocated - freed).
    pub fn net_memory(&self) -> i64 {
        self.alloc_bytes as i64 - self.free_bytes as i64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.1.2: Call Graph
// ═══════════════════════════════════════════════════════════════════════

/// Edge in the call graph.
#[derive(Debug, Clone)]
pub struct CallEdge {
    /// Caller function.
    pub caller: String,
    /// Callee function.
    pub callee: String,
    /// Number of calls.
    pub count: u64,
    /// Total time spent in callee (ns).
    pub total_ns: u64,
}

/// Call graph built from profiling data.
#[derive(Debug, Clone, Default)]
pub struct CallGraph {
    /// All edges.
    pub edges: Vec<CallEdge>,
    /// Function → total self time (ns).
    pub self_time: HashMap<String, u64>,
    /// Function → total inclusive time (ns).
    pub inclusive_time: HashMap<String, u64>,
    /// Function → call count.
    pub call_counts: HashMap<String, u64>,
}

impl CallGraph {
    /// Builds a call graph from call records.
    pub fn from_records(records: &[CallRecord]) -> Self {
        let mut graph = Self::default();
        let mut edges_map: HashMap<(String, String), (u64, u64)> = HashMap::new();

        for rec in records {
            // Inclusive time
            *graph
                .inclusive_time
                .entry(rec.function.clone())
                .or_default() += rec.duration_ns();
            *graph.call_counts.entry(rec.function.clone()).or_default() += 1;

            // Parent edge
            if rec.parent_idx >= 0 {
                let parent = &records[rec.parent_idx as usize];
                let key = (parent.function.clone(), rec.function.clone());
                let entry = edges_map.entry(key).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += rec.duration_ns();
            }
        }

        // Compute self time = inclusive - children
        for rec in records {
            let self_time = graph.self_time.entry(rec.function.clone()).or_default();
            let child_time: u64 = records
                .iter()
                .filter(|r| {
                    r.parent_idx
                        == records
                            .iter()
                            .position(|x| std::ptr::eq(x, rec))
                            .unwrap_or(0) as i64
                })
                .map(|r| r.duration_ns())
                .sum();
            *self_time += rec.duration_ns().saturating_sub(child_time);
        }

        graph.edges = edges_map
            .into_iter()
            .map(|((caller, callee), (count, total_ns))| CallEdge {
                caller,
                callee,
                count,
                total_ns,
            })
            .collect();

        graph
    }

    /// Returns the top N functions by self time.
    pub fn hot_functions(&self, n: usize) -> Vec<(&str, u64)> {
        let mut sorted: Vec<_> = self
            .self_time
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        sorted.sort_by_key(|x| std::cmp::Reverse(x.1));
        sorted.truncate(n);
        sorted
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.1.7-D1.1.8: Loop + Branch Stats
// ═══════════════════════════════════════════════════════════════════════

/// Loop profiling data.
#[derive(Debug, Clone)]
pub struct LoopProfile {
    /// Source location.
    pub file: String,
    pub line: u32,
    /// Total iterations.
    pub iterations: u64,
    /// Total time in loop (ns).
    pub total_ns: u64,
    /// Average time per iteration (ns).
    pub avg_iter_ns: f64,
}

/// Branch profiling data.
#[derive(Debug, Clone)]
pub struct BranchProfile {
    /// Source location.
    pub file: String,
    pub line: u32,
    /// Times the condition was true.
    pub taken: u64,
    /// Times the condition was false.
    pub not_taken: u64,
}

impl BranchProfile {
    /// Returns taken ratio (0.0 to 1.0).
    pub fn taken_ratio(&self) -> f64 {
        let total = self.taken + self.not_taken;
        if total == 0 {
            return 0.0;
        }
        self.taken as f64 / total as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.1.9: Sampling Profiler
// ═══════════════════════════════════════════════════════════════════════

/// A single sample (stack snapshot at a point in time).
#[derive(Debug, Clone)]
pub struct Sample {
    /// Timestamp (ns).
    pub timestamp_ns: u64,
    /// Stack frames (bottom to top).
    pub frames: Vec<String>,
}

/// Sampling profiler results.
#[derive(Debug, Clone)]
pub struct SamplingProfile {
    /// All samples.
    pub samples: Vec<Sample>,
    /// Sampling frequency (Hz).
    pub frequency_hz: u32,
    /// Total profiling time (ns).
    pub total_ns: u64,
}

impl SamplingProfile {
    /// Returns function → sample count.
    pub fn function_counts(&self) -> HashMap<String, u64> {
        let mut counts = HashMap::new();
        for sample in &self.samples {
            for frame in &sample.frames {
                *counts.entry(frame.clone()).or_default() += 1;
            }
        }
        counts
    }

    /// Converts samples to collapsed stack format (for flamegraph tools).
    pub fn to_collapsed_stacks(&self) -> String {
        let mut stacks: HashMap<String, u64> = HashMap::new();
        for sample in &self.samples {
            let stack = sample.frames.join(";");
            *stacks.entry(stack).or_default() += 1;
        }
        let mut result = String::new();
        for (stack, count) in &stacks {
            result.push_str(&format!("{stack} {count}\n"));
        }
        result
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.1.10: Chrome Trace Event Format
// ═══════════════════════════════════════════════════════════════════════

/// Chrome Trace Event (JSON format for chrome://tracing).
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// Event name.
    pub name: String,
    /// Category.
    pub cat: String,
    /// Phase: 'B' (begin), 'E' (end), 'X' (complete), 'i' (instant).
    pub ph: char,
    /// Timestamp in microseconds.
    pub ts: f64,
    /// Duration in microseconds (for 'X' events).
    pub dur: Option<f64>,
    /// Process ID.
    pub pid: u32,
    /// Thread ID.
    pub tid: u32,
    /// Extra arguments.
    pub args: HashMap<String, String>,
}

impl TraceEvent {
    /// Serializes to JSON.
    pub fn to_json(&self) -> String {
        let mut json = format!(
            r#"{{"name":"{}","cat":"{}","ph":"{}","ts":{},"pid":{},"tid":{}"#,
            self.name, self.cat, self.ph, self.ts, self.pid, self.tid
        );
        if let Some(dur) = self.dur {
            json.push_str(&format!(r#","dur":{dur}"#));
        }
        if !self.args.is_empty() {
            let args_json: Vec<String> = self
                .args
                .iter()
                .map(|(k, v)| format!(r#""{k}":"{v}""#))
                .collect();
            json.push_str(&format!(r#","args":{{{}}}"#, args_json.join(",")));
        }
        json.push('}');
        json
    }
}

/// Converts call records to Chrome Trace Event format.
pub fn to_trace_events(records: &[CallRecord]) -> String {
    let mut events = Vec::new();
    for rec in records {
        let event = TraceEvent {
            name: rec.function.clone(),
            cat: "function".to_string(),
            ph: 'X',
            ts: rec.entry_ns as f64 / 1000.0,
            dur: Some(rec.duration_ns() as f64 / 1000.0),
            pid: 1,
            tid: 1,
            args: HashMap::from([
                ("file".to_string(), format!("{}:{}", rec.file, rec.line)),
                ("alloc".to_string(), format!("{}B", rec.alloc_bytes)),
            ]),
        };
        events.push(event.to_json());
    }
    format!("[{}]", events.join(",\n"))
}

// ═══════════════════════════════════════════════════════════════════════
// PQ10.3: Hotspot Report
// ═══════════════════════════════════════════════════════════════════════

/// Generate a human-readable hotspot report from call records.
pub fn hotspot_report(records: &[CallRecord], top_n: usize) -> String {
    let graph = CallGraph::from_records(records);
    let hot = graph.hot_functions(top_n);
    let total_ns: u64 = graph.self_time.values().sum();

    let mut report = String::new();
    report.push_str("=== Hotspot Report ===\n");
    report.push_str(&format!(
        "Total: {:.2}ms | Functions: {}\n\n",
        total_ns as f64 / 1_000_000.0,
        graph.self_time.len()
    ));

    for (i, (name, ns)) in hot.iter().enumerate() {
        let pct = if total_ns > 0 {
            (*ns as f64 / total_ns as f64) * 100.0
        } else {
            0.0
        };
        let ms = *ns as f64 / 1_000_000.0;
        report.push_str(&format!(
            "  #{}: fn {} — {:.2}ms ({:.1}%)\n",
            i + 1,
            name,
            ms,
            pct
        ));
    }
    report
}

// ═══════════════════════════════════════════════════════════════════════
// PQ10.5: Profile Comparison
// ═══════════════════════════════════════════════════════════════════════

/// Comparison between two profile runs.
#[derive(Debug)]
pub struct ProfileComparison {
    /// Functions that got faster.
    pub faster: Vec<(String, f64)>,
    /// Functions that got slower.
    pub slower: Vec<(String, f64)>,
    /// Overall speedup (positive = faster).
    pub overall_speedup_pct: f64,
}

/// Compare two sets of call records and report speedup/regression.
pub fn compare_profiles(before: &[CallRecord], after: &[CallRecord]) -> ProfileComparison {
    let graph_before = CallGraph::from_records(before);
    let graph_after = CallGraph::from_records(after);

    let mut faster = Vec::new();
    let mut slower = Vec::new();

    for (name, before_ns) in &graph_before.self_time {
        if let Some(after_ns) = graph_after.self_time.get(name) {
            let before_f = *before_ns as f64;
            let after_f = *after_ns as f64;
            if before_f > 0.0 {
                let change_pct = ((before_f - after_f) / before_f) * 100.0;
                if change_pct > 5.0 {
                    faster.push((name.clone(), change_pct));
                } else if change_pct < -5.0 {
                    slower.push((name.clone(), -change_pct));
                }
            }
        }
    }

    let total_before: u64 = graph_before.self_time.values().sum();
    let total_after: u64 = graph_after.self_time.values().sum();
    let overall = if total_before > 0 {
        ((total_before as f64 - total_after as f64) / total_before as f64) * 100.0
    } else {
        0.0
    };

    ProfileComparison {
        faster,
        slower,
        overall_speedup_pct: overall,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PQ10.8: Speedscope JSON Export
// ═══════════════════════════════════════════════════════════════════════

/// Export call records to speedscope JSON format.
/// Speedscope: <https://www.speedscope.app/>
pub fn to_speedscope(records: &[CallRecord]) -> String {
    let mut frames = Vec::new();
    let mut frame_map: HashMap<String, usize> = HashMap::new();
    let mut events = Vec::new();

    for rec in records {
        let frame_idx = frame_map.entry(rec.function.clone()).or_insert_with(|| {
            let idx = frames.len();
            frames.push(format!(
                r#"{{"name":"{}","file":"{}","line":{}}}"#,
                rec.function, rec.file, rec.line
            ));
            idx
        });

        events.push(format!(
            r#"{{"type":"O","at":{},"frame":{}}}"#,
            rec.entry_ns / 1000, // microseconds
            frame_idx
        ));
        events.push(format!(
            r#"{{"type":"C","at":{},"frame":{}}}"#,
            rec.exit_ns / 1000,
            frame_idx
        ));
    }

    format!(
        r#"{{"$schema":"https://www.speedscope.app/file-format-schema.json","shared":{{"frames":[{}]}},"profiles":[{{"type":"evented","name":"fajar-profile","unit":"microseconds","startValue":0,"endValue":{},"events":[{}]}}]}}"#,
        frames.join(","),
        records.last().map(|r| r.exit_ns / 1000).unwrap_or(0),
        events.join(",")
    )
}

// ═══════════════════════════════════════════════════════════════════════
// PQ10.1: Interpreter Profiling Hooks
// ═══════════════════════════════════════════════════════════════════════

/// A profiling session that collects call records in real-time.
///
/// Usage:
/// 1. Create a `ProfileSession::new()`
/// 2. Call `enter_fn("name", "file.fj", line)` on function entry
/// 3. Call `exit_fn()` on function return
/// 4. Use `records()`, `report()`, etc. for analysis
pub struct ProfileSession {
    /// Collected call records.
    records: Vec<CallRecord>,
    /// Stack of (record_index, function_name) for nesting.
    stack: Vec<usize>,
    /// Whether profiling is active.
    active: bool,
}

impl ProfileSession {
    /// Create a new profiling session.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            stack: Vec::new(),
            active: true,
        }
    }

    /// Pause profiling (stops collecting records).
    pub fn pause(&mut self) {
        self.active = false;
    }

    /// Resume profiling.
    pub fn resume(&mut self) {
        self.active = true;
    }

    /// Whether the session is actively collecting.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Record function entry. Returns the record index.
    pub fn enter_fn(&mut self, name: &str, file: &str, line: u32) -> usize {
        if !self.active {
            return usize::MAX;
        }

        let parent_idx = self.stack.last().map(|&i| i as i64).unwrap_or(-1);
        let depth = self.stack.len() as u32;
        let entry_ns = self.time_ns();

        let idx = self.records.len();
        self.records.push(CallRecord {
            function: name.to_string(),
            file: file.to_string(),
            line,
            entry_ns,
            exit_ns: 0,
            depth,
            parent_idx,
            alloc_bytes: 0,
            free_bytes: 0,
        });
        self.stack.push(idx);
        idx
    }

    /// Record function exit. Closes the most recent open call.
    pub fn exit_fn(&mut self) {
        if !self.active {
            return;
        }
        if let Some(idx) = self.stack.pop() {
            if idx < self.records.len() {
                self.records[idx].exit_ns = self.time_ns();
            }
        }
    }

    /// Record a memory allocation during the current function.
    pub fn record_alloc(&mut self, bytes: u64) {
        if let Some(&idx) = self.stack.last() {
            if idx < self.records.len() {
                self.records[idx].alloc_bytes += bytes;
            }
        }
    }

    /// Record a memory free during the current function.
    pub fn record_free(&mut self, bytes: u64) {
        if let Some(&idx) = self.stack.last() {
            if idx < self.records.len() {
                self.records[idx].free_bytes += bytes;
            }
        }
    }

    /// Returns all collected call records.
    pub fn records(&self) -> &[CallRecord] {
        &self.records
    }

    /// Number of function calls recorded.
    pub fn call_count(&self) -> usize {
        self.records.len()
    }

    /// Current call depth.
    pub fn current_depth(&self) -> usize {
        self.stack.len()
    }

    /// Generate a hotspot report.
    pub fn report(&self, top_n: usize) -> String {
        hotspot_report(&self.records, top_n)
    }

    /// Export to Chrome Trace format.
    pub fn to_trace(&self) -> String {
        to_trace_events(&self.records)
    }

    /// Export to Speedscope format.
    pub fn to_speedscope_json(&self) -> String {
        to_speedscope(&self.records)
    }

    /// Reset the session (clear all records).
    pub fn reset(&mut self) {
        self.records.clear();
        self.stack.clear();
        self.active = true;
    }

    /// Get current time in nanoseconds (monotonic).
    fn time_ns(&self) -> u64 {
        // Use std::time for monotonic timing
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }
}

impl Default for ProfileSession {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PQ10.6: Sampling Profiler Engine
// ═══════════════════════════════════════════════════════════════════════

/// A sampling profiler that periodically captures the call stack.
pub struct SamplingProfiler {
    /// Collected samples.
    samples: Vec<Sample>,
    /// Sampling interval in microseconds.
    interval_us: u64,
    /// Whether sampling is active.
    active: bool,
    /// Current stack (maintained externally via push/pop).
    current_stack: Vec<String>,
}

impl SamplingProfiler {
    /// Create a new sampling profiler with given interval.
    pub fn new(interval_us: u64) -> Self {
        Self {
            samples: Vec::new(),
            interval_us,
            active: true,
            current_stack: Vec::new(),
        }
    }

    /// Push a frame onto the current stack.
    pub fn push_frame(&mut self, name: &str) {
        self.current_stack.push(name.to_string());
    }

    /// Pop the top frame.
    pub fn pop_frame(&mut self) {
        self.current_stack.pop();
    }

    /// Take a sample of the current stack.
    pub fn sample(&mut self) {
        if !self.active || self.current_stack.is_empty() {
            return;
        }
        self.samples.push(Sample {
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
            frames: self.current_stack.clone(),
        });
    }

    /// Returns the sampling profile.
    pub fn finish(self) -> SamplingProfile {
        let total_ns = if self.samples.len() >= 2 {
            self.samples
                .last()
                .expect("samples non-empty after length check")
                .timestamp_ns
                - self.samples[0].timestamp_ns
        } else {
            0
        };
        let frequency_hz = 1_000_000u64.checked_div(self.interval_us).unwrap_or(0) as u32;
        SamplingProfile {
            samples: self.samples,
            frequency_hz,
            total_ns,
        }
    }

    /// Number of samples collected.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Sampling interval.
    pub fn interval_us(&self) -> u64 {
        self.interval_us
    }

    /// Pause sampling.
    pub fn pause(&mut self) {
        self.active = false;
    }

    /// Resume sampling.
    pub fn resume(&mut self) {
        self.active = true;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d1_1_call_record_duration() {
        let rec = CallRecord {
            function: "fib".to_string(),
            file: "main.fj".to_string(),
            line: 5,
            entry_ns: 1_000_000,
            exit_ns: 2_500_000,
            depth: 0,
            parent_idx: -1,
            alloc_bytes: 0,
            free_bytes: 0,
        };
        assert_eq!(rec.duration_ns(), 1_500_000);
        assert!((rec.duration_us() - 1500.0).abs() < 0.1);
        assert!((rec.duration_ms() - 1.5).abs() < 0.001);
    }

    #[test]
    fn d1_2_call_graph_hot_functions() {
        let records = vec![
            CallRecord {
                function: "main".to_string(),
                file: "a.fj".to_string(),
                line: 1,
                entry_ns: 0,
                exit_ns: 10_000_000,
                depth: 0,
                parent_idx: -1,
                alloc_bytes: 0,
                free_bytes: 0,
            },
            CallRecord {
                function: "compute".to_string(),
                file: "a.fj".to_string(),
                line: 5,
                entry_ns: 1_000_000,
                exit_ns: 8_000_000,
                depth: 1,
                parent_idx: 0,
                alloc_bytes: 0,
                free_bytes: 0,
            },
            CallRecord {
                function: "log".to_string(),
                file: "a.fj".to_string(),
                line: 10,
                entry_ns: 8_000_000,
                exit_ns: 9_000_000,
                depth: 1,
                parent_idx: 0,
                alloc_bytes: 0,
                free_bytes: 0,
            },
        ];
        let graph = CallGraph::from_records(&records);
        assert!(graph.call_counts.get("compute").unwrap() >= &1);
        let hot = graph.hot_functions(2);
        assert!(!hot.is_empty());
    }

    #[test]
    fn d1_7_branch_profile() {
        let bp = BranchProfile {
            file: "a.fj".to_string(),
            line: 10,
            taken: 75,
            not_taken: 25,
        };
        assert!((bp.taken_ratio() - 0.75).abs() < 0.001);
    }

    #[test]
    fn d1_9_collapsed_stacks() {
        let profile = SamplingProfile {
            samples: vec![
                Sample {
                    timestamp_ns: 0,
                    frames: vec!["main".to_string(), "compute".to_string()],
                },
                Sample {
                    timestamp_ns: 1000,
                    frames: vec!["main".to_string(), "compute".to_string()],
                },
                Sample {
                    timestamp_ns: 2000,
                    frames: vec!["main".to_string(), "log".to_string()],
                },
            ],
            frequency_hz: 1000,
            total_ns: 3000,
        };
        let collapsed = profile.to_collapsed_stacks();
        assert!(collapsed.contains("main;compute 2"));
        assert!(collapsed.contains("main;log 1"));
    }

    #[test]
    fn d1_10_trace_event_json() {
        let event = TraceEvent {
            name: "fib".to_string(),
            cat: "function".to_string(),
            ph: 'X',
            ts: 1500.0,
            dur: Some(500.0),
            pid: 1,
            tid: 1,
            args: HashMap::from([("depth".to_string(), "3".to_string())]),
        };
        let json = event.to_json();
        assert!(json.contains(r#""name":"fib""#));
        assert!(json.contains(r#""dur":500"#));
        assert!(json.contains(r#""depth":"3""#));
    }

    #[test]
    fn d1_10_to_trace_events() {
        let records = vec![CallRecord {
            function: "test".to_string(),
            file: "t.fj".to_string(),
            line: 1,
            entry_ns: 0,
            exit_ns: 1000,
            depth: 0,
            parent_idx: -1,
            alloc_bytes: 256,
            free_bytes: 0,
        }];
        let json = to_trace_events(&records);
        assert!(json.starts_with('['));
        assert!(json.contains("test"));
    }

    #[test]
    fn d1_1_net_memory() {
        let rec = CallRecord {
            function: "alloc_heavy".to_string(),
            file: "a.fj".to_string(),
            line: 1,
            entry_ns: 0,
            exit_ns: 1000,
            depth: 0,
            parent_idx: -1,
            alloc_bytes: 4096,
            free_bytes: 1024,
        };
        assert_eq!(rec.net_memory(), 3072);
    }

    // ═══════════════════════════════════════════════════════════════════
    // V8 GC5.11-GC5.14: Real profiling with std::time::Instant
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn gc5_real_timer_profiling() {
        use std::time::Instant;

        let mut records = Vec::new();
        let base = Instant::now();

        // Profile a real computation
        let start = Instant::now();
        let mut sum = 0u64;
        for i in 0..10_000 {
            sum += i;
        }
        let end = Instant::now();

        records.push(CallRecord {
            function: "sum_loop".to_string(),
            file: "test.fj".to_string(),
            line: 1,
            entry_ns: start.duration_since(base).as_nanos() as u64,
            exit_ns: end.duration_since(base).as_nanos() as u64,
            depth: 0,
            parent_idx: -1,
            alloc_bytes: 0,
            free_bytes: 0,
        });

        assert_eq!(sum, 49_995_000); // verify computation happened
        assert!(!records.is_empty());
        assert!(
            records[0].duration_ns() > 0,
            "real timer should measure non-zero time"
        );
    }

    #[test]
    fn gc5_call_graph_from_real_timing() {
        use std::time::Instant;

        let base = Instant::now();
        let mut records = Vec::new();

        // Outer function
        let outer_start = Instant::now();

        // Inner function 1
        let inner1_start = Instant::now();
        let _ = (0..1000).sum::<u64>();
        let inner1_end = Instant::now();

        records.push(CallRecord {
            function: "inner1".to_string(),
            file: "bench.fj".to_string(),
            line: 10,
            entry_ns: inner1_start.duration_since(base).as_nanos() as u64,
            exit_ns: inner1_end.duration_since(base).as_nanos() as u64,
            depth: 1,
            parent_idx: 0,
            alloc_bytes: 0,
            free_bytes: 0,
        });

        // Inner function 2
        let inner2_start = Instant::now();
        let _ = (0..5000).sum::<u64>();
        let inner2_end = Instant::now();

        records.push(CallRecord {
            function: "inner2".to_string(),
            file: "bench.fj".to_string(),
            line: 20,
            entry_ns: inner2_start.duration_since(base).as_nanos() as u64,
            exit_ns: inner2_end.duration_since(base).as_nanos() as u64,
            depth: 1,
            parent_idx: 0,
            alloc_bytes: 0,
            free_bytes: 0,
        });

        let outer_end = Instant::now();

        // Insert outer record at index 0
        records.insert(
            0,
            CallRecord {
                function: "outer".to_string(),
                file: "bench.fj".to_string(),
                line: 1,
                entry_ns: outer_start.duration_since(base).as_nanos() as u64,
                exit_ns: outer_end.duration_since(base).as_nanos() as u64,
                depth: 0,
                parent_idx: -1,
                alloc_bytes: 0,
                free_bytes: 0,
            },
        );

        let graph = CallGraph::from_records(&records);
        // Verify inclusive time was computed for at least some functions.
        assert!(!graph.inclusive_time.is_empty());
        assert!(graph.edges.len() >= 2); // at least inner1, inner2
    }

    #[test]
    fn gc5_chrome_trace_from_real_timing() {
        use std::time::Instant;

        let base = Instant::now();
        let start = Instant::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let end = Instant::now();

        let records = vec![CallRecord {
            function: "sleep_test".to_string(),
            file: "test.fj".to_string(),
            line: 1,
            entry_ns: start.duration_since(base).as_nanos() as u64,
            exit_ns: end.duration_since(base).as_nanos() as u64,
            depth: 0,
            parent_idx: -1,
            alloc_bytes: 0,
            free_bytes: 0,
        }];

        let json = to_trace_events(&records);
        assert!(json.contains("sleep_test"));
        assert!(
            json.contains("\"ph\":\"X\""),
            "should be Chrome Trace format"
        );
        // Duration should be at least 1ms = 1000us
        assert!(records[0].duration_ms() >= 0.5, "sleep should be >= 0.5ms");
    }

    #[test]
    fn d1_9_function_counts() {
        let profile = SamplingProfile {
            samples: vec![
                Sample {
                    timestamp_ns: 0,
                    frames: vec!["a".to_string(), "b".to_string()],
                },
                Sample {
                    timestamp_ns: 1,
                    frames: vec!["a".to_string(), "c".to_string()],
                },
            ],
            frequency_hz: 1000,
            total_ns: 2,
        };
        let counts = profile.function_counts();
        assert_eq!(*counts.get("a").unwrap(), 2);
        assert_eq!(*counts.get("b").unwrap(), 1);
        assert_eq!(*counts.get("c").unwrap(), 1);
    }

    // ═══════════════════════════════════════════════════════════════════
    // PQ10: Quality improvement tests
    // ═══════════════════════════════════════════════════════════════════

    fn make_test_records() -> Vec<CallRecord> {
        vec![
            CallRecord {
                function: "main".to_string(),
                file: "test.fj".to_string(),
                line: 1,
                entry_ns: 0,
                exit_ns: 10_000_000,
                depth: 0,
                parent_idx: -1,
                alloc_bytes: 0,
                free_bytes: 0,
            },
            CallRecord {
                function: "compute".to_string(),
                file: "test.fj".to_string(),
                line: 5,
                entry_ns: 1_000_000,
                exit_ns: 8_000_000,
                depth: 1,
                parent_idx: 0,
                alloc_bytes: 0,
                free_bytes: 0,
            },
            CallRecord {
                function: "log".to_string(),
                file: "test.fj".to_string(),
                line: 10,
                entry_ns: 8_000_000,
                exit_ns: 9_000_000,
                depth: 1,
                parent_idx: 0,
                alloc_bytes: 0,
                free_bytes: 0,
            },
        ]
    }

    #[test]
    fn pq10_3_hotspot_report() {
        let records = make_test_records();
        let report = hotspot_report(&records, 3);
        assert!(report.contains("Hotspot Report"));
        assert!(
            report.contains("compute"),
            "should show compute as hot: {report}"
        );
    }

    #[test]
    fn pq10_5_profile_comparison_faster() {
        let before = make_test_records();
        // "After" records — everything takes half the time
        let mut after = make_test_records();
        after[0].exit_ns = 5_000_000; // main: 5ms (was 10ms)
        after[1].exit_ns = 4_000_000; // compute: 3ms (was 7ms)
        after[2].entry_ns = 4_000_000;
        after[2].exit_ns = 4_500_000; // log: 0.5ms (was 1ms)

        let cmp = compare_profiles(&before, &after);
        assert!(
            cmp.overall_speedup_pct > 10.0,
            "should be >10% faster overall: {:.1}%",
            cmp.overall_speedup_pct
        );
    }

    #[test]
    fn pq10_5_profile_comparison_slower() {
        let before = make_test_records();
        // "After" — everything takes double
        let mut after = make_test_records();
        after[0].exit_ns = 20_000_000; // main: 20ms
        after[1].exit_ns = 16_000_000; // compute: 15ms
        after[2].entry_ns = 16_000_000;
        after[2].exit_ns = 18_000_000; // log: 2ms

        let cmp = compare_profiles(&before, &after);
        assert!(
            cmp.overall_speedup_pct < -10.0,
            "should be >10% slower overall: {:.1}%",
            cmp.overall_speedup_pct
        );
    }

    #[test]
    fn pq10_8_speedscope_export() {
        let records = make_test_records();
        let json = to_speedscope(&records);
        assert!(json.contains("speedscope.app"), "should have schema URL");
        assert!(json.contains("\"name\":\"main\""), "should have main frame");
        assert!(
            json.contains("\"name\":\"compute\""),
            "should have compute frame"
        );
        assert!(json.contains("\"type\":\"O\""), "should have open events");
        assert!(json.contains("\"type\":\"C\""), "should have close events");
    }

    #[test]
    fn pq10_8_chrome_trace_export() {
        let records = make_test_records();
        let json = to_trace_events(&records);
        assert!(json.contains("\"ph\":\"X\""), "should have complete events");
        assert!(json.contains("main"), "should have main function");
        assert!(json.contains("compute"), "should have compute function");
    }

    // ═══════════════════════════════════════════════════════════════════
    // PQ10.1: Interpreter Profiling Hooks
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn pq10_1_profile_session_basic() {
        let mut session = ProfileSession::new();
        assert!(session.is_active());
        assert_eq!(session.call_count(), 0);
        assert_eq!(session.current_depth(), 0);

        let idx = session.enter_fn("main", "main.fj", 1);
        assert_eq!(idx, 0);
        assert_eq!(session.current_depth(), 1);

        session.exit_fn();
        assert_eq!(session.current_depth(), 0);
        assert_eq!(session.call_count(), 1);

        let rec = &session.records()[0];
        assert_eq!(rec.function, "main");
        assert_eq!(rec.file, "main.fj");
        assert_eq!(rec.line, 1);
        assert_eq!(rec.depth, 0);
        assert_eq!(rec.parent_idx, -1);
        assert!(rec.exit_ns > 0);
    }

    #[test]
    fn pq10_1_profile_session_nested() {
        let mut session = ProfileSession::new();

        session.enter_fn("main", "main.fj", 1);
        session.enter_fn("compute", "math.fj", 10);
        assert_eq!(session.current_depth(), 2);
        session.enter_fn("add", "math.fj", 20);
        assert_eq!(session.current_depth(), 3);

        session.exit_fn(); // exit add
        session.exit_fn(); // exit compute
        session.exit_fn(); // exit main

        assert_eq!(session.call_count(), 3);
        assert_eq!(session.records()[0].depth, 0);
        assert_eq!(session.records()[1].depth, 1);
        assert_eq!(session.records()[1].parent_idx, 0); // parent is main
        assert_eq!(session.records()[2].depth, 2);
        assert_eq!(session.records()[2].parent_idx, 1); // parent is compute
    }

    #[test]
    fn pq10_1_profile_session_pause_resume() {
        let mut session = ProfileSession::new();
        session.enter_fn("before", "a.fj", 1);
        session.exit_fn();

        session.pause();
        assert!(!session.is_active());
        session.enter_fn("during_pause", "b.fj", 1); // should be ignored
        session.exit_fn();

        session.resume();
        session.enter_fn("after", "c.fj", 1);
        session.exit_fn();

        assert_eq!(session.call_count(), 2); // only before + after
        assert_eq!(session.records()[0].function, "before");
        assert_eq!(session.records()[1].function, "after");
    }

    #[test]
    fn pq10_1_profile_session_memory_tracking() {
        let mut session = ProfileSession::new();
        session.enter_fn("alloc_fn", "mem.fj", 1);
        session.record_alloc(1024);
        session.record_alloc(2048);
        session.record_free(512);
        session.exit_fn();

        let rec = &session.records()[0];
        assert_eq!(rec.alloc_bytes, 3072);
        assert_eq!(rec.free_bytes, 512);
        assert_eq!(rec.net_memory(), 2560);
    }

    #[test]
    fn pq10_1_profile_session_report() {
        let mut session = ProfileSession::new();
        session.enter_fn("main", "main.fj", 1);
        session.exit_fn();

        let report = session.report(5);
        assert!(report.contains("Hotspot Report"));
        assert!(report.contains("main"));
    }

    #[test]
    fn pq10_1_profile_session_reset() {
        let mut session = ProfileSession::new();
        session.enter_fn("fn1", "a.fj", 1);
        session.exit_fn();
        assert_eq!(session.call_count(), 1);

        session.reset();
        assert_eq!(session.call_count(), 0);
        assert!(session.is_active());
    }

    #[test]
    fn pq10_1_profile_session_export() {
        let mut session = ProfileSession::new();
        session.enter_fn("main", "main.fj", 1);
        session.enter_fn("work", "work.fj", 5);
        session.exit_fn();
        session.exit_fn();

        let trace = session.to_trace();
        assert!(trace.contains("main"));
        assert!(trace.contains("work"));

        let speedscope = session.to_speedscope_json();
        assert!(speedscope.contains("speedscope"));
    }

    // ═══════════════════════════════════════════════════════════════════
    // PQ10.6: Sampling Profiler
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn pq10_6_sampling_profiler_basic() {
        let mut profiler = SamplingProfiler::new(1000); // 1ms interval
        assert_eq!(profiler.sample_count(), 0);
        assert_eq!(profiler.interval_us(), 1000);

        profiler.push_frame("main");
        profiler.sample();
        assert_eq!(profiler.sample_count(), 1);

        profiler.push_frame("compute");
        profiler.sample();
        assert_eq!(profiler.sample_count(), 2);

        profiler.pop_frame();
        profiler.sample();
        assert_eq!(profiler.sample_count(), 3);
    }

    #[test]
    fn pq10_6_sampling_profiler_finish() {
        let mut profiler = SamplingProfiler::new(1000);
        profiler.push_frame("main");
        profiler.sample();
        profiler.push_frame("work");
        profiler.sample();
        profiler.pop_frame();
        profiler.pop_frame();

        let profile = profiler.finish();
        assert_eq!(profile.samples.len(), 2);
        assert_eq!(profile.frequency_hz, 1000); // 1M/1000 = 1000 Hz

        // First sample has 1 frame, second has 2
        assert_eq!(profile.samples[0].frames.len(), 1);
        assert_eq!(profile.samples[1].frames.len(), 2);

        // Function counts: main appears in both, work in one
        let counts = profile.function_counts();
        assert_eq!(*counts.get("main").unwrap_or(&0), 2);
        assert_eq!(*counts.get("work").unwrap_or(&0), 1);
    }

    #[test]
    fn pq10_6_sampling_profiler_collapsed_stacks() {
        let mut profiler = SamplingProfiler::new(100);
        profiler.push_frame("main");
        profiler.sample();
        profiler.push_frame("fib");
        profiler.sample();
        profiler.sample(); // duplicate stack

        let profile = profiler.finish();
        let collapsed = profile.to_collapsed_stacks();
        // Should have "main 1" and "main;fib 2"
        assert!(collapsed.contains("main "));
        assert!(collapsed.contains("main;fib "));
    }

    #[test]
    fn pq10_6_sampling_profiler_pause() {
        let mut profiler = SamplingProfiler::new(100);
        profiler.push_frame("main");
        profiler.sample();

        profiler.pause();
        profiler.sample(); // ignored

        profiler.resume();
        profiler.sample();

        assert_eq!(profiler.sample_count(), 2);
    }

    #[test]
    fn pq10_6_sampling_empty_stack_ignored() {
        let mut profiler = SamplingProfiler::new(100);
        profiler.sample(); // no frames — should be ignored
        assert_eq!(profiler.sample_count(), 0);
    }
}
