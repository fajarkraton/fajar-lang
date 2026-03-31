//! Fault Tolerance V2 — Sprint D7: leader failover, worker failover,
//! network partition handling, graceful shutdown, data replication,
//! circuit breaker v2, backpressure, idempotent ops, split-brain recovery.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D7.1: Leader Failover
// ═══════════════════════════════════════════════════════════════════════

/// Unique identifier for a node in failover scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node({})", self.0)
    }
}

/// State of a leader election.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaderState {
    /// No leader elected yet.
    NoLeader,
    /// This node is the leader.
    Leader(NodeId),
    /// Election is in progress (candidate term).
    Election { term: u64 },
    /// Leader has failed, failover pending.
    FailoverPending { previous_leader: NodeId },
}

impl fmt::Display for LeaderState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LeaderState::NoLeader => write!(f, "NoLeader"),
            LeaderState::Leader(id) => write!(f, "Leader({id})"),
            LeaderState::Election { term } => write!(f, "Election(term={term})"),
            LeaderState::FailoverPending { previous_leader } => {
                write!(f, "FailoverPending(prev={previous_leader})")
            }
        }
    }
}

/// Manages leader election and failover.
#[derive(Debug)]
pub struct LeaderFailover {
    /// This node's ID.
    pub self_id: NodeId,
    /// Current leader state.
    pub state: LeaderState,
    /// Current election term.
    pub term: u64,
    /// Known peers and their last heartbeat (ms since epoch).
    pub peer_heartbeats: HashMap<NodeId, u64>,
    /// Heartbeat timeout in milliseconds.
    pub heartbeat_timeout_ms: u64,
    /// Number of failovers that have occurred.
    pub failover_count: u64,
}

impl LeaderFailover {
    /// Creates a new leader failover manager.
    pub fn new(self_id: NodeId, heartbeat_timeout_ms: u64) -> Self {
        LeaderFailover {
            self_id,
            state: LeaderState::NoLeader,
            term: 0,
            peer_heartbeats: HashMap::new(),
            heartbeat_timeout_ms,
            failover_count: 0,
        }
    }

    /// Registers a peer node.
    pub fn add_peer(&mut self, peer: NodeId, now_ms: u64) {
        self.peer_heartbeats.insert(peer, now_ms);
    }

    /// Records a heartbeat from a peer.
    pub fn receive_heartbeat(&mut self, peer: NodeId, now_ms: u64) {
        self.peer_heartbeats.insert(peer, now_ms);
    }

    /// Checks if the current leader has timed out.
    pub fn check_leader_timeout(&mut self, now_ms: u64) -> bool {
        if let LeaderState::Leader(leader_id) = &self.state {
            if let Some(&last_hb) = self.peer_heartbeats.get(leader_id) {
                if now_ms - last_hb > self.heartbeat_timeout_ms {
                    let prev = *leader_id;
                    self.state = LeaderState::FailoverPending {
                        previous_leader: prev,
                    };
                    return true;
                }
            }
        }
        false
    }

    /// Initiates a new election. If we were in a failover state, mark
    /// the failover as in progress so that the subsequent `elect()` can
    /// record it.
    pub fn start_election(&mut self) -> u64 {
        self.term += 1;
        let was_failover = matches!(self.state, LeaderState::FailoverPending { .. });
        self.state = LeaderState::Election { term: self.term };
        if was_failover {
            self.failover_count += 1;
        }
        self.term
    }

    /// Completes election: the given node becomes leader.
    pub fn elect(&mut self, leader: NodeId, term: u64) {
        if term >= self.term {
            self.term = term;
            self.state = LeaderState::Leader(leader);
        }
    }

    /// Returns true if this node is the current leader.
    pub fn is_leader(&self) -> bool {
        matches!(self.state, LeaderState::Leader(id) if id == self.self_id)
    }

    /// Returns the current leader, if any.
    pub fn current_leader(&self) -> Option<NodeId> {
        if let LeaderState::Leader(id) = &self.state {
            Some(*id)
        } else {
            None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D7.2: Worker Failover & Task Reassignment
// ═══════════════════════════════════════════════════════════════════════

/// Status of a distributed task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    /// Queued, not yet assigned.
    Pending,
    /// Assigned to a worker.
    Running { worker: NodeId },
    /// Completed successfully.
    Completed,
    /// Failed and needs reassignment.
    Failed { reason: String },
    /// Being reassigned to a new worker.
    Reassigning,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "Pending"),
            TaskStatus::Running { worker } => write!(f, "Running({worker})"),
            TaskStatus::Completed => write!(f, "Completed"),
            TaskStatus::Failed { reason } => write!(f, "Failed({reason})"),
            TaskStatus::Reassigning => write!(f, "Reassigning"),
        }
    }
}

/// A task in the distributed system.
#[derive(Debug, Clone)]
pub struct DistTask {
    /// Unique task ID.
    pub id: u64,
    /// Task name/description.
    pub name: String,
    /// Current status.
    pub status: TaskStatus,
    /// Number of times this task has been reassigned.
    pub reassign_count: u32,
    /// Maximum allowed reassignments.
    pub max_reassign: u32,
}

/// Manages worker failover and task reassignment.
#[derive(Debug)]
pub struct WorkerFailover {
    /// All tracked tasks.
    pub tasks: HashMap<u64, DistTask>,
    /// Available workers.
    pub workers: Vec<NodeId>,
    /// Workers known to have failed.
    pub failed_workers: Vec<NodeId>,
    /// Next task ID.
    next_task_id: u64,
}

impl WorkerFailover {
    /// Creates a new worker failover manager.
    pub fn new() -> Self {
        WorkerFailover {
            tasks: HashMap::new(),
            workers: Vec::new(),
            failed_workers: Vec::new(),
            next_task_id: 1,
        }
    }

    /// Adds a worker.
    pub fn add_worker(&mut self, worker: NodeId) {
        if !self.workers.contains(&worker) {
            self.workers.push(worker);
        }
    }

    /// Submits a new task.
    pub fn submit_task(&mut self, name: &str, max_reassign: u32) -> u64 {
        let id = self.next_task_id;
        self.next_task_id += 1;
        self.tasks.insert(
            id,
            DistTask {
                id,
                name: name.to_string(),
                status: TaskStatus::Pending,
                reassign_count: 0,
                max_reassign,
            },
        );
        id
    }

    /// Assigns a pending task to a worker.
    pub fn assign_task(&mut self, task_id: u64, worker: NodeId) -> Result<(), String> {
        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| format!("task {task_id} not found"))?;
        if !self.workers.contains(&worker) || self.failed_workers.contains(&worker) {
            return Err(format!("worker {worker} is not available"));
        }
        task.status = TaskStatus::Running { worker };
        Ok(())
    }

    /// Marks a worker as failed and reassigns its tasks.
    pub fn worker_failed(&mut self, worker: NodeId) -> Vec<u64> {
        self.failed_workers.push(worker);
        self.workers.retain(|w| *w != worker);

        let mut reassigned = Vec::new();
        for task in self.tasks.values_mut() {
            if matches!(task.status, TaskStatus::Running { worker: w } if w == worker) {
                if task.reassign_count < task.max_reassign {
                    task.status = TaskStatus::Reassigning;
                    task.reassign_count += 1;
                    reassigned.push(task.id);
                } else {
                    task.status = TaskStatus::Failed {
                        reason: format!("max reassign limit ({}) exceeded", task.max_reassign),
                    };
                }
            }
        }
        reassigned
    }

    /// Marks a task as completed.
    pub fn complete_task(&mut self, task_id: u64) -> Result<(), String> {
        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| format!("task {task_id} not found"))?;
        task.status = TaskStatus::Completed;
        Ok(())
    }

    /// Returns the count of pending/reassigning tasks.
    pub fn pending_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::Reassigning))
            .count()
    }
}

impl Default for WorkerFailover {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D7.3: Network Partition Handling
// ═══════════════════════════════════════════════════════════════════════

/// Result of a partition detection check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartitionStatus {
    /// Cluster is fully connected.
    Connected,
    /// Partition detected — lists the reachable nodes.
    Partitioned {
        reachable: Vec<NodeId>,
        unreachable: Vec<NodeId>,
    },
}

impl fmt::Display for PartitionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartitionStatus::Connected => write!(f, "Connected"),
            PartitionStatus::Partitioned {
                reachable,
                unreachable,
            } => write!(
                f,
                "Partitioned(reachable={}, unreachable={})",
                reachable.len(),
                unreachable.len()
            ),
        }
    }
}

/// A partition detector that checks node reachability.
#[derive(Debug)]
pub struct PartitionDetector {
    /// All known nodes.
    pub all_nodes: Vec<NodeId>,
    /// Last known reachability timestamp per node.
    pub last_seen: HashMap<NodeId, u64>,
    /// Timeout before a node is considered unreachable.
    pub timeout_ms: u64,
}

impl PartitionDetector {
    /// Creates a new partition detector.
    pub fn new(timeout_ms: u64) -> Self {
        PartitionDetector {
            all_nodes: Vec::new(),
            last_seen: HashMap::new(),
            timeout_ms,
        }
    }

    /// Adds a node to track.
    pub fn add_node(&mut self, node: NodeId, now_ms: u64) {
        if !self.all_nodes.contains(&node) {
            self.all_nodes.push(node);
        }
        self.last_seen.insert(node, now_ms);
    }

    /// Records a node as seen.
    pub fn mark_seen(&mut self, node: NodeId, now_ms: u64) {
        self.last_seen.insert(node, now_ms);
    }

    /// Checks for partitions at the given time.
    pub fn detect(&self, now_ms: u64) -> PartitionStatus {
        let mut reachable = Vec::new();
        let mut unreachable = Vec::new();

        for &node in &self.all_nodes {
            let last = self.last_seen.get(&node).copied().unwrap_or(0);
            if now_ms - last <= self.timeout_ms {
                reachable.push(node);
            } else {
                unreachable.push(node);
            }
        }

        if unreachable.is_empty() {
            PartitionStatus::Connected
        } else {
            PartitionStatus::Partitioned {
                reachable,
                unreachable,
            }
        }
    }

    /// Returns the number of reachable nodes at the given time.
    pub fn reachable_count(&self, now_ms: u64) -> usize {
        self.all_nodes
            .iter()
            .filter(|n| {
                let last = self.last_seen.get(n).copied().unwrap_or(0);
                now_ms - last <= self.timeout_ms
            })
            .count()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D7.4: Graceful Shutdown (SIGTERM Drain)
// ═══════════════════════════════════════════════════════════════════════

/// Phase of a graceful shutdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownPhase {
    /// Normal operation.
    Running,
    /// Draining in-flight requests (no new requests accepted).
    Draining,
    /// All requests drained, cleaning up.
    Cleanup,
    /// Shutdown complete.
    Terminated,
}

impl fmt::Display for ShutdownPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShutdownPhase::Running => write!(f, "Running"),
            ShutdownPhase::Draining => write!(f, "Draining"),
            ShutdownPhase::Cleanup => write!(f, "Cleanup"),
            ShutdownPhase::Terminated => write!(f, "Terminated"),
        }
    }
}

/// Coordinates graceful shutdown with drain timeout.
#[derive(Debug)]
pub struct GracefulShutdown {
    /// Current shutdown phase.
    pub phase: ShutdownPhase,
    /// Maximum time to wait for in-flight requests (ms).
    pub drain_timeout_ms: u64,
    /// Number of in-flight requests.
    pub in_flight: u32,
    /// Timestamp when drain started (ms since epoch).
    pub drain_started_ms: Option<u64>,
}

impl GracefulShutdown {
    /// Creates a new shutdown coordinator.
    pub fn new(drain_timeout_ms: u64) -> Self {
        GracefulShutdown {
            phase: ShutdownPhase::Running,
            drain_timeout_ms,
            in_flight: 0,
            drain_started_ms: None,
        }
    }

    /// Initiates graceful shutdown (SIGTERM received).
    pub fn initiate(&mut self, now_ms: u64) {
        if self.phase == ShutdownPhase::Running {
            self.phase = ShutdownPhase::Draining;
            self.drain_started_ms = Some(now_ms);
        }
    }

    /// Adds an in-flight request. Returns false if draining/terminated.
    pub fn accept_request(&mut self) -> bool {
        if self.phase == ShutdownPhase::Running {
            self.in_flight += 1;
            true
        } else {
            false
        }
    }

    /// Marks a request as completed.
    pub fn complete_request(&mut self) {
        if self.in_flight > 0 {
            self.in_flight -= 1;
        }
    }

    /// Advances the shutdown state machine.
    pub fn tick(&mut self, now_ms: u64) {
        match self.phase {
            ShutdownPhase::Draining => {
                let drain_start = self.drain_started_ms.unwrap_or(now_ms);
                if self.in_flight == 0 || now_ms - drain_start >= self.drain_timeout_ms {
                    self.phase = ShutdownPhase::Cleanup;
                }
            }
            ShutdownPhase::Cleanup => {
                self.phase = ShutdownPhase::Terminated;
            }
            _ => {}
        }
    }

    /// Returns true if shutdown is complete.
    pub fn is_terminated(&self) -> bool {
        self.phase == ShutdownPhase::Terminated
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D7.5: Data Replication
// ═══════════════════════════════════════════════════════════════════════

/// Replication factor and consistency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplicationStrategy {
    /// No replication.
    None,
    /// Synchronous replication to N replicas (wait for all acks).
    Sync { replicas: u32 },
    /// Asynchronous replication (fire-and-forget to replicas).
    Async { replicas: u32 },
    /// Quorum-based (wait for majority).
    Quorum { replicas: u32, quorum: u32 },
}

impl fmt::Display for ReplicationStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplicationStrategy::None => write!(f, "None"),
            ReplicationStrategy::Sync { replicas } => write!(f, "Sync(replicas={replicas})"),
            ReplicationStrategy::Async { replicas } => write!(f, "Async(replicas={replicas})"),
            ReplicationStrategy::Quorum { replicas, quorum } => {
                write!(f, "Quorum(replicas={replicas}, quorum={quorum})")
            }
        }
    }
}

/// A replicated data entry.
#[derive(Debug, Clone)]
pub struct ReplicatedEntry {
    /// Key.
    pub key: String,
    /// Value bytes.
    pub value: Vec<u8>,
    /// Version (logical clock).
    pub version: u64,
    /// Nodes holding this entry.
    pub replicas: Vec<NodeId>,
}

/// Manages data replication across nodes.
#[derive(Debug)]
pub struct ReplicationManager {
    /// Replication strategy.
    pub strategy: ReplicationStrategy,
    /// Replicated entries.
    entries: HashMap<String, ReplicatedEntry>,
    /// Next version number.
    next_version: u64,
}

impl ReplicationManager {
    /// Creates a new replication manager.
    pub fn new(strategy: ReplicationStrategy) -> Self {
        ReplicationManager {
            strategy,
            entries: HashMap::new(),
            next_version: 1,
        }
    }

    /// Writes a key-value pair and replicates to the given nodes.
    pub fn write(&mut self, key: &str, value: Vec<u8>, replicas: Vec<NodeId>) -> u64 {
        let version = self.next_version;
        self.next_version += 1;
        self.entries.insert(
            key.to_string(),
            ReplicatedEntry {
                key: key.to_string(),
                value,
                version,
                replicas,
            },
        );
        version
    }

    /// Reads a key.
    pub fn read(&self, key: &str) -> Option<&ReplicatedEntry> {
        self.entries.get(key)
    }

    /// Returns the number of replicated entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the required quorum size for the current strategy.
    pub fn required_quorum(&self) -> u32 {
        match &self.strategy {
            ReplicationStrategy::Quorum { quorum, .. } => *quorum,
            ReplicationStrategy::Sync { replicas } => *replicas,
            ReplicationStrategy::Async { .. } => 1,
            ReplicationStrategy::None => 0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D7.6: Circuit Breaker V2 (with windowed failure tracking)
// ═══════════════════════════════════════════════════════════════════════

/// Circuit breaker V2 with time-windowed failure tracking.
#[derive(Debug)]
pub struct CircuitBreakerV2 {
    /// Service name.
    pub service: String,
    /// Current state.
    pub state: CircuitBreakerV2State,
    /// Failure timestamps within the window (ms since epoch).
    failures_window: Vec<u64>,
    /// Success timestamps within the window.
    successes_window: Vec<u64>,
    /// Window duration in milliseconds.
    pub window_ms: u64,
    /// Failure rate threshold (0.0 - 1.0) to trip.
    pub failure_rate_threshold: f64,
    /// Minimum calls in window before evaluating.
    pub min_calls: usize,
    /// Probe successes needed to close.
    pub close_after: u32,
    /// Probe successes so far in half-open.
    half_open_successes: u32,
}

/// State of the V2 circuit breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitBreakerV2State {
    /// Closed — requests pass through.
    Closed,
    /// Open — requests rejected.
    Open,
    /// Half-open — probing.
    HalfOpen,
}

impl fmt::Display for CircuitBreakerV2State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitBreakerV2State::Closed => write!(f, "Closed"),
            CircuitBreakerV2State::Open => write!(f, "Open"),
            CircuitBreakerV2State::HalfOpen => write!(f, "HalfOpen"),
        }
    }
}

impl CircuitBreakerV2 {
    /// Creates a new V2 circuit breaker.
    pub fn new(
        service: &str,
        window_ms: u64,
        failure_rate_threshold: f64,
        min_calls: usize,
        close_after: u32,
    ) -> Self {
        CircuitBreakerV2 {
            service: service.to_string(),
            state: CircuitBreakerV2State::Closed,
            failures_window: Vec::new(),
            successes_window: Vec::new(),
            window_ms,
            failure_rate_threshold,
            min_calls,
            close_after,
            half_open_successes: 0,
        }
    }

    /// Prunes events older than the window.
    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.failures_window.retain(|&t| t >= cutoff);
        self.successes_window.retain(|&t| t >= cutoff);
    }

    /// Records a successful call.
    pub fn record_success(&mut self, now_ms: u64) {
        self.prune(now_ms);
        self.successes_window.push(now_ms);

        if self.state == CircuitBreakerV2State::HalfOpen {
            self.half_open_successes += 1;
            if self.half_open_successes >= self.close_after {
                self.state = CircuitBreakerV2State::Closed;
                self.half_open_successes = 0;
            }
        }
    }

    /// Records a failed call.
    pub fn record_failure(&mut self, now_ms: u64) {
        self.prune(now_ms);
        self.failures_window.push(now_ms);

        match self.state {
            CircuitBreakerV2State::Closed => {
                let total = self.failures_window.len() + self.successes_window.len();
                if total >= self.min_calls {
                    let rate = self.failures_window.len() as f64 / total as f64;
                    if rate >= self.failure_rate_threshold {
                        self.state = CircuitBreakerV2State::Open;
                    }
                }
            }
            CircuitBreakerV2State::HalfOpen => {
                self.state = CircuitBreakerV2State::Open;
                self.half_open_successes = 0;
            }
            CircuitBreakerV2State::Open => {}
        }
    }

    /// Transitions to half-open for probing.
    pub fn try_half_open(&mut self) {
        if self.state == CircuitBreakerV2State::Open {
            self.state = CircuitBreakerV2State::HalfOpen;
            self.half_open_successes = 0;
        }
    }

    /// Whether requests are allowed.
    pub fn is_allowed(&self) -> bool {
        !matches!(self.state, CircuitBreakerV2State::Open)
    }

    /// Returns the current failure rate within the window.
    pub fn failure_rate(&self) -> f64 {
        let total = self.failures_window.len() + self.successes_window.len();
        if total == 0 {
            0.0
        } else {
            self.failures_window.len() as f64 / total as f64
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D7.7: Backpressure
// ═══════════════════════════════════════════════════════════════════════

/// Backpressure signal from a downstream service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureSignal {
    /// No backpressure — proceed at full speed.
    None,
    /// Reduce rate by the given percentage (0-100).
    Reduce(u32),
    /// Stop sending entirely.
    Stop,
}

impl fmt::Display for BackpressureSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackpressureSignal::None => write!(f, "None"),
            BackpressureSignal::Reduce(pct) => write!(f, "Reduce({pct}%)"),
            BackpressureSignal::Stop => write!(f, "Stop"),
        }
    }
}

/// A backpressure controller tracking queue depth.
#[derive(Debug)]
pub struct BackpressureController {
    /// Current queue depth.
    pub queue_depth: usize,
    /// Threshold to start reducing.
    pub low_watermark: usize,
    /// Threshold to stop.
    pub high_watermark: usize,
    /// Current signal.
    pub signal: BackpressureSignal,
}

impl BackpressureController {
    /// Creates a new controller with watermarks.
    pub fn new(low_watermark: usize, high_watermark: usize) -> Self {
        BackpressureController {
            queue_depth: 0,
            low_watermark,
            high_watermark,
            signal: BackpressureSignal::None,
        }
    }

    /// Updates the queue depth and recalculates the signal.
    pub fn update(&mut self, queue_depth: usize) {
        self.queue_depth = queue_depth;
        if queue_depth >= self.high_watermark {
            self.signal = BackpressureSignal::Stop;
        } else if queue_depth >= self.low_watermark {
            let range = self.high_watermark - self.low_watermark;
            let over = queue_depth - self.low_watermark;
            let pct = if range > 0 {
                ((over as f64 / range as f64) * 100.0) as u32
            } else {
                100
            };
            self.signal = BackpressureSignal::Reduce(pct.min(100));
        } else {
            self.signal = BackpressureSignal::None;
        }
    }

    /// Returns true if sending is allowed (not stopped).
    pub fn can_send(&self) -> bool {
        !matches!(self.signal, BackpressureSignal::Stop)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D7.8: Idempotent Operations
// ═══════════════════════════════════════════════════════════════════════

/// An idempotency key tracking result of a previous execution.
#[derive(Debug, Clone)]
pub struct IdempotencyRecord {
    /// Idempotency key.
    pub key: String,
    /// Cached result bytes.
    pub result: Vec<u8>,
    /// Timestamp when this was recorded (ms since epoch).
    pub recorded_at_ms: u64,
    /// Time-to-live in milliseconds.
    pub ttl_ms: u64,
}

/// An idempotency store ensuring at-most-once execution.
#[derive(Debug, Default)]
pub struct IdempotencyStore {
    /// Key -> record.
    records: HashMap<String, IdempotencyRecord>,
}

impl IdempotencyStore {
    /// Creates a new idempotency store.
    pub fn new() -> Self {
        IdempotencyStore::default()
    }

    /// Checks if an operation has already been executed. Returns the cached
    /// result if it exists and has not expired, or None if the operation
    /// should be executed.
    pub fn check(&self, key: &str, now_ms: u64) -> Option<&[u8]> {
        if let Some(record) = self.records.get(key) {
            if now_ms - record.recorded_at_ms < record.ttl_ms {
                return Some(&record.result);
            }
        }
        None
    }

    /// Records the result of an operation.
    pub fn record(&mut self, key: &str, result: Vec<u8>, now_ms: u64, ttl_ms: u64) {
        self.records.insert(
            key.to_string(),
            IdempotencyRecord {
                key: key.to_string(),
                result,
                recorded_at_ms: now_ms,
                ttl_ms,
            },
        );
    }

    /// Prunes expired records.
    pub fn prune(&mut self, now_ms: u64) {
        self.records
            .retain(|_, r| now_ms - r.recorded_at_ms < r.ttl_ms);
    }

    /// Returns the number of stored records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D7.9: Split-Brain Recovery
// ═══════════════════════════════════════════════════════════════════════

/// Result of split-brain resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitBrainResolution {
    /// The majority partition continues as the cluster.
    MajorityWins { majority_nodes: Vec<NodeId> },
    /// No quorum achieved — cluster paused.
    NoQuorum,
    /// Conflict resolved by selecting the partition with highest term.
    TermWins { winning_term: u64 },
}

impl fmt::Display for SplitBrainResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SplitBrainResolution::MajorityWins { majority_nodes } => {
                write!(f, "MajorityWins({} nodes)", majority_nodes.len())
            }
            SplitBrainResolution::NoQuorum => write!(f, "NoQuorum"),
            SplitBrainResolution::TermWins { winning_term } => {
                write!(f, "TermWins(term={winning_term})")
            }
        }
    }
}

/// Resolves split-brain scenarios using quorum.
#[derive(Debug)]
pub struct SplitBrainResolver {
    /// Total cluster size.
    pub cluster_size: usize,
}

impl SplitBrainResolver {
    /// Creates a new resolver.
    pub fn new(cluster_size: usize) -> Self {
        SplitBrainResolver { cluster_size }
    }

    /// Resolves a split by checking if a partition has a quorum (majority).
    pub fn resolve(&self, partition_nodes: &[NodeId]) -> SplitBrainResolution {
        let majority = self.cluster_size / 2 + 1;
        if partition_nodes.len() >= majority {
            SplitBrainResolution::MajorityWins {
                majority_nodes: partition_nodes.to_vec(),
            }
        } else {
            SplitBrainResolution::NoQuorum
        }
    }

    /// Resolves a split by comparing election terms.
    pub fn resolve_by_term(
        &self,
        partition_a: (&[NodeId], u64),
        partition_b: (&[NodeId], u64),
    ) -> SplitBrainResolution {
        if partition_a.1 > partition_b.1 {
            SplitBrainResolution::TermWins {
                winning_term: partition_a.1,
            }
        } else if partition_b.1 > partition_a.1 {
            SplitBrainResolution::TermWins {
                winning_term: partition_b.1,
            }
        } else {
            // Same term — fall back to majority
            self.resolve(partition_a.0)
        }
    }

    /// Returns the quorum size.
    pub fn quorum_size(&self) -> usize {
        self.cluster_size / 2 + 1
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D7.10: Integration — Fault Tolerance Coordinator
// ═══════════════════════════════════════════════════════════════════════

/// A coordinator combining all D7 fault tolerance features.
#[derive(Debug)]
pub struct FaultToleranceCoordinator {
    /// Leader failover.
    pub leader: LeaderFailover,
    /// Worker failover.
    pub workers: WorkerFailover,
    /// Partition detector.
    pub partitions: PartitionDetector,
    /// Graceful shutdown.
    pub shutdown: GracefulShutdown,
    /// Data replication.
    pub replication: ReplicationManager,
    /// Backpressure.
    pub backpressure: BackpressureController,
    /// Idempotency store.
    pub idempotency: IdempotencyStore,
    /// Split-brain resolver.
    pub split_brain: SplitBrainResolver,
}

impl FaultToleranceCoordinator {
    /// Creates a new fault tolerance coordinator.
    pub fn new(self_id: NodeId, cluster_size: usize) -> Self {
        FaultToleranceCoordinator {
            leader: LeaderFailover::new(self_id, 5000),
            workers: WorkerFailover::new(),
            partitions: PartitionDetector::new(10000),
            shutdown: GracefulShutdown::new(30000),
            replication: ReplicationManager::new(ReplicationStrategy::Quorum {
                replicas: 3,
                quorum: 2,
            }),
            backpressure: BackpressureController::new(100, 500),
            idempotency: IdempotencyStore::new(),
            split_brain: SplitBrainResolver::new(cluster_size),
        }
    }

    /// Returns a summary status string.
    pub fn status_summary(&self) -> String {
        format!(
            "leader={}, workers={}, partitions={}, shutdown={}, entries={}, bp={}",
            self.leader.state,
            self.workers.workers.len(),
            self.partitions.all_nodes.len(),
            self.shutdown.phase,
            self.replication.entry_count(),
            self.backpressure.signal,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // D7.1 — Leader Failover
    #[test]
    fn d7_1_leader_election() {
        let mut lf = LeaderFailover::new(NodeId(1), 5000);
        lf.add_peer(NodeId(2), 1000);
        lf.add_peer(NodeId(3), 1000);

        let term = lf.start_election();
        assert_eq!(term, 1);
        lf.elect(NodeId(1), 1);
        assert!(lf.is_leader());
        assert_eq!(lf.current_leader(), Some(NodeId(1)));
    }

    #[test]
    fn d7_1_leader_timeout_failover() {
        let mut lf = LeaderFailover::new(NodeId(2), 5000);
        lf.add_peer(NodeId(1), 1000);
        lf.elect(NodeId(1), 1);

        // Leader heartbeat times out
        assert!(lf.check_leader_timeout(7000));
        assert!(matches!(lf.state, LeaderState::FailoverPending { .. }));

        // New election and failover
        lf.start_election();
        lf.elect(NodeId(2), 2);
        assert_eq!(lf.failover_count, 1);
    }

    // D7.2 — Worker Failover
    #[test]
    fn d7_2_task_reassignment() {
        let mut wf = WorkerFailover::new();
        wf.add_worker(NodeId(10));
        wf.add_worker(NodeId(20));

        let t1 = wf.submit_task("train_batch_1", 3);
        let t2 = wf.submit_task("train_batch_2", 3);
        wf.assign_task(t1, NodeId(10)).unwrap();
        wf.assign_task(t2, NodeId(10)).unwrap();

        let reassigned = wf.worker_failed(NodeId(10));
        assert_eq!(reassigned.len(), 2);
        assert_eq!(wf.pending_count(), 2);
    }

    #[test]
    fn d7_2_max_reassign_exceeded() {
        let mut wf = WorkerFailover::new();
        wf.add_worker(NodeId(10));
        wf.add_worker(NodeId(20));

        let t1 = wf.submit_task("job", 0); // max_reassign = 0
        wf.assign_task(t1, NodeId(10)).unwrap();
        let reassigned = wf.worker_failed(NodeId(10));
        assert!(reassigned.is_empty()); // Cannot reassign
        assert!(matches!(
            wf.tasks.get(&t1).unwrap().status,
            TaskStatus::Failed { .. }
        ));
    }

    // D7.3 — Network Partition
    #[test]
    fn d7_3_partition_detection() {
        let mut pd = PartitionDetector::new(5000);
        pd.add_node(NodeId(1), 1000);
        pd.add_node(NodeId(2), 1000);
        pd.add_node(NodeId(3), 1000);

        // All nodes recently seen
        assert_eq!(pd.detect(3000), PartitionStatus::Connected);

        // Node 3 goes silent
        pd.mark_seen(NodeId(1), 10000);
        pd.mark_seen(NodeId(2), 10000);
        // Node 3 last seen at 1000, now is 10000 (9000ms > 5000ms timeout)

        let status = pd.detect(10000);
        assert!(matches!(status, PartitionStatus::Partitioned { .. }));
        assert_eq!(pd.reachable_count(10000), 2);
    }

    // D7.4 — Graceful Shutdown
    #[test]
    fn d7_4_graceful_shutdown_drain() {
        let mut gs = GracefulShutdown::new(30000);
        assert!(gs.accept_request());
        assert!(gs.accept_request());
        assert_eq!(gs.in_flight, 2);

        gs.initiate(1000);
        assert!(!gs.accept_request()); // Draining, no new requests

        gs.complete_request();
        gs.complete_request();
        gs.tick(2000); // in_flight == 0
        assert_eq!(gs.phase, ShutdownPhase::Cleanup);
        gs.tick(3000);
        assert!(gs.is_terminated());
    }

    #[test]
    fn d7_4_drain_timeout() {
        let mut gs = GracefulShutdown::new(5000);
        gs.accept_request();
        gs.initiate(1000);

        // Even with in-flight, timeout forces cleanup
        gs.tick(7000); // 7000 - 1000 = 6000 > 5000
        assert_eq!(gs.phase, ShutdownPhase::Cleanup);
    }

    // D7.5 — Data Replication
    #[test]
    fn d7_5_replication() {
        let mut rm = ReplicationManager::new(ReplicationStrategy::Sync { replicas: 3 });
        let v1 = rm.write(
            "model_weights",
            vec![1, 2, 3],
            vec![NodeId(1), NodeId(2), NodeId(3)],
        );
        assert_eq!(v1, 1);

        let entry = rm.read("model_weights").unwrap();
        assert_eq!(entry.replicas.len(), 3);
        assert_eq!(entry.value, vec![1, 2, 3]);
    }

    #[test]
    fn d7_5_quorum_requirement() {
        let rm = ReplicationManager::new(ReplicationStrategy::Quorum {
            replicas: 5,
            quorum: 3,
        });
        assert_eq!(rm.required_quorum(), 3);
    }

    // D7.6 — Circuit Breaker V2
    #[test]
    fn d7_6_circuit_breaker_v2_windowed() {
        let mut cb = CircuitBreakerV2::new("svc", 10000, 0.5, 4, 2);

        // 2 successes, 2 failures = 50% failure rate (>= 0.5 threshold)
        cb.record_success(1000);
        cb.record_success(2000);
        cb.record_failure(3000);
        cb.record_failure(4000);

        assert_eq!(cb.state, CircuitBreakerV2State::Open);
        assert!(!cb.is_allowed());

        cb.try_half_open();
        cb.record_success(5000);
        cb.record_success(6000);
        assert_eq!(cb.state, CircuitBreakerV2State::Closed);
    }

    #[test]
    fn d7_6_window_pruning() {
        let mut cb = CircuitBreakerV2::new("svc", 5000, 0.5, 2, 1);
        cb.record_failure(1000);
        cb.record_failure(2000);
        // Should be open now (2 failures, 0 successes, 100% rate >= 50%)
        assert_eq!(cb.state, CircuitBreakerV2State::Open);

        cb.try_half_open();
        // Old failures should prune when window advances
        cb.record_success(20000); // 20000 - 5000 = 15000 cutoff, old events pruned
        assert_eq!(cb.state, CircuitBreakerV2State::Closed);
    }

    // D7.7 — Backpressure
    #[test]
    fn d7_7_backpressure_levels() {
        let mut bp = BackpressureController::new(100, 500);

        bp.update(50);
        assert_eq!(bp.signal, BackpressureSignal::None);
        assert!(bp.can_send());

        bp.update(300);
        assert!(matches!(bp.signal, BackpressureSignal::Reduce(_)));
        assert!(bp.can_send());

        bp.update(500);
        assert_eq!(bp.signal, BackpressureSignal::Stop);
        assert!(!bp.can_send());
    }

    // D7.8 — Idempotent Operations
    #[test]
    fn d7_8_idempotency() {
        let mut store = IdempotencyStore::new();
        assert!(store.check("op-001", 1000).is_none());

        store.record("op-001", vec![42], 1000, 5000);
        assert_eq!(store.check("op-001", 2000), Some(vec![42].as_slice()));

        // Expired
        assert!(store.check("op-001", 7000).is_none());
    }

    #[test]
    fn d7_8_idempotency_prune() {
        let mut store = IdempotencyStore::new();
        store.record("a", vec![1], 1000, 5000);
        store.record("b", vec![2], 5000, 5000);

        store.prune(8000); // "a" expired (8000-1000=7000 >= 5000), "b" valid (8000-5000=3000 < 5000)
        assert_eq!(store.len(), 1);
    }

    // D7.9 — Split-Brain Recovery
    #[test]
    fn d7_9_majority_wins() {
        let resolver = SplitBrainResolver::new(5);
        let result = resolver.resolve(&[NodeId(1), NodeId(2), NodeId(3)]);
        assert!(matches!(result, SplitBrainResolution::MajorityWins { .. }));
    }

    #[test]
    fn d7_9_no_quorum() {
        let resolver = SplitBrainResolver::new(5);
        let result = resolver.resolve(&[NodeId(1), NodeId(2)]); // 2 < 3 (majority)
        assert_eq!(result, SplitBrainResolution::NoQuorum);
    }

    #[test]
    fn d7_9_term_resolution() {
        let resolver = SplitBrainResolver::new(4);
        let result =
            resolver.resolve_by_term((&[NodeId(1), NodeId(2)], 5), (&[NodeId(3), NodeId(4)], 3));
        assert_eq!(result, SplitBrainResolution::TermWins { winning_term: 5 });
    }

    // D7.10 — Integration
    #[test]
    fn d7_10_coordinator_creation() {
        let coord = FaultToleranceCoordinator::new(NodeId(1), 5);
        let summary = coord.status_summary();
        assert!(summary.contains("leader=NoLeader"));
        assert!(summary.contains("shutdown=Running"));
    }

    #[test]
    fn d7_10_coordinator_full_lifecycle() {
        let mut coord = FaultToleranceCoordinator::new(NodeId(1), 3);

        // Add workers
        coord.workers.add_worker(NodeId(10));
        coord.workers.add_worker(NodeId(20));

        // Elect leader
        coord.leader.start_election();
        coord.leader.elect(NodeId(1), 1);
        assert!(coord.leader.is_leader());

        // Submit and assign task
        let task = coord.workers.submit_task("training", 2);
        coord.workers.assign_task(task, NodeId(10)).unwrap();

        // Replication
        coord
            .replication
            .write("weights", vec![1, 2, 3], vec![NodeId(1), NodeId(10)]);
        assert_eq!(coord.replication.entry_count(), 1);
    }
}
