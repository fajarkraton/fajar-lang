//! Software Transactional Memory — TVar, atomic transactions, retry,
//! orElse, conflict detection, nested transactions, STM collections.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S8.1: TVar Primitive
// ═══════════════════════════════════════════════════════════════════════

/// Unique identifier for a transactional variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TVarId(pub u64);

impl fmt::Display for TVarId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TVar({})", self.0)
    }
}

/// A transactional variable that can only be read/written within `atomically`.
#[derive(Debug, Clone)]
pub struct TVar {
    /// Variable identifier.
    pub id: TVarId,
    /// Current committed value.
    pub value: i64,
    /// Version number (incremented on each commit).
    pub version: u64,
}

impl TVar {
    /// Creates a new TVar with an initial value.
    pub fn new(id: TVarId, initial: i64) -> Self {
        TVar {
            id,
            value: initial,
            version: 0,
        }
    }
}

impl fmt::Display for TVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TVar({}, value={}, v={})",
            self.id.0, self.value, self.version
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.2: STM Transaction
// ═══════════════════════════════════════════════════════════════════════

/// An operation within an STM transaction.
#[derive(Debug, Clone)]
pub enum TxOp {
    /// Read a TVar.
    Read(TVarId),
    /// Write a value to a TVar.
    Write(TVarId, i64),
    /// Retry the transaction (block until a read TVar changes).
    Retry,
}

/// Result of a transaction attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxResult {
    /// Transaction committed successfully.
    Committed,
    /// Transaction needs to retry (blocking until a read variable changes).
    Retry,
    /// Transaction aborted due to conflict.
    Conflict(TVarId),
    /// Transaction failed with an error.
    Error(String),
}

impl fmt::Display for TxResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TxResult::Committed => write!(f, "Committed"),
            TxResult::Retry => write!(f, "Retry"),
            TxResult::Conflict(id) => write!(f, "Conflict({})", id),
            TxResult::Error(e) => write!(f, "Error({e})"),
        }
    }
}

/// A transaction log recording reads and writes.
#[derive(Debug, Clone, Default)]
pub struct TxLog {
    /// Reads: TVar ID -> version read at.
    pub reads: HashMap<TVarId, u64>,
    /// Writes: TVar ID -> new value.
    pub writes: HashMap<TVarId, i64>,
    /// Operations in order.
    pub ops: Vec<TxOp>,
    /// Whether retry was requested.
    pub retry_requested: bool,
}

impl TxLog {
    /// Creates a new empty transaction log.
    pub fn new() -> Self {
        TxLog::default()
    }

    /// Records a read of a TVar.
    pub fn record_read(&mut self, id: TVarId, version: u64) {
        self.reads.insert(id, version);
        self.ops.push(TxOp::Read(id));
    }

    /// Records a write to a TVar.
    pub fn record_write(&mut self, id: TVarId, value: i64) {
        self.writes.insert(id, value);
        self.ops.push(TxOp::Write(id, value));
    }

    /// Marks this transaction as needing retry.
    pub fn request_retry(&mut self) {
        self.retry_requested = true;
        self.ops.push(TxOp::Retry);
    }

    /// Returns the read set (TVar IDs that were read).
    pub fn read_set(&self) -> Vec<TVarId> {
        self.reads.keys().copied().collect()
    }

    /// Returns the write set (TVar IDs that were written).
    pub fn write_set(&self) -> Vec<TVarId> {
        self.writes.keys().copied().collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.3-S8.4: Retry & OrElse
// ═══════════════════════════════════════════════════════════════════════

/// Combines two transaction alternatives with orElse semantics.
pub fn or_else(tx1_result: &TxResult, tx2_result: &TxResult) -> TxResult {
    match tx1_result {
        TxResult::Retry => tx2_result.clone(),
        other => other.clone(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.5: Conflict Detection
// ═══════════════════════════════════════════════════════════════════════

/// An STM store that manages TVars and validates transactions.
#[derive(Debug, Default)]
pub struct StmStore {
    /// All transactional variables.
    vars: HashMap<TVarId, TVar>,
    /// Next TVar ID.
    next_id: u64,
    /// Transaction metrics.
    pub metrics: StmMetrics,
}

impl StmStore {
    /// Creates a new empty STM store.
    pub fn new() -> Self {
        StmStore {
            vars: HashMap::new(),
            next_id: 1,
            metrics: StmMetrics::default(),
        }
    }

    /// Creates a new TVar with an initial value.
    pub fn new_tvar(&mut self, initial: i64) -> TVarId {
        let id = TVarId(self.next_id);
        self.next_id += 1;
        self.vars.insert(id, TVar::new(id, initial));
        id
    }

    /// Reads a TVar's current value and version.
    pub fn read(&self, id: TVarId) -> Option<(i64, u64)> {
        self.vars.get(&id).map(|v| (v.value, v.version))
    }

    /// Validates and commits a transaction log.
    pub fn commit(&mut self, log: &TxLog) -> TxResult {
        self.metrics.attempts += 1;

        // Check for retry
        if log.retry_requested {
            self.metrics.retries += 1;
            return TxResult::Retry;
        }

        // Validate: check that all reads are still at the same version
        for (&id, &read_version) in &log.reads {
            if let Some(var) = self.vars.get(&id) {
                if var.version != read_version {
                    self.metrics.conflicts += 1;
                    return TxResult::Conflict(id);
                }
            } else {
                return TxResult::Error(format!("TVar {} does not exist", id));
            }
        }

        // Apply writes
        for (&id, &new_value) in &log.writes {
            if let Some(var) = self.vars.get_mut(&id) {
                var.value = new_value;
                var.version += 1;
            } else {
                return TxResult::Error(format!("TVar {} does not exist", id));
            }
        }

        self.metrics.commits += 1;
        TxResult::Committed
    }

    /// Returns the current value of a TVar.
    pub fn get_value(&self, id: TVarId) -> Option<i64> {
        self.vars.get(&id).map(|v| v.value)
    }

    /// Returns the number of TVars.
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    /// Returns true if there are no TVars.
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.6: Nested Transactions
// ═══════════════════════════════════════════════════════════════════════

/// A nested transaction context that can roll back independently.
#[derive(Debug)]
pub struct NestedTx {
    /// Parent transaction log.
    pub parent_log: TxLog,
    /// Child transaction log.
    pub child_log: TxLog,
    /// Nesting depth.
    pub depth: usize,
}

impl NestedTx {
    /// Creates a nested transaction from a parent log.
    pub fn new(parent_log: TxLog, depth: usize) -> Self {
        NestedTx {
            parent_log,
            child_log: TxLog::new(),
            depth,
        }
    }

    /// Merges the child log into the parent on successful commit.
    pub fn merge_into_parent(mut self) -> TxLog {
        for (id, version) in self.child_log.reads {
            self.parent_log.reads.entry(id).or_insert(version);
        }
        for (id, value) in self.child_log.writes {
            self.parent_log.writes.insert(id, value);
        }
        self.parent_log.ops.extend(self.child_log.ops);
        self.parent_log
    }

    /// Rolls back the child transaction (discards child log).
    pub fn rollback(self) -> TxLog {
        self.parent_log
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.7: STM + Async
// ═══════════════════════════════════════════════════════════════════════

/// Marker for whether an STM transaction can be used in async context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StmAsyncMode {
    /// Synchronous only.
    Sync,
    /// Safe to use in async contexts (no blocking).
    Async,
}

impl fmt::Display for StmAsyncMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StmAsyncMode::Sync => write!(f, "Sync"),
            StmAsyncMode::Async => write!(f, "Async"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.8: TVar Collections
// ═══════════════════════════════════════════════════════════════════════

/// A transactional map (TMap<K, V>) backed by STM.
#[derive(Debug, Default)]
pub struct TMap {
    /// Entries: key -> TVar ID that holds the value.
    entries: HashMap<String, TVarId>,
    /// Version for conflict detection.
    pub version: u64,
}

impl TMap {
    /// Creates a new empty transactional map.
    pub fn new() -> Self {
        TMap {
            entries: HashMap::new(),
            version: 0,
        }
    }

    /// Inserts a key-value pair, returning the TVar ID for the value.
    pub fn insert(&mut self, key: &str, tvar_id: TVarId) {
        self.entries.insert(key.to_string(), tvar_id);
        self.version += 1;
    }

    /// Gets the TVar ID for a key.
    pub fn get(&self, key: &str) -> Option<TVarId> {
        self.entries.get(key).copied()
    }

    /// Removes a key, returning the TVar ID if it existed.
    pub fn remove(&mut self, key: &str) -> Option<TVarId> {
        let result = self.entries.remove(key);
        if result.is_some() {
            self.version += 1;
        }
        result
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A transactional queue (TQueue<T>) backed by STM.
#[derive(Debug, Default)]
pub struct TQueue {
    /// Queue items as TVar IDs.
    items: Vec<TVarId>,
    /// Version for conflict detection.
    pub version: u64,
}

impl TQueue {
    /// Creates a new empty transactional queue.
    pub fn new() -> Self {
        TQueue {
            items: Vec::new(),
            version: 0,
        }
    }

    /// Enqueues a TVar ID.
    pub fn enqueue(&mut self, tvar_id: TVarId) {
        self.items.push(tvar_id);
        self.version += 1;
    }

    /// Dequeues the front item.
    pub fn dequeue(&mut self) -> Option<TVarId> {
        if self.items.is_empty() {
            None
        } else {
            self.version += 1;
            Some(self.items.remove(0))
        }
    }

    /// Returns the number of items in the queue.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Peeks at the front item without removing it.
    pub fn peek(&self) -> Option<TVarId> {
        self.items.first().copied()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.9: STM Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Metrics for STM performance tuning.
#[derive(Debug, Clone, Default)]
pub struct StmMetrics {
    /// Total transaction attempts.
    pub attempts: usize,
    /// Successful commits.
    pub commits: usize,
    /// Retries (blocking).
    pub retries: usize,
    /// Conflicts detected.
    pub conflicts: usize,
}

impl StmMetrics {
    /// Returns the commit rate as a percentage.
    pub fn commit_rate(&self) -> f64 {
        if self.attempts == 0 {
            return 100.0;
        }
        (self.commits as f64 / self.attempts as f64) * 100.0
    }

    /// Returns the conflict rate as a percentage.
    pub fn conflict_rate(&self) -> f64 {
        if self.attempts == 0 {
            return 0.0;
        }
        (self.conflicts as f64 / self.attempts as f64) * 100.0
    }
}

impl fmt::Display for StmMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "attempts={}, commits={}, retries={}, conflicts={}",
            self.attempts, self.commits, self.retries, self.conflicts
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S8.1 — TVar Primitive
    #[test]
    fn s8_1_tvar_create() {
        let tvar = TVar::new(TVarId(1), 42);
        assert_eq!(tvar.value, 42);
        assert_eq!(tvar.version, 0);
    }

    #[test]
    fn s8_1_tvar_display() {
        let tvar = TVar::new(TVarId(1), 100);
        assert!(tvar.to_string().contains("value=100"));
    }

    // S8.2 — STM Transaction
    #[test]
    fn s8_2_simple_transaction() {
        let mut store = StmStore::new();
        let x = store.new_tvar(10);
        let y = store.new_tvar(20);

        // Transaction: read x, write x+y to y
        let mut log = TxLog::new();
        let (x_val, x_ver) = store.read(x).unwrap();
        log.record_read(x, x_ver);
        log.record_write(y, x_val + 20);

        let result = store.commit(&log);
        assert_eq!(result, TxResult::Committed);
        assert_eq!(store.get_value(y), Some(30));
    }

    #[test]
    fn s8_2_tx_result_display() {
        assert_eq!(TxResult::Committed.to_string(), "Committed");
        assert_eq!(TxResult::Retry.to_string(), "Retry");
    }

    // S8.3 — Retry Semantics
    #[test]
    fn s8_3_retry_transaction() {
        let mut store = StmStore::new();
        store.new_tvar(0);

        let mut log = TxLog::new();
        log.request_retry();

        let result = store.commit(&log);
        assert_eq!(result, TxResult::Retry);
        assert_eq!(store.metrics.retries, 1);
    }

    // S8.4 — OrElse Combinator
    #[test]
    fn s8_4_or_else_retry_fallback() {
        let tx1 = TxResult::Retry;
        let tx2 = TxResult::Committed;
        assert_eq!(or_else(&tx1, &tx2), TxResult::Committed);
    }

    #[test]
    fn s8_4_or_else_first_succeeds() {
        let tx1 = TxResult::Committed;
        let tx2 = TxResult::Retry;
        assert_eq!(or_else(&tx1, &tx2), TxResult::Committed);
    }

    // S8.5 — Conflict Detection
    #[test]
    fn s8_5_conflict_detection() {
        let mut store = StmStore::new();
        let x = store.new_tvar(10);

        // Transaction 1 reads x at version 0
        let mut log1 = TxLog::new();
        let (_, x_ver) = store.read(x).unwrap();
        log1.record_read(x, x_ver);
        log1.record_write(x, 20);

        // Transaction 2 modifies x first
        let mut log2 = TxLog::new();
        log2.record_write(x, 30);
        assert_eq!(store.commit(&log2), TxResult::Committed);

        // Transaction 1 should conflict (x version changed)
        let result = store.commit(&log1);
        assert_eq!(result, TxResult::Conflict(x));
        assert_eq!(store.metrics.conflicts, 1);
    }

    // S8.6 — Nested Transactions
    #[test]
    fn s8_6_nested_merge() {
        let mut parent = TxLog::new();
        parent.record_read(TVarId(1), 0);
        parent.record_write(TVarId(1), 10);

        let mut nested = NestedTx::new(parent, 1);
        nested.child_log.record_write(TVarId(2), 20);

        let merged = nested.merge_into_parent();
        assert!(merged.writes.contains_key(&TVarId(1)));
        assert!(merged.writes.contains_key(&TVarId(2)));
    }

    #[test]
    fn s8_6_nested_rollback() {
        let mut parent = TxLog::new();
        parent.record_write(TVarId(1), 10);

        let mut nested = NestedTx::new(parent, 1);
        nested.child_log.record_write(TVarId(2), 20);

        let rolled_back = nested.rollback();
        assert!(rolled_back.writes.contains_key(&TVarId(1)));
        assert!(!rolled_back.writes.contains_key(&TVarId(2)));
    }

    // S8.7 — STM + Async
    #[test]
    fn s8_7_async_mode() {
        assert_eq!(StmAsyncMode::Sync.to_string(), "Sync");
        assert_eq!(StmAsyncMode::Async.to_string(), "Async");
        assert_ne!(StmAsyncMode::Sync, StmAsyncMode::Async);
    }

    // S8.8 — TVar Collections
    #[test]
    fn s8_8_tmap_operations() {
        let mut tmap = TMap::new();
        tmap.insert("key1", TVarId(1));
        tmap.insert("key2", TVarId(2));

        assert_eq!(tmap.get("key1"), Some(TVarId(1)));
        assert_eq!(tmap.len(), 2);
        tmap.remove("key1");
        assert_eq!(tmap.len(), 1);
        assert!(tmap.get("key1").is_none());
    }

    #[test]
    fn s8_8_tqueue_fifo() {
        let mut queue = TQueue::new();
        queue.enqueue(TVarId(1));
        queue.enqueue(TVarId(2));
        queue.enqueue(TVarId(3));

        assert_eq!(queue.peek(), Some(TVarId(1)));
        assert_eq!(queue.dequeue(), Some(TVarId(1)));
        assert_eq!(queue.dequeue(), Some(TVarId(2)));
        assert_eq!(queue.len(), 1);
    }

    // S8.9 — STM Metrics
    #[test]
    fn s8_9_metrics_rates() {
        let metrics = StmMetrics {
            attempts: 10,
            commits: 7,
            retries: 1,
            conflicts: 2,
        };
        assert!((metrics.commit_rate() - 70.0).abs() < 0.1);
        assert!((metrics.conflict_rate() - 20.0).abs() < 0.1);
    }

    #[test]
    fn s8_9_metrics_display() {
        let metrics = StmMetrics {
            attempts: 5,
            commits: 4,
            retries: 0,
            conflicts: 1,
        };
        let s = metrics.to_string();
        assert!(s.contains("attempts=5"));
        assert!(s.contains("commits=4"));
    }

    // S8.10 — Integration
    #[test]
    fn s8_10_store_multiple_tvars() {
        let mut store = StmStore::new();
        let a = store.new_tvar(1);
        let b = store.new_tvar(2);
        let c = store.new_tvar(3);
        assert_eq!(store.len(), 3);
        assert_eq!(store.get_value(a), Some(1));
        assert_eq!(store.get_value(b), Some(2));
        assert_eq!(store.get_value(c), Some(3));
    }

    #[test]
    fn s8_10_tx_log_sets() {
        let mut log = TxLog::new();
        log.record_read(TVarId(1), 0);
        log.record_read(TVarId(2), 0);
        log.record_write(TVarId(3), 42);
        assert_eq!(log.read_set().len(), 2);
        assert_eq!(log.write_set().len(), 1);
    }

    #[test]
    fn s8_10_tvar_id_display() {
        assert_eq!(TVarId(42).to_string(), "TVar(42)");
    }
}
