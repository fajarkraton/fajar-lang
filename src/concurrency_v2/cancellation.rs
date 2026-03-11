//! Cancellation & Graceful Shutdown — cancellation tokens, propagation,
//! cancel-safe operations, cleanup handlers, graceful shutdown, drain mode.

use std::fmt;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// S6.1: Cancellation Token
// ═══════════════════════════════════════════════════════════════════════

/// A cancellation token that can be shared across tasks for cooperative cancellation.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    /// Whether cancellation has been requested.
    cancelled: bool,
    /// Reason for cancellation (if any).
    reason: Option<String>,
    /// Child tokens that inherit cancellation from this parent.
    children: Vec<usize>,
    /// Token identifier.
    pub id: usize,
}

impl CancellationToken {
    /// Creates a new, non-cancelled token.
    pub fn new(id: usize) -> Self {
        CancellationToken {
            cancelled: false,
            reason: None,
            children: Vec::new(),
            id,
        }
    }

    /// Checks if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Requests cancellation without a reason.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Requests cancellation with a reason.
    pub fn cancel_with_reason(&mut self, reason: &str) {
        self.cancelled = true;
        self.reason = Some(reason.to_string());
    }

    /// Returns the cancellation reason, if any.
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    /// Registers a child token ID for propagation.
    pub fn add_child(&mut self, child_id: usize) {
        self.children.push(child_id);
    }

    /// Returns child token IDs.
    pub fn children(&self) -> &[usize] {
        &self.children
    }
}

impl fmt::Display for CancellationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.cancelled {
            if let Some(ref reason) = self.reason {
                write!(
                    f,
                    "CancellationToken(id={}, cancelled: {})",
                    self.id, reason
                )
            } else {
                write!(f, "CancellationToken(id={}, cancelled)", self.id)
            }
        } else {
            write!(f, "CancellationToken(id={}, active)", self.id)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.2: Cancel Propagation
// ═══════════════════════════════════════════════════════════════════════

/// A tree of cancellation tokens with parent-child propagation.
#[derive(Debug, Default)]
pub struct CancellationTree {
    /// All tokens in the tree.
    tokens: Vec<CancellationToken>,
    /// Next token ID.
    next_id: usize,
}

impl CancellationTree {
    /// Creates a new empty tree.
    pub fn new() -> Self {
        CancellationTree {
            tokens: Vec::new(),
            next_id: 1,
        }
    }

    /// Creates a root token.
    pub fn create_root(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.tokens.push(CancellationToken::new(id));
        id
    }

    /// Creates a child token linked to a parent.
    pub fn create_child(&mut self, parent_id: usize) -> Option<usize> {
        let child_id = self.next_id;
        self.next_id += 1;

        // Register child with parent
        let parent = self.tokens.iter_mut().find(|t| t.id == parent_id)?;
        parent.add_child(child_id);
        let parent_cancelled = parent.is_cancelled();

        let mut child = CancellationToken::new(child_id);
        // Inherit cancellation from parent
        if parent_cancelled {
            child.cancel();
        }
        self.tokens.push(child);
        Some(child_id)
    }

    /// Cancels a token and propagates to all descendants.
    pub fn cancel(&mut self, id: usize) {
        self.cancel_with_reason(id, None);
    }

    /// Cancels with a reason, propagating to all descendants.
    pub fn cancel_with_reason(&mut self, id: usize, reason: Option<&str>) {
        let mut to_cancel = vec![id];

        while let Some(current_id) = to_cancel.pop() {
            if let Some(token) = self.tokens.iter_mut().find(|t| t.id == current_id) {
                if !token.is_cancelled() {
                    if let Some(reason) = reason {
                        token.cancel_with_reason(reason);
                    } else {
                        token.cancel();
                    }
                    to_cancel.extend_from_slice(token.children());
                }
            }
        }
    }

    /// Checks if a token is cancelled.
    pub fn is_cancelled(&self, id: usize) -> bool {
        self.tokens.iter().any(|t| t.id == id && t.is_cancelled())
    }

    /// Gets a token by ID.
    pub fn get(&self, id: usize) -> Option<&CancellationToken> {
        self.tokens.iter().find(|t| t.id == id)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.3: Cancel-Safe Operations
// ═══════════════════════════════════════════════════════════════════════

/// Whether an operation is safe to cancel mid-execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelSafety {
    /// Safe to cancel at any point — no side effects or state corruption.
    Safe,
    /// Unsafe to cancel — may leave state inconsistent.
    Unsafe,
    /// Idempotent — safe to retry after cancellation.
    Idempotent,
}

impl fmt::Display for CancelSafety {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CancelSafety::Safe => write!(f, "cancel-safe"),
            CancelSafety::Unsafe => write!(f, "cancel-unsafe"),
            CancelSafety::Idempotent => write!(f, "idempotent"),
        }
    }
}

/// An operation with cancel-safety annotation.
#[derive(Debug, Clone)]
pub struct AnnotatedOp {
    /// Operation name.
    pub name: String,
    /// Cancel safety level.
    pub safety: CancelSafety,
    /// Description of cancel behavior.
    pub description: String,
}

/// Lint result for cancel-safety analysis.
#[derive(Debug, Clone)]
pub struct CancelSafetyLint {
    /// Operations that are cancel-unsafe.
    pub warnings: Vec<String>,
}

/// Checks a list of operations for cancel-safety issues.
pub fn lint_cancel_safety(ops: &[AnnotatedOp]) -> CancelSafetyLint {
    let warnings = ops
        .iter()
        .filter(|op| op.safety == CancelSafety::Unsafe)
        .map(|op| {
            format!(
                "operation `{}` is cancel-unsafe: {}",
                op.name, op.description
            )
        })
        .collect();
    CancelSafetyLint { warnings }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.4: Cleanup Handlers
// ═══════════════════════════════════════════════════════════════════════

/// A deferred cleanup handler that runs on scope exit.
#[derive(Debug, Clone)]
pub struct CleanupHandler {
    /// Handler name.
    pub name: String,
    /// Code to execute.
    pub action: String,
    /// Whether this handler has been executed.
    pub executed: bool,
}

/// A stack of cleanup handlers (LIFO execution order).
#[derive(Debug, Default)]
pub struct CleanupStack {
    /// Handlers in registration order.
    handlers: Vec<CleanupHandler>,
}

impl CleanupStack {
    /// Creates a new empty cleanup stack.
    pub fn new() -> Self {
        CleanupStack {
            handlers: Vec::new(),
        }
    }

    /// Registers a cleanup handler.
    pub fn defer(&mut self, name: &str, action: &str) {
        self.handlers.push(CleanupHandler {
            name: name.to_string(),
            action: action.to_string(),
            executed: false,
        });
    }

    /// Executes all handlers in LIFO order, returning the actions.
    pub fn run_all(&mut self) -> Vec<String> {
        let mut actions = Vec::new();
        for handler in self.handlers.iter_mut().rev() {
            if !handler.executed {
                handler.executed = true;
                actions.push(handler.action.clone());
            }
        }
        actions
    }

    /// Returns the number of registered handlers.
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// Returns true if there are no handlers.
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.5-S6.7: Graceful Shutdown
// ═══════════════════════════════════════════════════════════════════════

/// Shutdown state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownPhase {
    /// Normal operation.
    Running,
    /// Shutdown signal received — stop accepting new work.
    Draining,
    /// All work drained — executing cleanup handlers.
    Cleaning,
    /// Shutdown complete.
    Stopped,
    /// Force-killed due to timeout.
    ForceKilled,
}

impl fmt::Display for ShutdownPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShutdownPhase::Running => write!(f, "Running"),
            ShutdownPhase::Draining => write!(f, "Draining"),
            ShutdownPhase::Cleaning => write!(f, "Cleaning"),
            ShutdownPhase::Stopped => write!(f, "Stopped"),
            ShutdownPhase::ForceKilled => write!(f, "ForceKilled"),
        }
    }
}

/// Graceful shutdown controller.
#[derive(Debug)]
pub struct ShutdownController {
    /// Current phase.
    pub phase: ShutdownPhase,
    /// Shutdown timeout (default: 30s).
    pub timeout: Duration,
    /// Number of in-progress tasks.
    pub in_progress: usize,
}

impl ShutdownController {
    /// Creates a new controller with default timeout.
    pub fn new() -> Self {
        ShutdownController {
            phase: ShutdownPhase::Running,
            timeout: Duration::from_secs(30),
            in_progress: 0,
        }
    }

    /// Creates a controller with custom timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        ShutdownController {
            phase: ShutdownPhase::Running,
            timeout,
            in_progress: 0,
        }
    }

    /// Initiates graceful shutdown.
    pub fn shutdown(&mut self) {
        if self.phase == ShutdownPhase::Running {
            self.phase = ShutdownPhase::Draining;
        }
    }

    /// Checks if new work should be accepted.
    pub fn accepts_new_work(&self) -> bool {
        self.phase == ShutdownPhase::Running
    }

    /// Called when a task completes during drain.
    pub fn task_completed(&mut self) {
        if self.in_progress > 0 {
            self.in_progress -= 1;
        }
        if self.phase == ShutdownPhase::Draining && self.in_progress == 0 {
            self.phase = ShutdownPhase::Cleaning;
        }
    }

    /// Transitions to stopped after cleanup.
    pub fn cleanup_done(&mut self) {
        if self.phase == ShutdownPhase::Cleaning {
            self.phase = ShutdownPhase::Stopped;
        }
    }

    /// Force-kill if timeout exceeded.
    pub fn force_kill_if_timeout(&mut self, elapsed: Duration) -> bool {
        if self.phase == ShutdownPhase::Draining && elapsed > self.timeout {
            self.phase = ShutdownPhase::ForceKilled;
            return true;
        }
        false
    }
}

impl Default for ShutdownController {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S6.9: Select with Cancel
// ═══════════════════════════════════════════════════════════════════════

/// Result of a select operation — which branch completed first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectResult {
    /// The first branch completed with a value.
    First(String),
    /// The second branch completed with a value.
    Second(String),
    /// The cancellation token was triggered.
    Cancelled,
}

impl fmt::Display for SelectResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelectResult::First(v) => write!(f, "First({v})"),
            SelectResult::Second(v) => write!(f, "Second({v})"),
            SelectResult::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Simulates a select operation with two tasks and a cancellation token.
pub fn select_with_cancel(
    task1_result: Option<&str>,
    task2_result: Option<&str>,
    token: &CancellationToken,
) -> SelectResult {
    if token.is_cancelled() {
        return SelectResult::Cancelled;
    }
    if let Some(v) = task1_result {
        return SelectResult::First(v.to_string());
    }
    if let Some(v) = task2_result {
        return SelectResult::Second(v.to_string());
    }
    SelectResult::Cancelled
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S6.1 — Cancellation Token
    #[test]
    fn s6_1_token_not_cancelled() {
        let token = CancellationToken::new(1);
        assert!(!token.is_cancelled());
        assert!(token.reason().is_none());
    }

    #[test]
    fn s6_1_token_cancel() {
        let mut token = CancellationToken::new(1);
        token.cancel();
        assert!(token.is_cancelled());
    }

    // S6.2 — Cancel Propagation
    #[test]
    fn s6_2_parent_cancel_propagates() {
        let mut tree = CancellationTree::new();
        let parent = tree.create_root();
        let child1 = tree.create_child(parent).unwrap();
        let child2 = tree.create_child(parent).unwrap();

        tree.cancel(parent);

        assert!(tree.is_cancelled(parent));
        assert!(tree.is_cancelled(child1));
        assert!(tree.is_cancelled(child2));
    }

    #[test]
    fn s6_2_child_cancel_no_propagate_up() {
        let mut tree = CancellationTree::new();
        let parent = tree.create_root();
        let child = tree.create_child(parent).unwrap();

        tree.cancel(child);

        assert!(!tree.is_cancelled(parent));
        assert!(tree.is_cancelled(child));
    }

    // S6.3 — Cancel-Safe Operations
    #[test]
    fn s6_3_lint_unsafe_ops() {
        let ops = vec![
            AnnotatedOp {
                name: "read_file".into(),
                safety: CancelSafety::Safe,
                description: "reads are idempotent".into(),
            },
            AnnotatedOp {
                name: "write_db".into(),
                safety: CancelSafety::Unsafe,
                description: "partial write may corrupt data".into(),
            },
        ];
        let lint = lint_cancel_safety(&ops);
        assert_eq!(lint.warnings.len(), 1);
        assert!(lint.warnings[0].contains("write_db"));
    }

    #[test]
    fn s6_3_cancel_safety_display() {
        assert_eq!(CancelSafety::Safe.to_string(), "cancel-safe");
        assert_eq!(CancelSafety::Unsafe.to_string(), "cancel-unsafe");
        assert_eq!(CancelSafety::Idempotent.to_string(), "idempotent");
    }

    // S6.4 — Cleanup Handlers
    #[test]
    fn s6_4_cleanup_lifo_order() {
        let mut stack = CleanupStack::new();
        stack.defer("close_file", "file.close()");
        stack.defer("release_lock", "lock.release()");
        stack.defer("flush_buffer", "buffer.flush()");

        let actions = stack.run_all();
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0], "buffer.flush()");
        assert_eq!(actions[1], "lock.release()");
        assert_eq!(actions[2], "file.close()");
    }

    #[test]
    fn s6_4_cleanup_runs_once() {
        let mut stack = CleanupStack::new();
        stack.defer("cleanup", "do_cleanup()");
        let first = stack.run_all();
        let second = stack.run_all();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 0); // Already executed
    }

    // S6.5-S6.6 — Graceful Shutdown & Drain Mode
    #[test]
    fn s6_5_shutdown_lifecycle() {
        let mut ctrl = ShutdownController::new();
        assert!(ctrl.accepts_new_work());
        assert_eq!(ctrl.phase, ShutdownPhase::Running);

        ctrl.shutdown();
        assert!(!ctrl.accepts_new_work());
        assert_eq!(ctrl.phase, ShutdownPhase::Draining);
    }

    #[test]
    fn s6_6_drain_then_stop() {
        let mut ctrl = ShutdownController::new();
        ctrl.in_progress = 2;
        ctrl.shutdown();

        ctrl.task_completed();
        assert_eq!(ctrl.phase, ShutdownPhase::Draining);

        ctrl.task_completed();
        assert_eq!(ctrl.phase, ShutdownPhase::Cleaning);

        ctrl.cleanup_done();
        assert_eq!(ctrl.phase, ShutdownPhase::Stopped);
    }

    // S6.7 — Shutdown Timeout
    #[test]
    fn s6_7_force_kill_on_timeout() {
        let mut ctrl = ShutdownController::with_timeout(Duration::from_secs(5));
        ctrl.in_progress = 1;
        ctrl.shutdown();

        assert!(!ctrl.force_kill_if_timeout(Duration::from_secs(3)));
        assert!(ctrl.force_kill_if_timeout(Duration::from_secs(6)));
        assert_eq!(ctrl.phase, ShutdownPhase::ForceKilled);
    }

    // S6.8 — Cancellation Reasons
    #[test]
    fn s6_8_cancel_with_reason() {
        let mut token = CancellationToken::new(1);
        token.cancel_with_reason("timeout");
        assert!(token.is_cancelled());
        assert_eq!(token.reason(), Some("timeout"));
    }

    #[test]
    fn s6_8_tree_cancel_with_reason() {
        let mut tree = CancellationTree::new();
        let root = tree.create_root();
        let child = tree.create_child(root).unwrap();

        tree.cancel_with_reason(root, Some("shutdown"));

        assert!(tree.is_cancelled(root));
        assert!(tree.is_cancelled(child));
        assert_eq!(tree.get(root).unwrap().reason(), Some("shutdown"));
    }

    // S6.9 — Select with Cancel
    #[test]
    fn s6_9_select_first_wins() {
        let token = CancellationToken::new(1);
        let result = select_with_cancel(Some("value1"), None, &token);
        assert_eq!(result, SelectResult::First("value1".into()));
    }

    #[test]
    fn s6_9_select_cancelled() {
        let mut token = CancellationToken::new(1);
        token.cancel();
        let result = select_with_cancel(Some("value1"), None, &token);
        assert_eq!(result, SelectResult::Cancelled);
    }

    #[test]
    fn s6_9_select_second_wins() {
        let token = CancellationToken::new(1);
        let result = select_with_cancel(None, Some("value2"), &token);
        assert_eq!(result, SelectResult::Second("value2".into()));
    }

    // S6.10 — Integration
    #[test]
    fn s6_10_token_display() {
        let mut token = CancellationToken::new(42);
        assert!(token.to_string().contains("active"));
        token.cancel_with_reason("user request");
        assert!(token.to_string().contains("user request"));
    }

    #[test]
    fn s6_10_shutdown_phase_display() {
        assert_eq!(ShutdownPhase::Running.to_string(), "Running");
        assert_eq!(ShutdownPhase::Draining.to_string(), "Draining");
        assert_eq!(ShutdownPhase::ForceKilled.to_string(), "ForceKilled");
    }

    #[test]
    fn s6_10_cleanup_stack_empty() {
        let stack = CleanupStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }
}
