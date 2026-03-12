//! Time-Travel Replay — replay mode, reverse continue/step,
//! reverse search, watchpoint reverse, root cause analysis,
//! timeline view, state diff, replay fidelity.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S26.1: Replay Mode
// ═══════════════════════════════════════════════════════════════════════

/// Replay session state.
#[derive(Debug, Clone)]
pub struct ReplaySession {
    /// Current position in the event log.
    pub position: u64,
    /// Total events in recording.
    pub total_events: u64,
    /// Current execution state (variable values).
    pub state: HashMap<String, String>,
    /// Breakpoints.
    pub breakpoints: Vec<Breakpoint>,
    /// Watchpoints.
    pub watchpoints: Vec<Watchpoint>,
    /// Bookmarks.
    pub bookmarks: Vec<Bookmark>,
    /// Replay direction.
    pub direction: ReplayDirection,
}

/// Replay direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayDirection {
    /// Forward execution.
    Forward,
    /// Reverse (backward) execution.
    Reverse,
}

impl fmt::Display for ReplayDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplayDirection::Forward => write!(f, "Forward"),
            ReplayDirection::Reverse => write!(f, "Reverse"),
        }
    }
}

impl ReplaySession {
    /// Creates a new replay session.
    pub fn new(total_events: u64) -> Self {
        Self {
            position: 0,
            total_events,
            state: HashMap::new(),
            breakpoints: Vec::new(),
            watchpoints: Vec::new(),
            bookmarks: Vec::new(),
            direction: ReplayDirection::Forward,
        }
    }

    /// Whether replay is at the beginning.
    pub fn at_start(&self) -> bool {
        self.position == 0
    }

    /// Whether replay is at the end.
    pub fn at_end(&self) -> bool {
        self.position >= self.total_events
    }

    /// Progress as a fraction (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        if self.total_events == 0 {
            return 0.0;
        }
        self.position as f64 / self.total_events as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S26.2: Reverse Continue
// ═══════════════════════════════════════════════════════════════════════

/// A breakpoint.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    /// Breakpoint ID.
    pub id: u32,
    /// Function name or source location.
    pub location: String,
    /// Whether enabled.
    pub enabled: bool,
    /// Hit count.
    pub hit_count: u32,
}

/// Result of a continue/reverse-continue operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinueResult {
    /// Hit a breakpoint.
    BreakpointHit(u32),
    /// Hit a watchpoint.
    WatchpointHit(u32),
    /// Reached beginning of recording.
    ReachedStart,
    /// Reached end of recording.
    ReachedEnd,
    /// Stopped (user interrupt).
    Stopped,
}

/// Simulates reverse-continue: search backward for a breakpoint match.
pub fn reverse_continue(
    session: &ReplaySession,
    event_locations: &[(u64, String)],
) -> Option<(u64, ContinueResult)> {
    if session.position == 0 {
        return Some((0, ContinueResult::ReachedStart));
    }

    // Search backwards from current position
    for &(seq, ref loc) in event_locations.iter().rev() {
        if seq >= session.position {
            continue;
        }
        for bp in &session.breakpoints {
            if bp.enabled && loc.contains(&bp.location) {
                return Some((seq, ContinueResult::BreakpointHit(bp.id)));
            }
        }
    }

    Some((0, ContinueResult::ReachedStart))
}

// ═══════════════════════════════════════════════════════════════════════
// S26.3: Reverse Step
// ═══════════════════════════════════════════════════════════════════════

/// Step direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// Step forward one statement.
    StepForward,
    /// Step backward one statement.
    StepBackward,
    /// Step into function.
    StepInto,
    /// Step out of function.
    StepOut,
}

/// Result of a step operation.
#[derive(Debug, Clone)]
pub struct StepResult {
    /// New position.
    pub new_position: u64,
    /// Whether a boundary was reached.
    pub at_boundary: bool,
    /// Current source location.
    pub location: Option<String>,
}

/// Performs a step operation.
pub fn step(session: &ReplaySession, kind: StepKind) -> StepResult {
    let new_pos = match kind {
        StepKind::StepForward => (session.position + 1).min(session.total_events),
        StepKind::StepBackward => session.position.saturating_sub(1),
        StepKind::StepInto => (session.position + 1).min(session.total_events),
        StepKind::StepOut => (session.position + 1).min(session.total_events),
    };
    let at_boundary = new_pos == 0 || new_pos >= session.total_events;
    StepResult {
        new_position: new_pos,
        at_boundary,
        location: None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S26.4: Reverse Search
// ═══════════════════════════════════════════════════════════════════════

/// Search query for reverse search.
#[derive(Debug, Clone)]
pub enum SearchQuery {
    /// Last assignment to a variable.
    LastAssignment(String),
    /// Last call to a function.
    LastCall(String),
    /// Last value of variable matching a condition.
    ValueMatch {
        /// Variable name.
        var_name: String,
        /// Expected value.
        expected: String,
    },
}

/// Search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Event sequence number where found.
    pub position: u64,
    /// Description.
    pub description: String,
    /// Found or not.
    pub found: bool,
}

/// Searches backward through variable assignment history.
pub fn reverse_search_var(
    assignments: &[(u64, String, String)], // (seq, var_name, value)
    var_name: &str,
    from_pos: u64,
) -> Option<SearchResult> {
    assignments
        .iter()
        .rev()
        .find(|(seq, name, _)| *seq < from_pos && name == var_name)
        .map(|(seq, name, value)| SearchResult {
            position: *seq,
            description: format!("{name} = {value}"),
            found: true,
        })
}

// ═══════════════════════════════════════════════════════════════════════
// S26.5: Watchpoint Reverse
// ═══════════════════════════════════════════════════════════════════════

/// A watchpoint (break when variable changes).
#[derive(Debug, Clone)]
pub struct Watchpoint {
    /// Watchpoint ID.
    pub id: u32,
    /// Variable name.
    pub variable: String,
    /// Optional condition.
    pub condition: Option<WatchCondition>,
    /// Whether enabled.
    pub enabled: bool,
}

/// Watchpoint condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchCondition {
    /// Break when value equals.
    Equals(String),
    /// Break when value changes from.
    ChangesFrom(String),
    /// Break on any change.
    AnyChange,
}

/// Finds the last point where a watched variable was set.
pub fn reverse_watchpoint(
    assignments: &[(u64, String, String)],
    watchpoint: &Watchpoint,
    from_pos: u64,
) -> Option<u64> {
    assignments
        .iter()
        .rev()
        .filter(|(seq, name, _)| *seq < from_pos && name == &watchpoint.variable)
        .map(|(seq, _, _)| *seq)
        .next()
}

// ═══════════════════════════════════════════════════════════════════════
// S26.6: Root Cause Analysis
// ═══════════════════════════════════════════════════════════════════════

/// Root cause trace — chain of events leading to a crash.
#[derive(Debug, Clone)]
pub struct RootCauseTrace {
    /// Chain of events from first wrong value to crash.
    pub chain: Vec<CauseLink>,
    /// Suspected root cause position.
    pub root_position: u64,
}

/// A link in the causal chain.
#[derive(Debug, Clone)]
pub struct CauseLink {
    /// Event position.
    pub position: u64,
    /// Description.
    pub description: String,
    /// Variable involved.
    pub variable: Option<String>,
}

/// Traces back from a crash point to find the root cause.
pub fn trace_root_cause(
    crash_pos: u64,
    assignments: &[(u64, String, String)],
    crash_var: &str,
) -> RootCauseTrace {
    let mut chain = Vec::new();

    // Find all assignments to the crash variable before the crash
    let relevant: Vec<&(u64, String, String)> = assignments
        .iter()
        .filter(|(seq, name, _)| *seq <= crash_pos && name == crash_var)
        .collect();

    for (seq, name, value) in relevant.iter().rev() {
        chain.push(CauseLink {
            position: *seq,
            description: format!("{name} = {value}"),
            variable: Some(name.clone()),
        });
    }

    let root = chain.last().map_or(crash_pos, |c| c.position);
    chain.reverse();

    RootCauseTrace {
        chain,
        root_position: root,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S26.7: Timeline View
// ═══════════════════════════════════════════════════════════════════════

/// A bookmark on the execution timeline.
#[derive(Debug, Clone)]
pub struct Bookmark {
    /// Position.
    pub position: u64,
    /// Label.
    pub label: String,
}

/// Timeline region.
#[derive(Debug, Clone)]
pub struct TimelineRegion {
    /// Start position.
    pub start: u64,
    /// End position.
    pub end: u64,
    /// Label.
    pub label: String,
    /// Category (e.g., "function", "loop", "io").
    pub category: String,
}

// ═══════════════════════════════════════════════════════════════════════
// S26.8: Diff at Points
// ═══════════════════════════════════════════════════════════════════════

/// State diff between two execution points.
#[derive(Debug, Clone)]
pub struct StateDiff {
    /// Variables that changed.
    pub changed: Vec<VarDiff>,
    /// Variables added.
    pub added: Vec<(String, String)>,
    /// Variables removed.
    pub removed: Vec<String>,
}

/// A variable difference.
#[derive(Debug, Clone)]
pub struct VarDiff {
    /// Variable name.
    pub name: String,
    /// Value at point A.
    pub value_a: String,
    /// Value at point B.
    pub value_b: String,
}

/// Computes state diff between two snapshots.
pub fn diff_states(
    state_a: &HashMap<String, String>,
    state_b: &HashMap<String, String>,
) -> StateDiff {
    let mut changed = Vec::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();

    for (k, va) in state_a {
        match state_b.get(k) {
            Some(vb) if va != vb => changed.push(VarDiff {
                name: k.clone(),
                value_a: va.clone(),
                value_b: vb.clone(),
            }),
            None => removed.push(k.clone()),
            _ => {}
        }
    }

    for (k, vb) in state_b {
        if !state_a.contains_key(k) {
            added.push((k.clone(), vb.clone()));
        }
    }

    StateDiff {
        changed,
        added,
        removed,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S26.9: Replay Fidelity (covered by deterministic replay design)
// ═══════════════════════════════════════════════════════════════════════

/// Replay fidelity check result.
#[derive(Debug, Clone)]
pub struct FidelityCheck {
    /// Whether replay matches original.
    pub matches: bool,
    /// First divergence point (if any).
    pub divergence_at: Option<u64>,
    /// Events verified.
    pub events_verified: u64,
}

/// Verifies replay fidelity by comparing state at checkpoints.
pub fn verify_fidelity(
    original: &[(u64, HashMap<String, String>)],
    replayed: &[(u64, HashMap<String, String>)],
) -> FidelityCheck {
    let mut verified = 0u64;

    for (orig, repl) in original.iter().zip(replayed.iter()) {
        if orig.0 != repl.0 || orig.1 != repl.1 {
            return FidelityCheck {
                matches: false,
                divergence_at: Some(orig.0),
                events_verified: verified,
            };
        }
        verified += 1;
    }

    FidelityCheck {
        matches: true,
        divergence_at: None,
        events_verified: verified,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S26.1 — Replay Session
    #[test]
    fn s26_1_replay_session() {
        let session = ReplaySession::new(1000);
        assert!(session.at_start());
        assert!(!session.at_end());
        assert!((session.progress() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn s26_1_replay_direction() {
        assert_eq!(ReplayDirection::Forward.to_string(), "Forward");
        assert_eq!(ReplayDirection::Reverse.to_string(), "Reverse");
    }

    // S26.2 — Reverse Continue
    #[test]
    fn s26_2_reverse_continue_hit() {
        let mut session = ReplaySession::new(100);
        session.position = 50;
        session.breakpoints.push(Breakpoint {
            id: 1,
            location: "main".into(),
            enabled: true,
            hit_count: 0,
        });

        let events = vec![(10, "main:5".to_string()), (30, "main:10".to_string())];
        let result = reverse_continue(&session, &events);
        assert!(result.is_some());
        let (pos, cr) = result.unwrap();
        assert_eq!(pos, 30);
        assert_eq!(cr, ContinueResult::BreakpointHit(1));
    }

    #[test]
    fn s26_2_reverse_continue_start() {
        let mut session = ReplaySession::new(100);
        session.position = 0;
        let result = reverse_continue(&session, &[]);
        assert!(matches!(result, Some((0, ContinueResult::ReachedStart))));
    }

    // S26.3 — Reverse Step
    #[test]
    fn s26_3_step_forward() {
        let mut session = ReplaySession::new(100);
        session.position = 50;
        let result = step(&session, StepKind::StepForward);
        assert_eq!(result.new_position, 51);
    }

    #[test]
    fn s26_3_step_backward() {
        let mut session = ReplaySession::new(100);
        session.position = 50;
        let result = step(&session, StepKind::StepBackward);
        assert_eq!(result.new_position, 49);
    }

    #[test]
    fn s26_3_step_backward_at_start() {
        let session = ReplaySession::new(100);
        let result = step(&session, StepKind::StepBackward);
        assert_eq!(result.new_position, 0);
        assert!(result.at_boundary);
    }

    // S26.4 — Reverse Search
    #[test]
    fn s26_4_reverse_search_var() {
        let assignments = vec![
            (10, "x".to_string(), "1".to_string()),
            (20, "x".to_string(), "2".to_string()),
            (30, "y".to_string(), "5".to_string()),
        ];
        let result = reverse_search_var(&assignments, "x", 25);
        assert!(result.is_some());
        assert_eq!(result.unwrap().position, 20);
    }

    #[test]
    fn s26_4_reverse_search_not_found() {
        let assignments = vec![(10, "x".to_string(), "1".to_string())];
        let result = reverse_search_var(&assignments, "y", 100);
        assert!(result.is_none());
    }

    // S26.5 — Watchpoint Reverse
    #[test]
    fn s26_5_reverse_watchpoint() {
        let assignments = vec![
            (5, "x".to_string(), "0".to_string()),
            (15, "x".to_string(), "42".to_string()),
            (25, "y".to_string(), "7".to_string()),
        ];
        let wp = Watchpoint {
            id: 1,
            variable: "x".into(),
            condition: None,
            enabled: true,
        };
        let pos = reverse_watchpoint(&assignments, &wp, 20);
        assert_eq!(pos, Some(15));
    }

    // S26.6 — Root Cause Analysis
    #[test]
    fn s26_6_trace_root_cause() {
        let assignments = vec![
            (5, "ptr".to_string(), "0x1000".to_string()),
            (10, "ptr".to_string(), "null".to_string()),
            (15, "ptr".to_string(), "null".to_string()),
        ];
        let trace = trace_root_cause(20, &assignments, "ptr");
        assert!(!trace.chain.is_empty());
        assert_eq!(trace.root_position, 5);
    }

    // S26.7 — Timeline
    #[test]
    fn s26_7_bookmark() {
        let bm = Bookmark {
            position: 42,
            label: "crash point".into(),
        };
        assert_eq!(bm.position, 42);
    }

    // S26.8 — State Diff
    #[test]
    fn s26_8_diff_states() {
        let mut a = HashMap::new();
        a.insert("x".into(), "1".into());
        a.insert("y".into(), "2".into());
        a.insert("z".into(), "3".into());

        let mut b = HashMap::new();
        b.insert("x".into(), "1".into());
        b.insert("y".into(), "99".into());
        b.insert("w".into(), "new".into());

        let diff = diff_states(&a, &b);
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].name, "y");
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 1);
    }

    // S26.9 — Replay Fidelity
    #[test]
    fn s26_9_fidelity_match() {
        let mut s = HashMap::new();
        s.insert("x".into(), "42".into());
        let original = vec![(0, s.clone()), (1, s.clone())];
        let replayed = vec![(0, s.clone()), (1, s.clone())];
        let check = verify_fidelity(&original, &replayed);
        assert!(check.matches);
        assert_eq!(check.events_verified, 2);
    }

    #[test]
    fn s26_9_fidelity_mismatch() {
        let mut s1 = HashMap::new();
        s1.insert("x".into(), "42".into());
        let mut s2 = HashMap::new();
        s2.insert("x".into(), "99".into());
        let original = vec![(0, s1.clone()), (1, s1.clone())];
        let replayed = vec![(0, s1), (1, s2)];
        let check = verify_fidelity(&original, &replayed);
        assert!(!check.matches);
        assert_eq!(check.divergence_at, Some(1));
    }

    // S26.10 — Watch condition
    #[test]
    fn s26_10_watch_condition() {
        let wc = WatchCondition::Equals("42".into());
        assert_eq!(wc, WatchCondition::Equals("42".into()));
        assert_ne!(wc, WatchCondition::AnyChange);
    }
}
