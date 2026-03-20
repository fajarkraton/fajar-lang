//! Debug information generation for Fajar Lang DAP debugger.
//!
//! Provides source mapping, breakpoint management, stack frame tracking,
//! and debug state management for IDE-integrated debugging.
//!
//! # Architecture
//!
//! ```text
//! SourceMap (instruction index → SourceLocation)
//!     │
//!     ├── add_mapping(idx, location)
//!     ├── lookup(idx) → Option<&SourceLocation>
//!     └── find_line(file, line) → Vec<usize>
//!
//! BreakpointManager
//!     │
//!     ├── set_breakpoint(file, line) → DapBreakpoint
//!     ├── clear_breakpoint(id)
//!     └── check_breakpoint(file, line) → Option<&DapBreakpoint>
//!
//! DebugState
//!     │
//!     ├── instruction_ptr
//!     ├── call_stack: Vec<DapStackFrame>
//!     └── running / paused_reason
//! ```

pub mod advanced;
pub mod protocol;
pub mod vscode;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

/// Global breakpoint ID counter for the DAP module.
static NEXT_DAP_BP_ID: AtomicU32 = AtomicU32::new(1);

/// Global stack frame ID counter.
static NEXT_FRAME_ID: AtomicU32 = AtomicU32::new(1);

// ═══════════════════════════════════════════════════════════════════════
// SourceLocation
// ═══════════════════════════════════════════════════════════════════════

/// A source code location mapping an instruction to a position in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DapSourceLocation {
    /// Source file path.
    pub file: String,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub column: u32,
}

impl DapSourceLocation {
    /// Creates a new source location.
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SourceMap
// ═══════════════════════════════════════════════════════════════════════

/// Maps instruction indices to source code locations.
///
/// Used by the debugger to correlate execution position with source files,
/// enabling breakpoint resolution and step-through debugging.
#[derive(Debug, Clone, Default)]
pub struct DapSourceMap {
    /// Instruction index to source location mappings.
    mappings: HashMap<usize, DapSourceLocation>,
}

impl DapSourceMap {
    /// Creates a new empty source map.
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    /// Adds a mapping from instruction index to source location.
    pub fn add_mapping(&mut self, instruction_idx: usize, location: DapSourceLocation) {
        self.mappings.insert(instruction_idx, location);
    }

    /// Looks up the source location for an instruction index.
    pub fn lookup(&self, instruction_idx: usize) -> Option<&DapSourceLocation> {
        self.mappings.get(&instruction_idx)
    }

    /// Finds all instruction indices that map to a given file and line.
    pub fn find_line(&self, file: &str, line: u32) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .mappings
            .iter()
            .filter(|(_, loc)| loc.file == file && loc.line == line)
            .map(|(&idx, _)| idx)
            .collect();
        indices.sort();
        indices
    }

    /// Returns the total number of mappings.
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Returns whether the source map has no mappings.
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// VariableInfo
// ═══════════════════════════════════════════════════════════════════════

/// The scope kind for a debug variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarScope {
    /// A local variable within a function body.
    Local,
    /// A function parameter.
    Parameter,
    /// A global/module-level variable.
    Global,
}

/// Information about a variable visible in the debugger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableInfo {
    /// Variable name.
    pub name: String,
    /// Type name (e.g., "i64", "f64", "str").
    pub type_name: String,
    /// Scope kind (local, parameter, or global).
    pub scope: VarScope,
    /// String representation of the current value.
    pub value: String,
}

impl VariableInfo {
    /// Creates a new variable info entry.
    pub fn new(
        name: impl Into<String>,
        type_name: impl Into<String>,
        scope: VarScope,
        value: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            scope,
            value: value.into(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DapBreakpoint & BreakpointManager
// ═══════════════════════════════════════════════════════════════════════

/// A breakpoint managed by the DAP debugger.
#[derive(Debug, Clone)]
pub struct DapBreakpoint {
    /// Unique breakpoint ID.
    pub id: u32,
    /// Source file path.
    pub file: String,
    /// 1-based line number.
    pub line: u32,
    /// Whether this breakpoint is currently enabled.
    pub enabled: bool,
    /// Optional condition expression (Fajar Lang source).
    pub condition: Option<String>,
    /// Number of times this breakpoint has been hit.
    pub hit_count: u32,
}

impl DapBreakpoint {
    /// Creates a new enabled breakpoint with a unique ID.
    pub fn new(file: impl Into<String>, line: u32) -> Self {
        Self {
            id: NEXT_DAP_BP_ID.fetch_add(1, Ordering::Relaxed),
            file: file.into(),
            line,
            enabled: true,
            condition: None,
            hit_count: 0,
        }
    }
}

/// Manages breakpoints for the DAP debugger.
///
/// Stores breakpoints keyed by (file, line) for efficient lookup during
/// instruction execution, and supports set/clear/check operations.
#[derive(Debug, Clone, Default)]
pub struct BreakpointManager {
    /// Breakpoints indexed by (file, line).
    breakpoints: HashMap<(String, u32), DapBreakpoint>,
    /// Reverse lookup: breakpoint ID to (file, line).
    id_to_key: HashMap<u32, (String, u32)>,
}

impl BreakpointManager {
    /// Creates a new empty breakpoint manager.
    pub fn new() -> Self {
        Self {
            breakpoints: HashMap::new(),
            id_to_key: HashMap::new(),
        }
    }

    /// Sets a breakpoint at the given file and line. Returns the breakpoint.
    pub fn set_breakpoint(&mut self, file: &str, line: u32) -> DapBreakpoint {
        let bp = DapBreakpoint::new(file, line);
        let key = (file.to_string(), line);
        self.id_to_key.insert(bp.id, key.clone());
        self.breakpoints.insert(key, bp.clone());
        bp
    }

    /// Clears (removes) a breakpoint by its ID.
    pub fn clear_breakpoint(&mut self, id: u32) {
        if let Some(key) = self.id_to_key.remove(&id) {
            self.breakpoints.remove(&key);
        }
    }

    /// Checks whether a breakpoint is set at the given file and line.
    pub fn check_breakpoint(&self, file: &str, line: u32) -> Option<&DapBreakpoint> {
        let key = (file.to_string(), line);
        self.breakpoints.get(&key).filter(|bp| bp.enabled)
    }

    /// Returns the total number of breakpoints.
    pub fn count(&self) -> usize {
        self.breakpoints.len()
    }

    /// Returns a breakpoint by ID.
    pub fn get_by_id(&self, id: u32) -> Option<&DapBreakpoint> {
        self.id_to_key
            .get(&id)
            .and_then(|key| self.breakpoints.get(key))
    }

    /// Returns a mutable reference to a breakpoint by its key.
    pub fn get_mut(&mut self, file: &str, line: u32) -> Option<&mut DapBreakpoint> {
        let key = (file.to_string(), line);
        self.breakpoints.get_mut(&key)
    }

    /// Returns all breakpoints as an iterator.
    pub fn all_breakpoints(&self) -> impl Iterator<Item = &DapBreakpoint> {
        self.breakpoints.values()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DapStackFrame
// ═══════════════════════════════════════════════════════════════════════

/// A stack frame in the debugger call stack.
#[derive(Debug, Clone)]
pub struct DapStackFrame {
    /// Unique frame ID.
    pub id: u32,
    /// Function or scope name.
    pub name: String,
    /// Source location of the current instruction in this frame.
    pub source: DapSourceLocation,
    /// Local variables visible in this frame.
    pub locals: Vec<VariableInfo>,
}

impl DapStackFrame {
    /// Creates a new stack frame with a unique ID.
    pub fn new(name: impl Into<String>, source: DapSourceLocation) -> Self {
        Self {
            id: NEXT_FRAME_ID.fetch_add(1, Ordering::Relaxed),
            name: name.into(),
            source,
            locals: Vec::new(),
        }
    }

    /// Adds a local variable to this frame.
    pub fn add_local(&mut self, var: VariableInfo) {
        self.locals.push(var);
    }

    /// Looks up a local variable by name.
    pub fn find_local(&self, name: &str) -> Option<&VariableInfo> {
        self.locals.iter().find(|v| v.name == name)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DapDebugState
// ═══════════════════════════════════════════════════════════════════════

/// The current execution state of the debugger.
///
/// Tracks the instruction pointer, call stack, and whether the program
/// is running or paused (and why).
#[derive(Debug, Clone)]
pub struct DapDebugState {
    /// Current instruction pointer (index into bytecode or AST node).
    pub instruction_ptr: usize,
    /// Call stack (most recent frame last).
    pub call_stack: Vec<DapStackFrame>,
    /// Whether the program is currently running (not paused).
    pub running: bool,
    /// Reason for being paused (if not running).
    pub paused_reason: Option<String>,
}

impl DapDebugState {
    /// Creates a new debug state at instruction 0, not running.
    pub fn new() -> Self {
        Self {
            instruction_ptr: 0,
            call_stack: Vec::new(),
            running: false,
            paused_reason: None,
        }
    }

    /// Pauses execution with the given reason.
    pub fn pause(&mut self, reason: impl Into<String>) {
        self.running = false;
        self.paused_reason = Some(reason.into());
    }

    /// Resumes execution, clearing the paused reason.
    pub fn resume(&mut self) {
        self.running = true;
        self.paused_reason = None;
    }

    /// Pushes a new frame onto the call stack.
    pub fn push_frame(&mut self, frame: DapStackFrame) {
        self.call_stack.push(frame);
    }

    /// Pops the top frame from the call stack.
    pub fn pop_frame(&mut self) -> Option<DapStackFrame> {
        self.call_stack.pop()
    }

    /// Returns the current (topmost) stack frame.
    pub fn current_frame(&self) -> Option<&DapStackFrame> {
        self.call_stack.last()
    }

    /// Returns a mutable reference to the current frame.
    pub fn current_frame_mut(&mut self) -> Option<&mut DapStackFrame> {
        self.call_stack.last_mut()
    }

    /// Returns the call stack depth.
    pub fn depth(&self) -> usize {
        self.call_stack.len()
    }

    /// Finds a stack frame by its ID.
    pub fn find_frame(&self, frame_id: u32) -> Option<&DapStackFrame> {
        self.call_stack.iter().find(|f| f.id == frame_id)
    }

    /// Finds a mutable stack frame by its ID.
    pub fn find_frame_mut(&mut self, frame_id: u32) -> Option<&mut DapStackFrame> {
        self.call_stack.iter_mut().find(|f| f.id == frame_id)
    }
}

impl Default for DapDebugState {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DebugInfo
// ═══════════════════════════════════════════════════════════════════════

/// Aggregated debug information for a debugging session.
///
/// Combines the source map, per-scope variable info, and breakpoint
/// manager into a single container used by the DAP protocol handler.
#[derive(Debug, Clone, Default)]
pub struct DebugInfo {
    /// Source map: instruction index to source location.
    pub source_map: DapSourceMap,
    /// Variables indexed by scope name.
    pub variables: HashMap<String, Vec<VariableInfo>>,
    /// Breakpoint manager.
    pub breakpoint_manager: BreakpointManager,
}

impl DebugInfo {
    /// Creates a new empty debug info container.
    pub fn new() -> Self {
        Self {
            source_map: DapSourceMap::new(),
            variables: HashMap::new(),
            breakpoint_manager: BreakpointManager::new(),
        }
    }

    /// Adds a variable to the named scope.
    pub fn add_variable(&mut self, scope_name: &str, var: VariableInfo) {
        self.variables
            .entry(scope_name.to_string())
            .or_default()
            .push(var);
    }

    /// Returns variables for a given scope.
    pub fn variables_in_scope(&self, scope_name: &str) -> &[VariableInfo] {
        self.variables
            .get(scope_name)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DebugError
// ═══════════════════════════════════════════════════════════════════════

/// Errors that can occur during debugging operations.
#[derive(Debug, thiserror::Error)]
pub enum DebugError {
    /// Breakpoint not found by ID.
    #[error("breakpoint {id} not found")]
    BreakpointNotFound {
        /// The breakpoint ID that was not found.
        id: u32,
    },

    /// Stack frame not found by ID.
    #[error("stack frame {frame_id} not found")]
    FrameNotFound {
        /// The frame ID that was not found.
        frame_id: u32,
    },

    /// Variable not found in the given scope.
    #[error("variable '{name}' not found in scope")]
    VariableNotFound {
        /// The variable name that was not found.
        name: String,
    },

    /// Expression evaluation failed.
    #[error("evaluation error: {message}")]
    EvalError {
        /// Description of the evaluation failure.
        message: String,
    },

    /// Program not loaded.
    #[error("no program loaded for debugging")]
    NoProgramLoaded,

    /// Invalid source location.
    #[error("invalid source location: {file}:{line}")]
    InvalidLocation {
        /// Source file path.
        file: String,
        /// Line number.
        line: u32,
    },

    /// Launch configuration error.
    #[error("launch config error: {message}")]
    LaunchConfigError {
        /// Description of the configuration problem.
        message: String,
    },

    /// JSON parsing error.
    #[error("JSON parse error: {message}")]
    JsonError {
        /// Description of the parse failure.
        message: String,
    },

    /// Watch expression error.
    #[error("watch error: {message}")]
    WatchError {
        /// Description of the watch failure.
        message: String,
    },

    /// I/O error.
    #[error("I/O error: {message}")]
    IoError {
        /// Description of the I/O failure.
        message: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_location_creation() {
        let loc = DapSourceLocation::new("main.fj", 10, 5);
        assert_eq!(loc.file, "main.fj");
        assert_eq!(loc.line, 10);
        assert_eq!(loc.column, 5);
    }

    #[test]
    fn source_map_add_and_lookup() {
        let mut sm = DapSourceMap::new();
        sm.add_mapping(0, DapSourceLocation::new("test.fj", 1, 1));
        sm.add_mapping(5, DapSourceLocation::new("test.fj", 3, 5));

        let loc = sm.lookup(0).expect("mapping at 0");
        assert_eq!(loc.line, 1);

        let loc = sm.lookup(5).expect("mapping at 5");
        assert_eq!(loc.line, 3);
        assert_eq!(loc.column, 5);

        assert!(sm.lookup(99).is_none());
    }

    #[test]
    fn source_map_find_line() {
        let mut sm = DapSourceMap::new();
        sm.add_mapping(0, DapSourceLocation::new("a.fj", 5, 1));
        sm.add_mapping(3, DapSourceLocation::new("a.fj", 5, 10));
        sm.add_mapping(7, DapSourceLocation::new("a.fj", 6, 1));
        sm.add_mapping(10, DapSourceLocation::new("b.fj", 5, 1));

        let indices = sm.find_line("a.fj", 5);
        assert_eq!(indices, vec![0, 3]);

        let indices = sm.find_line("b.fj", 5);
        assert_eq!(indices, vec![10]);

        let indices = sm.find_line("a.fj", 99);
        assert!(indices.is_empty());
    }

    #[test]
    fn source_map_len_and_empty() {
        let mut sm = DapSourceMap::new();
        assert!(sm.is_empty());
        assert_eq!(sm.len(), 0);

        sm.add_mapping(0, DapSourceLocation::new("t.fj", 1, 1));
        assert!(!sm.is_empty());
        assert_eq!(sm.len(), 1);
    }

    #[test]
    fn variable_info_creation() {
        let var = VariableInfo::new("counter", "i64", VarScope::Local, "42");
        assert_eq!(var.name, "counter");
        assert_eq!(var.type_name, "i64");
        assert_eq!(var.scope, VarScope::Local);
        assert_eq!(var.value, "42");
    }

    #[test]
    fn breakpoint_manager_set_and_check() {
        let mut mgr = BreakpointManager::new();
        let bp = mgr.set_breakpoint("main.fj", 10);
        assert_eq!(bp.line, 10);
        assert!(bp.enabled);

        let found = mgr.check_breakpoint("main.fj", 10);
        assert!(found.is_some());
        assert_eq!(found.map(|b| b.id), Some(bp.id));

        assert!(mgr.check_breakpoint("main.fj", 99).is_none());
    }

    #[test]
    fn breakpoint_manager_clear() {
        let mut mgr = BreakpointManager::new();
        let bp = mgr.set_breakpoint("x.fj", 5);
        assert_eq!(mgr.count(), 1);

        mgr.clear_breakpoint(bp.id);
        assert_eq!(mgr.count(), 0);
        assert!(mgr.check_breakpoint("x.fj", 5).is_none());
    }

    #[test]
    fn stack_frame_locals() {
        let loc = DapSourceLocation::new("test.fj", 1, 1);
        let mut frame = DapStackFrame::new("main", loc);
        frame.add_local(VariableInfo::new("x", "i64", VarScope::Local, "10"));
        frame.add_local(VariableInfo::new("y", "f64", VarScope::Parameter, "3.14"));

        assert_eq!(frame.locals.len(), 2);
        let x = frame.find_local("x").expect("should find x");
        assert_eq!(x.value, "10");
        assert!(frame.find_local("z").is_none());
    }

    #[test]
    fn debug_state_pause_and_resume() {
        let mut state = DapDebugState::new();
        assert!(!state.running);

        state.resume();
        assert!(state.running);
        assert!(state.paused_reason.is_none());

        state.pause("breakpoint hit");
        assert!(!state.running);
        assert_eq!(state.paused_reason.as_deref(), Some("breakpoint hit"));
    }

    #[test]
    fn debug_state_call_stack() {
        let mut state = DapDebugState::new();
        assert_eq!(state.depth(), 0);
        assert!(state.current_frame().is_none());

        let frame1 = DapStackFrame::new("main", DapSourceLocation::new("t.fj", 1, 1));
        let f1_id = frame1.id;
        state.push_frame(frame1);
        assert_eq!(state.depth(), 1);
        assert_eq!(
            state.current_frame().map(|f| &f.name),
            Some(&"main".to_string())
        );

        let frame2 = DapStackFrame::new("helper", DapSourceLocation::new("t.fj", 10, 1));
        state.push_frame(frame2);
        assert_eq!(state.depth(), 2);
        assert_eq!(
            state.current_frame().map(|f| &f.name),
            Some(&"helper".to_string())
        );

        assert!(state.find_frame(f1_id).is_some());

        let popped = state.pop_frame().expect("pop");
        assert_eq!(popped.name, "helper");
        assert_eq!(state.depth(), 1);
    }

    #[test]
    fn debug_info_aggregation() {
        let mut info = DebugInfo::new();
        info.source_map
            .add_mapping(0, DapSourceLocation::new("test.fj", 1, 1));
        info.add_variable("main", VariableInfo::new("x", "i64", VarScope::Local, "42"));
        info.add_variable(
            "main",
            VariableInfo::new("y", "f64", VarScope::Local, "3.14"),
        );

        let vars = info.variables_in_scope("main");
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "x");

        let empty = info.variables_in_scope("nonexistent");
        assert!(empty.is_empty());

        let bp = info.breakpoint_manager.set_breakpoint("test.fj", 5);
        assert!(
            info.breakpoint_manager
                .check_breakpoint("test.fj", 5)
                .is_some()
        );
        assert_eq!(bp.line, 5);
    }
}
