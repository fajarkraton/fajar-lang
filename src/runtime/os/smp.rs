//! SMP multicore scheduler for FajarOS Nova v2.0.
//!
//! Provides advanced scheduling primitives for symmetric multiprocessing:
//! - **CFS** (Completely Fair Scheduler) — vruntime-based fairness
//! - **EDF** (Earliest Deadline First) — real-time deadline scheduling
//! - **Load balancing** — migrate tasks to equalize CPU utilization
//! - **Work stealing** — idle CPUs steal from busiest
//! - **Synchronization** — TicketLock (fair spinlock) and RCU
//!
//! All structures are simulated in-memory for the interpreter and testing.
//! For real hardware, the native-compiled kernel uses these algorithms
//! with actual per-CPU data structures and atomic operations.
//!
//! # Known limitations
//!
//! - **Priority inheritance is not implemented.** When a low-priority task
//!   holds a lock needed by a high-priority task, the low-priority task is
//!   *not* temporarily boosted. This means priority inversion can occur in
//!   bare-metal deployments with mixed real-time and normal workloads.
//!   Implementing priority inheritance (or priority ceiling) for the
//!   `TicketLock` is a prerequisite for production use in hard real-time
//!   systems.

use std::collections::VecDeque;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors from SMP scheduler operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SmpError {
    /// CPU ID is out of range.
    #[error("invalid CPU: {cpu} (max: {max})")]
    InvalidCpu {
        /// The requested CPU index.
        cpu: u32,
        /// Maximum valid CPU index.
        max: u32,
    },

    /// Process not found.
    #[error("process {pid} not found")]
    ProcessNotFound {
        /// The missing process ID.
        pid: u32,
    },

    /// EDF admission rejected — system would be overloaded.
    #[error("EDF admission rejected: total utilization {utilization:.3} >= 1.0")]
    AdmissionRejected {
        /// The total utilization that would result.
        utilization: String,
    },

    /// Process cannot be migrated (pinned to a CPU).
    #[error("process {pid} is pinned to CPU {cpu}")]
    PinnedProcess {
        /// The process ID.
        pid: u32,
        /// The CPU it is pinned to.
        cpu: u32,
    },

    /// Run queue is empty.
    #[error("run queue for CPU {cpu} is empty")]
    EmptyQueue {
        /// The CPU whose queue is empty.
        cpu: u32,
    },

    /// Lock contention error.
    #[error("ticket lock contention exceeded limit")]
    LockContention,
}

// ═══════════════════════════════════════════════════════════════════════
// Scheduling Policy
// ═══════════════════════════════════════════════════════════════════════

/// Scheduling policy for a process.
///
/// Determines how the scheduler selects and preempts processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SchedPolicy {
    /// Standard time-sharing with CFS vruntime tracking.
    #[default]
    Normal,
    /// POSIX SCHED_FIFO — runs until it yields or is preempted by higher priority.
    Fifo,
    /// POSIX SCHED_RR — round-robin within same priority level.
    RoundRobin,
    /// Earliest Deadline First — real-time with hard deadlines.
    Deadline,
    /// Idle policy — only runs when no other process is ready.
    Idle,
}

// ═══════════════════════════════════════════════════════════════════════
// Priority
// ═══════════════════════════════════════════════════════════════════════

/// Process priority level (0 = highest, 31 = lowest).
///
/// Maps to Linux-style nice values: nice -20 = priority 0, nice +19 = priority 31.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(u8);

/// Maximum valid priority level.
pub const MAX_PRIORITY: u8 = 31;

/// Default priority (nice 0).
pub const DEFAULT_PRIORITY: u8 = 20;

/// Minimum nice value.
pub const MIN_NICE: i8 = -20;

/// Maximum nice value.
pub const MAX_NICE: i8 = 19;

impl Priority {
    /// Creates a new priority, clamping to [0, 31].
    pub fn new(level: u8) -> Self {
        Self(if level > MAX_PRIORITY {
            MAX_PRIORITY
        } else {
            level
        })
    }

    /// Converts a UNIX nice value (-20..+19) to a priority (0..31).
    ///
    /// nice -20 → priority 0 (highest)
    /// nice   0 → priority 20
    /// nice +19 → priority 31 (lowest, mapped from 39 via clamp)
    pub fn from_nice(nice: i8) -> Self {
        let clamped = nice.clamp(MIN_NICE, MAX_NICE);
        // Map [-20, +19] to [0, 39], then scale to [0, 31]
        let raw = (clamped as i32 - MIN_NICE as i32) as u8;
        // Scale: 0..39 → 0..31
        let scaled = ((raw as u32 * MAX_PRIORITY as u32) / 39) as u8;
        Self(scaled)
    }

    /// Returns the raw priority level.
    pub fn level(&self) -> u8 {
        self.0
    }
}

impl Default for Priority {
    fn default() -> Self {
        Self(DEFAULT_PRIORITY)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CPU Identifier
// ═══════════════════════════════════════════════════════════════════════

/// Wrapper for a CPU index in an SMP system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CpuId(u32);

impl CpuId {
    /// Creates a new CPU identifier.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns the raw CPU index.
    pub fn index(&self) -> u32 {
        self.0
    }
}

impl From<u32> for CpuId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Process State
// ═══════════════════════════════════════════════════════════════════════

/// Execution state of a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessState {
    /// Currently executing on a CPU.
    Running,
    /// Ready to run, waiting in a run queue.
    #[default]
    Ready,
    /// Blocked on I/O or synchronization.
    Blocked,
    /// Terminated, awaiting cleanup.
    Zombie,
}

// ═══════════════════════════════════════════════════════════════════════
// Process
// ═══════════════════════════════════════════════════════════════════════

/// Default time slice in nanoseconds (10ms).
pub const DEFAULT_TIME_SLICE_NS: u64 = 10_000_000;

/// A schedulable process (or thread/task).
///
/// Contains all scheduling-relevant metadata. The actual process memory
/// and context are managed separately by the kernel.
#[derive(Debug, Clone)]
pub struct Process {
    /// Unique process identifier.
    pub pid: u32,
    /// Human-readable name.
    pub name: String,
    /// Scheduling policy.
    pub policy: SchedPolicy,
    /// Priority level (0 = highest).
    pub priority: Priority,
    /// Virtual runtime for CFS (nanoseconds).
    pub vruntime: u64,
    /// CPU affinity — if set, process can only run on this CPU.
    pub cpu_affinity: Option<CpuId>,
    /// Current execution state.
    pub state: ProcessState,
    /// Remaining time slice in nanoseconds.
    pub time_slice_remaining: u64,
    /// Deadline in nanoseconds (absolute, for EDF scheduling).
    pub deadline_ns: u64,
    /// Period in nanoseconds (for periodic deadline tasks).
    pub period_ns: u64,
    /// Worst-case execution time in nanoseconds (for EDF admission).
    pub wcet_ns: u64,
}

impl Process {
    /// Creates a new process with the given PID, name, policy, and priority.
    pub fn new(pid: u32, name: &str, policy: SchedPolicy, priority: Priority) -> Self {
        Self {
            pid,
            name: name.to_string(),
            policy,
            priority,
            vruntime: 0,
            cpu_affinity: None,
            state: ProcessState::Ready,
            time_slice_remaining: DEFAULT_TIME_SLICE_NS,
            deadline_ns: 0,
            period_ns: 0,
            wcet_ns: 0,
        }
    }

    /// Creates a deadline (EDF) process with timing parameters.
    pub fn new_deadline(
        pid: u32,
        name: &str,
        deadline_ns: u64,
        period_ns: u64,
        wcet_ns: u64,
    ) -> Self {
        Self {
            pid,
            name: name.to_string(),
            policy: SchedPolicy::Deadline,
            priority: Priority::new(0), // Deadline tasks have highest priority
            vruntime: 0,
            cpu_affinity: None,
            state: ProcessState::Ready,
            time_slice_remaining: wcet_ns,
            deadline_ns,
            period_ns,
            wcet_ns,
        }
    }

    /// Returns the CFS weight based on priority.
    ///
    /// Lower priority number = higher weight = more CPU time.
    /// Weight formula: `1024 / (1 + priority)`. This means priority 0
    /// gets weight 1024, priority 31 gets weight 32.
    pub fn weight(&self) -> u64 {
        1024 / (1 + self.priority.level() as u64)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Run Queue
// ═══════════════════════════════════════════════════════════════════════

/// Per-CPU ready queue, ordered by priority then vruntime.
///
/// Processes with lower priority numbers (higher priority) come first.
/// Within the same priority, lower vruntime comes first (CFS fairness).
#[derive(Debug, Clone)]
pub struct RunQueue {
    /// CPU this queue belongs to.
    cpu: CpuId,
    /// Processes in the queue, maintained in sorted order.
    tasks: Vec<Process>,
}

impl RunQueue {
    /// Creates a new empty run queue for the given CPU.
    pub fn new(cpu: CpuId) -> Self {
        Self {
            cpu,
            tasks: Vec::new(),
        }
    }

    /// Returns the CPU this queue belongs to.
    pub fn cpu(&self) -> CpuId {
        self.cpu
    }

    /// Inserts a process into the queue in sorted position.
    ///
    /// Sorting: by priority (ascending = higher priority first),
    /// then by vruntime (ascending = less runtime first).
    pub fn push(&mut self, process: Process) {
        let pos = self
            .tasks
            .binary_search_by(|existing| {
                existing
                    .priority
                    .level()
                    .cmp(&process.priority.level())
                    .then(existing.vruntime.cmp(&process.vruntime))
            })
            .unwrap_or_else(|pos| pos);
        self.tasks.insert(pos, process);
    }

    /// Removes and returns the highest-priority process (front of queue).
    ///
    /// Returns `None` if the queue is empty.
    pub fn pop_next(&mut self) -> Option<Process> {
        if self.tasks.is_empty() {
            None
        } else {
            Some(self.tasks.remove(0))
        }
    }

    /// Returns the number of processes in the queue.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Returns a reference to all tasks in the queue.
    pub fn tasks(&self) -> &[Process] {
        &self.tasks
    }

    /// Removes a process by PID from the queue.
    ///
    /// Returns the process if found, `None` otherwise.
    pub fn remove_by_pid(&mut self, pid: u32) -> Option<Process> {
        if let Some(idx) = self.tasks.iter().position(|p| p.pid == pid) {
            Some(self.tasks.remove(idx))
        } else {
            None
        }
    }

    /// Returns the total vruntime load of this queue.
    pub fn total_load(&self) -> u64 {
        self.tasks.iter().map(|p| p.weight()).sum()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CFS Scheduler
// ═══════════════════════════════════════════════════════════════════════

/// Completely Fair Scheduler — vruntime-based proportional sharing.
///
/// Each process accumulates virtual runtime proportional to the inverse
/// of its weight. Higher-weight (higher-priority) processes accumulate
/// vruntime slower, so they get more CPU time before being preempted.
///
/// The `min_vruntime` field tracks the minimum vruntime across all
/// processes. New processes start at `min_vruntime` to prevent starvation
/// and avoid giving newly created processes an unfair advantage.
#[derive(Debug, Clone)]
pub struct CfsScheduler {
    /// Minimum vruntime across all tracked processes.
    min_vruntime: u64,
    /// Target scheduling latency in nanoseconds (6ms default).
    /// The total period in which every runnable process should run at least once.
    sched_latency_ns: u64,
    /// Minimum granularity in nanoseconds (750us default).
    /// A process is guaranteed at least this much CPU time before preemption.
    min_granularity_ns: u64,
}

/// Default scheduling latency (6ms).
pub const DEFAULT_SCHED_LATENCY_NS: u64 = 6_000_000;

/// Default minimum granularity (750us).
pub const DEFAULT_MIN_GRANULARITY_NS: u64 = 750_000;

/// Default weight for priority 20 (nice 0) processes.
pub const NICE_0_WEIGHT: u64 = 1024;

impl CfsScheduler {
    /// Creates a new CFS scheduler with default parameters.
    pub fn new() -> Self {
        Self {
            min_vruntime: 0,
            sched_latency_ns: DEFAULT_SCHED_LATENCY_NS,
            min_granularity_ns: DEFAULT_MIN_GRANULARITY_NS,
        }
    }

    /// Creates a CFS scheduler with custom latency and granularity.
    pub fn with_params(sched_latency_ns: u64, min_granularity_ns: u64) -> Self {
        Self {
            min_vruntime: 0,
            sched_latency_ns,
            min_granularity_ns,
        }
    }

    /// Returns the current minimum vruntime.
    pub fn min_vruntime(&self) -> u64 {
        self.min_vruntime
    }

    /// Returns the scheduling latency.
    pub fn sched_latency_ns(&self) -> u64 {
        self.sched_latency_ns
    }

    /// Returns the minimum granularity.
    pub fn min_granularity_ns(&self) -> u64 {
        self.min_granularity_ns
    }

    /// Picks the next process to run from a run queue.
    ///
    /// Returns the process with the lowest vruntime among Normal-policy
    /// processes. Non-Normal policies are handled by upper layers.
    pub fn pick_next(&self, queue: &mut RunQueue) -> Option<Process> {
        // Find the process with the lowest vruntime
        if queue.is_empty() {
            return None;
        }

        let mut best_idx = 0;
        let mut best_vruntime = u64::MAX;

        for (idx, task) in queue.tasks().iter().enumerate() {
            if task.policy == SchedPolicy::Normal && task.vruntime < best_vruntime {
                best_vruntime = task.vruntime;
                best_idx = idx;
            }
        }

        if best_vruntime == u64::MAX {
            // No Normal-policy tasks, fall back to first available
            queue.pop_next()
        } else {
            Some(queue.tasks.remove(best_idx))
        }
    }

    /// Updates a process's vruntime after it ran for `ran_ns` nanoseconds.
    ///
    /// The vruntime increment is scaled by the inverse of the process weight:
    /// `delta_vruntime = ran_ns * (NICE_0_WEIGHT / weight)`.
    /// Higher-weight processes accumulate vruntime slower.
    ///
    /// After updating, `min_vruntime` is recomputed as the true minimum
    /// across the current process and all processes in `queue_tasks`.
    /// `min_vruntime` never goes backward (monotonically non-decreasing).
    pub fn update_vruntime(&mut self, process: &mut Process, ran_ns: u64, queue_tasks: &[Process]) {
        let weight = process.weight();
        // Avoid division by zero — minimum weight is 1
        let effective_weight = if weight == 0 { 1 } else { weight };
        let delta = ran_ns * NICE_0_WEIGHT / effective_weight;
        process.vruntime = process.vruntime.saturating_add(delta);

        // Compute the true minimum vruntime across the current process
        // and all Normal-policy processes in the run queue.
        let mut true_min = process.vruntime;
        for task in queue_tasks {
            if task.policy == SchedPolicy::Normal && task.vruntime < true_min {
                true_min = task.vruntime;
            }
        }

        // min_vruntime never goes backward (monotonically non-decreasing)
        if true_min > self.min_vruntime {
            self.min_vruntime = true_min;
        }
    }

    /// Initializes a new process's vruntime to `min_vruntime`.
    ///
    /// This prevents new processes from getting an unfair burst of CPU time
    /// (which would happen if they started at vruntime 0 while others are
    /// at millions of nanoseconds).
    pub fn init_vruntime(&self, process: &mut Process) {
        process.vruntime = self.min_vruntime;
    }

    /// Computes the ideal time slice for a process given the total number
    /// of runnable processes.
    ///
    /// `slice = max(sched_latency / num_runnable, min_granularity)`
    pub fn compute_time_slice(&self, num_runnable: usize) -> u64 {
        if num_runnable == 0 {
            return self.sched_latency_ns;
        }
        let slice = self.sched_latency_ns / num_runnable as u64;
        slice.max(self.min_granularity_ns)
    }

    /// Returns true if a process should be preempted (its time slice expired
    /// or a higher-priority process is available).
    pub fn should_preempt(&self, current: &Process, candidate: &Process) -> bool {
        if candidate.priority.level() < current.priority.level() {
            return true;
        }
        if candidate.priority.level() == current.priority.level()
            && candidate.vruntime + self.min_granularity_ns < current.vruntime
        {
            return true;
        }
        false
    }
}

impl Default for CfsScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// EDF Scheduler
// ═══════════════════════════════════════════════════════════════════════

/// Earliest Deadline First scheduler for real-time tasks.
///
/// EDF is optimal among preemptive uniprocessor schedulers: if a feasible
/// schedule exists, EDF will find it. The schedulability test is:
///
///   sum(wcet_i / period_i) <= 1.0
///
/// Tasks are always dispatched in order of their absolute deadline.
#[derive(Debug, Clone)]
pub struct EdfScheduler {
    /// Currently admitted tasks (for utilization tracking).
    admitted: Vec<EdfTaskInfo>,
}

/// Summary information for an admitted EDF task.
#[derive(Debug, Clone)]
struct EdfTaskInfo {
    /// Process ID.
    pid: u32,
    /// Worst-case execution time (ns).
    wcet_ns: u64,
    /// Period (ns).
    period_ns: u64,
}

impl EdfScheduler {
    /// Creates a new EDF scheduler with no admitted tasks.
    pub fn new() -> Self {
        Self {
            admitted: Vec::new(),
        }
    }

    /// Returns the total utilization of all admitted tasks.
    ///
    /// Utilization = sum(wcet_i / period_i). Must be < 1.0 for feasibility.
    pub fn total_utilization(&self) -> f64 {
        self.admitted
            .iter()
            .map(|t| {
                if t.period_ns == 0 {
                    0.0
                } else {
                    t.wcet_ns as f64 / t.period_ns as f64
                }
            })
            .sum()
    }

    /// Admits a deadline task if the system remains schedulable.
    ///
    /// Returns `Ok(())` if the task is admitted, or `Err(AdmissionRejected)`
    /// if adding the task would push total utilization >= 1.0.
    pub fn admit(&mut self, process: &Process) -> Result<(), SmpError> {
        let new_util = if process.period_ns == 0 {
            0.0
        } else {
            process.wcet_ns as f64 / process.period_ns as f64
        };

        let total = self.total_utilization() + new_util;
        if total >= 1.0 {
            return Err(SmpError::AdmissionRejected {
                utilization: format!("{total:.3}"),
            });
        }

        self.admitted.push(EdfTaskInfo {
            pid: process.pid,
            wcet_ns: process.wcet_ns,
            period_ns: process.period_ns,
        });

        Ok(())
    }

    /// Removes a task from the admitted set.
    pub fn remove(&mut self, pid: u32) {
        self.admitted.retain(|t| t.pid != pid);
    }

    /// Picks the next task to run — the one with the earliest deadline.
    ///
    /// Only considers Deadline-policy tasks. Returns `None` if no
    /// deadline tasks are in the queue.
    pub fn pick_next(&self, queue: &mut RunQueue) -> Option<Process> {
        if queue.is_empty() {
            return None;
        }

        let mut best_idx: Option<usize> = None;
        let mut best_deadline = u64::MAX;

        for (idx, task) in queue.tasks().iter().enumerate() {
            if task.policy == SchedPolicy::Deadline && task.deadline_ns < best_deadline {
                best_deadline = task.deadline_ns;
                best_idx = Some(idx);
            }
        }

        best_idx.map(|idx| queue.tasks.remove(idx))
    }

    /// Returns the number of admitted tasks.
    pub fn num_admitted(&self) -> usize {
        self.admitted.len()
    }
}

impl Default for EdfScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Load Balancer
// ═══════════════════════════════════════════════════════════════════════

/// Imbalance threshold — rebalance if load differs by more than 25%.
pub const IMBALANCE_THRESHOLD_PCT: u64 = 25;

/// Result of load imbalance analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImbalanceInfo {
    /// Index of the busiest CPU.
    pub busiest_cpu: u32,
    /// Index of the idlest CPU.
    pub idlest_cpu: u32,
    /// Number of tasks to migrate from busiest to idlest.
    pub tasks_to_move: usize,
}

/// Load balancer — detects and corrects load imbalance across CPUs.
///
/// Periodically checks if any CPU has significantly more load than others.
/// If imbalance exceeds the threshold, migrates tasks from the busiest
/// CPU to the idlest.
#[derive(Debug, Clone)]
pub struct LoadBalancer {
    /// Minimum interval between rebalance operations (in ticks).
    rebalance_interval: u64,
    /// Ticks since last rebalance.
    ticks_since_rebalance: u64,
}

impl LoadBalancer {
    /// Creates a new load balancer with the given rebalance interval.
    pub fn new(rebalance_interval: u64) -> Self {
        Self {
            rebalance_interval,
            ticks_since_rebalance: 0,
        }
    }

    /// Computes the load imbalance across CPU run queues.
    ///
    /// Returns the busiest and idlest CPU indices, plus how many tasks
    /// should be migrated to equalize load.
    pub fn compute_imbalance(&self, queues: &[RunQueue]) -> Option<ImbalanceInfo> {
        if queues.len() < 2 {
            return None;
        }

        let mut busiest_idx: usize = 0;
        let mut busiest_load: usize = 0;
        let mut idlest_idx: usize = 0;
        let mut idlest_load: usize = usize::MAX;

        for (idx, q) in queues.iter().enumerate() {
            let load = q.len();
            if load > busiest_load {
                busiest_load = load;
                busiest_idx = idx;
            }
            if load < idlest_load {
                idlest_load = load;
                idlest_idx = idx;
            }
        }

        if busiest_idx == idlest_idx {
            return None;
        }

        let diff = busiest_load.saturating_sub(idlest_load);
        let tasks_to_move = diff / 2;

        if tasks_to_move == 0 {
            return None;
        }

        Some(ImbalanceInfo {
            busiest_cpu: busiest_idx as u32,
            idlest_cpu: idlest_idx as u32,
            tasks_to_move,
        })
    }

    /// Returns true if the load imbalance exceeds the threshold (25%).
    pub fn should_rebalance(&self, queues: &[RunQueue]) -> bool {
        if queues.len() < 2 {
            return false;
        }

        let loads: Vec<usize> = queues.iter().map(|q| q.len()).collect();
        let max_load = loads.iter().copied().max().unwrap_or(0);
        let min_load = loads.iter().copied().min().unwrap_or(0);

        if max_load == 0 {
            return false;
        }

        let diff = max_load.saturating_sub(min_load);
        let pct = (diff as u64 * 100) / max_load as u64;
        pct > IMBALANCE_THRESHOLD_PCT
    }

    /// Records a tick and returns true if it's time to rebalance.
    pub fn tick(&mut self) -> bool {
        self.ticks_since_rebalance += 1;
        if self.ticks_since_rebalance >= self.rebalance_interval {
            self.ticks_since_rebalance = 0;
            true
        } else {
            false
        }
    }
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::new(100)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Work Stealing
// ═══════════════════════════════════════════════════════════════════════

/// Work-stealing scheduler — idle CPUs steal tasks from busiest CPUs.
///
/// When a CPU's run queue becomes empty, it steals a task from the CPU
/// with the most queued work. Only non-pinned tasks are eligible for
/// stealing.
#[derive(Debug, Clone)]
pub struct WorkStealing {
    /// Total number of tasks stolen (for metrics).
    tasks_stolen: u64,
}

impl WorkStealing {
    /// Creates a new work-stealing instance.
    pub fn new() -> Self {
        Self { tasks_stolen: 0 }
    }

    /// Steals a task from `from` queue and places it in `to` queue.
    ///
    /// Selects the lowest-priority non-pinned task from the source queue.
    /// Returns the stolen process, or `None` if no eligible tasks exist.
    pub fn steal(&mut self, from: &mut RunQueue, to: &mut RunQueue) -> Option<Process> {
        if from.is_empty() {
            return None;
        }

        // Find the last (lowest-priority) non-pinned task
        let steal_idx = from
            .tasks
            .iter()
            .enumerate()
            .rev()
            .find(|(_, t)| t.cpu_affinity.is_none() && t.policy != SchedPolicy::Deadline)
            .map(|(idx, _)| idx);

        if let Some(idx) = steal_idx {
            let mut task = from.tasks.remove(idx);
            task.state = ProcessState::Ready;
            let stolen = task.clone();
            to.push(task);
            self.tasks_stolen += 1;
            Some(stolen)
        } else {
            None
        }
    }

    /// Returns the total number of tasks stolen.
    pub fn tasks_stolen(&self) -> u64 {
        self.tasks_stolen
    }
}

impl Default for WorkStealing {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Ticket Lock (Fair Spinlock)
// ═══════════════════════════════════════════════════════════════════════

/// Ticket-based fair spinlock (**simulation only**).
///
/// Guarantees FIFO ordering — threads acquire the lock in the order
/// they requested it. Each caller takes a "ticket" (incrementing counter)
/// and waits until the "serving" counter matches their ticket.
///
/// **Simulation note:** This is a single-threaded simulation of a ticket
/// lock. The `lock()` method grants access immediately without spinning,
/// because the interpreter and test harness run in a single thread. The
/// fairness ordering (ticket numbering) is tracked correctly so that
/// scheduling algorithms can reason about lock acquisition order, but
/// there is no actual contention or spin-wait loop.
///
/// On bare metal, `next_ticket` and `now_serving` would be atomics with
/// `Acquire`/`Release` memory ordering, and `lock()` would spin until
/// `now_serving` matches the caller's ticket.
#[derive(Debug, Clone)]
pub struct TicketLock {
    /// Next ticket to be issued.
    next_ticket: u64,
    /// Currently serving ticket number.
    now_serving: u64,
    /// Name of the lock (for debugging).
    name: String,
}

impl TicketLock {
    /// Creates a new unlocked ticket lock.
    pub fn new(name: &str) -> Self {
        Self {
            next_ticket: 0,
            now_serving: 0,
            name: name.to_string(),
        }
    }

    /// Acquires the lock, returning the ticket number.
    ///
    /// **Simulation only:** Grants the lock immediately by setting
    /// `now_serving` to the caller's ticket. In a real kernel this would
    /// spin-wait (`while now_serving != my_ticket {}`) until the lock
    /// holder calls `unlock()`. The immediate grant is correct for the
    /// single-threaded interpreter — no other thread can interleave.
    pub fn lock(&mut self) -> u64 {
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        // Simulation: immediately grant — no spin-wait needed in
        // single-threaded context.
        self.now_serving = ticket;
        ticket
    }

    /// Releases the lock, advancing `now_serving` to the next ticket.
    ///
    /// In a real kernel, this write (with `Release` ordering) would
    /// unblock the next spinning thread whose ticket matches the new
    /// `now_serving` value.
    pub fn unlock(&mut self) {
        self.now_serving += 1;
    }

    /// Returns true if the lock is currently held.
    pub fn is_locked(&self) -> bool {
        self.next_ticket != self.now_serving
    }

    /// Returns the name of the lock.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the number of waiters (tickets issued but not yet served).
    pub fn waiters(&self) -> u64 {
        self.next_ticket.saturating_sub(self.now_serving)
    }

    /// Returns the current serving ticket number.
    pub fn now_serving(&self) -> u64 {
        self.now_serving
    }

    /// Returns the next ticket number to be issued.
    pub fn next_ticket(&self) -> u64 {
        self.next_ticket
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RCU (Read-Copy-Update)
// ═══════════════════════════════════════════════════════════════════════

/// Read-Copy-Update synchronization primitive.
///
/// RCU allows concurrent reads without locking. Writers create a copy,
/// modify the copy, then atomically swap it in. Old data is freed after
/// a "grace period" — when all pre-existing readers have finished.
///
/// This simulated RCU tracks grace periods with a generation counter
/// and a deferred callback queue.
#[derive(Debug, Clone)]
pub struct Rcu {
    /// Current generation number (incremented on each synchronize).
    generation: u64,
    /// Number of active readers in the current generation.
    active_readers: u64,
    /// Deferred callbacks: (generation, callback_id) pairs.
    /// The callback_id is an opaque identifier; in a real kernel it
    /// would be a function pointer.
    deferred_callbacks: VecDeque<(u64, u64)>,
    /// Next callback ID to assign.
    next_callback_id: u64,
}

impl Rcu {
    /// Creates a new RCU instance at generation 0.
    pub fn new() -> Self {
        Self {
            generation: 0,
            active_readers: 0,
            deferred_callbacks: VecDeque::new(),
            next_callback_id: 0,
        }
    }

    /// Enters an RCU read-side critical section.
    ///
    /// The caller must call `read_unlock()` when done. While inside
    /// a read-side section, the data is guaranteed to remain valid.
    pub fn read_lock(&mut self) {
        self.active_readers += 1;
    }

    /// Exits an RCU read-side critical section.
    pub fn read_unlock(&mut self) {
        self.active_readers = self.active_readers.saturating_sub(1);
    }

    /// Waits for all pre-existing readers to finish (grace period).
    ///
    /// In a real implementation, this blocks until all CPUs have passed
    /// through a quiescent state. In simulation, it advances the
    /// generation and processes any callbacks whose grace period is over.
    ///
    /// Returns the number of callbacks that were executed.
    pub fn synchronize(&mut self) -> usize {
        self.generation += 1;

        // Process callbacks whose grace period has elapsed
        // (callbacks registered before the current generation)
        let mut executed = 0;
        while let Some(&(cb_gen, _)) = self.deferred_callbacks.front() {
            if cb_gen < self.generation {
                self.deferred_callbacks.pop_front();
                executed += 1;
            } else {
                break;
            }
        }

        executed
    }

    /// Registers a deferred callback to be executed after the next
    /// grace period.
    ///
    /// Returns the callback ID for tracking.
    pub fn call_rcu(&mut self) -> u64 {
        let id = self.next_callback_id;
        self.next_callback_id += 1;
        self.deferred_callbacks.push_back((self.generation, id));
        id
    }

    /// Returns the current generation number.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the number of active readers.
    pub fn active_readers(&self) -> u64 {
        self.active_readers
    }

    /// Returns the number of pending deferred callbacks.
    pub fn pending_callbacks(&self) -> usize {
        self.deferred_callbacks.len()
    }
}

impl Default for Rcu {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IPI Messages
// ═══════════════════════════════════════════════════════════════════════

/// Inter-Processor Interrupt message types.
///
/// IPIs are used to communicate between CPUs in an SMP system.
/// The APIC (or GIC on ARM) delivers these as interrupts to specific CPUs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpiMessage {
    /// Request the target CPU to reschedule (check its run queue).
    Reschedule,
    /// Request the target CPU to flush its TLB (after page table update).
    TlbFlush {
        /// Virtual address to invalidate (0 = flush all).
        address: u64,
    },
    /// Request the target CPU to execute a function by ID.
    FunctionCall {
        /// Opaque function identifier.
        function_id: u64,
        /// Argument to the function.
        argument: u64,
    },
    /// Request the target CPU to stop (for shutdown or hot-unplug).
    Stop,
}

// ═══════════════════════════════════════════════════════════════════════
// Scheduler Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Aggregated scheduler performance metrics.
///
/// Tracked by the SMP scheduler to monitor scheduling quality and
/// detect performance anomalies.
#[derive(Debug, Clone, Default)]
pub struct SchedulerMetrics {
    /// Total number of context switches across all CPUs.
    pub context_switches: u64,
    /// Total number of task migrations between CPUs.
    pub migrations: u64,
    /// Total number of tasks stolen via work stealing.
    pub stolen_tasks: u64,
    /// Maximum observed scheduling latency in nanoseconds.
    pub max_latency_ns: u64,
    /// Sum of all scheduling latencies (for computing average).
    total_latency_ns: u64,
    /// Number of latency samples.
    latency_samples: u64,
    /// Number of load balance operations performed.
    pub balance_operations: u64,
    /// Number of IPI messages sent.
    pub ipi_count: u64,
}

impl SchedulerMetrics {
    /// Creates new zeroed metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a scheduling latency sample.
    pub fn record_latency(&mut self, latency_ns: u64) {
        self.total_latency_ns = self.total_latency_ns.saturating_add(latency_ns);
        self.latency_samples += 1;
        if latency_ns > self.max_latency_ns {
            self.max_latency_ns = latency_ns;
        }
    }

    /// Returns the average scheduling latency in nanoseconds.
    ///
    /// Returns 0 if no samples have been recorded.
    pub fn avg_latency_ns(&self) -> u64 {
        self.total_latency_ns
            .checked_div(self.latency_samples)
            .unwrap_or(0)
    }

    /// Records a context switch.
    pub fn record_context_switch(&mut self) {
        self.context_switches += 1;
    }

    /// Records a task migration.
    pub fn record_migration(&mut self) {
        self.migrations += 1;
    }

    /// Records a stolen task.
    pub fn record_steal(&mut self) {
        self.stolen_tasks += 1;
    }

    /// Records a load balance operation.
    pub fn record_balance(&mut self) {
        self.balance_operations += 1;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SMP Scheduler (Aggregate)
// ═══════════════════════════════════════════════════════════════════════

/// Aggregate SMP scheduler managing multiple CPUs.
///
/// Coordinates per-CPU run queues, CFS/EDF scheduling, load balancing,
/// and work stealing. This is the top-level entry point for all
/// scheduling decisions in a multicore system.
///
/// # Thread safety
///
/// `SmpScheduler` is **not internally synchronized**. All methods take
/// `&mut self`, so Rust's ownership rules prevent data races at compile
/// time for single-owner use. If the scheduler must be shared across
/// OS threads (e.g., per-CPU timer interrupt handlers), wrap it in
/// `Arc<Mutex<SmpScheduler>>` or an equivalent lock. In the
/// single-threaded interpreter this is unnecessary.
///
/// # Architecture
///
/// ```text
///   SmpScheduler
///   +-- per_cpu_queues[0..N]   (RunQueue per CPU)
///   +-- cfs                    (CfsScheduler — vruntime fairness)
///   +-- edf                    (EdfScheduler — deadline tasks)
///   +-- balancer               (LoadBalancer — equalize load)
///   +-- stealer                (WorkStealing — idle CPU steals)
///   +-- metrics                (SchedulerMetrics — perf tracking)
///   +-- ipi_queue              (pending IPI messages)
/// ```
#[derive(Debug, Clone)]
pub struct SmpScheduler {
    /// Number of CPUs in the system.
    num_cpus: u32,
    /// Per-CPU run queues.
    queues: Vec<RunQueue>,
    /// CFS scheduler (for Normal policy tasks).
    cfs: CfsScheduler,
    /// EDF scheduler (for Deadline policy tasks).
    edf: EdfScheduler,
    /// Load balancer.
    balancer: LoadBalancer,
    /// Work stealer.
    stealer: WorkStealing,
    /// Performance metrics.
    metrics: SchedulerMetrics,
    /// Pending IPI messages: (target_cpu, message).
    ipi_queue: VecDeque<(CpuId, IpiMessage)>,
    /// Currently running process on each CPU (None if idle).
    current: Vec<Option<Process>>,
    /// Global tick counter.
    tick_count: u64,
}

impl SmpScheduler {
    /// Creates a new SMP scheduler for the given number of CPUs.
    ///
    /// Each CPU gets its own run queue. The CFS, EDF, load balancer,
    /// and work stealer are initialized with default parameters.
    pub fn new(num_cpus: u32) -> Self {
        let queues = (0..num_cpus)
            .map(|i| RunQueue::new(CpuId::new(i)))
            .collect();
        let current = (0..num_cpus).map(|_| None).collect();

        Self {
            num_cpus,
            queues,
            cfs: CfsScheduler::new(),
            edf: EdfScheduler::new(),
            balancer: LoadBalancer::new(100),
            stealer: WorkStealing::new(),
            metrics: SchedulerMetrics::new(),
            ipi_queue: VecDeque::new(),
            current,
            tick_count: 0,
        }
    }

    /// Returns the number of CPUs.
    pub fn num_cpus(&self) -> u32 {
        self.num_cpus
    }

    /// Validates a CPU ID.
    fn validate_cpu(&self, cpu: CpuId) -> Result<(), SmpError> {
        if cpu.index() >= self.num_cpus {
            Err(SmpError::InvalidCpu {
                cpu: cpu.index(),
                max: self.num_cpus.saturating_sub(1),
            })
        } else {
            Ok(())
        }
    }

    /// Enqueues a process on a specific CPU's run queue.
    ///
    /// If the process has CPU affinity set, it must match `target_cpu`.
    /// New CFS processes have their vruntime initialized to `min_vruntime`.
    pub fn enqueue(&mut self, mut process: Process, target_cpu: CpuId) -> Result<(), SmpError> {
        self.validate_cpu(target_cpu)?;

        // Check CPU affinity
        if let Some(affinity) = process.cpu_affinity {
            if affinity != target_cpu {
                return Err(SmpError::PinnedProcess {
                    pid: process.pid,
                    cpu: affinity.index(),
                });
            }
        }

        // Initialize vruntime for CFS tasks
        if process.policy == SchedPolicy::Normal {
            self.cfs.init_vruntime(&mut process);
        }

        // Admit EDF tasks
        if process.policy == SchedPolicy::Deadline {
            self.edf.admit(&process)?;
        }

        process.state = ProcessState::Ready;
        self.queues[target_cpu.index() as usize].push(process);

        Ok(())
    }

    /// Picks the next process to run on the given CPU.
    ///
    /// # Scheduling priority order
    ///
    /// 1. **Deadline (EDF)** -- earliest absolute deadline wins. These are
    ///    hard real-time tasks admitted through the EDF schedulability test.
    /// 2. **FIFO / RoundRobin** -- the first FIFO or RR task found in the
    ///    queue is selected. FIFO tasks run until they yield; RR tasks
    ///    share time slices within the same priority band.
    /// 3. **Normal (CFS)** -- the task with the lowest virtual runtime is
    ///    selected, ensuring proportional CPU sharing based on weight.
    /// 4. **Idle** -- only runs when no other policy class has a runnable
    ///    task. Idle tasks are guaranteed to never starve higher classes.
    ///
    /// Mixing FIFO with Normal tasks in the same run queue is supported:
    /// FIFO takes precedence by **policy class**, not by numeric priority.
    /// A FIFO task at priority 20 still runs before a Normal task at
    /// priority 0 because the scheduler checks policy classes in the order
    /// above, regardless of the priority field.
    ///
    /// If the local queue is empty after all classes are exhausted, the
    /// scheduler attempts **work stealing** from the busiest CPU.
    pub fn schedule(&mut self, cpu_id: CpuId) -> Result<Option<Process>, SmpError> {
        self.validate_cpu(cpu_id)?;
        let idx = cpu_id.index() as usize;

        // Put current process back in queue if it exists and is still runnable
        if let Some(mut current) = self.current[idx].take() {
            if current.state == ProcessState::Running {
                current.state = ProcessState::Ready;
                self.queues[idx].push(current);
            }
        }

        // Try EDF first (deadline tasks)
        if let Some(mut proc) = self.edf.pick_next(&mut self.queues[idx]) {
            proc.state = ProcessState::Running;
            self.metrics.record_context_switch();
            self.current[idx] = Some(proc.clone());
            return Ok(Some(proc));
        }

        // Try RT tasks (FIFO/RoundRobin) — they're at the front by priority
        let rt_idx = self.queues[idx]
            .tasks()
            .iter()
            .position(|t| t.policy == SchedPolicy::Fifo || t.policy == SchedPolicy::RoundRobin);
        if let Some(rt_pos) = rt_idx {
            let mut proc = self.queues[idx].tasks.remove(rt_pos);
            proc.state = ProcessState::Running;
            self.metrics.record_context_switch();
            self.current[idx] = Some(proc.clone());
            return Ok(Some(proc));
        }

        // Try CFS (Normal tasks)
        if let Some(mut proc) = self.cfs.pick_next(&mut self.queues[idx]) {
            proc.state = ProcessState::Running;
            proc.time_slice_remaining = self.cfs.compute_time_slice(self.queues[idx].len() + 1);
            self.metrics.record_context_switch();
            self.current[idx] = Some(proc.clone());
            return Ok(Some(proc));
        }

        // Try Idle tasks
        let idle_idx = self.queues[idx]
            .tasks()
            .iter()
            .position(|t| t.policy == SchedPolicy::Idle);
        if let Some(idle_pos) = idle_idx {
            let mut proc = self.queues[idx].tasks.remove(idle_pos);
            proc.state = ProcessState::Running;
            self.metrics.record_context_switch();
            self.current[idx] = Some(proc.clone());
            return Ok(Some(proc));
        }

        // Queue is empty — try work stealing
        let busiest = self.find_busiest_cpu(cpu_id);
        if let Some(busiest_cpu) = busiest {
            let busiest_idx = busiest_cpu.index() as usize;
            // Split borrows: take the two queues we need
            if idx != busiest_idx {
                let (from, to) = if idx < busiest_idx {
                    let (left, right) = self.queues.split_at_mut(busiest_idx);
                    (&mut right[0], &mut left[idx])
                } else {
                    let (left, right) = self.queues.split_at_mut(idx);
                    (&mut left[busiest_idx], &mut right[0])
                };

                if let Some(mut proc) = self.stealer.steal(from, to) {
                    // The task was pushed to `to` by steal(); pop it back out
                    // to set as current
                    if let Some(mut stolen) = to.remove_by_pid(proc.pid) {
                        stolen.state = ProcessState::Running;
                        self.metrics.record_context_switch();
                        self.metrics.record_steal();
                        self.current[idx] = Some(stolen.clone());
                        return Ok(Some(stolen));
                    }
                    // Fallback: use the returned copy
                    proc.state = ProcessState::Running;
                    self.metrics.record_context_switch();
                    self.metrics.record_steal();
                    self.current[idx] = Some(proc.clone());
                    return Ok(Some(proc));
                }
            }
        }

        Ok(None)
    }

    /// Handles a timer tick on the given CPU.
    ///
    /// Updates the current process's vruntime, decrements its time slice,
    /// and triggers preemption if the slice is exhausted. Also periodically
    /// triggers load balancing.
    pub fn tick(&mut self, cpu_id: CpuId) -> Result<(), SmpError> {
        self.validate_cpu(cpu_id)?;
        let idx = cpu_id.index() as usize;
        self.tick_count += 1;

        // Update current process
        if let Some(ref mut proc) = self.current[idx] {
            // Simulate 1ms per tick
            let tick_ns: u64 = 1_000_000;

            // Update vruntime for CFS tasks, passing the queue so
            // min_vruntime can be computed as the true minimum.
            if proc.policy == SchedPolicy::Normal {
                self.cfs
                    .update_vruntime(proc, tick_ns, self.queues[idx].tasks());
            }

            // Decrement time slice
            proc.time_slice_remaining = proc.time_slice_remaining.saturating_sub(tick_ns);

            // Check for preemption
            if proc.time_slice_remaining == 0 {
                proc.state = ProcessState::Ready;
                let preempted = proc.clone();
                self.queues[idx].push(preempted);
                self.current[idx] = None;

                // Send reschedule IPI
                self.ipi_queue.push_back((cpu_id, IpiMessage::Reschedule));
            }
        }

        // Periodic load balancing
        if self.balancer.tick() {
            self.balance()?;
        }

        Ok(())
    }

    /// Runs the load balancer across all CPUs.
    ///
    /// If imbalance is detected, migrates tasks from the busiest CPU
    /// to the idlest CPU.
    pub fn balance(&mut self) -> Result<(), SmpError> {
        if !self.balancer.should_rebalance(&self.queues) {
            return Ok(());
        }

        if let Some(info) = self.balancer.compute_imbalance(&self.queues) {
            let from_idx = info.busiest_cpu as usize;
            let to_idx = info.idlest_cpu as usize;

            for _ in 0..info.tasks_to_move {
                // Find a non-pinned task to migrate
                let migratable = self.queues[from_idx]
                    .tasks()
                    .iter()
                    .position(|t| t.cpu_affinity.is_none());

                if let Some(task_idx) = migratable {
                    let task = self.queues[from_idx].tasks.remove(task_idx);
                    self.queues[to_idx].push(task);
                    self.metrics.record_migration();
                }
            }

            self.metrics.record_balance();

            // Send reschedule IPI to the idlest CPU
            self.ipi_queue
                .push_back((CpuId::new(info.idlest_cpu), IpiMessage::Reschedule));
        }

        Ok(())
    }

    /// Migrates a process from one CPU to another.
    ///
    /// Returns an error if the process is pinned to a different CPU
    /// or if either CPU ID is invalid.
    pub fn migrate(&mut self, pid: u32, from_cpu: CpuId, to_cpu: CpuId) -> Result<(), SmpError> {
        self.validate_cpu(from_cpu)?;
        self.validate_cpu(to_cpu)?;

        let from_idx = from_cpu.index() as usize;
        let to_idx = to_cpu.index() as usize;

        // Check current process on from_cpu
        let mut task = None;
        if let Some(ref current) = self.current[from_idx] {
            if current.pid == pid {
                task = self.current[from_idx].take();
            }
        }

        // If not current, check the run queue
        if task.is_none() {
            task = self.queues[from_idx].remove_by_pid(pid);
        }

        let task = task.ok_or(SmpError::ProcessNotFound { pid })?;

        // Check affinity
        if let Some(affinity) = task.cpu_affinity {
            if affinity != to_cpu {
                // Put it back
                self.queues[from_idx].push(task);
                return Err(SmpError::PinnedProcess {
                    pid,
                    cpu: affinity.index(),
                });
            }
        }

        self.queues[to_idx].push(task);
        self.metrics.record_migration();

        // Send reschedule IPIs
        self.ipi_queue.push_back((from_cpu, IpiMessage::Reschedule));
        self.ipi_queue.push_back((to_cpu, IpiMessage::Reschedule));

        Ok(())
    }

    /// Returns a snapshot of the current scheduler metrics.
    pub fn metrics(&self) -> &SchedulerMetrics {
        &self.metrics
    }

    /// Returns the total number of queued processes across all CPUs.
    pub fn total_queued(&self) -> usize {
        self.queues.iter().map(|q| q.len()).sum()
    }

    /// Returns the run queue for a specific CPU.
    pub fn queue(&self, cpu: CpuId) -> Result<&RunQueue, SmpError> {
        self.validate_cpu(cpu)?;
        Ok(&self.queues[cpu.index() as usize])
    }

    /// Returns the currently running process on a CPU.
    pub fn current_on(&self, cpu: CpuId) -> Result<Option<&Process>, SmpError> {
        self.validate_cpu(cpu)?;
        Ok(self.current[cpu.index() as usize].as_ref())
    }

    /// Drains all pending IPI messages.
    pub fn drain_ipis(&mut self) -> Vec<(CpuId, IpiMessage)> {
        self.ipi_queue.drain(..).collect()
    }

    /// Returns the global tick counter.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Finds the busiest CPU (excluding the given CPU).
    fn find_busiest_cpu(&self, exclude: CpuId) -> Option<CpuId> {
        let mut busiest: Option<(CpuId, usize)> = None;

        for (idx, q) in self.queues.iter().enumerate() {
            let cpu = CpuId::new(idx as u32);
            if cpu == exclude {
                continue;
            }
            let load = q.len();
            if load > 0 {
                match busiest {
                    None => busiest = Some((cpu, load)),
                    Some((_, best_load)) if load > best_load => {
                        busiest = Some((cpu, load));
                    }
                    _ => {}
                }
            }
        }

        busiest.map(|(cpu, _)| cpu)
    }

    /// Sends an IPI message to a target CPU.
    pub fn send_ipi(&mut self, target: CpuId, message: IpiMessage) -> Result<(), SmpError> {
        self.validate_cpu(target)?;
        self.ipi_queue.push_back((target, message));
        self.metrics.ipi_count += 1;
        Ok(())
    }

    // ── CPU Hotplug (OS1.10) ──────────────────────────────────────────

    /// Adds a new CPU to the scheduler at runtime (CPU hot-add).
    ///
    /// A fresh, empty [`RunQueue`] is created for the new CPU and a
    /// `None` current-process slot is appended. The internal `num_cpus`
    /// counter is incremented with saturating arithmetic to prevent
    /// overflow. Returns the [`CpuId`] of the newly added CPU.
    pub fn add_cpu(&mut self) -> CpuId {
        let new_id = CpuId::new(self.num_cpus);
        self.queues.push(RunQueue::new(new_id));
        self.current.push(None);
        self.num_cpus = self.num_cpus.saturating_add(1);
        new_id
    }

    /// Removes a CPU from the scheduler at runtime (CPU hot-remove).
    ///
    /// All tasks in the CPU's run queue and any currently running process
    /// on that CPU are returned to the caller as orphaned processes. The
    /// CPU's queue and current-slot are removed so it is no longer
    /// accessible. Returns an error if `cpu` is not a valid online CPU.
    ///
    /// The caller is responsible for re-enqueueing orphaned processes on
    /// the remaining CPUs. A [`IpiMessage::Stop`] is sent to the removed
    /// CPU before removal so that bare-metal implementations can park the
    /// hardware thread safely.
    pub fn remove_cpu(&mut self, cpu: CpuId) -> Result<Vec<Process>, SmpError> {
        self.validate_cpu(cpu)?;
        let idx = cpu.index() as usize;

        // Notify the CPU to stop before we pull it out
        self.ipi_queue.push_back((cpu, IpiMessage::Stop));

        // Collect orphaned tasks: currently running + queued
        let mut orphans: Vec<Process> = Vec::new();
        if let Some(running) = self.current.remove(idx) {
            orphans.push(running);
        }
        let removed_queue = self.queues.remove(idx);
        orphans.extend(removed_queue.tasks);

        // Rebuild CpuIds for all queues with index > removed so they
        // stay consistent with their new position in the Vec.
        for (new_idx, q) in self.queues.iter_mut().enumerate() {
            q.cpu = CpuId::new(new_idx as u32);
        }

        self.num_cpus = self.num_cpus.saturating_sub(1);

        Ok(orphans)
    }

    /// Returns `true` if `cpu` is a currently online CPU.
    ///
    /// A CPU is online if its index is within `[0, num_cpus)`.
    pub fn is_cpu_online(&self, cpu: CpuId) -> bool {
        cpu.index() < self.num_cpus
    }

    /// Returns a `Vec` of all currently online [`CpuId`]s.
    pub fn online_cpus(&self) -> Vec<CpuId> {
        (0..self.num_cpus).map(CpuId::new).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// NUMA Topology (OS1.7)
// ═══════════════════════════════════════════════════════════════════════

/// NUMA distance representing local-node access latency.
///
/// Linux convention: local = 10, one hop = 20, two hops = 40, etc.
pub const NUMA_LOCAL_DISTANCE: u8 = 10;

/// A single NUMA node with its CPU set and local memory capacity.
#[derive(Debug, Clone)]
pub struct NumaNode {
    /// Unique node identifier.
    pub id: u32,
    /// List of logical CPU IDs that belong to this node.
    pub cpus: Vec<CpuId>,
    /// Local memory capacity in megabytes.
    pub memory_mb: u64,
}

impl NumaNode {
    /// Creates a new NUMA node with no CPUs and no memory.
    pub fn new(id: u32) -> Self {
        Self {
            id,
            cpus: Vec::new(),
            memory_mb: 0,
        }
    }

    /// Creates a NUMA node with a known CPU list and memory size.
    pub fn with_resources(id: u32, cpus: Vec<CpuId>, memory_mb: u64) -> Self {
        Self {
            id,
            cpus,
            memory_mb,
        }
    }
}

/// NUMA topology — nodes connected by an asymmetric distance matrix.
///
/// The distance matrix models relative memory-access latency between nodes.
/// A distance of [`NUMA_LOCAL_DISTANCE`] (10) means local access. Higher
/// values represent inter-node hops with increasing latency (20, 40, …).
///
/// # Example
///
/// A dual-socket machine where each socket has 4 CPUs:
///
/// ```text
/// node 0: CPUs 0-3, 16 GB, distance to node 1 = 20
/// node 1: CPUs 4-7, 16 GB, distance to node 0 = 20
/// ```
#[derive(Debug, Clone)]
pub struct NumaTopology {
    /// All NUMA nodes in the system.
    nodes: Vec<NumaNode>,
    /// Symmetric distance matrix: `distances[from][to]`.
    ///
    /// `distances[i][i]` is always [`NUMA_LOCAL_DISTANCE`].
    distances: Vec<Vec<u8>>,
}

impl NumaTopology {
    /// Creates an empty topology with no nodes.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            distances: Vec::new(),
        }
    }

    /// Adds a node to the topology.
    ///
    /// The distance matrix is expanded: the new node has local distance
    /// to itself and `NUMA_LOCAL_DISTANCE` * 2 to all existing nodes
    /// as a conservative default. Call `set_distance` to override.
    pub fn add_node(&mut self, node: NumaNode) {
        let old_len = self.nodes.len();
        let new_len = old_len + 1;

        // Expand each existing row with a default remote distance
        for row in &mut self.distances {
            row.push(NUMA_LOCAL_DISTANCE * 2);
        }

        // Add a new row for the new node
        let mut new_row = vec![NUMA_LOCAL_DISTANCE * 2; new_len];
        new_row[old_len] = NUMA_LOCAL_DISTANCE; // self-distance is always local
        self.distances.push(new_row);

        self.nodes.push(node);
    }

    /// Sets the distance between two nodes (both directions).
    ///
    /// Returns `false` if either node index is out of range.
    pub fn set_distance(&mut self, from: u32, to: u32, dist: u8) -> bool {
        let n = self.nodes.len();
        let fi = from as usize;
        let ti = to as usize;
        if fi >= n || ti >= n {
            return false;
        }
        self.distances[fi][ti] = dist;
        self.distances[ti][fi] = dist;
        true
    }

    /// Returns the access distance from node `from` to node `to`.
    ///
    /// Returns `None` if either index is out of range.
    pub fn distance(&self, from: u32, to: u32) -> Option<u8> {
        let n = self.nodes.len();
        let fi = from as usize;
        let ti = to as usize;
        if fi >= n || ti >= n {
            None
        } else {
            Some(self.distances[fi][ti])
        }
    }

    /// Returns the NUMA node that owns the given CPU, if any.
    pub fn node_for_cpu(&self, cpu: CpuId) -> Option<&NumaNode> {
        self.nodes
            .iter()
            .find(|n| n.cpus.iter().any(|c| c.index() == cpu.index()))
    }

    /// Returns all CPUs that belong to the given NUMA node.
    ///
    /// Returns an empty slice if the node ID is not found.
    pub fn cpus_on_node(&self, node_id: u32) -> &[CpuId] {
        self.nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| n.cpus.as_slice())
            .unwrap_or(&[])
    }

    /// Picks the best CPU for a new task, preferring the node that owns
    /// `preferred_cpu` for locality.
    ///
    /// Strategy: find all CPUs on the same node as `preferred_cpu`, then
    /// return the one provided (simplest NUMA-aware placement). If no
    /// node contains `preferred_cpu`, returns `preferred_cpu` unchanged.
    pub fn prefer_local(&self, preferred_cpu: CpuId) -> CpuId {
        match self.node_for_cpu(preferred_cpu) {
            None => preferred_cpu,
            Some(node) => {
                // Return the first CPU on the same node as a locality hint.
                // Callers can iterate cpus_on_node() for a fuller search.
                node.cpus.first().copied().unwrap_or(preferred_cpu)
            }
        }
    }

    /// Returns a slice of all nodes.
    pub fn nodes(&self) -> &[NumaNode] {
        &self.nodes
    }

    /// Returns the number of nodes.
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for NumaTopology {
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

    // ── Priority ──

    #[test]
    fn priority_from_nice_zero_gives_default() {
        let p = Priority::from_nice(0);
        assert_eq!(p.level(), 15); // ~middle of 0..31
    }

    #[test]
    fn priority_from_nice_minus20_gives_highest() {
        let p = Priority::from_nice(-20);
        assert_eq!(p.level(), 0);
    }

    #[test]
    fn priority_from_nice_plus19_gives_lowest() {
        let p = Priority::from_nice(19);
        assert_eq!(p.level(), MAX_PRIORITY);
    }

    #[test]
    fn priority_from_nice_clamps_out_of_range() {
        let low = Priority::from_nice(-100);
        let high = Priority::from_nice(100);
        assert_eq!(low.level(), 0);
        assert_eq!(high.level(), MAX_PRIORITY);
    }

    #[test]
    fn priority_new_clamps_to_max() {
        let p = Priority::new(200);
        assert_eq!(p.level(), MAX_PRIORITY);
    }

    // ── RunQueue ──

    #[test]
    fn run_queue_push_pop_ordering() {
        let mut q = RunQueue::new(CpuId::new(0));
        let low_prio = Process::new(1, "low", SchedPolicy::Normal, Priority::new(20));
        let high_prio = Process::new(2, "high", SchedPolicy::Normal, Priority::new(5));

        q.push(low_prio);
        q.push(high_prio);

        assert_eq!(q.len(), 2);
        let first = q.pop_next().unwrap();
        assert_eq!(first.pid, 2); // Higher priority (lower number) first
        let second = q.pop_next().unwrap();
        assert_eq!(second.pid, 1);
    }

    #[test]
    fn run_queue_empty_pop_returns_none() {
        let mut q = RunQueue::new(CpuId::new(0));
        assert!(q.is_empty());
        assert!(q.pop_next().is_none());
    }

    #[test]
    fn run_queue_same_priority_ordered_by_vruntime() {
        let mut q = RunQueue::new(CpuId::new(0));
        let mut p1 = Process::new(1, "a", SchedPolicy::Normal, Priority::new(10));
        let mut p2 = Process::new(2, "b", SchedPolicy::Normal, Priority::new(10));
        p1.vruntime = 5000;
        p2.vruntime = 1000; // Lower vruntime → should come first

        q.push(p1);
        q.push(p2);

        let first = q.pop_next().unwrap();
        assert_eq!(first.pid, 2); // Lower vruntime first
    }

    #[test]
    fn run_queue_remove_by_pid() {
        let mut q = RunQueue::new(CpuId::new(0));
        q.push(Process::new(
            10,
            "ten",
            SchedPolicy::Normal,
            Priority::new(10),
        ));
        q.push(Process::new(
            20,
            "twenty",
            SchedPolicy::Normal,
            Priority::new(10),
        ));

        let removed = q.remove_by_pid(10);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().pid, 10);
        assert_eq!(q.len(), 1);
        assert!(q.remove_by_pid(10).is_none()); // Already removed
    }

    // ── CFS Scheduler ──

    #[test]
    fn cfs_picks_lowest_vruntime() {
        let cfs = CfsScheduler::new();
        let mut q = RunQueue::new(CpuId::new(0));

        let mut p1 = Process::new(1, "fast", SchedPolicy::Normal, Priority::new(10));
        let mut p2 = Process::new(2, "slow", SchedPolicy::Normal, Priority::new(10));
        p1.vruntime = 5000;
        p2.vruntime = 2000;

        q.push(p1);
        q.push(p2);

        let picked = cfs.pick_next(&mut q).unwrap();
        assert_eq!(picked.pid, 2); // Lowest vruntime
    }

    #[test]
    fn cfs_vruntime_fairness_two_processes() {
        let mut cfs = CfsScheduler::new();
        let mut p1 = Process::new(1, "a", SchedPolicy::Normal, Priority::new(20));
        let mut p2 = Process::new(2, "b", SchedPolicy::Normal, Priority::new(20));

        // Both run for the same amount of time
        cfs.update_vruntime(&mut p1, 1_000_000, &[]);
        cfs.update_vruntime(&mut p2, 1_000_000, &[]);

        // Same priority → same weight → same vruntime increment
        assert_eq!(p1.vruntime, p2.vruntime);
    }

    #[test]
    fn cfs_higher_priority_gets_slower_vruntime() {
        let mut cfs = CfsScheduler::new();
        let mut high = Process::new(1, "high", SchedPolicy::Normal, Priority::new(0));
        let mut low = Process::new(2, "low", SchedPolicy::Normal, Priority::new(31));

        cfs.update_vruntime(&mut high, 1_000_000, &[]);
        cfs.update_vruntime(&mut low, 1_000_000, &[]);

        // Higher priority (lower number) = higher weight = slower vruntime growth
        assert!(high.vruntime < low.vruntime);
    }

    #[test]
    fn cfs_init_vruntime_uses_min() {
        let mut cfs = CfsScheduler::new();

        // Advance min_vruntime
        let mut p1 = Process::new(1, "old", SchedPolicy::Normal, Priority::new(20));
        cfs.update_vruntime(&mut p1, 10_000_000, &[]);
        let min_vrt = cfs.min_vruntime();
        assert!(min_vrt > 0);

        // New process should start at min_vruntime
        let mut p2 = Process::new(2, "new", SchedPolicy::Normal, Priority::new(20));
        cfs.init_vruntime(&mut p2);
        assert_eq!(p2.vruntime, min_vrt);
    }

    #[test]
    fn cfs_should_preempt_higher_priority() {
        let cfs = CfsScheduler::new();
        let current = Process::new(1, "cur", SchedPolicy::Normal, Priority::new(20));
        let candidate = Process::new(2, "cand", SchedPolicy::Normal, Priority::new(5));
        assert!(cfs.should_preempt(&current, &candidate));
    }

    #[test]
    fn cfs_compute_time_slice() {
        let cfs = CfsScheduler::new();
        // With 6 processes: 6ms / 6 = 1ms per process
        let slice = cfs.compute_time_slice(6);
        assert_eq!(slice, 1_000_000);

        // With 1 process: gets full latency
        let slice = cfs.compute_time_slice(1);
        assert_eq!(slice, DEFAULT_SCHED_LATENCY_NS);

        // With 0 processes: returns full latency
        let slice = cfs.compute_time_slice(0);
        assert_eq!(slice, DEFAULT_SCHED_LATENCY_NS);
    }

    // ── EDF Scheduler ──

    #[test]
    fn edf_picks_earliest_deadline() {
        let edf = EdfScheduler::new();
        let mut q = RunQueue::new(CpuId::new(0));

        let p1 = Process::new_deadline(1, "later", 20_000_000, 50_000_000, 5_000_000);
        let p2 = Process::new_deadline(2, "sooner", 10_000_000, 50_000_000, 5_000_000);

        q.push(p1);
        q.push(p2);

        let picked = edf.pick_next(&mut q).unwrap();
        assert_eq!(picked.pid, 2); // Earliest deadline
    }

    #[test]
    fn edf_admit_accepts_within_capacity() {
        let mut edf = EdfScheduler::new();
        let p = Process::new_deadline(1, "task", 10_000_000, 20_000_000, 5_000_000);
        // Utilization: 5/20 = 0.25 < 1.0
        assert!(edf.admit(&p).is_ok());
        assert_eq!(edf.num_admitted(), 1);

        let total = edf.total_utilization();
        assert!((total - 0.25).abs() < 1e-6);
    }

    #[test]
    fn edf_admit_rejects_overloaded() {
        let mut edf = EdfScheduler::new();

        // Admit tasks that total >1.0 utilization
        let p1 = Process::new_deadline(1, "heavy1", 10_000_000, 20_000_000, 12_000_000);
        assert!(edf.admit(&p1).is_ok()); // 0.6

        let p2 = Process::new_deadline(2, "heavy2", 10_000_000, 20_000_000, 12_000_000);
        // 0.6 + 0.6 = 1.2 >= 1.0 → reject
        let result = edf.admit(&p2);
        assert!(result.is_err());
        match result {
            Err(SmpError::AdmissionRejected { .. }) => {}
            _ => panic!("expected AdmissionRejected"),
        }
    }

    // ── Load Balancer ──

    #[test]
    fn load_balancer_detects_imbalance() {
        let lb = LoadBalancer::new(100);
        let mut q0 = RunQueue::new(CpuId::new(0));
        let q1 = RunQueue::new(CpuId::new(1));

        // CPU 0 has 10 tasks, CPU 1 has 0
        for i in 0..10 {
            q0.push(Process::new(
                i,
                "task",
                SchedPolicy::Normal,
                Priority::new(10),
            ));
        }

        let queues = vec![q0, q1];
        assert!(lb.should_rebalance(&queues));

        let info = lb.compute_imbalance(&queues).unwrap();
        assert_eq!(info.busiest_cpu, 0);
        assert_eq!(info.idlest_cpu, 1);
        assert_eq!(info.tasks_to_move, 5); // Half of difference
    }

    #[test]
    fn load_balancer_no_imbalance_when_balanced() {
        let lb = LoadBalancer::new(100);
        let mut q0 = RunQueue::new(CpuId::new(0));
        let mut q1 = RunQueue::new(CpuId::new(1));

        // Equal load
        for i in 0..5 {
            q0.push(Process::new(i, "a", SchedPolicy::Normal, Priority::new(10)));
            q1.push(Process::new(
                i + 100,
                "b",
                SchedPolicy::Normal,
                Priority::new(10),
            ));
        }

        let queues = vec![q0, q1];
        assert!(!lb.should_rebalance(&queues));
    }

    // ── Work Stealing ──

    #[test]
    fn work_stealing_moves_task() {
        let mut ws = WorkStealing::new();
        let mut from = RunQueue::new(CpuId::new(0));
        let mut to = RunQueue::new(CpuId::new(1));

        from.push(Process::new(
            1,
            "stealable",
            SchedPolicy::Normal,
            Priority::new(10),
        ));
        from.push(Process::new(
            2,
            "also",
            SchedPolicy::Normal,
            Priority::new(20),
        ));

        let stolen = ws.steal(&mut from, &mut to);
        assert!(stolen.is_some());
        assert_eq!(from.len(), 1);
        assert_eq!(to.len(), 1);
        assert_eq!(ws.tasks_stolen(), 1);
    }

    #[test]
    fn work_stealing_skips_pinned_tasks() {
        let mut ws = WorkStealing::new();
        let mut from = RunQueue::new(CpuId::new(0));
        let mut to = RunQueue::new(CpuId::new(1));

        let mut pinned = Process::new(1, "pinned", SchedPolicy::Normal, Priority::new(10));
        pinned.cpu_affinity = Some(CpuId::new(0));
        from.push(pinned);

        let stolen = ws.steal(&mut from, &mut to);
        assert!(stolen.is_none());
        assert_eq!(from.len(), 1); // Still there
    }

    // ── Ticket Lock ──

    #[test]
    fn ticket_lock_fairness() {
        let mut lock = TicketLock::new("test_lock");
        assert!(!lock.is_locked());

        // First acquisition
        let t1 = lock.lock();
        assert_eq!(t1, 0);
        assert_eq!(lock.now_serving(), 0);

        // Release
        lock.unlock();
        assert_eq!(lock.now_serving(), 1);

        // Second acquisition
        let t2 = lock.lock();
        assert_eq!(t2, 1);
        assert_eq!(lock.now_serving(), 1);
    }

    #[test]
    fn ticket_lock_waiters_count() {
        let mut lock = TicketLock::new("waiter_test");
        lock.lock();
        lock.unlock();
        // After lock+unlock, next_ticket=1, now_serving=2
        // So lock+unlock means: next=1, serving=1 after unlock
        // Actually: lock sets next=1, serving=0, then unlock sets serving=1
        assert_eq!(lock.waiters(), 0);
    }

    // ── RCU ──

    #[test]
    fn rcu_generation_tracking() {
        let mut rcu = Rcu::new();
        assert_eq!(rcu.generation(), 0);

        rcu.synchronize();
        assert_eq!(rcu.generation(), 1);

        rcu.synchronize();
        assert_eq!(rcu.generation(), 2);
    }

    #[test]
    fn rcu_read_lock_unlock() {
        let mut rcu = Rcu::new();
        assert_eq!(rcu.active_readers(), 0);

        rcu.read_lock();
        assert_eq!(rcu.active_readers(), 1);

        rcu.read_lock();
        assert_eq!(rcu.active_readers(), 2);

        rcu.read_unlock();
        assert_eq!(rcu.active_readers(), 1);

        rcu.read_unlock();
        assert_eq!(rcu.active_readers(), 0);
    }

    #[test]
    fn rcu_deferred_callbacks() {
        let mut rcu = Rcu::new();

        // Register callbacks in generation 0
        let _cb1 = rcu.call_rcu();
        let _cb2 = rcu.call_rcu();
        assert_eq!(rcu.pending_callbacks(), 2);

        // First synchronize: advances to gen 1, processes gen 0 callbacks
        let executed = rcu.synchronize();
        assert_eq!(executed, 2);
        assert_eq!(rcu.pending_callbacks(), 0);
    }

    #[test]
    fn rcu_callback_not_executed_until_grace_period() {
        let mut rcu = Rcu::new();

        rcu.synchronize(); // gen 0 → 1

        // Register callback in generation 1
        let _cb = rcu.call_rcu();
        assert_eq!(rcu.pending_callbacks(), 1);

        // Synchronize to gen 2 — callback from gen 1 should execute
        let executed = rcu.synchronize();
        assert_eq!(executed, 1);
    }

    // ── SmpScheduler ──

    #[test]
    fn smp_scheduler_multi_cpu_schedule() {
        let mut smp = SmpScheduler::new(4);
        assert_eq!(smp.num_cpus(), 4);

        let p1 = Process::new(1, "task1", SchedPolicy::Normal, Priority::new(10));
        let p2 = Process::new(2, "task2", SchedPolicy::Normal, Priority::new(10));

        smp.enqueue(p1, CpuId::new(0)).unwrap();
        smp.enqueue(p2, CpuId::new(1)).unwrap();

        let next0 = smp.schedule(CpuId::new(0)).unwrap();
        let next1 = smp.schedule(CpuId::new(1)).unwrap();

        assert!(next0.is_some());
        assert!(next1.is_some());
        assert_eq!(next0.unwrap().pid, 1);
        assert_eq!(next1.unwrap().pid, 2);
    }

    #[test]
    fn smp_scheduler_invalid_cpu_error() {
        let mut smp = SmpScheduler::new(2);
        let p = Process::new(1, "test", SchedPolicy::Normal, Priority::new(10));

        let result = smp.enqueue(p, CpuId::new(5));
        assert!(result.is_err());
        match result {
            Err(SmpError::InvalidCpu { cpu: 5, max: 1 }) => {}
            other => panic!("expected InvalidCpu, got: {:?}", other),
        }
    }

    #[test]
    fn smp_scheduler_migration() {
        let mut smp = SmpScheduler::new(2);
        let p = Process::new(42, "migratable", SchedPolicy::Normal, Priority::new(10));

        smp.enqueue(p, CpuId::new(0)).unwrap();
        assert_eq!(smp.queue(CpuId::new(0)).unwrap().len(), 1);
        assert_eq!(smp.queue(CpuId::new(1)).unwrap().len(), 0);

        smp.migrate(42, CpuId::new(0), CpuId::new(1)).unwrap();
        assert_eq!(smp.queue(CpuId::new(0)).unwrap().len(), 0);
        assert_eq!(smp.queue(CpuId::new(1)).unwrap().len(), 1);
        assert_eq!(smp.metrics().migrations, 1);
    }

    #[test]
    fn smp_scheduler_migration_pinned_fails() {
        let mut smp = SmpScheduler::new(2);
        let mut p = Process::new(42, "pinned", SchedPolicy::Normal, Priority::new(10));
        p.cpu_affinity = Some(CpuId::new(0));

        smp.enqueue(p, CpuId::new(0)).unwrap();
        let result = smp.migrate(42, CpuId::new(0), CpuId::new(1));
        assert!(result.is_err());
    }

    #[test]
    fn smp_scheduler_balance() {
        let mut smp = SmpScheduler::new(2);

        // Load all tasks onto CPU 0
        for i in 0..10 {
            let p = Process::new(i, "task", SchedPolicy::Normal, Priority::new(10));
            smp.enqueue(p, CpuId::new(0)).unwrap();
        }

        assert_eq!(smp.queue(CpuId::new(0)).unwrap().len(), 10);
        assert_eq!(smp.queue(CpuId::new(1)).unwrap().len(), 0);

        // Force rebalance
        smp.balance().unwrap();

        // After balance, load should be more even
        let q0 = smp.queue(CpuId::new(0)).unwrap().len();
        let q1 = smp.queue(CpuId::new(1)).unwrap().len();
        assert!(q1 > 0, "idlest CPU should have received tasks");
        assert_eq!(q0 + q1, 10, "total tasks should be preserved");
    }

    #[test]
    fn smp_scheduler_tick_updates_vruntime() {
        let mut smp = SmpScheduler::new(1);
        let p = Process::new(1, "ticked", SchedPolicy::Normal, Priority::new(10));
        smp.enqueue(p, CpuId::new(0)).unwrap();

        // Schedule the process
        let proc = smp.schedule(CpuId::new(0)).unwrap().unwrap();
        assert_eq!(proc.vruntime, 0);

        // Tick
        smp.tick(CpuId::new(0)).unwrap();
        let current = smp.current_on(CpuId::new(0)).unwrap().unwrap();
        assert!(current.vruntime > 0);
    }

    #[test]
    fn smp_scheduler_metrics_tracking() {
        let mut smp = SmpScheduler::new(2);

        let p = Process::new(1, "tracked", SchedPolicy::Normal, Priority::new(10));
        smp.enqueue(p, CpuId::new(0)).unwrap();
        smp.schedule(CpuId::new(0)).unwrap();

        assert_eq!(smp.metrics().context_switches, 1);
    }

    #[test]
    fn smp_scheduler_deadline_has_priority_over_normal() {
        let mut smp = SmpScheduler::new(1);

        let normal = Process::new(1, "normal", SchedPolicy::Normal, Priority::new(0));
        let deadline = Process::new_deadline(2, "deadline", 5_000_000, 20_000_000, 2_000_000);

        smp.enqueue(normal, CpuId::new(0)).unwrap();
        smp.enqueue(deadline, CpuId::new(0)).unwrap();

        let picked = smp.schedule(CpuId::new(0)).unwrap().unwrap();
        assert_eq!(picked.pid, 2); // Deadline task should be picked first
    }

    #[test]
    fn smp_scheduler_send_ipi() {
        let mut smp = SmpScheduler::new(4);
        smp.send_ipi(CpuId::new(2), IpiMessage::TlbFlush { address: 0x1000 })
            .unwrap();
        smp.send_ipi(CpuId::new(3), IpiMessage::Stop).unwrap();

        let ipis = smp.drain_ipis();
        assert_eq!(ipis.len(), 2);
        assert_eq!(ipis[0].0, CpuId::new(2));
        assert_eq!(ipis[0].1, IpiMessage::TlbFlush { address: 0x1000 });
        assert_eq!(smp.metrics().ipi_count, 2);
    }

    #[test]
    fn scheduler_metrics_latency_tracking() {
        let mut m = SchedulerMetrics::new();
        assert_eq!(m.avg_latency_ns(), 0);

        m.record_latency(1000);
        m.record_latency(3000);
        assert_eq!(m.avg_latency_ns(), 2000);
        assert_eq!(m.max_latency_ns, 3000);
    }

    #[test]
    fn process_weight_varies_with_priority() {
        let high = Process::new(1, "high", SchedPolicy::Normal, Priority::new(0));
        let low = Process::new(2, "low", SchedPolicy::Normal, Priority::new(31));

        assert!(high.weight() > low.weight());
        assert_eq!(high.weight(), 1024); // 1024 / (1+0)
        assert_eq!(low.weight(), 32); // 1024 / (1+31)
    }

    #[test]
    fn ipi_message_variants() {
        let msgs = vec![
            IpiMessage::Reschedule,
            IpiMessage::TlbFlush { address: 0xDEAD },
            IpiMessage::FunctionCall {
                function_id: 42,
                argument: 99,
            },
            IpiMessage::Stop,
        ];
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0], IpiMessage::Reschedule);
        assert_ne!(msgs[0], IpiMessage::Stop);
    }

    // ── NUMA Topology (OS1.7) ──

    #[test]
    fn numa_local_distance() {
        let mut topo = NumaTopology::new();
        topo.add_node(NumaNode::with_resources(
            0,
            vec![CpuId::new(0), CpuId::new(1)],
            8192,
        ));
        topo.add_node(NumaNode::with_resources(
            1,
            vec![CpuId::new(2), CpuId::new(3)],
            8192,
        ));

        // Node self-distance must equal NUMA_LOCAL_DISTANCE (10)
        assert_eq!(topo.distance(0, 0).unwrap(), NUMA_LOCAL_DISTANCE);
        assert_eq!(topo.distance(1, 1).unwrap(), NUMA_LOCAL_DISTANCE);
    }

    #[test]
    fn numa_remote_distance() {
        let mut topo = NumaTopology::new();
        topo.add_node(NumaNode::with_resources(0, vec![CpuId::new(0)], 4096));
        topo.add_node(NumaNode::with_resources(1, vec![CpuId::new(1)], 4096));
        topo.set_distance(0, 1, 20);

        let remote = topo.distance(0, 1).unwrap();
        let local = topo.distance(0, 0).unwrap();
        assert!(
            remote > local,
            "remote ({remote}) must exceed local ({local})"
        );
        assert_eq!(remote, 20);
    }

    #[test]
    fn numa_prefer_local_cpu() {
        let mut topo = NumaTopology::new();
        // Node 0 owns CPUs 0 and 1
        topo.add_node(NumaNode::with_resources(
            0,
            vec![CpuId::new(0), CpuId::new(1)],
            8192,
        ));
        // Node 1 owns CPUs 2 and 3
        topo.add_node(NumaNode::with_resources(
            1,
            vec![CpuId::new(2), CpuId::new(3)],
            8192,
        ));

        // prefer_local for CPU 1 should return a CPU on node 0 (CPUs 0 or 1)
        let chosen = topo.prefer_local(CpuId::new(1));
        let node0_cpus = topo.cpus_on_node(0);
        assert!(
            node0_cpus.iter().any(|c| c.index() == chosen.index()),
            "prefer_local should return a CPU on the same node"
        );
    }

    #[test]
    fn numa_node_for_cpu() {
        let mut topo = NumaTopology::new();
        topo.add_node(NumaNode::with_resources(
            0,
            vec![CpuId::new(0), CpuId::new(1)],
            4096,
        ));
        topo.add_node(NumaNode::with_resources(
            1,
            vec![CpuId::new(2), CpuId::new(3)],
            4096,
        ));

        let node = topo.node_for_cpu(CpuId::new(2)).unwrap();
        assert_eq!(node.id, 1);

        let node = topo.node_for_cpu(CpuId::new(0)).unwrap();
        assert_eq!(node.id, 0);

        // CPU not in any node
        assert!(topo.node_for_cpu(CpuId::new(99)).is_none());
    }

    // ── CPU Hotplug (OS1.10) ──

    #[test]
    fn hotplug_add_cpu() {
        let mut smp = SmpScheduler::new(2);
        assert_eq!(smp.num_cpus(), 2);
        assert_eq!(smp.online_cpus().len(), 2);

        let new_cpu = smp.add_cpu();
        assert_eq!(new_cpu.index(), 2);
        assert_eq!(smp.num_cpus(), 3);
        assert_eq!(smp.online_cpus().len(), 3);
        assert!(smp.is_cpu_online(CpuId::new(2)));

        // New CPU's queue should be accessible and empty
        let q = smp.queue(CpuId::new(2)).unwrap();
        assert!(q.is_empty());
    }

    #[test]
    fn hotplug_remove_cpu() {
        let mut smp = SmpScheduler::new(3);

        // Put two tasks on CPU 1
        let p1 = Process::new(10, "orphan1", SchedPolicy::Normal, Priority::new(10));
        let p2 = Process::new(11, "orphan2", SchedPolicy::Normal, Priority::new(10));
        smp.enqueue(p1, CpuId::new(1)).unwrap();
        smp.enqueue(p2, CpuId::new(1)).unwrap();

        // Remove CPU 1 — should return the 2 queued processes
        let orphans = smp.remove_cpu(CpuId::new(1)).unwrap();
        assert_eq!(orphans.len(), 2, "both queued tasks should be orphaned");

        // After removal, num_cpus shrinks and the old CPU is no longer online
        assert_eq!(smp.num_cpus(), 2);
        assert!(!smp.is_cpu_online(CpuId::new(2)), "index 2 no longer valid");

        // Remaining CPUs keep their correct relative indices
        assert!(smp.is_cpu_online(CpuId::new(0)));
        assert!(smp.is_cpu_online(CpuId::new(1)));
    }

    #[test]
    fn hotplug_remove_invalid_cpu() {
        let mut smp = SmpScheduler::new(2);
        let result = smp.remove_cpu(CpuId::new(5));
        assert!(result.is_err());
        match result {
            Err(SmpError::InvalidCpu { cpu: 5, .. }) => {}
            other => panic!("expected InvalidCpu, got: {:?}", other),
        }
    }
}
