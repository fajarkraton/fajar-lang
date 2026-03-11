//! Fault Tolerance — checkpointing, recovery, exactly-once semantics,
//! saga pattern, dead letter queue, circuit breaker, bulkhead, chaos.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S12.1-S12.2: Checkpointing & Storage
// ═══════════════════════════════════════════════════════════════════════

/// Checkpoint storage backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointBackend {
    /// Local disk storage.
    LocalDisk(String),
    /// Network file system.
    Nfs(String),
    /// Object storage (e.g., S3).
    ObjectStorage(String),
}

impl fmt::Display for CheckpointBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckpointBackend::LocalDisk(path) => write!(f, "LocalDisk({path})"),
            CheckpointBackend::Nfs(path) => write!(f, "NFS({path})"),
            CheckpointBackend::ObjectStorage(bucket) => write!(f, "ObjectStorage({bucket})"),
        }
    }
}

/// A checkpoint of computation state.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    /// Unique checkpoint ID.
    pub id: u64,
    /// Computation/job ID.
    pub job_id: u64,
    /// Step number at checkpoint.
    pub step: u64,
    /// Serialized state.
    pub state: Vec<u8>,
    /// Timestamp (milliseconds since epoch).
    pub timestamp_ms: u64,
}

/// A checkpoint manager that stores periodic snapshots.
#[derive(Debug)]
pub struct CheckpointManager {
    /// Storage backend.
    pub backend: CheckpointBackend,
    /// All checkpoints, keyed by job ID.
    checkpoints: HashMap<u64, Vec<Checkpoint>>,
    /// Maximum checkpoints to retain per job.
    pub max_retained: usize,
    /// Next checkpoint ID.
    next_id: u64,
}

impl CheckpointManager {
    /// Creates a new checkpoint manager.
    pub fn new(backend: CheckpointBackend, max_retained: usize) -> Self {
        CheckpointManager {
            backend,
            checkpoints: HashMap::new(),
            max_retained,
            next_id: 1,
        }
    }

    /// Saves a checkpoint.
    pub fn save(&mut self, job_id: u64, step: u64, state: Vec<u8>, timestamp_ms: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let cp = Checkpoint {
            id,
            job_id,
            step,
            state,
            timestamp_ms,
        };

        let list = self.checkpoints.entry(job_id).or_default();
        list.push(cp);

        // Trim old checkpoints
        while list.len() > self.max_retained {
            list.remove(0);
        }

        id
    }

    /// Loads the latest checkpoint for a job.
    pub fn latest(&self, job_id: u64) -> Option<&Checkpoint> {
        self.checkpoints.get(&job_id).and_then(|list| list.last())
    }

    /// Returns the number of checkpoints for a job.
    pub fn count(&self, job_id: u64) -> usize {
        self.checkpoints.get(&job_id).map_or(0, |l| l.len())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S12.3: Recovery
// ═══════════════════════════════════════════════════════════════════════

/// Recovery status after restarting from a checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryStatus {
    /// Recovery successful — resumed from step N.
    Resumed { from_step: u64 },
    /// No checkpoint available — starting fresh.
    Fresh,
    /// Recovery failed.
    Failed(String),
}

impl fmt::Display for RecoveryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecoveryStatus::Resumed { from_step } => {
                write!(f, "Resumed from step {from_step}")
            }
            RecoveryStatus::Fresh => write!(f, "Fresh start"),
            RecoveryStatus::Failed(e) => write!(f, "Recovery failed: {e}"),
        }
    }
}

/// Attempts recovery from the latest checkpoint.
pub fn recover(manager: &CheckpointManager, job_id: u64) -> RecoveryStatus {
    match manager.latest(job_id) {
        Some(cp) => RecoveryStatus::Resumed { from_step: cp.step },
        None => RecoveryStatus::Fresh,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S12.4: Exactly-Once Semantics
// ═══════════════════════════════════════════════════════════════════════

/// Deduplication table for exactly-once message delivery.
#[derive(Debug, Default)]
pub struct DeduplicationTable {
    /// Processed message IDs.
    seen: HashMap<String, bool>,
}

impl DeduplicationTable {
    /// Creates a new deduplication table.
    pub fn new() -> Self {
        DeduplicationTable::default()
    }

    /// Checks if a message has been processed. Returns true if it's new.
    pub fn try_process(&mut self, message_id: &str) -> bool {
        if self.seen.contains_key(message_id) {
            false // Duplicate
        } else {
            self.seen.insert(message_id.to_string(), true);
            true // New message
        }
    }

    /// Returns the number of processed messages.
    pub fn processed_count(&self) -> usize {
        self.seen.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S12.5: Saga Pattern
// ═══════════════════════════════════════════════════════════════════════

/// A step in a saga with a forward action and compensating action.
#[derive(Debug, Clone)]
pub struct SagaStep {
    /// Step name.
    pub name: String,
    /// Forward action description.
    pub action: String,
    /// Compensating action description.
    pub compensation: String,
    /// Whether this step has been executed.
    pub executed: bool,
    /// Whether this step succeeded.
    pub succeeded: bool,
}

/// A saga (distributed transaction with compensations).
#[derive(Debug)]
pub struct Saga {
    /// Saga name.
    pub name: String,
    /// Steps in execution order.
    pub steps: Vec<SagaStep>,
}

impl Saga {
    /// Creates a new saga.
    pub fn new(name: &str) -> Self {
        Saga {
            name: name.to_string(),
            steps: Vec::new(),
        }
    }

    /// Adds a step with forward and compensating actions.
    pub fn add_step(&mut self, name: &str, action: &str, compensation: &str) {
        self.steps.push(SagaStep {
            name: name.to_string(),
            action: action.to_string(),
            compensation: compensation.to_string(),
            executed: false,
            succeeded: false,
        });
    }

    /// Simulates executing the saga. Returns the names of compensating actions if a step fails.
    pub fn execute(&mut self, fail_at: Option<usize>) -> Result<(), Vec<String>> {
        for (i, step) in self.steps.iter_mut().enumerate() {
            step.executed = true;
            if Some(i) == fail_at {
                step.succeeded = false;
                // Compensate all previously succeeded steps in reverse
                let compensations: Vec<String> = self.steps[..i]
                    .iter()
                    .rev()
                    .filter(|s| s.succeeded)
                    .map(|s| s.compensation.clone())
                    .collect();
                return Err(compensations);
            }
            step.succeeded = true;
        }
        Ok(())
    }

    /// Returns how many steps were executed.
    pub fn executed_count(&self) -> usize {
        self.steps.iter().filter(|s| s.executed).count()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S12.6: Dead Letter Queue
// ═══════════════════════════════════════════════════════════════════════

/// A message that failed processing.
#[derive(Debug, Clone)]
pub struct DeadLetter {
    /// Original message ID.
    pub message_id: String,
    /// Message content.
    pub content: String,
    /// Error that caused the failure.
    pub error: String,
    /// Number of retry attempts.
    pub retry_count: u32,
}

/// A dead letter queue for failed messages.
#[derive(Debug, Default)]
pub struct DeadLetterQueue {
    /// Failed messages.
    letters: Vec<DeadLetter>,
}

impl DeadLetterQueue {
    /// Creates a new empty DLQ.
    pub fn new() -> Self {
        DeadLetterQueue::default()
    }

    /// Adds a dead letter.
    pub fn push(&mut self, letter: DeadLetter) {
        self.letters.push(letter);
    }

    /// Retrieves and removes the oldest dead letter for retry.
    pub fn pop(&mut self) -> Option<DeadLetter> {
        if self.letters.is_empty() {
            None
        } else {
            Some(self.letters.remove(0))
        }
    }

    /// Returns the number of dead letters.
    pub fn len(&self) -> usize {
        self.letters.len()
    }

    /// Returns true if the DLQ is empty.
    pub fn is_empty(&self) -> bool {
        self.letters.is_empty()
    }

    /// Peeks at the oldest dead letter.
    pub fn peek(&self) -> Option<&DeadLetter> {
        self.letters.first()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S12.7: Circuit Breaker (fault-tolerance variant)
// ═══════════════════════════════════════════════════════════════════════

/// Circuit breaker state for fault tolerance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FtCircuitState {
    /// Normal — requests pass through.
    Closed,
    /// Tripped — requests are rejected.
    Open,
    /// Testing — allowing probe requests.
    HalfOpen,
}

impl fmt::Display for FtCircuitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FtCircuitState::Closed => write!(f, "Closed"),
            FtCircuitState::Open => write!(f, "Open"),
            FtCircuitState::HalfOpen => write!(f, "HalfOpen"),
        }
    }
}

/// Per-service circuit breaker with configurable thresholds.
#[derive(Debug)]
pub struct ServiceCircuitBreaker {
    /// Service name.
    pub service: String,
    /// Current state.
    pub state: FtCircuitState,
    /// Consecutive failure count.
    pub failures: u32,
    /// Failure threshold to open.
    pub threshold: u32,
    /// Successes needed to close from half-open.
    pub close_after: u32,
    /// Half-open successes so far.
    pub half_open_successes: u32,
}

impl ServiceCircuitBreaker {
    /// Creates a new circuit breaker.
    pub fn new(service: &str, threshold: u32, close_after: u32) -> Self {
        ServiceCircuitBreaker {
            service: service.to_string(),
            state: FtCircuitState::Closed,
            failures: 0,
            threshold,
            close_after,
            half_open_successes: 0,
        }
    }

    /// Records a success.
    pub fn success(&mut self) {
        match self.state {
            FtCircuitState::Closed => {
                self.failures = 0;
            }
            FtCircuitState::HalfOpen => {
                self.half_open_successes += 1;
                if self.half_open_successes >= self.close_after {
                    self.state = FtCircuitState::Closed;
                    self.failures = 0;
                }
            }
            FtCircuitState::Open => {}
        }
    }

    /// Records a failure.
    pub fn failure(&mut self) {
        match self.state {
            FtCircuitState::Closed => {
                self.failures += 1;
                if self.failures >= self.threshold {
                    self.state = FtCircuitState::Open;
                }
            }
            FtCircuitState::HalfOpen => {
                self.state = FtCircuitState::Open;
                self.half_open_successes = 0;
            }
            FtCircuitState::Open => {}
        }
    }

    /// Tries to transition to half-open.
    pub fn try_half_open(&mut self) {
        if self.state == FtCircuitState::Open {
            self.state = FtCircuitState::HalfOpen;
            self.half_open_successes = 0;
        }
    }

    /// Whether requests should be allowed.
    pub fn is_allowed(&self) -> bool {
        !matches!(self.state, FtCircuitState::Open)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S12.8: Bulkhead Isolation
// ═══════════════════════════════════════════════════════════════════════

/// A bulkhead isolating a failure domain with a concurrency limit.
#[derive(Debug)]
pub struct Bulkhead {
    /// Bulkhead name.
    pub name: String,
    /// Maximum concurrent requests.
    pub max_concurrent: usize,
    /// Currently active requests.
    pub active: usize,
}

impl Bulkhead {
    /// Creates a new bulkhead.
    pub fn new(name: &str, max_concurrent: usize) -> Self {
        Bulkhead {
            name: name.to_string(),
            max_concurrent,
            active: 0,
        }
    }

    /// Tries to acquire a slot. Returns false if bulkhead is full.
    pub fn try_acquire(&mut self) -> bool {
        if self.active >= self.max_concurrent {
            false
        } else {
            self.active += 1;
            true
        }
    }

    /// Releases a slot.
    pub fn release(&mut self) {
        if self.active > 0 {
            self.active -= 1;
        }
    }

    /// Returns available slots.
    pub fn available(&self) -> usize {
        self.max_concurrent - self.active
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S12.9: Chaos Engineering
// ═══════════════════════════════════════════════════════════════════════

/// A chaos injection rule.
#[derive(Debug, Clone)]
pub struct ChaosRule {
    /// Target service or component.
    pub target: String,
    /// Type of fault to inject.
    pub fault_type: ChaosType,
    /// Probability of injection (0.0 - 1.0).
    pub probability: f64,
    /// Whether this rule is enabled.
    pub enabled: bool,
}

/// Type of chaos fault to inject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChaosType {
    /// Inject a random delay.
    Latency,
    /// Inject a random error.
    Error,
    /// Drop the request entirely.
    Drop,
    /// Return garbage data.
    Corruption,
}

impl fmt::Display for ChaosType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChaosType::Latency => write!(f, "Latency"),
            ChaosType::Error => write!(f, "Error"),
            ChaosType::Drop => write!(f, "Drop"),
            ChaosType::Corruption => write!(f, "Corruption"),
        }
    }
}

/// A chaos engine that manages injection rules.
#[derive(Debug, Default)]
pub struct ChaosEngine {
    /// Injection rules.
    pub rules: Vec<ChaosRule>,
    /// Whether the engine is globally enabled.
    pub enabled: bool,
}

impl ChaosEngine {
    /// Creates a new disabled engine.
    pub fn new() -> Self {
        ChaosEngine {
            rules: Vec::new(),
            enabled: false,
        }
    }

    /// Adds a chaos rule.
    pub fn add_rule(&mut self, target: &str, fault_type: ChaosType, probability: f64) {
        self.rules.push(ChaosRule {
            target: target.to_string(),
            fault_type,
            probability,
            enabled: true,
        });
    }

    /// Returns active rules for a target.
    pub fn rules_for(&self, target: &str) -> Vec<&ChaosRule> {
        if !self.enabled {
            return Vec::new();
        }
        self.rules
            .iter()
            .filter(|r| r.enabled && r.target == target)
            .collect()
    }

    /// Enables the engine.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disables the engine.
    pub fn disable(&mut self) {
        self.enabled = false;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S12.1-S12.2 — Checkpointing
    #[test]
    fn s12_1_checkpoint_save_load() {
        let mut mgr = CheckpointManager::new(CheckpointBackend::LocalDisk("/tmp/ckpt".into()), 3);
        mgr.save(1, 100, vec![1, 2, 3], 1000);
        mgr.save(1, 200, vec![4, 5, 6], 2000);

        let latest = mgr.latest(1).unwrap();
        assert_eq!(latest.step, 200);
        assert_eq!(mgr.count(1), 2);
    }

    #[test]
    fn s12_2_checkpoint_retention() {
        let mut mgr = CheckpointManager::new(CheckpointBackend::Nfs("/mnt/nfs".into()), 2);
        mgr.save(1, 100, vec![], 1000);
        mgr.save(1, 200, vec![], 2000);
        mgr.save(1, 300, vec![], 3000);
        assert_eq!(mgr.count(1), 2); // Oldest trimmed
        assert_eq!(mgr.latest(1).unwrap().step, 300);
    }

    #[test]
    fn s12_2_backend_display() {
        assert!(CheckpointBackend::ObjectStorage("s3://bucket".into())
            .to_string()
            .contains("ObjectStorage"));
    }

    // S12.3 — Recovery
    #[test]
    fn s12_3_recovery_from_checkpoint() {
        let mut mgr = CheckpointManager::new(CheckpointBackend::LocalDisk("/tmp".into()), 5);
        mgr.save(1, 50, vec![], 1000);

        let status = recover(&mgr, 1);
        assert_eq!(status, RecoveryStatus::Resumed { from_step: 50 });
    }

    #[test]
    fn s12_3_fresh_start() {
        let mgr = CheckpointManager::new(CheckpointBackend::LocalDisk("/tmp".into()), 5);
        assert_eq!(recover(&mgr, 999), RecoveryStatus::Fresh);
    }

    // S12.4 — Exactly-Once
    #[test]
    fn s12_4_deduplication() {
        let mut dedup = DeduplicationTable::new();
        assert!(dedup.try_process("msg-001"));
        assert!(!dedup.try_process("msg-001")); // Duplicate
        assert!(dedup.try_process("msg-002"));
        assert_eq!(dedup.processed_count(), 2);
    }

    // S12.5 — Saga Pattern
    #[test]
    fn s12_5_saga_success() {
        let mut saga = Saga::new("create_order");
        saga.add_step("reserve_stock", "reserve()", "release()");
        saga.add_step("charge_payment", "charge()", "refund()");
        saga.add_step("send_email", "send()", "cancel_email()");

        assert!(saga.execute(None).is_ok());
        assert_eq!(saga.executed_count(), 3);
    }

    #[test]
    fn s12_5_saga_compensate() {
        let mut saga = Saga::new("create_order");
        saga.add_step("reserve_stock", "reserve()", "release()");
        saga.add_step("charge_payment", "charge()", "refund()");
        saga.add_step("send_email", "send()", "cancel_email()");

        let err = saga.execute(Some(2)).unwrap_err();
        // Should compensate charge and reserve in reverse
        assert_eq!(err, vec!["refund()", "release()"]);
    }

    // S12.6 — Dead Letter Queue
    #[test]
    fn s12_6_dlq() {
        let mut dlq = DeadLetterQueue::new();
        dlq.push(DeadLetter {
            message_id: "msg-001".into(),
            content: "data".into(),
            error: "timeout".into(),
            retry_count: 3,
        });
        assert_eq!(dlq.len(), 1);

        let letter = dlq.pop().unwrap();
        assert_eq!(letter.message_id, "msg-001");
        assert!(dlq.is_empty());
    }

    // S12.7 — Circuit Breaker
    #[test]
    fn s12_7_service_circuit_breaker() {
        let mut cb = ServiceCircuitBreaker::new("user-svc", 3, 2);
        assert!(cb.is_allowed());

        cb.failure();
        cb.failure();
        assert!(cb.is_allowed()); // Still closed
        cb.failure();
        assert!(!cb.is_allowed()); // Open

        cb.try_half_open();
        assert!(cb.is_allowed());
        cb.success();
        cb.success();
        assert_eq!(cb.state, FtCircuitState::Closed);
    }

    // S12.8 — Bulkhead Isolation
    #[test]
    fn s12_8_bulkhead() {
        let mut bh = Bulkhead::new("db-pool", 3);
        assert!(bh.try_acquire());
        assert!(bh.try_acquire());
        assert!(bh.try_acquire());
        assert!(!bh.try_acquire()); // Full
        assert_eq!(bh.available(), 0);

        bh.release();
        assert_eq!(bh.available(), 1);
        assert!(bh.try_acquire());
    }

    // S12.9 — Chaos Engineering
    #[test]
    fn s12_9_chaos_engine() {
        let mut engine = ChaosEngine::new();
        engine.add_rule("user-svc", ChaosType::Latency, 0.1);
        engine.add_rule("user-svc", ChaosType::Error, 0.05);

        // Disabled by default
        assert!(engine.rules_for("user-svc").is_empty());

        engine.enable();
        assert_eq!(engine.rules_for("user-svc").len(), 2);
        assert!(engine.rules_for("other-svc").is_empty());
    }

    #[test]
    fn s12_9_chaos_type_display() {
        assert_eq!(ChaosType::Latency.to_string(), "Latency");
        assert_eq!(ChaosType::Drop.to_string(), "Drop");
    }

    // S12.10 — Integration
    #[test]
    fn s12_10_recovery_display() {
        assert_eq!(
            RecoveryStatus::Resumed { from_step: 42 }.to_string(),
            "Resumed from step 42"
        );
        assert_eq!(RecoveryStatus::Fresh.to_string(), "Fresh start");
    }

    #[test]
    fn s12_10_circuit_state_display() {
        assert_eq!(FtCircuitState::Closed.to_string(), "Closed");
        assert_eq!(FtCircuitState::Open.to_string(), "Open");
        assert_eq!(FtCircuitState::HalfOpen.to_string(), "HalfOpen");
    }
}
