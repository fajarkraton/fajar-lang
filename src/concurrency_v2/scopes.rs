//! Async Scopes — structured concurrency with well-defined lifetimes,
//! error propagation, nesting, concurrency limits, timeouts, and metrics.

use std::fmt;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// S5.1: Scope Primitive
// ═══════════════════════════════════════════════════════════════════════

/// Unique identifier for a spawned task within a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

/// Priority level for task scheduling within a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum TaskPriority {
    /// Low priority — runs after Normal and High tasks.
    Low,
    /// Normal priority — default.
    #[default]
    Normal,
    /// High priority — runs before Normal and Low tasks.
    High,
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskPriority::Low => write!(f, "Low"),
            TaskPriority::Normal => write!(f, "Normal"),
            TaskPriority::High => write!(f, "High"),
        }
    }
}

/// Status of a spawned task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is pending execution.
    Pending,
    /// Task is currently running.
    Running,
    /// Task completed successfully with a result description.
    Completed(String),
    /// Task failed with an error.
    Failed(String),
    /// Task was cancelled.
    Cancelled,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "Pending"),
            TaskStatus::Running => write!(f, "Running"),
            TaskStatus::Completed(v) => write!(f, "Completed({v})"),
            TaskStatus::Failed(e) => write!(f, "Failed({e})"),
            TaskStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// A task descriptor within an async scope.
#[derive(Debug, Clone)]
pub struct ScopedTask {
    /// Unique task identifier.
    pub id: TaskId,
    /// Human-readable task name.
    pub name: String,
    /// Task priority.
    pub priority: TaskPriority,
    /// Current status.
    pub status: TaskStatus,
}

// ═══════════════════════════════════════════════════════════════════════
// S5.2-S5.4: Scope Lifetime, Error Propagation, Nesting
// ═══════════════════════════════════════════════════════════════════════

/// Error that can occur within an async scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeError {
    /// A task within the scope failed.
    TaskFailed {
        /// Which task failed.
        task_id: TaskId,
        /// Error message.
        message: String,
    },
    /// The scope timed out.
    Timeout {
        /// Configured timeout duration.
        duration: Duration,
    },
    /// Concurrency limit exceeded.
    ConcurrencyLimitExceeded {
        /// Maximum allowed.
        limit: usize,
        /// Attempted count.
        attempted: usize,
    },
    /// A nested scope error propagated.
    NestedScopeError {
        /// Depth of nesting.
        depth: usize,
        /// The underlying error.
        inner: Box<ScopeError>,
    },
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeError::TaskFailed { task_id, message } => {
                write!(f, "task {} failed: {}", task_id.0, message)
            }
            ScopeError::Timeout { duration } => {
                write!(f, "scope timed out after {:?}", duration)
            }
            ScopeError::ConcurrencyLimitExceeded { limit, attempted } => {
                write!(
                    f,
                    "concurrency limit exceeded: limit={limit}, attempted={attempted}"
                )
            }
            ScopeError::NestedScopeError { depth, inner } => {
                write!(f, "nested scope error at depth {depth}: {inner}")
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S5.5-S5.8: Return Values, Concurrency Limit, Timeout, Priority
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for an async scope.
#[derive(Debug, Clone)]
pub struct ScopeConfig {
    /// Maximum number of concurrent tasks (None = unlimited).
    pub max_concurrent: Option<usize>,
    /// Timeout for the entire scope (None = no timeout).
    pub timeout: Option<Duration>,
    /// Default task priority.
    pub default_priority: TaskPriority,
}

impl Default for ScopeConfig {
    fn default() -> Self {
        ScopeConfig {
            max_concurrent: None,
            timeout: None,
            default_priority: TaskPriority::Normal,
        }
    }
}

impl ScopeConfig {
    /// Creates a config with a concurrency limit.
    pub fn with_max_concurrent(mut self, limit: usize) -> Self {
        self.max_concurrent = Some(limit);
        self
    }

    /// Creates a config with a timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Creates a config with a default priority.
    pub fn with_default_priority(mut self, priority: TaskPriority) -> Self {
        self.default_priority = priority;
        self
    }
}

/// An async scope that owns all spawned tasks.
#[derive(Debug)]
pub struct AsyncScope {
    /// Scope configuration.
    pub config: ScopeConfig,
    /// All tasks in this scope.
    pub tasks: Vec<ScopedTask>,
    /// Next task ID counter.
    next_id: u64,
    /// Nesting depth (0 = top-level).
    pub depth: usize,
    /// Scope metrics.
    pub metrics: ScopeMetrics,
}

impl AsyncScope {
    /// Creates a new top-level async scope with default config.
    pub fn new() -> Self {
        AsyncScope {
            config: ScopeConfig::default(),
            tasks: Vec::new(),
            next_id: 1,
            depth: 0,
            metrics: ScopeMetrics::default(),
        }
    }

    /// Creates a scope with custom configuration.
    pub fn with_config(config: ScopeConfig) -> Self {
        AsyncScope {
            config,
            tasks: Vec::new(),
            next_id: 1,
            depth: 0,
            metrics: ScopeMetrics::default(),
        }
    }

    /// Creates a nested scope.
    pub fn nested(parent_depth: usize) -> Self {
        let mut scope = AsyncScope::new();
        scope.depth = parent_depth + 1;
        scope
    }

    /// Spawns a new task in this scope with default priority.
    pub fn spawn(&mut self, name: &str) -> Result<TaskId, ScopeError> {
        self.spawn_with_priority(name, self.config.default_priority)
    }

    /// Spawns a new task with explicit priority.
    pub fn spawn_with_priority(
        &mut self,
        name: &str,
        priority: TaskPriority,
    ) -> Result<TaskId, ScopeError> {
        if let Some(limit) = self.config.max_concurrent {
            let active_count = self
                .tasks
                .iter()
                .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::Running))
                .count();
            if active_count >= limit {
                return Err(ScopeError::ConcurrencyLimitExceeded {
                    limit,
                    attempted: active_count + 1,
                });
            }
        }

        let id = TaskId(self.next_id);
        self.next_id += 1;

        self.tasks.push(ScopedTask {
            id,
            name: name.to_string(),
            priority,
            status: TaskStatus::Pending,
        });

        self.metrics.tasks_spawned += 1;
        Ok(id)
    }

    /// Marks a task as completed.
    pub fn complete_task(&mut self, id: TaskId, result: &str) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.status = TaskStatus::Completed(result.to_string());
            self.metrics.tasks_completed += 1;
        }
    }

    /// Marks a task as failed, propagating error if configured.
    pub fn fail_task(&mut self, id: TaskId, error: &str) -> ScopeError {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.status = TaskStatus::Failed(error.to_string());
        }
        self.metrics.tasks_failed += 1;

        // Cancel all sibling tasks (structured concurrency guarantee)
        self.cancel_siblings(id);

        ScopeError::TaskFailed {
            task_id: id,
            message: error.to_string(),
        }
    }

    /// Cancels all tasks except the one specified.
    fn cancel_siblings(&mut self, except: TaskId) {
        for task in &mut self.tasks {
            if task.id != except && matches!(task.status, TaskStatus::Pending | TaskStatus::Running)
            {
                task.status = TaskStatus::Cancelled;
                self.metrics.tasks_cancelled += 1;
            }
        }
    }

    /// Cancels all tasks in the scope.
    pub fn cancel_all(&mut self) {
        for task in &mut self.tasks {
            if matches!(task.status, TaskStatus::Pending | TaskStatus::Running) {
                task.status = TaskStatus::Cancelled;
                self.metrics.tasks_cancelled += 1;
            }
        }
    }

    /// Collects return values from all completed tasks.
    pub fn collect_results(&self) -> Vec<String> {
        self.tasks
            .iter()
            .filter_map(|t| {
                if let TaskStatus::Completed(v) = &t.status {
                    Some(v.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Checks if all tasks have finished (completed, failed, or cancelled).
    pub fn is_finished(&self) -> bool {
        self.tasks.iter().all(|t| {
            matches!(
                t.status,
                TaskStatus::Completed(_) | TaskStatus::Failed(_) | TaskStatus::Cancelled
            )
        })
    }

    /// Returns tasks sorted by priority (High first).
    pub fn tasks_by_priority(&self) -> Vec<&ScopedTask> {
        let mut sorted: Vec<_> = self.tasks.iter().collect();
        sorted.sort_by_key(|x| std::cmp::Reverse(x.priority));
        sorted
    }

    /// Checks if the scope would time out.
    pub fn check_timeout(&self, elapsed: Duration) -> Result<(), ScopeError> {
        if let Some(timeout) = self.config.timeout {
            if elapsed > timeout {
                return Err(ScopeError::Timeout { duration: timeout });
            }
        }
        Ok(())
    }

    /// Propagates an error from a nested scope.
    pub fn propagate_nested_error(&self, inner: ScopeError) -> ScopeError {
        ScopeError::NestedScopeError {
            depth: self.depth + 1,
            inner: Box::new(inner),
        }
    }
}

impl Default for AsyncScope {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S5.9: Scope Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Metrics for an async scope.
#[derive(Debug, Clone, Default)]
pub struct ScopeMetrics {
    /// Total tasks spawned.
    pub tasks_spawned: usize,
    /// Tasks that completed successfully.
    pub tasks_completed: usize,
    /// Tasks that failed.
    pub tasks_failed: usize,
    /// Tasks that were cancelled.
    pub tasks_cancelled: usize,
}

impl ScopeMetrics {
    /// Returns the success rate as a percentage (0-100).
    pub fn success_rate(&self) -> f64 {
        if self.tasks_spawned == 0 {
            return 100.0;
        }
        (self.tasks_completed as f64 / self.tasks_spawned as f64) * 100.0
    }
}

impl fmt::Display for ScopeMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "spawned={}, completed={}, failed={}, cancelled={}",
            self.tasks_spawned, self.tasks_completed, self.tasks_failed, self.tasks_cancelled
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S5.1 — Scope Primitive
    #[test]
    fn s5_1_scope_spawn_tasks() {
        let mut scope = AsyncScope::new();
        let t1 = scope.spawn("task1").unwrap();
        let t2 = scope.spawn("task2").unwrap();
        assert_eq!(scope.tasks.len(), 2);
        assert_ne!(t1, t2);
    }

    #[test]
    fn s5_1_task_status_pending() {
        let mut scope = AsyncScope::new();
        let id = scope.spawn("task").unwrap();
        assert_eq!(scope.tasks[0].status, TaskStatus::Pending);
        assert_eq!(scope.tasks[0].id, id);
    }

    // S5.2 — Scope Lifetime
    #[test]
    fn s5_2_scope_owns_tasks() {
        let mut scope = AsyncScope::new();
        scope.spawn("t1").unwrap();
        scope.spawn("t2").unwrap();
        assert!(!scope.is_finished());
        scope.complete_task(TaskId(1), "done");
        scope.complete_task(TaskId(2), "done");
        assert!(scope.is_finished());
    }

    // S5.3 — Scope Error Propagation
    #[test]
    fn s5_3_task_failure_cancels_siblings() {
        let mut scope = AsyncScope::new();
        scope.spawn("t1").unwrap();
        scope.spawn("t2").unwrap();
        scope.spawn("t3").unwrap();

        let _err = scope.fail_task(TaskId(1), "boom");

        // t2 and t3 should be cancelled
        assert_eq!(scope.tasks[1].status, TaskStatus::Cancelled);
        assert_eq!(scope.tasks[2].status, TaskStatus::Cancelled);
    }

    #[test]
    fn s5_3_error_display() {
        let err = ScopeError::TaskFailed {
            task_id: TaskId(42),
            message: "crash".into(),
        };
        assert!(err.to_string().contains("task 42 failed: crash"));
    }

    // S5.4 — Nested Scopes
    #[test]
    fn s5_4_nested_scope_depth() {
        let parent = AsyncScope::new();
        let child = AsyncScope::nested(parent.depth);
        let grandchild = AsyncScope::nested(child.depth);
        assert_eq!(parent.depth, 0);
        assert_eq!(child.depth, 1);
        assert_eq!(grandchild.depth, 2);
    }

    #[test]
    fn s5_4_nested_error_propagation() {
        let parent = AsyncScope::new();
        let inner_err = ScopeError::TaskFailed {
            task_id: TaskId(1),
            message: "inner fail".into(),
        };
        let outer_err = parent.propagate_nested_error(inner_err);
        if let ScopeError::NestedScopeError { depth, inner } = outer_err {
            assert_eq!(depth, 1);
            assert!(inner.to_string().contains("inner fail"));
        } else {
            panic!("expected NestedScopeError");
        }
    }

    // S5.5 — Scope Return Values
    #[test]
    fn s5_5_collect_results() {
        let mut scope = AsyncScope::new();
        let t1 = scope.spawn("t1").unwrap();
        let t2 = scope.spawn("t2").unwrap();
        scope.complete_task(t1, "result_a");
        scope.complete_task(t2, "result_b");
        let results = scope.collect_results();
        assert_eq!(results, vec!["result_a", "result_b"]);
    }

    // S5.6 — Concurrency Limit
    #[test]
    fn s5_6_concurrency_limit() {
        let config = ScopeConfig::default().with_max_concurrent(2);
        let mut scope = AsyncScope::with_config(config);
        scope.spawn("t1").unwrap();
        scope.spawn("t2").unwrap();
        let result = scope.spawn("t3");
        assert!(result.is_err());
        if let Err(ScopeError::ConcurrencyLimitExceeded { limit, attempted }) = result {
            assert_eq!(limit, 2);
            assert_eq!(attempted, 3);
        }
    }

    // S5.7 — Scope Timeout
    #[test]
    fn s5_7_timeout_check() {
        let config = ScopeConfig::default().with_timeout(Duration::from_secs(5));
        let scope = AsyncScope::with_config(config);
        assert!(scope.check_timeout(Duration::from_secs(3)).is_ok());
        assert!(scope.check_timeout(Duration::from_secs(6)).is_err());
    }

    #[test]
    fn s5_7_no_timeout() {
        let scope = AsyncScope::new();
        assert!(scope.check_timeout(Duration::from_secs(999)).is_ok());
    }

    // S5.8 — Task Priority
    #[test]
    fn s5_8_task_priority_ordering() {
        let mut scope = AsyncScope::new();
        scope.spawn_with_priority("low", TaskPriority::Low).unwrap();
        scope
            .spawn_with_priority("high", TaskPriority::High)
            .unwrap();
        scope
            .spawn_with_priority("normal", TaskPriority::Normal)
            .unwrap();

        let sorted = scope.tasks_by_priority();
        assert_eq!(sorted[0].name, "high");
        assert_eq!(sorted[1].name, "normal");
        assert_eq!(sorted[2].name, "low");
    }

    #[test]
    fn s5_8_priority_display() {
        assert_eq!(TaskPriority::High.to_string(), "High");
        assert_eq!(TaskPriority::Normal.to_string(), "Normal");
        assert_eq!(TaskPriority::Low.to_string(), "Low");
    }

    // S5.9 — Scope Metrics
    #[test]
    fn s5_9_scope_metrics() {
        let mut scope = AsyncScope::new();
        let t1 = scope.spawn("t1").unwrap();
        let t2 = scope.spawn("t2").unwrap();
        scope.spawn("t3").unwrap();

        scope.complete_task(t1, "ok");
        let _err = scope.fail_task(t2, "boom");

        assert_eq!(scope.metrics.tasks_spawned, 3);
        assert_eq!(scope.metrics.tasks_completed, 1);
        assert_eq!(scope.metrics.tasks_failed, 1);
        assert_eq!(scope.metrics.tasks_cancelled, 1); // t3 cancelled as sibling
    }

    #[test]
    fn s5_9_success_rate() {
        let mut metrics = ScopeMetrics::default();
        assert_eq!(metrics.success_rate(), 100.0);
        metrics.tasks_spawned = 4;
        metrics.tasks_completed = 3;
        assert!((metrics.success_rate() - 75.0).abs() < 0.1);
    }

    #[test]
    fn s5_9_metrics_display() {
        let metrics = ScopeMetrics {
            tasks_spawned: 5,
            tasks_completed: 3,
            tasks_failed: 1,
            tasks_cancelled: 1,
        };
        let s = metrics.to_string();
        assert!(s.contains("spawned=5"));
        assert!(s.contains("completed=3"));
    }

    // S5.10 — Integration
    #[test]
    fn s5_10_cancel_all() {
        let mut scope = AsyncScope::new();
        scope.spawn("t1").unwrap();
        scope.spawn("t2").unwrap();
        scope.spawn("t3").unwrap();
        scope.cancel_all();
        assert!(scope.is_finished());
        assert_eq!(scope.metrics.tasks_cancelled, 3);
    }

    #[test]
    fn s5_10_config_builder() {
        let config = ScopeConfig::default()
            .with_max_concurrent(8)
            .with_timeout(Duration::from_secs(30))
            .with_default_priority(TaskPriority::High);
        assert_eq!(config.max_concurrent, Some(8));
        assert_eq!(config.timeout, Some(Duration::from_secs(30)));
        assert_eq!(config.default_priority, TaskPriority::High);
    }
}
