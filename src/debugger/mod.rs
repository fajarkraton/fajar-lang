//! Debugger infrastructure for Fajar Lang.
//!
//! Provides breakpoint management, stepping modes, debug hooks,
//! and a DAP (Debug Adapter Protocol) server for IDE integration.
//!
//! # Architecture
//!
//! ```text
//! DebugState (breakpoints, step mode, call depth)
//!     │
//!     ▼
//! debug_hook() — called at each statement in eval_stmt()
//!     │
//!     ├── check breakpoints (file + line match)
//!     ├── check step mode (StepIn/StepOver/StepOut)
//!     └── pause execution → notify via callback
//!
//! DAP Server (stdin/stdout)
//!     │
//!     ├── Initialize → Capabilities
//!     ├── SetBreakpoints → register file:line
//!     ├── Continue/Next/StepIn/StepOut → resume
//!     └── Stopped/Terminated events → IDE
//! ```

pub mod dap_server;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Global breakpoint ID counter.
static NEXT_BREAKPOINT_ID: AtomicU32 = AtomicU32::new(1);

/// Stepping mode for the debugger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    /// Run until a breakpoint is hit.
    Continue,
    /// Stop at the next statement (including inside function calls).
    StepIn,
    /// Stop at the next statement at the same or lower call depth.
    StepOver,
    /// Stop when call depth decreases (return from current function).
    StepOut,
    /// Execution is paused (waiting for user command).
    Paused,
}

/// A breakpoint set at a specific source location.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    /// Unique breakpoint ID.
    pub id: u32,
    /// Source file path (or "<repl>" for REPL).
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// Optional condition expression (Fajar Lang source).
    pub condition: Option<String>,
    /// Number of times this breakpoint has been hit.
    pub hit_count: u32,
    /// Optional log message (logpoint — print without stopping).
    pub log_message: Option<String>,
    /// Whether this breakpoint is currently enabled.
    pub enabled: bool,
}

impl Breakpoint {
    /// Creates a new breakpoint at the given file and line.
    pub fn new(file: String, line: usize) -> Self {
        Self {
            id: NEXT_BREAKPOINT_ID.fetch_add(1, Ordering::Relaxed),
            file,
            line,
            condition: None,
            hit_count: 0,
            log_message: None,
            enabled: true,
        }
    }

    /// Creates a conditional breakpoint.
    pub fn with_condition(mut self, condition: String) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Creates a logpoint (logs a message without stopping).
    pub fn with_log_message(mut self, message: String) -> Self {
        self.log_message = Some(message);
        self
    }
}

/// Current location in the source code during debugging.
#[derive(Debug, Clone)]
pub struct SourceLocation {
    /// Source file path.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number.
    pub column: usize,
    /// Byte offset in source.
    pub offset: usize,
}

/// Reason why execution stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Hit a breakpoint.
    Breakpoint(u32),
    /// Step completed.
    Step,
    /// Program entry (stop on entry).
    Entry,
    /// Program terminated.
    Terminated,
}

/// Callback type for stop events.
type StopCallback = Arc<Mutex<dyn FnMut(StopReason, &SourceLocation) + Send>>;

/// Debugger state shared between the interpreter and debug controller.
///
/// The `DebugState` tracks breakpoints, the current stepping mode,
/// call depth for step-over/step-out, and the current source location.
pub struct DebugState {
    /// Breakpoints indexed by (file, line).
    breakpoints: HashMap<(String, usize), Breakpoint>,
    /// All breakpoints indexed by ID (for lookup/removal).
    breakpoints_by_id: HashMap<u32, (String, usize)>,
    /// Current stepping mode.
    step_mode: StepMode,
    /// Call depth when step-over/step-out was initiated.
    step_start_depth: usize,
    /// Current source location.
    current_location: Option<SourceLocation>,
    /// Callback invoked when execution stops.
    on_stop: Option<StopCallback>,
    /// Whether to stop at the program entry point.
    stop_on_entry: bool,
    /// Whether the first statement has been executed.
    first_stmt: bool,
}

impl std::fmt::Debug for DebugState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DebugState")
            .field("breakpoints", &self.breakpoints.len())
            .field("step_mode", &self.step_mode)
            .field("step_start_depth", &self.step_start_depth)
            .field("current_location", &self.current_location)
            .field("stop_on_entry", &self.stop_on_entry)
            .finish()
    }
}

impl DebugState {
    /// Creates a new debug state with no breakpoints and Continue mode.
    pub fn new() -> Self {
        Self {
            breakpoints: HashMap::new(),
            breakpoints_by_id: HashMap::new(),
            step_mode: StepMode::Continue,
            step_start_depth: 0,
            current_location: None,
            on_stop: None,
            stop_on_entry: false,
            first_stmt: true,
        }
    }

    /// Sets whether to stop at the program entry point.
    pub fn set_stop_on_entry(&mut self, stop: bool) {
        self.stop_on_entry = stop;
    }

    /// Registers a callback invoked when execution stops.
    pub fn set_on_stop<F>(&mut self, callback: F)
    where
        F: FnMut(StopReason, &SourceLocation) + Send + 'static,
    {
        self.on_stop = Some(Arc::new(Mutex::new(callback)));
    }

    /// Adds a breakpoint. Returns the breakpoint ID.
    pub fn add_breakpoint(&mut self, bp: Breakpoint) -> u32 {
        let id = bp.id;
        let key = (bp.file.clone(), bp.line);
        self.breakpoints_by_id.insert(id, key.clone());
        self.breakpoints.insert(key, bp);
        id
    }

    /// Removes a breakpoint by ID. Returns whether it existed.
    pub fn remove_breakpoint(&mut self, id: u32) -> bool {
        if let Some(key) = self.breakpoints_by_id.remove(&id) {
            self.breakpoints.remove(&key);
            true
        } else {
            false
        }
    }

    /// Removes all breakpoints for a given file.
    pub fn clear_breakpoints_for_file(&mut self, file: &str) {
        let keys_to_remove: Vec<(String, usize)> = self
            .breakpoints
            .keys()
            .filter(|(f, _)| f == file)
            .cloned()
            .collect();
        for key in keys_to_remove {
            if let Some(bp) = self.breakpoints.remove(&key) {
                self.breakpoints_by_id.remove(&bp.id);
            }
        }
    }

    /// Returns all breakpoints for a given file, sorted by line.
    pub fn breakpoints_for_file(&self, file: &str) -> Vec<&Breakpoint> {
        let mut bps: Vec<&Breakpoint> = self
            .breakpoints
            .iter()
            .filter(|((f, _), _)| f == file)
            .map(|(_, bp)| bp)
            .collect();
        bps.sort_by_key(|bp| bp.line);
        bps
    }

    /// Returns the total number of breakpoints.
    pub fn breakpoint_count(&self) -> usize {
        self.breakpoints.len()
    }

    /// Gets a breakpoint by ID.
    pub fn get_breakpoint(&self, id: u32) -> Option<&Breakpoint> {
        self.breakpoints_by_id
            .get(&id)
            .and_then(|key| self.breakpoints.get(key))
    }

    /// Sets the stepping mode and records the current call depth.
    pub fn set_step_mode(&mut self, mode: StepMode, current_depth: usize) {
        self.step_mode = mode;
        self.step_start_depth = current_depth;
    }

    /// Returns the current stepping mode.
    pub fn step_mode(&self) -> StepMode {
        self.step_mode
    }

    /// Returns the current source location.
    pub fn current_location(&self) -> Option<&SourceLocation> {
        self.current_location.as_ref()
    }

    /// Computes a 1-based line number from a byte offset in the source.
    pub fn line_from_offset(source: &str, offset: usize) -> usize {
        source[..offset.min(source.len())]
            .chars()
            .filter(|&c| c == '\n')
            .count()
            + 1
    }

    /// Computes a 1-based column from a byte offset in the source.
    pub fn column_from_offset(source: &str, offset: usize) -> usize {
        let clamped = offset.min(source.len());
        let last_newline = source[..clamped].rfind('\n').map(|p| p + 1).unwrap_or(0);
        clamped - last_newline + 1
    }

    /// The main debug hook. Called before each statement is evaluated.
    ///
    /// Returns `true` if execution should pause (breakpoint hit or step completed).
    pub fn debug_hook(
        &mut self,
        file: &str,
        source: &str,
        span_start: usize,
        call_depth: usize,
    ) -> Option<StopReason> {
        // Update current location
        let line = Self::line_from_offset(source, span_start);
        let column = Self::column_from_offset(source, span_start);
        self.current_location = Some(SourceLocation {
            file: file.to_string(),
            line,
            column,
            offset: span_start,
        });

        // Check stop-on-entry for first statement
        if self.first_stmt {
            self.first_stmt = false;
            if self.stop_on_entry {
                self.step_mode = StepMode::Paused;
                let reason = StopReason::Entry;
                self.fire_on_stop(&reason);
                return Some(reason);
            }
        }

        // Check breakpoints
        let key = (file.to_string(), line);
        if let Some(bp) = self.breakpoints.get_mut(&key) {
            if bp.enabled {
                bp.hit_count += 1;

                // Logpoint: log message without stopping
                if let Some(ref msg) = bp.log_message {
                    eprintln!("[logpoint] {msg}");
                    return None;
                }

                let reason = StopReason::Breakpoint(bp.id);
                self.step_mode = StepMode::Paused;
                self.fire_on_stop(&reason);
                return Some(reason);
            }
        }

        // Check step mode
        match self.step_mode {
            StepMode::Continue | StepMode::Paused => None,
            StepMode::StepIn => {
                self.step_mode = StepMode::Paused;
                let reason = StopReason::Step;
                self.fire_on_stop(&reason);
                Some(reason)
            }
            StepMode::StepOver => {
                if call_depth <= self.step_start_depth {
                    self.step_mode = StepMode::Paused;
                    let reason = StopReason::Step;
                    self.fire_on_stop(&reason);
                    Some(reason)
                } else {
                    None
                }
            }
            StepMode::StepOut => {
                if call_depth < self.step_start_depth {
                    self.step_mode = StepMode::Paused;
                    let reason = StopReason::Step;
                    self.fire_on_stop(&reason);
                    Some(reason)
                } else {
                    None
                }
            }
        }
    }

    /// Fires the on_stop callback if registered.
    fn fire_on_stop(&self, reason: &StopReason) {
        if let Some(ref cb) = self.on_stop {
            if let Some(ref loc) = self.current_location {
                if let Ok(mut guard) = cb.lock() {
                    guard(reason.clone(), loc);
                }
            }
        }
    }

    /// Checks a conditional breakpoint by evaluating the condition expression.
    ///
    /// Returns `true` if the condition is met (or if there's no condition).
    pub fn check_condition(&self, bp: &Breakpoint, eval_fn: &mut dyn FnMut(&str) -> bool) -> bool {
        match &bp.condition {
            None => true,
            Some(cond) => eval_fn(cond),
        }
    }
}

impl Default for DebugState {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_state_new_defaults() {
        let state = DebugState::new();
        assert_eq!(state.step_mode(), StepMode::Continue);
        assert_eq!(state.breakpoint_count(), 0);
        assert!(state.current_location().is_none());
    }

    #[test]
    fn breakpoint_add_and_remove() {
        let mut state = DebugState::new();
        let bp = Breakpoint::new("test.fj".into(), 10);
        let id = state.add_breakpoint(bp);

        assert_eq!(state.breakpoint_count(), 1);
        assert!(state.get_breakpoint(id).is_some());
        assert_eq!(state.get_breakpoint(id).map(|b| b.line), Some(10));

        assert!(state.remove_breakpoint(id));
        assert_eq!(state.breakpoint_count(), 0);
        assert!(!state.remove_breakpoint(id)); // already removed
    }

    #[test]
    fn breakpoint_hit_increments_count() {
        let mut state = DebugState::new();
        let bp = Breakpoint::new("test.fj".into(), 2);
        let id = state.add_breakpoint(bp);

        let source = "line1\nline2\nline3\n";
        // Offset of "line2" is 6 (after "line1\n")
        let reason = state.debug_hook("test.fj", source, 6, 0);
        assert_eq!(reason, Some(StopReason::Breakpoint(id)));
        assert_eq!(state.get_breakpoint(id).map(|b| b.hit_count), Some(1));

        // Hit again
        state.step_mode = StepMode::Continue;
        let reason = state.debug_hook("test.fj", source, 6, 0);
        assert_eq!(reason, Some(StopReason::Breakpoint(id)));
        assert_eq!(state.get_breakpoint(id).map(|b| b.hit_count), Some(2));
    }

    #[test]
    fn step_in_stops_at_every_statement() {
        let mut state = DebugState::new();
        state.set_step_mode(StepMode::StepIn, 0);

        let source = "let x = 1\nlet y = 2\n";
        let reason = state.debug_hook("test.fj", source, 0, 0);
        assert_eq!(reason, Some(StopReason::Step));
        assert_eq!(state.step_mode(), StepMode::Paused);

        // After resume with StepIn, should stop again
        state.set_step_mode(StepMode::StepIn, 0);
        let reason = state.debug_hook("test.fj", source, 10, 0);
        assert_eq!(reason, Some(StopReason::Step));
    }

    #[test]
    fn step_over_skips_deeper_calls() {
        let mut state = DebugState::new();
        state.set_step_mode(StepMode::StepOver, 1);

        let source = "a\nb\nc\n";
        // Inside a function call (depth 2) — should not stop
        let reason = state.debug_hook("test.fj", source, 0, 2);
        assert_eq!(reason, None);

        // Back at original depth — should stop
        let reason = state.debug_hook("test.fj", source, 2, 1);
        assert_eq!(reason, Some(StopReason::Step));
    }

    #[test]
    fn step_out_stops_on_depth_decrease() {
        let mut state = DebugState::new();
        state.set_step_mode(StepMode::StepOut, 2);

        let source = "a\nb\nc\n";
        // Same depth — should not stop
        let reason = state.debug_hook("test.fj", source, 0, 2);
        assert_eq!(reason, None);

        // Deeper — should not stop
        let reason = state.debug_hook("test.fj", source, 0, 3);
        assert_eq!(reason, None);

        // Shallower — should stop
        let reason = state.debug_hook("test.fj", source, 2, 1);
        assert_eq!(reason, Some(StopReason::Step));
    }

    #[test]
    fn conditional_breakpoint_evaluation() {
        let state = DebugState::new();
        let bp_no_cond = Breakpoint::new("test.fj".into(), 1);
        assert!(state.check_condition(&bp_no_cond, &mut |_| false));

        let bp_true = Breakpoint::new("test.fj".into(), 1).with_condition("x > 5".into());
        assert!(state.check_condition(&bp_true, &mut |_| true));

        let bp_false = Breakpoint::new("test.fj".into(), 1).with_condition("x > 5".into());
        assert!(!state.check_condition(&bp_false, &mut |_| false));
    }

    #[test]
    fn line_and_column_from_offset() {
        let source = "first\nsecond\nthird";
        assert_eq!(DebugState::line_from_offset(source, 0), 1); // 'f' in first
        assert_eq!(DebugState::line_from_offset(source, 6), 2); // 's' in second
        assert_eq!(DebugState::line_from_offset(source, 13), 3); // 't' in third

        assert_eq!(DebugState::column_from_offset(source, 0), 1); // col 1
        assert_eq!(DebugState::column_from_offset(source, 3), 4); // col 4
        assert_eq!(DebugState::column_from_offset(source, 6), 1); // col 1 of line 2
        assert_eq!(DebugState::column_from_offset(source, 9), 4); // col 4 of line 2
    }

    #[test]
    fn clear_breakpoints_for_file() {
        let mut state = DebugState::new();
        state.add_breakpoint(Breakpoint::new("a.fj".into(), 1));
        state.add_breakpoint(Breakpoint::new("a.fj".into(), 5));
        state.add_breakpoint(Breakpoint::new("b.fj".into(), 3));

        assert_eq!(state.breakpoint_count(), 3);
        state.clear_breakpoints_for_file("a.fj");
        assert_eq!(state.breakpoint_count(), 1);

        let remaining = state.breakpoints_for_file("b.fj");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].line, 3);
    }

    #[test]
    fn stop_on_entry() {
        let mut state = DebugState::new();
        state.set_stop_on_entry(true);

        let source = "let x = 1\n";
        let reason = state.debug_hook("test.fj", source, 0, 0);
        assert_eq!(reason, Some(StopReason::Entry));
        assert_eq!(state.step_mode(), StepMode::Paused);

        // Second call should not trigger entry stop again
        state.step_mode = StepMode::Continue;
        let reason = state.debug_hook("test.fj", source, 0, 0);
        assert_eq!(reason, None);
    }

    #[test]
    fn logpoint_does_not_stop() {
        let mut state = DebugState::new();
        let bp = Breakpoint::new("test.fj".into(), 1).with_log_message("x = {x}".into());
        state.add_breakpoint(bp);

        let source = "let x = 1\n";
        let reason = state.debug_hook("test.fj", source, 0, 0);
        assert_eq!(reason, None); // logpoint doesn't stop
    }

    #[test]
    fn on_stop_callback_fires() {
        let mut state = DebugState::new();
        let stops = Arc::new(Mutex::new(Vec::new()));
        let stops_clone = stops.clone();
        state.set_on_stop(move |reason, loc| {
            stops_clone.lock().unwrap().push((reason, loc.line));
        });

        let bp = Breakpoint::new("test.fj".into(), 2);
        let id = state.add_breakpoint(bp);

        let source = "line1\nline2\n";
        state.debug_hook("test.fj", source, 6, 0);

        let recorded = stops.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, StopReason::Breakpoint(id));
        assert_eq!(recorded[0].1, 2);
    }
}
