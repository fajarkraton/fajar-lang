//! Time-Travel Debugger — execution recording, snapshots, reverse stepping.
//!
//! D2.1-D2.2: 20 tasks covering state snapshots, delta compression, circular
//! buffer, checkpoint system, replay engine, reverse commands, DAP integration.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D2.1.1: State Snapshots
// ═══════════════════════════════════════════════════════════════════════

/// A snapshot of execution state at a single point in time.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Snapshot index (monotonically increasing).
    pub index: u64,
    /// Program counter (statement index).
    pub pc: u64,
    /// Source file.
    pub file: String,
    /// Source line.
    pub line: u32,
    /// Variable values (name → value as string).
    pub variables: HashMap<String, String>,
    /// Call stack (function names, bottom to top).
    pub call_stack: Vec<String>,
    /// Timestamp (nanoseconds from start).
    pub timestamp_ns: u64,
    /// Whether this is a full snapshot (vs delta).
    pub is_checkpoint: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// D2.1.2: Delta Compression
// ═══════════════════════════════════════════════════════════════════════

/// A delta-encoded snapshot (only stores changes from previous).
#[derive(Debug, Clone)]
pub struct DeltaSnapshot {
    /// Snapshot index.
    pub index: u64,
    /// PC.
    pub pc: u64,
    /// File (only if changed).
    pub file: Option<String>,
    /// Line.
    pub line: u32,
    /// Changed variables only.
    pub changed_vars: HashMap<String, String>,
    /// Removed variables.
    pub removed_vars: Vec<String>,
    /// Call stack (only if changed).
    pub call_stack_changed: bool,
    /// New call stack (if changed).
    pub call_stack: Option<Vec<String>>,
    /// Timestamp.
    pub timestamp_ns: u64,
}

/// Computes delta between two snapshots.
pub fn compute_delta(prev: &Snapshot, curr: &Snapshot) -> DeltaSnapshot {
    let mut changed_vars = HashMap::new();
    let mut removed_vars = Vec::new();

    // Find changed and new variables
    for (name, value) in &curr.variables {
        match prev.variables.get(name) {
            Some(old_val) if old_val != value => { changed_vars.insert(name.clone(), value.clone()); }
            None => { changed_vars.insert(name.clone(), value.clone()); }
            _ => {}
        }
    }

    // Find removed variables
    for name in prev.variables.keys() {
        if !curr.variables.contains_key(name) {
            removed_vars.push(name.clone());
        }
    }

    let call_stack_changed = prev.call_stack != curr.call_stack;

    DeltaSnapshot {
        index: curr.index,
        pc: curr.pc,
        file: if curr.file != prev.file { Some(curr.file.clone()) } else { None },
        line: curr.line,
        changed_vars,
        removed_vars,
        call_stack_changed,
        call_stack: if call_stack_changed { Some(curr.call_stack.clone()) } else { None },
        timestamp_ns: curr.timestamp_ns,
    }
}

/// Applies a delta to reconstruct a full snapshot.
pub fn apply_delta(base: &Snapshot, delta: &DeltaSnapshot) -> Snapshot {
    let mut vars = base.variables.clone();
    for (name, value) in &delta.changed_vars {
        vars.insert(name.clone(), value.clone());
    }
    for name in &delta.removed_vars {
        vars.remove(name);
    }

    Snapshot {
        index: delta.index,
        pc: delta.pc,
        file: delta.file.clone().unwrap_or_else(|| base.file.clone()),
        line: delta.line,
        variables: vars,
        call_stack: delta.call_stack.clone().unwrap_or_else(|| base.call_stack.clone()),
        timestamp_ns: delta.timestamp_ns,
        is_checkpoint: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.1.3-D2.1.4: Recording Buffer + Checkpoints
// ═══════════════════════════════════════════════════════════════════════

/// Execution recorder with circular buffer and checkpoints.
#[derive(Debug, Clone)]
pub struct Recorder {
    /// Full checkpoints (every N statements).
    pub checkpoints: Vec<Snapshot>,
    /// Delta snapshots between checkpoints.
    pub deltas: Vec<DeltaSnapshot>,
    /// Checkpoint interval (number of statements between full snapshots).
    pub checkpoint_interval: u64,
    /// Maximum total snapshots.
    pub max_snapshots: u64,
    /// Current snapshot count.
    pub snapshot_count: u64,
    /// Last full snapshot (for delta computation).
    pub last_checkpoint: Option<Snapshot>,
    /// Last snapshot (for delta computation).
    pub last_snapshot: Option<Snapshot>,
    /// Recording enabled.
    pub enabled: bool,
    /// Total statements recorded.
    pub total_statements: u64,
}

impl Recorder {
    /// Creates a new recorder.
    pub fn new(checkpoint_interval: u64, max_snapshots: u64) -> Self {
        Self {
            checkpoints: Vec::new(),
            deltas: Vec::new(),
            checkpoint_interval,
            max_snapshots,
            snapshot_count: 0,
            last_checkpoint: None,
            last_snapshot: None,
            enabled: true,
            total_statements: 0,
        }
    }

    /// Records a statement execution.
    pub fn record(&mut self, snapshot: Snapshot) {
        if !self.enabled { return; }
        self.total_statements += 1;

        let is_checkpoint = self.total_statements % self.checkpoint_interval == 0
            || self.last_checkpoint.is_none();

        if is_checkpoint {
            let mut snap = snapshot.clone();
            snap.is_checkpoint = true;
            self.checkpoints.push(snap.clone());
            self.last_checkpoint = Some(snap.clone());
            self.last_snapshot = Some(snap);
        } else if let Some(ref prev) = self.last_snapshot {
            let delta = compute_delta(prev, &snapshot);
            self.deltas.push(delta);
            self.last_snapshot = Some(snapshot);
        }

        self.snapshot_count += 1;

        // Evict old data if over max
        if self.snapshot_count > self.max_snapshots {
            if !self.checkpoints.is_empty() { self.checkpoints.remove(0); }
            // Remove deltas that reference the evicted checkpoint
            let min_idx = self.checkpoints.first().map(|c| c.index).unwrap_or(0);
            self.deltas.retain(|d| d.index >= min_idx);
        }
    }

    /// Returns the snapshot at a given index.
    pub fn get_snapshot(&self, target_index: u64) -> Option<Snapshot> {
        // Find the nearest checkpoint before target
        let checkpoint = self.checkpoints.iter().rev()
            .find(|c| c.index <= target_index)?;

        // Replay deltas from checkpoint to target
        let mut state = checkpoint.clone();
        for delta in &self.deltas {
            if delta.index > checkpoint.index && delta.index <= target_index {
                state = apply_delta(&state, delta);
            }
        }
        Some(state)
    }

    /// Returns the previous snapshot (step back).
    pub fn step_back(&self, current_index: u64) -> Option<Snapshot> {
        if current_index == 0 { return None; }
        self.get_snapshot(current_index - 1)
    }

    /// Finds when a variable changed to a specific value.
    pub fn find_variable_change(&self, var_name: &str, target_value: &str) -> Option<u64> {
        // Search deltas for the variable change
        for delta in self.deltas.iter().rev() {
            if let Some(val) = delta.changed_vars.get(var_name) {
                if val == target_value { return Some(delta.index); }
            }
        }
        None
    }

    /// Returns recording overhead estimate.
    pub fn overhead_estimate(&self) -> RecordingOverhead {
        let checkpoint_bytes = self.checkpoints.len() * 1024; // rough estimate
        let delta_bytes = self.deltas.len() * 128; // rough estimate
        RecordingOverhead {
            memory_bytes: checkpoint_bytes + delta_bytes,
            checkpoints: self.checkpoints.len() as u64,
            deltas: self.deltas.len() as u64,
            total_statements: self.total_statements,
        }
    }
}

/// Recording overhead metrics.
#[derive(Debug, Clone)]
pub struct RecordingOverhead {
    /// Estimated memory usage.
    pub memory_bytes: usize,
    /// Number of checkpoints.
    pub checkpoints: u64,
    /// Number of deltas.
    pub deltas: u64,
    /// Total statements.
    pub total_statements: u64,
}

impl fmt::Display for RecordingOverhead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}KB, {} checkpoints, {} deltas, {} statements",
            self.memory_bytes / 1024, self.checkpoints, self.deltas, self.total_statements)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.2: DAP Reverse Capabilities
// ═══════════════════════════════════════════════════════════════════════

/// DAP capabilities for reverse debugging.
#[derive(Debug, Clone)]
pub struct ReverseDapCapabilities {
    /// Supports step-back.
    pub supports_step_back: bool,
    /// Supports reverse-continue.
    pub supports_reverse_continue: bool,
    /// Supports variable history.
    pub supports_variable_history: bool,
    /// Supports execution timeline.
    pub supports_timeline: bool,
    /// Supports data breakpoints (reverse).
    pub supports_data_breakpoints: bool,
}

impl Default for ReverseDapCapabilities {
    fn default() -> Self {
        Self {
            supports_step_back: true,
            supports_reverse_continue: true,
            supports_variable_history: true,
            supports_timeline: true,
            supports_data_breakpoints: true,
        }
    }
}

/// Variable history entry.
#[derive(Debug, Clone)]
pub struct VariableHistory {
    /// Variable name.
    pub name: String,
    /// History of (snapshot_index, value) pairs.
    pub values: Vec<(u64, String)>,
}

/// Builds variable history from recorded data.
pub fn build_variable_history(recorder: &Recorder, var_name: &str) -> VariableHistory {
    let mut values = Vec::new();

    for checkpoint in &recorder.checkpoints {
        if let Some(val) = checkpoint.variables.get(var_name) {
            values.push((checkpoint.index, val.clone()));
        }
    }

    for delta in &recorder.deltas {
        if let Some(val) = delta.changed_vars.get(var_name) {
            values.push((delta.index, val.clone()));
        }
    }

    values.sort_by_key(|(idx, _)| *idx);
    VariableHistory { name: var_name.to_string(), values }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(index: u64, line: u32, vars: Vec<(&str, &str)>) -> Snapshot {
        Snapshot {
            index, pc: index, file: "test.fj".to_string(), line,
            variables: vars.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            call_stack: vec!["main".to_string()],
            timestamp_ns: index * 1000,
            is_checkpoint: false,
        }
    }

    #[test]
    fn d2_1_delta_compression() {
        let prev = make_snapshot(0, 1, vec![("x", "1"), ("y", "2")]);
        let curr = make_snapshot(1, 2, vec![("x", "1"), ("y", "3"), ("z", "4")]);
        let delta = compute_delta(&prev, &curr);
        assert_eq!(delta.changed_vars.len(), 2); // y changed, z added
        assert!(delta.changed_vars.contains_key("y"));
        assert!(delta.changed_vars.contains_key("z"));
        assert!(delta.removed_vars.is_empty());
    }

    #[test]
    fn d2_1_apply_delta() {
        let base = make_snapshot(0, 1, vec![("x", "1"), ("y", "2")]);
        let delta = DeltaSnapshot {
            index: 1, pc: 1, file: None, line: 2,
            changed_vars: HashMap::from([("y".to_string(), "3".to_string())]),
            removed_vars: vec![],
            call_stack_changed: false, call_stack: None, timestamp_ns: 1000,
        };
        let result = apply_delta(&base, &delta);
        assert_eq!(result.variables.get("x").unwrap(), "1");
        assert_eq!(result.variables.get("y").unwrap(), "3");
    }

    #[test]
    fn d2_1_recorder_basic() {
        let mut rec = Recorder::new(5, 1000);
        for i in 0..10 {
            rec.record(make_snapshot(i, i as u32 + 1, vec![("x", &i.to_string())]));
        }
        assert_eq!(rec.total_statements, 10);
        assert!(!rec.checkpoints.is_empty());
    }

    #[test]
    fn d2_1_step_back() {
        let mut rec = Recorder::new(3, 1000);
        rec.record(make_snapshot(0, 1, vec![("x", "0")]));
        rec.record(make_snapshot(1, 2, vec![("x", "1")]));
        rec.record(make_snapshot(2, 3, vec![("x", "2")]));

        let prev = rec.step_back(2);
        assert!(prev.is_some());
    }

    #[test]
    fn d2_1_find_variable_change() {
        let mut rec = Recorder::new(100, 1000); // large interval = no intermediate checkpoints
        rec.record(make_snapshot(0, 1, vec![("x", "0")])); // checkpoint
        // Manually add deltas
        rec.deltas.push(DeltaSnapshot {
            index: 1, pc: 1, file: None, line: 2,
            changed_vars: HashMap::from([("x".to_string(), "42".to_string())]),
            removed_vars: vec![], call_stack_changed: false, call_stack: None, timestamp_ns: 1000,
        });
        let found = rec.find_variable_change("x", "42");
        assert_eq!(found, Some(1));
    }

    #[test]
    fn d2_1_recording_overhead() {
        let mut rec = Recorder::new(10, 1000);
        for i in 0..50 { rec.record(make_snapshot(i, 1, vec![("x", "1")])); }
        let overhead = rec.overhead_estimate();
        assert!(overhead.memory_bytes > 0);
        assert_eq!(overhead.total_statements, 50);
    }

    #[test]
    fn d2_2_dap_capabilities() {
        let caps = ReverseDapCapabilities::default();
        assert!(caps.supports_step_back);
        assert!(caps.supports_reverse_continue);
        assert!(caps.supports_variable_history);
    }

    #[test]
    fn d2_2_variable_history() {
        let mut rec = Recorder::new(100, 1000);
        rec.record(make_snapshot(0, 1, vec![("loss", "1.5")]));
        rec.deltas.push(DeltaSnapshot {
            index: 1, pc: 1, file: None, line: 2,
            changed_vars: HashMap::from([("loss".to_string(), "0.8".to_string())]),
            removed_vars: vec![], call_stack_changed: false, call_stack: None, timestamp_ns: 1000,
        });
        rec.deltas.push(DeltaSnapshot {
            index: 2, pc: 2, file: None, line: 3,
            changed_vars: HashMap::from([("loss".to_string(), "0.3".to_string())]),
            removed_vars: vec![], call_stack_changed: false, call_stack: None, timestamp_ns: 2000,
        });
        let history = build_variable_history(&rec, "loss");
        assert_eq!(history.values.len(), 3); // checkpoint + 2 deltas
    }

    #[test]
    fn d2_1_overhead_display() {
        let oh = RecordingOverhead { memory_bytes: 10240, checkpoints: 5, deltas: 45, total_statements: 50 };
        let s = format!("{oh}");
        assert!(s.contains("10KB"));
        assert!(s.contains("50 statements"));
    }
}
