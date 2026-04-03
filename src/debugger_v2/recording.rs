//! Execution Recording — record mode, event log, delta compression,
//! binary format, overhead control, selective recording, size limits,
//! I/O capture, thread recording.

// ═══════════════════════════════════════════════════════════════════════
// S25.1: Record Mode
// ═══════════════════════════════════════════════════════════════════════

/// Recording configuration.
#[derive(Debug, Clone)]
pub struct RecordConfig {
    /// Output recording file path.
    pub output_path: String,
    /// Whether recording is enabled.
    pub enabled: bool,
    /// Maximum recording size in bytes (0 = unlimited).
    pub max_size_bytes: usize,
    /// Whether to use ring-buffer mode for long runs.
    pub ring_buffer: bool,
    /// Selective recording — only record annotated functions.
    pub selective: bool,
    /// Record I/O operations.
    pub capture_io: bool,
    /// Record thread scheduling.
    pub capture_threads: bool,
}

impl Default for RecordConfig {
    fn default() -> Self {
        Self {
            output_path: "recording.fjrec".to_string(),
            enabled: false,
            max_size_bytes: 0,
            ring_buffer: false,
            selective: false,
            capture_io: true,
            capture_threads: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.2: Event Log
// ═══════════════════════════════════════════════════════════════════════

/// An execution event.
#[derive(Debug, Clone)]
pub struct RecordEvent {
    /// Sequence number.
    pub seq: u64,
    /// Timestamp (nanoseconds from start).
    pub timestamp_ns: u64,
    /// Thread ID.
    pub thread_id: u32,
    /// Event kind.
    pub kind: EventKind,
}

/// Kind of recorded event.
#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    /// Function entry.
    FnEntry {
        /// Function name.
        name: String,
        /// Source location (file:line).
        location: String,
    },
    /// Function exit.
    FnExit {
        /// Function name.
        name: String,
        /// Return value (serialized).
        return_value: Option<String>,
    },
    /// Variable assignment.
    VarAssign {
        /// Variable name.
        name: String,
        /// New value (serialized).
        value: String,
        /// Scope depth.
        scope: u32,
    },
    /// Heap allocation.
    HeapAlloc {
        /// Address.
        addr: u64,
        /// Size in bytes.
        size: usize,
    },
    /// Heap deallocation.
    HeapFree {
        /// Address.
        addr: u64,
    },
    /// I/O operation.
    IoOp {
        /// Operation kind.
        op: IoOpKind,
        /// Data (truncated).
        data: Vec<u8>,
    },
    /// Thread scheduling event.
    ThreadEvent {
        /// Thread event kind.
        kind: ThreadEventKind,
    },
}

/// I/O operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoOpKind {
    /// File read.
    FileRead,
    /// File write.
    FileWrite,
    /// Network send.
    NetSend,
    /// Network receive.
    NetRecv,
    /// Stdin read.
    StdinRead,
    /// Stdout write.
    StdoutWrite,
}

/// Thread scheduling event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadEventKind {
    /// Thread spawned.
    Spawn,
    /// Thread joined.
    Join,
    /// Context switch.
    Switch,
    /// Mutex acquired.
    MutexAcquire,
    /// Mutex released.
    MutexRelease,
}

/// An event log that accumulates recorded events.
#[derive(Debug, Clone)]
pub struct EventLog {
    /// All events in order.
    events: Vec<RecordEvent>,
    /// Next sequence number.
    next_seq: u64,
}

impl EventLog {
    /// Creates a new empty event log.
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            next_seq: 0,
        }
    }

    /// Records an event.
    pub fn record(&mut self, timestamp_ns: u64, thread_id: u32, kind: EventKind) {
        self.events.push(RecordEvent {
            seq: self.next_seq,
            timestamp_ns,
            thread_id,
            kind,
        });
        self.next_seq += 1;
    }

    /// Returns event count.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Gets event by sequence number.
    pub fn get(&self, seq: u64) -> Option<&RecordEvent> {
        self.events.iter().find(|e| e.seq == seq)
    }

    /// Returns all events for a thread.
    pub fn events_for_thread(&self, thread_id: u32) -> Vec<&RecordEvent> {
        self.events
            .iter()
            .filter(|e| e.thread_id == thread_id)
            .collect()
    }

    /// Returns all events in a time range.
    pub fn events_in_range(&self, start_ns: u64, end_ns: u64) -> Vec<&RecordEvent> {
        self.events
            .iter()
            .filter(|e| e.timestamp_ns >= start_ns && e.timestamp_ns <= end_ns)
            .collect()
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLog {
    /// V20: Export the event log to a JSON string.
    pub fn to_json(&self) -> String {
        let mut entries = Vec::new();
        for event in &self.events {
            let kind_json = match &event.kind {
                EventKind::FnEntry { name, location } => {
                    format!(
                        r#"{{"type":"fn_entry","name":"{}","location":"{}"}}"#,
                        json_escape(name),
                        json_escape(location)
                    )
                }
                EventKind::FnExit { name, return_value } => {
                    let rv = return_value
                        .as_deref()
                        .map(|v| format!(r#","return":"{}""#, json_escape(v)))
                        .unwrap_or_default();
                    format!(
                        r#"{{"type":"fn_exit","name":"{}"{}}}"#,
                        json_escape(name),
                        rv
                    )
                }
                EventKind::VarAssign {
                    name, value, scope, ..
                } => {
                    format!(
                        r#"{{"type":"var_assign","name":"{}","value":"{}","scope":{}}}"#,
                        json_escape(name),
                        json_escape(value),
                        scope
                    )
                }
                EventKind::IoOp { op, data } => {
                    let op_str = match op {
                        IoOpKind::StdoutWrite => "stdout",
                        IoOpKind::StdinRead => "stdin",
                        IoOpKind::FileRead => "file_read",
                        IoOpKind::FileWrite => "file_write",
                        IoOpKind::NetSend => "net_send",
                        IoOpKind::NetRecv => "net_recv",
                    };
                    let text = String::from_utf8_lossy(data);
                    format!(
                        r#"{{"type":"io","op":"{}","data":"{}"}}"#,
                        op_str,
                        json_escape(&text)
                    )
                }
                EventKind::HeapAlloc { addr, size } => {
                    format!(r#"{{"type":"heap_alloc","addr":{},"size":{}}}"#, addr, size)
                }
                EventKind::HeapFree { addr } => {
                    format!(r#"{{"type":"heap_free","addr":{}}}"#, addr)
                }
                EventKind::ThreadEvent { kind } => {
                    let k = match kind {
                        ThreadEventKind::Spawn => "spawn",
                        ThreadEventKind::Join => "join",
                        ThreadEventKind::Switch => "switch",
                        ThreadEventKind::MutexAcquire => "mutex_acquire",
                        ThreadEventKind::MutexRelease => "mutex_release",
                    };
                    format!(r#"{{"type":"thread","kind":"{}"}}"#, k)
                }
            };
            entries.push(format!(
                r#"  {{"seq":{},"ts":{},"tid":{},"event":{}}}"#,
                event.seq, event.timestamp_ns, event.thread_id, kind_json
            ));
        }
        format!("[\n{}\n]", entries.join(",\n"))
    }
}

/// Escape a string for JSON.
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ═══════════════════════════════════════════════════════════════════════
// S25.3: Compact Encoding (Delta Compression)
// ═══════════════════════════════════════════════════════════════════════

/// A delta-compressed state change.
#[derive(Debug, Clone)]
pub struct DeltaChange {
    /// Variable or address that changed.
    pub target: String,
    /// Previous value (for reverse replay).
    pub old_value: Vec<u8>,
    /// New value.
    pub new_value: Vec<u8>,
}

/// Computes delta between two byte slices.
pub fn compute_delta(old: &[u8], new: &[u8]) -> Vec<DeltaPatch> {
    let mut patches = Vec::new();
    let len = old.len().min(new.len());

    let mut i = 0;
    while i < len {
        if old[i] != new[i] {
            let start = i;
            while i < len && old[i] != new[i] {
                i += 1;
            }
            patches.push(DeltaPatch {
                offset: start,
                old_bytes: old[start..i].to_vec(),
                new_bytes: new[start..i].to_vec(),
            });
        } else {
            i += 1;
        }
    }

    // Handle size differences
    if new.len() > old.len() {
        patches.push(DeltaPatch {
            offset: old.len(),
            old_bytes: Vec::new(),
            new_bytes: new[old.len()..].to_vec(),
        });
    }

    patches
}

/// A binary patch within a delta.
#[derive(Debug, Clone)]
pub struct DeltaPatch {
    /// Byte offset.
    pub offset: usize,
    /// Old bytes (for reverse).
    pub old_bytes: Vec<u8>,
    /// New bytes (for forward).
    pub new_bytes: Vec<u8>,
}

/// Computes the compression ratio of delta vs full snapshot.
pub fn compression_ratio(delta_size: usize, full_size: usize) -> f64 {
    if full_size == 0 {
        return 1.0;
    }
    1.0 - (delta_size as f64 / full_size as f64)
}

// ═══════════════════════════════════════════════════════════════════════
// S25.4: Recording Format
// ═══════════════════════════════════════════════════════════════════════

/// Recording file header.
#[derive(Debug, Clone)]
pub struct RecordingHeader {
    /// Magic number.
    pub magic: u32,
    /// Format version.
    pub version: u32,
    /// Total event count.
    pub event_count: u64,
    /// Start timestamp.
    pub start_time_ns: u64,
    /// End timestamp.
    pub end_time_ns: u64,
    /// Index offset (for random access).
    pub index_offset: u64,
}

/// Magic number for .fjrec files.
pub const FJREC_MAGIC: u32 = 0x464A5243; // "FJRC"

/// Current format version.
pub const FJREC_VERSION: u32 = 1;

impl RecordingHeader {
    /// Creates a new header.
    pub fn new(event_count: u64, start_ns: u64, end_ns: u64) -> Self {
        Self {
            magic: FJREC_MAGIC,
            version: FJREC_VERSION,
            event_count,
            start_time_ns: start_ns,
            end_time_ns: end_ns,
            index_offset: 0,
        }
    }

    /// Duration in nanoseconds.
    pub fn duration_ns(&self) -> u64 {
        self.end_time_ns - self.start_time_ns
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.5: Recording Overhead
// ═══════════════════════════════════════════════════════════════════════

/// Overhead measurement for recording.
#[derive(Debug, Clone)]
pub struct OverheadStats {
    /// Baseline execution time (ns).
    pub baseline_ns: u64,
    /// Recorded execution time (ns).
    pub recorded_ns: u64,
    /// Events per second.
    pub events_per_sec: f64,
    /// Bytes written per second.
    pub bytes_per_sec: f64,
}

impl OverheadStats {
    /// Slowdown factor.
    pub fn slowdown(&self) -> f64 {
        if self.baseline_ns == 0 {
            return 1.0;
        }
        self.recorded_ns as f64 / self.baseline_ns as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.6: Selective Recording
// ═══════════════════════════════════════════════════════════════════════

/// Selective recording filter.
#[derive(Debug, Clone)]
pub struct RecordFilter {
    /// Functions to record (empty = all).
    pub functions: Vec<String>,
    /// Modules to record.
    pub modules: Vec<String>,
    /// Whether to record heap events.
    pub record_heap: bool,
    /// Whether to record I/O events.
    pub record_io: bool,
}

impl RecordFilter {
    /// Checks if a function should be recorded.
    pub fn should_record_fn(&self, fn_name: &str) -> bool {
        if self.functions.is_empty() {
            return true;
        }
        self.functions.iter().any(|f| f == fn_name)
    }

    /// Checks if a module should be recorded.
    pub fn should_record_module(&self, module: &str) -> bool {
        if self.modules.is_empty() {
            return true;
        }
        self.modules.iter().any(|m| module.starts_with(m))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.7: Recording Size Limits
// ═══════════════════════════════════════════════════════════════════════

/// Ring buffer for bounded recording.
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    /// Buffer storage.
    buffer: Vec<Option<T>>,
    /// Write position.
    write_pos: usize,
    /// Total items written (including overwritten).
    total_written: u64,
}

impl<T: Clone> RingBuffer<T> {
    /// Creates a ring buffer with given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![None; capacity],
            write_pos: 0,
            total_written: 0,
        }
    }

    /// Pushes an item (overwrites oldest if full).
    pub fn push(&mut self, item: T) {
        self.buffer[self.write_pos] = Some(item);
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        self.total_written += 1;
    }

    /// Returns capacity.
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Returns number of valid items.
    pub fn len(&self) -> usize {
        self.buffer.iter().filter(|x| x.is_some()).count()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.total_written == 0
    }

    /// Total items written.
    pub fn total_written(&self) -> u64 {
        self.total_written
    }

    /// Items that were overwritten.
    pub fn items_dropped(&self) -> u64 {
        self.total_written.saturating_sub(self.buffer.len() as u64)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S25.8-S25.9: I/O Capture & Thread Recording (covered by EventKind)
// ═══════════════════════════════════════════════════════════════════════

/// I/O replay data for deterministic replay.
#[derive(Debug, Clone)]
pub struct IoCaptureEntry {
    /// Sequence number in event log.
    pub event_seq: u64,
    /// Operation kind.
    pub op: IoOpKind,
    /// Captured data.
    pub data: Vec<u8>,
    /// File descriptor or socket ID.
    pub fd: i32,
}

/// Thread schedule entry for deterministic replay.
#[derive(Debug, Clone)]
pub struct ThreadScheduleEntry {
    /// Sequence number.
    pub event_seq: u64,
    /// Thread that ran.
    pub thread_id: u32,
    /// Instructions executed before next switch.
    pub instructions: u64,
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S25.1 — Record Mode
    #[test]
    fn s25_1_record_config_default() {
        let cfg = RecordConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.output_path, "recording.fjrec");
        assert!(cfg.capture_io);
    }

    // S25.2 — Event Log
    #[test]
    fn s25_2_event_log() {
        let mut log = EventLog::new();
        log.record(
            100,
            0,
            EventKind::FnEntry {
                name: "main".into(),
                location: "main.fj:1".into(),
            },
        );
        log.record(
            200,
            0,
            EventKind::VarAssign {
                name: "x".into(),
                value: "42".into(),
                scope: 0,
            },
        );
        log.record(
            300,
            0,
            EventKind::FnExit {
                name: "main".into(),
                return_value: None,
            },
        );
        assert_eq!(log.len(), 3);
        assert!(!log.is_empty());
    }

    #[test]
    fn s25_2_events_by_thread() {
        let mut log = EventLog::new();
        log.record(
            100,
            0,
            EventKind::FnEntry {
                name: "f".into(),
                location: "a.fj:1".into(),
            },
        );
        log.record(
            200,
            1,
            EventKind::FnEntry {
                name: "g".into(),
                location: "b.fj:1".into(),
            },
        );
        assert_eq!(log.events_for_thread(0).len(), 1);
        assert_eq!(log.events_for_thread(1).len(), 1);
    }

    #[test]
    fn s25_2_events_in_range() {
        let mut log = EventLog::new();
        log.record(
            100,
            0,
            EventKind::VarAssign {
                name: "a".into(),
                value: "1".into(),
                scope: 0,
            },
        );
        log.record(
            500,
            0,
            EventKind::VarAssign {
                name: "b".into(),
                value: "2".into(),
                scope: 0,
            },
        );
        log.record(
            1000,
            0,
            EventKind::VarAssign {
                name: "c".into(),
                value: "3".into(),
                scope: 0,
            },
        );
        assert_eq!(log.events_in_range(100, 500).len(), 2);
    }

    // S25.3 — Delta Compression
    #[test]
    fn s25_3_compute_delta() {
        let old = vec![1, 2, 3, 4, 5];
        let new = vec![1, 9, 3, 4, 5];
        let patches = compute_delta(&old, &new);
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].offset, 1);
        assert_eq!(patches[0].old_bytes, vec![2]);
        assert_eq!(patches[0].new_bytes, vec![9]);
    }

    #[test]
    fn s25_3_delta_no_change() {
        let data = vec![1, 2, 3];
        let patches = compute_delta(&data, &data);
        assert!(patches.is_empty());
    }

    #[test]
    fn s25_3_compression_ratio() {
        assert!((compression_ratio(10, 100) - 0.9).abs() < 1e-10);
        assert!((compression_ratio(0, 100) - 1.0).abs() < 1e-10);
    }

    // S25.4 — Recording Format
    #[test]
    fn s25_4_recording_header() {
        let header = RecordingHeader::new(1000, 0, 5_000_000_000);
        assert_eq!(header.magic, FJREC_MAGIC);
        assert_eq!(header.version, FJREC_VERSION);
        assert_eq!(header.duration_ns(), 5_000_000_000);
    }

    // S25.5 — Overhead
    #[test]
    fn s25_5_overhead_stats() {
        let stats = OverheadStats {
            baseline_ns: 1_000_000,
            recorded_ns: 3_000_000,
            events_per_sec: 500_000.0,
            bytes_per_sec: 10_000_000.0,
        };
        assert!((stats.slowdown() - 3.0).abs() < 1e-10);
    }

    // S25.6 — Selective Recording
    #[test]
    fn s25_6_record_filter() {
        let filter = RecordFilter {
            functions: vec!["hot_loop".into(), "process".into()],
            modules: vec![],
            record_heap: true,
            record_io: false,
        };
        assert!(filter.should_record_fn("hot_loop"));
        assert!(!filter.should_record_fn("cold_path"));
    }

    #[test]
    fn s25_6_filter_empty_records_all() {
        let filter = RecordFilter {
            functions: vec![],
            modules: vec![],
            record_heap: true,
            record_io: true,
        };
        assert!(filter.should_record_fn("anything"));
        assert!(filter.should_record_module("any::module"));
    }

    // S25.7 — Ring Buffer
    #[test]
    fn s25_7_ring_buffer() {
        let mut rb: RingBuffer<u32> = RingBuffer::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.items_dropped(), 0);

        rb.push(4); // overwrites slot 0
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.total_written(), 4);
        assert_eq!(rb.items_dropped(), 1);
    }

    // S25.8 — I/O Capture
    #[test]
    fn s25_8_io_capture() {
        let entry = IoCaptureEntry {
            event_seq: 42,
            op: IoOpKind::FileRead,
            data: vec![72, 101, 108, 108, 111],
            fd: 3,
        };
        assert_eq!(entry.op, IoOpKind::FileRead);
        assert_eq!(entry.data.len(), 5);
    }

    // S25.9 — Thread Recording
    #[test]
    fn s25_9_thread_schedule() {
        let entry = ThreadScheduleEntry {
            event_seq: 100,
            thread_id: 2,
            instructions: 50000,
        };
        assert_eq!(entry.thread_id, 2);
    }

    // S25.10 — Event get by seq
    #[test]
    fn s25_10_get_by_seq() {
        let mut log = EventLog::new();
        log.record(
            100,
            0,
            EventKind::HeapAlloc {
                addr: 0x1000,
                size: 256,
            },
        );
        let event = log.get(0).unwrap();
        assert_eq!(event.seq, 0);
    }

    #[test]
    fn s25_10_heap_events() {
        let mut log = EventLog::new();
        log.record(
            100,
            0,
            EventKind::HeapAlloc {
                addr: 0x1000,
                size: 256,
            },
        );
        log.record(100, 0, EventKind::HeapFree { addr: 0x1000 });
        assert_eq!(log.len(), 2);
    }
}
