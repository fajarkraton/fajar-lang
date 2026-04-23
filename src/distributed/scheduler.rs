//! Task Scheduler — distributed task definition, placement strategies,
//! data locality, load balancing, priority queue with fairness,
//! cancellation, retry with backoff, task dependencies (DAG), resource reservations.
//!
//! Sprint D3: Task Scheduler (10 tasks)
//! All simulated — no real networking.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D3.1: Task Definition (@distributed annotation)
// ═══════════════════════════════════════════════════════════════════════

/// Unique identifier for a distributed task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Task({})", self.0)
    }
}

/// Unique identifier for a worker node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkerId(pub u64);

impl fmt::Display for WorkerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Worker({})", self.0)
    }
}

/// Task state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Waiting to be scheduled.
    Pending,
    /// Assigned to a worker and running.
    Running,
    /// Completed successfully.
    Completed,
    /// Failed.
    Failed,
    /// Cancelled by user.
    Cancelled,
    /// Waiting for dependencies.
    Blocked,
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskState::Pending => write!(f, "Pending"),
            TaskState::Running => write!(f, "Running"),
            TaskState::Completed => write!(f, "Completed"),
            TaskState::Failed => write!(f, "Failed"),
            TaskState::Cancelled => write!(f, "Cancelled"),
            TaskState::Blocked => write!(f, "Blocked"),
        }
    }
}

/// Resource requirements for a task.
#[derive(Debug, Clone)]
pub struct TaskResources {
    /// CPU cores required.
    pub cpu_cores: u32,
    /// GPU count required.
    pub gpu_count: u32,
    /// Memory in MB.
    pub memory_mb: u64,
}

/// A distributed task definition.
#[derive(Debug, Clone)]
pub struct DistributedTask {
    /// Task ID.
    pub id: TaskId,
    /// Human-readable name.
    pub name: String,
    /// Current state.
    pub state: TaskState,
    /// Resource requirements.
    pub resources: TaskResources,
    /// Priority (higher = more urgent).
    pub priority: u32,
    /// Dependencies (task IDs that must complete before this one).
    pub dependencies: Vec<TaskId>,
    /// Assigned worker (None if not yet scheduled).
    pub assigned_worker: Option<WorkerId>,
    /// Preferred data locality (node where data resides).
    pub data_locality: Option<WorkerId>,
    /// Retry count.
    pub retry_count: u32,
    /// Maximum retries allowed.
    pub max_retries: u32,
    /// Serialized task payload.
    pub payload: Vec<u8>,
}

impl DistributedTask {
    /// Creates a new pending task.
    pub fn new(id: TaskId, name: &str, resources: TaskResources) -> Self {
        DistributedTask {
            id,
            name: name.to_string(),
            state: TaskState::Pending,
            resources,
            priority: 0,
            dependencies: Vec::new(),
            assigned_worker: None,
            data_locality: None,
            retry_count: 0,
            max_retries: 3,
            payload: Vec::new(),
        }
    }

    /// Sets the task priority.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Adds a dependency.
    pub fn with_dependency(mut self, dep: TaskId) -> Self {
        self.dependencies.push(dep);
        self
    }

    /// Sets data locality preference.
    pub fn with_locality(mut self, worker: WorkerId) -> Self {
        self.data_locality = Some(worker);
        self
    }

    /// Returns true if all dependencies are in the completed set.
    pub fn dependencies_met(&self, completed: &[TaskId]) -> bool {
        self.dependencies.iter().all(|d| completed.contains(d))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D3.2: Placement Strategy
// ═══════════════════════════════════════════════════════════════════════

/// Strategy for placing tasks on workers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementStrategy {
    /// Round-robin across available workers.
    RoundRobin,
    /// Pick the worker with the least current load.
    LeastLoaded,
    /// Weighted distribution based on worker capacity.
    Weighted,
    /// Prefer the worker where data resides.
    DataLocal,
}

impl fmt::Display for PlacementStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlacementStrategy::RoundRobin => write!(f, "RoundRobin"),
            PlacementStrategy::LeastLoaded => write!(f, "LeastLoaded"),
            PlacementStrategy::Weighted => write!(f, "Weighted"),
            PlacementStrategy::DataLocal => write!(f, "DataLocal"),
        }
    }
}

/// A worker node in the scheduler.
#[derive(Debug, Clone)]
pub struct WorkerNode {
    /// Worker ID.
    pub id: WorkerId,
    /// Available CPU cores.
    pub cpu_cores: u32,
    /// Available GPUs.
    pub gpu_count: u32,
    /// Available memory in MB.
    pub memory_mb: u64,
    /// Current task count (load).
    pub current_tasks: u32,
    /// Weight (for weighted strategy, higher = more capacity).
    pub weight: u32,
    /// Whether the worker is online.
    pub online: bool,
}

impl WorkerNode {
    /// Returns true if the worker can accommodate the task's resources.
    pub fn can_fit(&self, resources: &TaskResources) -> bool {
        self.online
            && self.cpu_cores >= resources.cpu_cores
            && self.gpu_count >= resources.gpu_count
            && self.memory_mb >= resources.memory_mb
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D3.3: Data Locality
// ═══════════════════════════════════════════════════════════════════════

/// Selects a worker honoring data locality preference.
pub fn select_data_local(task: &DistributedTask, workers: &[WorkerNode]) -> Option<WorkerId> {
    // Prefer the data-local worker if it can fit the task.
    if let Some(preferred) = task.data_locality {
        let local_worker = workers.iter().find(|w| w.id == preferred);
        if let Some(w) = local_worker {
            if w.can_fit(&task.resources) {
                return Some(w.id);
            }
        }
    }
    // Fallback: pick any worker that fits.
    workers
        .iter()
        .filter(|w| w.can_fit(&task.resources))
        .min_by_key(|w| w.current_tasks)
        .map(|w| w.id)
}

// ═══════════════════════════════════════════════════════════════════════
// D3.4: Load Balancing
// ═══════════════════════════════════════════════════════════════════════

/// Load balancer for task scheduling.
#[derive(Debug)]
pub struct TaskLoadBalancer {
    /// Strategy.
    pub strategy: PlacementStrategy,
    /// Round-robin counter.
    rr_counter: usize,
}

impl TaskLoadBalancer {
    /// Creates a new load balancer.
    pub fn new(strategy: PlacementStrategy) -> Self {
        TaskLoadBalancer {
            strategy,
            rr_counter: 0,
        }
    }

    /// Selects the best worker for a task.
    pub fn select(&mut self, task: &DistributedTask, workers: &[WorkerNode]) -> Option<WorkerId> {
        let eligible: Vec<&WorkerNode> = workers
            .iter()
            .filter(|w| w.can_fit(&task.resources))
            .collect();

        if eligible.is_empty() {
            return None;
        }

        match &self.strategy {
            PlacementStrategy::RoundRobin => {
                let idx = self.rr_counter % eligible.len();
                self.rr_counter += 1;
                Some(eligible[idx].id)
            }
            PlacementStrategy::LeastLoaded => eligible
                .iter()
                .min_by_key(|w| w.current_tasks)
                .map(|w| w.id),
            PlacementStrategy::Weighted => eligible.iter().max_by_key(|w| w.weight).map(|w| w.id),
            PlacementStrategy::DataLocal => select_data_local(task, workers),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D3.5: Priority Queue with Fairness
// ═══════════════════════════════════════════════════════════════════════

/// A priority-based task queue with fairness guarantees.
#[derive(Debug)]
pub struct PriorityTaskQueue {
    /// Tasks sorted by priority (descending) and insertion order.
    tasks: Vec<DistributedTask>,
    /// Fairness: tracks how many tasks each priority level has consumed.
    fairness_counters: HashMap<u32, u64>,
    /// Fairness weight: max consecutive tasks from same priority.
    pub fairness_window: u64,
}

impl PriorityTaskQueue {
    /// Creates a new priority queue.
    pub fn new(fairness_window: u64) -> Self {
        PriorityTaskQueue {
            tasks: Vec::new(),
            fairness_counters: HashMap::new(),
            fairness_window,
        }
    }

    /// Enqueues a task.
    pub fn enqueue(&mut self, task: DistributedTask) {
        self.tasks.push(task);
        // Keep sorted by priority descending.
        self.tasks.sort_by_key(|t| std::cmp::Reverse(t.priority));
    }

    /// Dequeues the next task respecting fairness.
    pub fn dequeue(&mut self) -> Option<DistributedTask> {
        if self.tasks.is_empty() {
            return None;
        }

        // Find the first task whose priority hasn't exhausted its fairness window.
        let mut selected_idx = None;
        for (idx, task) in self.tasks.iter().enumerate() {
            let counter = self
                .fairness_counters
                .get(&task.priority)
                .copied()
                .unwrap_or(0);
            if counter < self.fairness_window || self.fairness_window == 0 {
                selected_idx = Some(idx);
                break;
            }
        }

        // If all priorities exhausted their window, reset counters and pick top.
        if selected_idx.is_none() {
            self.fairness_counters.clear();
            selected_idx = Some(0);
        }

        let idx = selected_idx.unwrap_or(0);
        let task = self.tasks.remove(idx);
        *self.fairness_counters.entry(task.priority).or_insert(0) += 1;
        Some(task)
    }

    /// Returns the queue length.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Peeks at the next task without removing it.
    pub fn peek(&self) -> Option<&DistributedTask> {
        self.tasks.first()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D3.6: Task Cancellation
// ═══════════════════════════════════════════════════════════════════════

/// Result of a cancellation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelResult {
    /// Task was cancelled successfully.
    Cancelled,
    /// Task already completed.
    AlreadyCompleted,
    /// Task not found.
    NotFound,
    /// Task is in a non-cancellable state.
    NotCancellable(String),
}

impl fmt::Display for CancelResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CancelResult::Cancelled => write!(f, "Cancelled"),
            CancelResult::AlreadyCompleted => write!(f, "Already completed"),
            CancelResult::NotFound => write!(f, "Not found"),
            CancelResult::NotCancellable(reason) => write!(f, "Not cancellable: {reason}"),
        }
    }
}

/// Attempts to cancel a task.
pub fn cancel_task(tasks: &mut [DistributedTask], task_id: TaskId) -> CancelResult {
    let task = tasks.iter_mut().find(|t| t.id == task_id);
    match task {
        None => CancelResult::NotFound,
        Some(t) => match t.state {
            TaskState::Completed => CancelResult::AlreadyCompleted,
            TaskState::Cancelled => CancelResult::NotCancellable("already cancelled".to_string()),
            TaskState::Pending | TaskState::Blocked => {
                t.state = TaskState::Cancelled;
                CancelResult::Cancelled
            }
            TaskState::Running => {
                t.state = TaskState::Cancelled;
                CancelResult::Cancelled
            }
            TaskState::Failed => {
                t.state = TaskState::Cancelled;
                CancelResult::Cancelled
            }
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D3.7: Retry with Exponential Backoff
// ═══════════════════════════════════════════════════════════════════════

/// Retry policy for failed tasks.
#[derive(Debug, Clone)]
pub struct TaskRetryPolicy {
    /// Maximum retry attempts.
    pub max_retries: u32,
    /// Initial backoff in milliseconds.
    pub initial_backoff_ms: u64,
    /// Backoff multiplier.
    pub multiplier: f64,
    /// Maximum backoff in milliseconds.
    pub max_backoff_ms: u64,
}

impl Default for TaskRetryPolicy {
    fn default() -> Self {
        TaskRetryPolicy {
            max_retries: 3,
            initial_backoff_ms: 1000,
            multiplier: 2.0,
            max_backoff_ms: 30_000,
        }
    }
}

impl TaskRetryPolicy {
    /// Computes the backoff duration for the given attempt (0-based).
    pub fn backoff_ms(&self, attempt: u32) -> u64 {
        let raw = self.initial_backoff_ms as f64 * self.multiplier.powi(attempt as i32);
        (raw as u64).min(self.max_backoff_ms)
    }

    /// Returns whether a retry should be attempted.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

/// Attempts to retry a failed task. Returns the backoff duration or None.
pub fn retry_task(task: &mut DistributedTask, policy: &TaskRetryPolicy) -> Option<u64> {
    if task.state != TaskState::Failed {
        return None;
    }
    if !policy.should_retry(task.retry_count) {
        return None;
    }
    let backoff = policy.backoff_ms(task.retry_count);
    task.retry_count += 1;
    task.state = TaskState::Pending;
    task.assigned_worker = None;
    Some(backoff)
}

// ═══════════════════════════════════════════════════════════════════════
// D3.8: Task Dependencies (DAG)
// ═══════════════════════════════════════════════════════════════════════

/// A Directed Acyclic Graph of task dependencies.
#[derive(Debug)]
pub struct TaskDag {
    /// All tasks in the DAG.
    pub tasks: HashMap<TaskId, DistributedTask>,
}

impl TaskDag {
    /// Creates a new empty DAG.
    pub fn new() -> Self {
        TaskDag {
            tasks: HashMap::new(),
        }
    }

    /// Adds a task to the DAG.
    pub fn add_task(&mut self, task: DistributedTask) {
        self.tasks.insert(task.id, task);
    }

    /// Returns tasks that are ready to run (all dependencies completed).
    pub fn ready_tasks(&self) -> Vec<TaskId> {
        let completed: Vec<TaskId> = self
            .tasks
            .values()
            .filter(|t| t.state == TaskState::Completed)
            .map(|t| t.id)
            .collect();

        self.tasks
            .values()
            .filter(|t| t.state == TaskState::Pending || t.state == TaskState::Blocked)
            .filter(|t| t.dependencies_met(&completed))
            .map(|t| t.id)
            .collect()
    }

    /// Performs a topological sort. Returns None if a cycle is detected.
    pub fn topological_sort(&self) -> Option<Vec<TaskId>> {
        let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
        for task in self.tasks.values() {
            in_degree.entry(task.id).or_insert(0);
            for &dep in &task.dependencies {
                *in_degree.entry(task.id).or_insert(0) += 1;
                in_degree.entry(dep).or_insert(0);
            }
        }

        let mut queue: Vec<TaskId> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(&id, _)| id)
            .collect();
        queue.sort_by_key(|id| id.0); // deterministic order

        let mut result = Vec::new();
        while let Some(node) = queue.pop() {
            result.push(node);
            // Find tasks that depend on this node.
            for task in self.tasks.values() {
                if task.dependencies.contains(&node) {
                    if let Some(deg) = in_degree.get_mut(&task.id) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push(task.id);
                            queue.sort_by_key(|id| id.0);
                        }
                    }
                }
            }
        }

        if result.len() == self.tasks.len() {
            Some(result)
        } else {
            None // Cycle detected.
        }
    }

    /// Returns the total task count.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

impl Default for TaskDag {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D3.9: Resource Reservations
// ═══════════════════════════════════════════════════════════════════════

/// A resource reservation for a scheduled task.
#[derive(Debug, Clone)]
pub struct ResourceReservation {
    /// Task that owns this reservation.
    pub task_id: TaskId,
    /// Worker holding the reservation.
    pub worker_id: WorkerId,
    /// Reserved CPU cores.
    pub cpu_cores: u32,
    /// Reserved GPUs.
    pub gpu_count: u32,
    /// Reserved memory in MB.
    pub memory_mb: u64,
}

/// Resource reservation manager.
#[derive(Debug, Default)]
pub struct ReservationManager {
    /// Active reservations.
    reservations: Vec<ResourceReservation>,
}

impl ReservationManager {
    /// Creates a new reservation manager.
    pub fn new() -> Self {
        ReservationManager::default()
    }

    /// Reserves resources for a task on a worker.
    pub fn reserve(
        &mut self,
        task: &DistributedTask,
        worker: &mut WorkerNode,
    ) -> Result<(), String> {
        if !worker.can_fit(&task.resources) {
            return Err(format!(
                "worker {} cannot fit task {} resources",
                worker.id, task.id
            ));
        }
        worker.cpu_cores -= task.resources.cpu_cores;
        worker.gpu_count -= task.resources.gpu_count;
        worker.memory_mb -= task.resources.memory_mb;
        worker.current_tasks += 1;

        self.reservations.push(ResourceReservation {
            task_id: task.id,
            worker_id: worker.id,
            cpu_cores: task.resources.cpu_cores,
            gpu_count: task.resources.gpu_count,
            memory_mb: task.resources.memory_mb,
        });
        Ok(())
    }

    /// Releases a reservation, returning resources to the worker.
    pub fn release(&mut self, task_id: TaskId, worker: &mut WorkerNode) -> bool {
        let pos = self
            .reservations
            .iter()
            .position(|r| r.task_id == task_id && r.worker_id == worker.id);

        if let Some(idx) = pos {
            let res = self.reservations.remove(idx);
            worker.cpu_cores += res.cpu_cores;
            worker.gpu_count += res.gpu_count;
            worker.memory_mb += res.memory_mb;
            worker.current_tasks = worker.current_tasks.saturating_sub(1);
            true
        } else {
            false
        }
    }

    /// Returns reserved resources for a worker.
    pub fn reservations_for(&self, worker_id: WorkerId) -> Vec<&ResourceReservation> {
        self.reservations
            .iter()
            .filter(|r| r.worker_id == worker_id)
            .collect()
    }

    /// Total active reservations.
    pub fn total_reservations(&self) -> usize {
        self.reservations.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_resources(cpu: u32, gpu: u32, mem: u64) -> TaskResources {
        TaskResources {
            cpu_cores: cpu,
            gpu_count: gpu,
            memory_mb: mem,
        }
    }

    fn make_worker(id: u64, cpu: u32, gpu: u32, mem: u64) -> WorkerNode {
        WorkerNode {
            id: WorkerId(id),
            cpu_cores: cpu,
            gpu_count: gpu,
            memory_mb: mem,
            current_tasks: 0,
            weight: 1,
            online: true,
        }
    }

    // D3.1 — Task Definition
    #[test]
    fn d3_1_task_creation() {
        let task = DistributedTask::new(TaskId(1), "train-model", make_resources(4, 1, 8192));
        assert_eq!(task.state, TaskState::Pending);
        assert_eq!(task.priority, 0);
        assert!(task.dependencies.is_empty());
    }

    #[test]
    fn d3_1_task_builder() {
        let task = DistributedTask::new(TaskId(1), "infer", make_resources(2, 0, 4096))
            .with_priority(10)
            .with_dependency(TaskId(0))
            .with_locality(WorkerId(3));
        assert_eq!(task.priority, 10);
        assert_eq!(task.dependencies, vec![TaskId(0)]);
        assert_eq!(task.data_locality, Some(WorkerId(3)));
    }

    #[test]
    fn d3_1_task_id_display() {
        assert_eq!(TaskId(42).to_string(), "Task(42)");
        assert_eq!(WorkerId(7).to_string(), "Worker(7)");
    }

    // D3.2 — Placement Strategy
    #[test]
    fn d3_2_placement_display() {
        assert_eq!(PlacementStrategy::RoundRobin.to_string(), "RoundRobin");
        assert_eq!(PlacementStrategy::LeastLoaded.to_string(), "LeastLoaded");
        assert_eq!(PlacementStrategy::DataLocal.to_string(), "DataLocal");
    }

    // D3.3 — Data Locality
    #[test]
    fn d3_3_data_local_selection() {
        let workers = vec![make_worker(1, 8, 1, 16384), make_worker(2, 8, 1, 16384)];
        let task = DistributedTask::new(TaskId(1), "local", make_resources(4, 1, 8192))
            .with_locality(WorkerId(2));
        let selected = select_data_local(&task, &workers);
        assert_eq!(selected, Some(WorkerId(2)));
    }

    #[test]
    fn d3_3_data_local_fallback() {
        let mut workers = vec![
            make_worker(1, 8, 1, 16384),
            make_worker(2, 1, 0, 1024), // too small
        ];
        workers[1].online = true;
        let task = DistributedTask::new(TaskId(1), "local", make_resources(4, 1, 8192))
            .with_locality(WorkerId(2));
        let selected = select_data_local(&task, &workers);
        assert_eq!(selected, Some(WorkerId(1))); // fallback
    }

    // D3.4 — Load Balancing
    #[test]
    fn d3_4_round_robin_scheduling() {
        let workers = vec![make_worker(1, 8, 1, 16384), make_worker(2, 8, 1, 16384)];
        let task = DistributedTask::new(TaskId(1), "t", make_resources(1, 0, 1024));
        let mut lb = TaskLoadBalancer::new(PlacementStrategy::RoundRobin);
        assert_eq!(lb.select(&task, &workers), Some(WorkerId(1)));
        assert_eq!(lb.select(&task, &workers), Some(WorkerId(2)));
        assert_eq!(lb.select(&task, &workers), Some(WorkerId(1)));
    }

    #[test]
    fn d3_4_least_loaded() {
        let mut workers = vec![make_worker(1, 8, 1, 16384), make_worker(2, 8, 1, 16384)];
        workers[0].current_tasks = 5;
        workers[1].current_tasks = 1;
        let task = DistributedTask::new(TaskId(1), "t", make_resources(1, 0, 1024));
        let mut lb = TaskLoadBalancer::new(PlacementStrategy::LeastLoaded);
        assert_eq!(lb.select(&task, &workers), Some(WorkerId(2)));
    }

    // D3.5 — Priority Queue with Fairness
    #[test]
    fn d3_5_priority_ordering() {
        let mut pq = PriorityTaskQueue::new(0); // no fairness limit
        pq.enqueue(
            DistributedTask::new(TaskId(1), "low", make_resources(1, 0, 1024)).with_priority(1),
        );
        pq.enqueue(
            DistributedTask::new(TaskId(2), "high", make_resources(1, 0, 1024)).with_priority(10),
        );
        pq.enqueue(
            DistributedTask::new(TaskId(3), "med", make_resources(1, 0, 1024)).with_priority(5),
        );
        assert_eq!(pq.dequeue().unwrap().id, TaskId(2)); // highest priority
        assert_eq!(pq.dequeue().unwrap().id, TaskId(3));
        assert_eq!(pq.dequeue().unwrap().id, TaskId(1));
    }

    #[test]
    fn d3_5_fairness_window() {
        let mut pq = PriorityTaskQueue::new(2); // max 2 from same priority
        for i in 0..5 {
            pq.enqueue(
                DistributedTask::new(TaskId(i), "high", make_resources(1, 0, 1024))
                    .with_priority(10),
            );
        }
        pq.enqueue(
            DistributedTask::new(TaskId(100), "low", make_resources(1, 0, 1024)).with_priority(1),
        );

        // First 2 from priority 10, then fairness kicks in.
        let t1 = pq.dequeue().unwrap();
        assert_eq!(t1.priority, 10);
        let t2 = pq.dequeue().unwrap();
        assert_eq!(t2.priority, 10);
        // Third dequeue: priority 10 exhausted fairness, should pick priority 1.
        let t3 = pq.dequeue().unwrap();
        assert_eq!(t3.priority, 1);
    }

    // D3.6 — Cancellation
    #[test]
    fn d3_6_cancel_pending_task() {
        let mut tasks = vec![DistributedTask::new(
            TaskId(1),
            "t",
            make_resources(1, 0, 1024),
        )];
        let result = cancel_task(&mut tasks, TaskId(1));
        assert_eq!(result, CancelResult::Cancelled);
        assert_eq!(tasks[0].state, TaskState::Cancelled);
    }

    #[test]
    fn d3_6_cancel_completed_task() {
        let mut tasks = vec![DistributedTask::new(
            TaskId(1),
            "t",
            make_resources(1, 0, 1024),
        )];
        tasks[0].state = TaskState::Completed;
        let result = cancel_task(&mut tasks, TaskId(1));
        assert_eq!(result, CancelResult::AlreadyCompleted);
    }

    #[test]
    fn d3_6_cancel_not_found() {
        let mut tasks: Vec<DistributedTask> = vec![];
        assert_eq!(cancel_task(&mut tasks, TaskId(99)), CancelResult::NotFound);
    }

    #[test]
    fn d3_6_cancel_result_display() {
        assert_eq!(CancelResult::Cancelled.to_string(), "Cancelled");
    }

    // D3.7 — Retry with Backoff
    #[test]
    fn d3_7_retry_backoff() {
        let policy = TaskRetryPolicy::default();
        assert_eq!(policy.backoff_ms(0), 1000);
        assert_eq!(policy.backoff_ms(1), 2000);
        assert_eq!(policy.backoff_ms(2), 4000);
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
    }

    #[test]
    fn d3_7_retry_task() {
        let mut task = DistributedTask::new(TaskId(1), "t", make_resources(1, 0, 1024));
        task.state = TaskState::Failed;
        let policy = TaskRetryPolicy::default();
        let backoff = retry_task(&mut task, &policy);
        assert_eq!(backoff, Some(1000));
        assert_eq!(task.state, TaskState::Pending);
        assert_eq!(task.retry_count, 1);
    }

    #[test]
    fn d3_7_retry_exhausted() {
        let mut task = DistributedTask::new(TaskId(1), "t", make_resources(1, 0, 1024));
        task.state = TaskState::Failed;
        task.retry_count = 3;
        let policy = TaskRetryPolicy::default();
        assert!(retry_task(&mut task, &policy).is_none());
    }

    // D3.8 — Task Dependencies (DAG)
    #[test]
    fn d3_8_dag_topological_sort() {
        let mut dag = TaskDag::new();
        let t1 = DistributedTask::new(TaskId(1), "data-load", make_resources(1, 0, 1024));
        let t2 = DistributedTask::new(TaskId(2), "preprocess", make_resources(1, 0, 1024))
            .with_dependency(TaskId(1));
        let t3 = DistributedTask::new(TaskId(3), "train", make_resources(4, 1, 8192))
            .with_dependency(TaskId(2));
        dag.add_task(t1);
        dag.add_task(t2);
        dag.add_task(t3);

        let order = dag.topological_sort().unwrap();
        assert_eq!(order, vec![TaskId(1), TaskId(2), TaskId(3)]);
    }

    #[test]
    fn d3_8_dag_ready_tasks() {
        let mut dag = TaskDag::new();
        let mut t1 = DistributedTask::new(TaskId(1), "done", make_resources(1, 0, 1024));
        t1.state = TaskState::Completed;
        let t2 = DistributedTask::new(TaskId(2), "ready", make_resources(1, 0, 1024))
            .with_dependency(TaskId(1));
        let t3 = DistributedTask::new(TaskId(3), "blocked", make_resources(1, 0, 1024))
            .with_dependency(TaskId(2));
        dag.add_task(t1);
        dag.add_task(t2);
        dag.add_task(t3);

        let ready = dag.ready_tasks();
        assert_eq!(ready, vec![TaskId(2)]);
    }

    // D3.9 — Resource Reservations
    #[test]
    fn d3_9_reserve_and_release() {
        let task = DistributedTask::new(TaskId(1), "t", make_resources(4, 1, 8192));
        let mut worker = make_worker(1, 8, 2, 16384);
        let mut mgr = ReservationManager::new();

        mgr.reserve(&task, &mut worker).unwrap();
        assert_eq!(worker.cpu_cores, 4);
        assert_eq!(worker.gpu_count, 1);
        assert_eq!(worker.memory_mb, 8192);
        assert_eq!(mgr.total_reservations(), 1);

        mgr.release(TaskId(1), &mut worker);
        assert_eq!(worker.cpu_cores, 8);
        assert_eq!(mgr.total_reservations(), 0);
    }

    #[test]
    fn d3_9_reserve_insufficient_resources() {
        let task = DistributedTask::new(TaskId(1), "big", make_resources(32, 8, 131072));
        let mut worker = make_worker(1, 8, 2, 16384);
        let mut mgr = ReservationManager::new();
        assert!(mgr.reserve(&task, &mut worker).is_err());
    }

    // D3.10 — Integration
    #[test]
    fn d3_10_task_state_display() {
        assert_eq!(TaskState::Pending.to_string(), "Pending");
        assert_eq!(TaskState::Running.to_string(), "Running");
        assert_eq!(TaskState::Completed.to_string(), "Completed");
        assert_eq!(TaskState::Failed.to_string(), "Failed");
        assert_eq!(TaskState::Cancelled.to_string(), "Cancelled");
        assert_eq!(TaskState::Blocked.to_string(), "Blocked");
    }

    #[test]
    fn d3_10_dependencies_met() {
        let task = DistributedTask::new(TaskId(3), "t", make_resources(1, 0, 1024))
            .with_dependency(TaskId(1))
            .with_dependency(TaskId(2));
        assert!(!task.dependencies_met(&[TaskId(1)]));
        assert!(task.dependencies_met(&[TaskId(1), TaskId(2)]));
    }

    #[test]
    fn d3_10_worker_can_fit() {
        let worker = make_worker(1, 8, 2, 16384);
        assert!(worker.can_fit(&make_resources(4, 1, 8192)));
        assert!(!worker.can_fit(&make_resources(16, 0, 0)));
    }
}
