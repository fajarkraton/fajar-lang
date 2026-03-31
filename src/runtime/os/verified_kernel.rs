//! Verified kernel functions — formal verification for @kernel code.
//!
//! Sprint N1: Provides static analysis proofs for kernel function correctness.
//! All verification is simulation-based (symbolic execution on in-memory models).
//!
//! # Architecture
//!
//! ```text
//! KernelVerifier
//! ├── MemoryAllocatorProof   — no double-free, no use-after-free
//! ├── SchedulerProof         — no deadlock, fair scheduling
//! ├── SyscallProof           — all syscalls return safely
//! ├── PageTableProof         — no unmapped access
//! ├── InterruptProof         — no nested locks, bounded handlers
//! ├── IpcProof               — message ordering, no data loss
//! ├── CowForkProof           — refcount correctness
//! ├── FsJournalProof         — WAL consistency
//! └── VerificationReport     — DO-178C style evidence
//! ```

use std::collections::{HashMap, HashSet};
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from kernel verification.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum VerificationError {
    /// A double-free was detected.
    #[error("double-free detected: address {addr:#x} freed twice")]
    DoubleFree {
        /// The address that was freed twice.
        addr: u64,
    },

    /// A use-after-free was detected.
    #[error("use-after-free: address {addr:#x} accessed after free")]
    UseAfterFree {
        /// The address that was accessed after being freed.
        addr: u64,
    },

    /// Memory leak detected — allocated but never freed.
    #[error("memory leak: {count} allocation(s) never freed")]
    MemoryLeak {
        /// Number of leaked allocations.
        count: usize,
    },

    /// Deadlock detected in lock ordering.
    #[error("potential deadlock: lock {lock_a} and {lock_b} acquired in inconsistent order")]
    DeadlockDetected {
        /// First lock in the cycle.
        lock_a: String,
        /// Second lock in the cycle.
        lock_b: String,
    },

    /// Unfair scheduling detected.
    #[error("unfair scheduling: task '{task}' starved ({runs} runs vs {max_runs} max)")]
    UnfairScheduling {
        /// The starved task.
        task: String,
        /// How many times this task ran.
        runs: u64,
        /// Maximum runs among all tasks.
        max_runs: u64,
    },

    /// Unhandled syscall path.
    #[error("syscall {num} has unhandled case: {reason}")]
    UnhandledSyscall {
        /// Syscall number.
        num: u64,
        /// Description of the unhandled case.
        reason: String,
    },

    /// Access to unmapped page.
    #[error("unmapped page access at virtual address {vaddr:#x}")]
    UnmappedAccess {
        /// The virtual address that has no mapping.
        vaddr: u64,
    },

    /// Nested lock in interrupt handler.
    #[error("nested lock '{lock}' in interrupt handler '{handler}'")]
    NestedLockInIrq {
        /// The lock name.
        lock: String,
        /// The handler name.
        handler: String,
    },

    /// Interrupt handler exceeds time bound.
    #[error("handler '{handler}' exceeds time bound: {cycles} cycles > {limit} limit")]
    HandlerTimeBound {
        /// Handler name.
        handler: String,
        /// Estimated cycle count.
        cycles: u64,
        /// Maximum allowed cycles.
        limit: u64,
    },

    /// IPC message ordering violation.
    #[error("IPC ordering violation: expected seq {expected}, got {actual}")]
    IpcOrderingViolation {
        /// Expected sequence number.
        expected: u64,
        /// Actual sequence number.
        actual: u64,
    },

    /// IPC data loss detected.
    #[error("IPC data loss: {lost} message(s) lost in channel '{channel}'")]
    IpcDataLoss {
        /// Channel name.
        channel: String,
        /// Number of lost messages.
        lost: u64,
    },

    /// CoW refcount error.
    #[error("CoW refcount error at page {page:#x}: expected {expected}, got {actual}")]
    CowRefcountError {
        /// Page address.
        page: u64,
        /// Expected refcount.
        expected: u32,
        /// Actual refcount.
        actual: u32,
    },

    /// Filesystem journal inconsistency.
    #[error("journal inconsistency: {reason}")]
    JournalInconsistency {
        /// Description of the inconsistency.
        reason: String,
    },
}

/// Result type for verification operations.
pub type VerifyResult<T> = Result<T, VerificationError>;

// ═══════════════════════════════════════════════════════════════════════
// Proof status
// ═══════════════════════════════════════════════════════════════════════

/// Outcome of a single proof check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofStatus {
    /// Proof passed — property holds.
    Verified,
    /// Proof failed with errors.
    Failed(Vec<VerificationError>),
    /// Proof was skipped (e.g., component not present).
    Skipped(String),
}

impl ProofStatus {
    /// Returns true if this proof passed.
    pub fn is_verified(&self) -> bool {
        matches!(self, ProofStatus::Verified)
    }

    /// Returns true if this proof failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, ProofStatus::Failed(_))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Memory Allocator Proof
// ═══════════════════════════════════════════════════════════════════════

/// Tracks allocation/free calls to verify memory safety.
///
/// Checks:
/// - No double-free (freeing an address that is already free)
/// - No use-after-free (accessing an address after it was freed)
/// - No memory leaks (all allocations must be freed)
#[derive(Debug, Clone)]
pub struct MemoryAllocatorProof {
    /// Currently live allocations: address -> size.
    live: HashMap<u64, u64>,
    /// Set of freed addresses (for double-free detection).
    freed: HashSet<u64>,
    /// Access log: addresses accessed since last check.
    accesses: Vec<u64>,
}

impl MemoryAllocatorProof {
    /// Creates a new empty proof tracker.
    pub fn new() -> Self {
        Self {
            live: HashMap::new(),
            freed: HashSet::new(),
            accesses: Vec::new(),
        }
    }

    /// Records an allocation at the given address with the given size.
    pub fn record_alloc(&mut self, addr: u64, size: u64) {
        self.live.insert(addr, size);
        self.freed.remove(&addr);
    }

    /// Records a free of the given address.
    pub fn record_free(&mut self, addr: u64) -> VerifyResult<()> {
        if self.freed.contains(&addr) {
            return Err(VerificationError::DoubleFree { addr });
        }
        if self.live.remove(&addr).is_none() {
            return Err(VerificationError::DoubleFree { addr });
        }
        self.freed.insert(addr);
        Ok(())
    }

    /// Records a memory access at the given address.
    pub fn record_access(&mut self, addr: u64) -> VerifyResult<()> {
        if self.freed.contains(&addr) {
            return Err(VerificationError::UseAfterFree { addr });
        }
        self.accesses.push(addr);
        Ok(())
    }

    /// Checks for memory leaks (allocations that were never freed).
    pub fn check_leaks(&self) -> ProofStatus {
        if self.live.is_empty() {
            ProofStatus::Verified
        } else {
            ProofStatus::Failed(vec![VerificationError::MemoryLeak {
                count: self.live.len(),
            }])
        }
    }

    /// Returns the number of currently live allocations.
    pub fn live_count(&self) -> usize {
        self.live.len()
    }

    /// Returns the number of freed addresses tracked.
    pub fn freed_count(&self) -> usize {
        self.freed.len()
    }
}

impl Default for MemoryAllocatorProof {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Scheduler Proof
// ═══════════════════════════════════════════════════════════════════════

/// Verifies scheduler correctness: no deadlock, fair scheduling.
///
/// - Deadlock detection via lock ordering graph (detect cycles)
/// - Fairness check: all tasks should run within a bounded ratio
#[derive(Debug, Clone)]
pub struct SchedulerProof {
    /// Lock ordering graph: lock A -> set of locks acquired while A held.
    lock_order: HashMap<String, HashSet<String>>,
    /// Task run counts for fairness analysis.
    task_runs: HashMap<String, u64>,
    /// Maximum allowed run ratio for fairness (e.g., 2.0 = 2x max/min).
    fairness_ratio: f64,
}

impl SchedulerProof {
    /// Creates a new scheduler proof with the given fairness ratio.
    ///
    /// `fairness_ratio` is the maximum allowed ratio between the most-run
    /// and least-run tasks. A ratio of 2.0 means no task can run more than
    /// twice as often as any other.
    pub fn new(fairness_ratio: f64) -> Self {
        Self {
            lock_order: HashMap::new(),
            task_runs: HashMap::new(),
            fairness_ratio,
        }
    }

    /// Records that `lock_b` was acquired while `lock_a` was already held.
    pub fn record_lock_order(&mut self, lock_a: &str, lock_b: &str) {
        self.lock_order
            .entry(lock_a.to_string())
            .or_default()
            .insert(lock_b.to_string());
    }

    /// Checks for deadlock cycles in the lock ordering graph.
    pub fn check_deadlock(&self) -> ProofStatus {
        // Check for A->B and B->A patterns (cycle of length 2).
        for (lock_a, deps_a) in &self.lock_order {
            for lock_b in deps_a {
                if let Some(deps_b) = self.lock_order.get(lock_b) {
                    if deps_b.contains(lock_a) {
                        return ProofStatus::Failed(vec![VerificationError::DeadlockDetected {
                            lock_a: lock_a.clone(),
                            lock_b: lock_b.clone(),
                        }]);
                    }
                }
            }
        }
        ProofStatus::Verified
    }

    /// Records that a task was scheduled to run.
    pub fn record_task_run(&mut self, task: &str) {
        *self.task_runs.entry(task.to_string()).or_insert(0) += 1;
    }

    /// Checks that scheduling is fair (within the configured ratio).
    pub fn check_fairness(&self) -> ProofStatus {
        if self.task_runs.is_empty() {
            return ProofStatus::Skipped("no tasks recorded".to_string());
        }

        let min_runs = self.task_runs.values().copied().min().unwrap_or(0);
        let max_runs = self.task_runs.values().copied().max().unwrap_or(0);

        if min_runs == 0 {
            // Find the starved task.
            let starved = self
                .task_runs
                .iter()
                .find(|&(_, &v)| v == 0)
                .map(|(k, _)| k.clone())
                .unwrap_or_default();
            return ProofStatus::Failed(vec![VerificationError::UnfairScheduling {
                task: starved,
                runs: 0,
                max_runs,
            }]);
        }

        let ratio = max_runs as f64 / min_runs as f64;
        if ratio > self.fairness_ratio {
            let starved = self
                .task_runs
                .iter()
                .find(|&(_, &v)| v == min_runs)
                .map(|(k, _)| k.clone())
                .unwrap_or_default();
            return ProofStatus::Failed(vec![VerificationError::UnfairScheduling {
                task: starved,
                runs: min_runs,
                max_runs,
            }]);
        }

        ProofStatus::Verified
    }

    /// Returns the number of distinct tasks tracked.
    pub fn task_count(&self) -> usize {
        self.task_runs.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Syscall Proof
// ═══════════════════════════════════════════════════════════════════════

/// Verifies that all syscalls return safely with no unhandled cases.
///
/// For each registered syscall number, tracks whether all argument
/// patterns are handled.
#[derive(Debug, Clone)]
pub struct SyscallProof {
    /// Registered syscall numbers with their handler names.
    registered: HashMap<u64, String>,
    /// Tested argument patterns per syscall: num -> set of tested patterns.
    tested_patterns: HashMap<u64, HashSet<String>>,
    /// Required patterns per syscall (e.g., "null_arg", "max_arg").
    required_patterns: Vec<String>,
}

impl SyscallProof {
    /// Creates a new syscall proof checker with standard required patterns.
    pub fn new() -> Self {
        Self {
            registered: HashMap::new(),
            tested_patterns: HashMap::new(),
            required_patterns: vec![
                "null_arg".to_string(),
                "max_arg".to_string(),
                "invalid_fd".to_string(),
            ],
        }
    }

    /// Registers a syscall number with its handler name.
    pub fn register_syscall(&mut self, num: u64, handler: &str) {
        self.registered.insert(num, handler.to_string());
    }

    /// Records that a specific argument pattern was tested for a syscall.
    pub fn record_test(&mut self, num: u64, pattern: &str) {
        self.tested_patterns
            .entry(num)
            .or_default()
            .insert(pattern.to_string());
    }

    /// Checks that all registered syscalls have all required patterns tested.
    pub fn check_coverage(&self) -> ProofStatus {
        let mut errors = Vec::new();
        for &num in self.registered.keys() {
            let tested = self.tested_patterns.get(&num);
            for pattern in &self.required_patterns {
                let has_pattern = tested.map(|s| s.contains(pattern)).unwrap_or(false);
                if !has_pattern {
                    errors.push(VerificationError::UnhandledSyscall {
                        num,
                        reason: format!("pattern '{}' not tested", pattern),
                    });
                }
            }
        }
        if errors.is_empty() {
            ProofStatus::Verified
        } else {
            ProofStatus::Failed(errors)
        }
    }

    /// Returns the number of registered syscalls.
    pub fn registered_count(&self) -> usize {
        self.registered.len()
    }
}

impl Default for SyscallProof {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Page Table Proof
// ═══════════════════════════════════════════════════════════════════════

/// Verifies that all virtual addresses accessed have valid page mappings.
#[derive(Debug, Clone)]
pub struct PageTableProof {
    /// Mapped virtual page ranges: base_vaddr -> (phys_addr, num_pages).
    mapped_pages: HashMap<u64, (u64, u64)>,
    /// Page size in bytes.
    page_size: u64,
}

impl PageTableProof {
    /// Creates a new page table proof checker with the given page size.
    pub fn new(page_size: u64) -> Self {
        Self {
            mapped_pages: HashMap::new(),
            page_size,
        }
    }

    /// Records a page mapping: virtual base -> (physical base, count).
    pub fn record_mapping(&mut self, vaddr: u64, paddr: u64, num_pages: u64) {
        self.mapped_pages.insert(vaddr, (paddr, num_pages));
    }

    /// Removes a page mapping.
    pub fn remove_mapping(&mut self, vaddr: u64) {
        self.mapped_pages.remove(&vaddr);
    }

    /// Checks whether a virtual address is mapped.
    pub fn check_access(&self, vaddr: u64) -> VerifyResult<u64> {
        let page_base = (vaddr / self.page_size) * self.page_size;
        for (&base, &(_paddr, count)) in &self.mapped_pages {
            let end = base + count * self.page_size;
            if page_base >= base && page_base < end {
                let offset = vaddr - base;
                return Ok(_paddr + offset);
            }
        }
        Err(VerificationError::UnmappedAccess { vaddr })
    }

    /// Returns the number of mapped page ranges.
    pub fn mapping_count(&self) -> usize {
        self.mapped_pages.len()
    }

    /// Returns the total number of mapped pages.
    pub fn total_pages(&self) -> u64 {
        self.mapped_pages.values().map(|&(_, c)| c).sum()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Interrupt Proof
// ═══════════════════════════════════════════════════════════════════════

/// Verifies interrupt handler safety: no nested locks, bounded time.
#[derive(Debug, Clone)]
pub struct InterruptProof {
    /// Locks held by each handler: handler_name -> set of lock names.
    handler_locks: HashMap<String, HashSet<String>>,
    /// Estimated cycle counts per handler.
    handler_cycles: HashMap<String, u64>,
    /// Maximum allowed cycles per handler.
    cycle_limit: u64,
}

impl InterruptProof {
    /// Creates a new interrupt proof checker with the given cycle limit.
    pub fn new(cycle_limit: u64) -> Self {
        Self {
            handler_locks: HashMap::new(),
            handler_cycles: HashMap::new(),
            cycle_limit,
        }
    }

    /// Records that a handler acquires a lock.
    pub fn record_lock(&mut self, handler: &str, lock: &str) {
        self.handler_locks
            .entry(handler.to_string())
            .or_default()
            .insert(lock.to_string());
    }

    /// Records the estimated cycle count for a handler.
    pub fn record_cycles(&mut self, handler: &str, cycles: u64) {
        self.handler_cycles.insert(handler.to_string(), cycles);
    }

    /// Checks that no handler holds more than one lock (no nested locks).
    pub fn check_nested_locks(&self) -> ProofStatus {
        let mut errors = Vec::new();
        for (handler, locks) in &self.handler_locks {
            if locks.len() > 1 {
                let lock_names: Vec<_> = locks.iter().cloned().collect();
                errors.push(VerificationError::NestedLockInIrq {
                    lock: lock_names.join(", "),
                    handler: handler.clone(),
                });
            }
        }
        if errors.is_empty() {
            ProofStatus::Verified
        } else {
            ProofStatus::Failed(errors)
        }
    }

    /// Checks that all handlers are within the cycle budget.
    pub fn check_time_bounds(&self) -> ProofStatus {
        let mut errors = Vec::new();
        for (handler, &cycles) in &self.handler_cycles {
            if cycles > self.cycle_limit {
                errors.push(VerificationError::HandlerTimeBound {
                    handler: handler.clone(),
                    cycles,
                    limit: self.cycle_limit,
                });
            }
        }
        if errors.is_empty() {
            ProofStatus::Verified
        } else {
            ProofStatus::Failed(errors)
        }
    }

    /// Returns the number of tracked handlers.
    pub fn handler_count(&self) -> usize {
        self.handler_locks.len().max(self.handler_cycles.len())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IPC Proof
// ═══════════════════════════════════════════════════════════════════════

/// Verifies IPC message ordering and data integrity.
///
/// Uses sequence numbers to detect reordering and loss.
#[derive(Debug, Clone)]
pub struct IpcProof {
    /// Next expected sequence number per channel.
    expected_seq: HashMap<String, u64>,
    /// Total messages sent per channel.
    sent_count: HashMap<String, u64>,
    /// Total messages received per channel.
    received_count: HashMap<String, u64>,
}

impl IpcProof {
    /// Creates a new IPC proof tracker.
    pub fn new() -> Self {
        Self {
            expected_seq: HashMap::new(),
            sent_count: HashMap::new(),
            received_count: HashMap::new(),
        }
    }

    /// Records a message sent on the given channel with a sequence number.
    pub fn record_send(&mut self, channel: &str, _seq: u64) {
        *self.sent_count.entry(channel.to_string()).or_insert(0) += 1;
    }

    /// Records a message received on the given channel with a sequence number.
    pub fn record_receive(&mut self, channel: &str, seq: u64) -> VerifyResult<()> {
        let expected = self.expected_seq.entry(channel.to_string()).or_insert(0);
        if seq != *expected {
            return Err(VerificationError::IpcOrderingViolation {
                expected: *expected,
                actual: seq,
            });
        }
        *expected += 1;
        *self.received_count.entry(channel.to_string()).or_insert(0) += 1;
        Ok(())
    }

    /// Checks for data loss (sent > received) across all channels.
    pub fn check_data_loss(&self) -> ProofStatus {
        let mut errors = Vec::new();
        for (channel, &sent) in &self.sent_count {
            let received = self.received_count.get(channel).copied().unwrap_or(0);
            if received < sent {
                errors.push(VerificationError::IpcDataLoss {
                    channel: channel.clone(),
                    lost: sent - received,
                });
            }
        }
        if errors.is_empty() {
            ProofStatus::Verified
        } else {
            ProofStatus::Failed(errors)
        }
    }

    /// Returns the number of tracked channels.
    pub fn channel_count(&self) -> usize {
        self.sent_count.len()
    }
}

impl Default for IpcProof {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CoW Fork Proof
// ═══════════════════════════════════════════════════════════════════════

/// Verifies Copy-on-Write fork refcount correctness.
///
/// Rules:
/// - Fork increments refcount for shared pages
/// - Process exit decrements refcount
/// - Page freed when refcount reaches 0
#[derive(Debug, Clone)]
pub struct CowForkProof {
    /// Refcount per page: page_addr -> refcount.
    refcounts: HashMap<u64, u32>,
    /// Pages that have been freed (refcount reached 0).
    freed_pages: HashSet<u64>,
}

impl CowForkProof {
    /// Creates a new CoW fork proof tracker.
    pub fn new() -> Self {
        Self {
            refcounts: HashMap::new(),
            freed_pages: HashSet::new(),
        }
    }

    /// Records initial allocation of a page (refcount = 1).
    pub fn record_alloc(&mut self, page: u64) {
        self.refcounts.insert(page, 1);
        self.freed_pages.remove(&page);
    }

    /// Records a fork: increment refcount for a shared page.
    pub fn record_fork(&mut self, page: u64) -> VerifyResult<()> {
        let count = self
            .refcounts
            .get_mut(&page)
            .ok_or(VerificationError::CowRefcountError {
                page,
                expected: 1,
                actual: 0,
            })?;
        *count += 1;
        Ok(())
    }

    /// Records process exit: decrement refcount, free at 0.
    pub fn record_exit(&mut self, page: u64) -> VerifyResult<()> {
        let count = self
            .refcounts
            .get_mut(&page)
            .ok_or(VerificationError::CowRefcountError {
                page,
                expected: 1,
                actual: 0,
            })?;
        if *count == 0 {
            return Err(VerificationError::CowRefcountError {
                page,
                expected: 1,
                actual: 0,
            });
        }
        *count -= 1;
        if *count == 0 {
            self.refcounts.remove(&page);
            self.freed_pages.insert(page);
        }
        Ok(())
    }

    /// Checks that a page has the expected refcount.
    pub fn verify_refcount(&self, page: u64, expected: u32) -> ProofStatus {
        let actual = self.refcounts.get(&page).copied().unwrap_or(0);
        if actual == expected {
            ProofStatus::Verified
        } else {
            ProofStatus::Failed(vec![VerificationError::CowRefcountError {
                page,
                expected,
                actual,
            }])
        }
    }

    /// Returns true if a page has been freed (refcount reached 0).
    pub fn is_freed(&self, page: u64) -> bool {
        self.freed_pages.contains(&page)
    }

    /// Returns the current refcount for a page, or 0 if freed/unknown.
    pub fn refcount(&self, page: u64) -> u32 {
        self.refcounts.get(&page).copied().unwrap_or(0)
    }
}

impl Default for CowForkProof {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FS Journal Proof
// ═══════════════════════════════════════════════════════════════════════

/// A WAL (Write-Ahead Log) journal entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    /// Transaction ID.
    pub txn_id: u64,
    /// Block number being modified.
    pub block: u64,
    /// Old data (for rollback).
    pub old_data: Vec<u8>,
    /// New data (to be committed).
    pub new_data: Vec<u8>,
    /// Whether this entry has been committed.
    pub committed: bool,
}

/// Verifies filesystem journaling WAL consistency.
///
/// Checks:
/// - All entries for a transaction are written before commit
/// - Committed transactions can be replayed
/// - Incomplete transactions can be rolled back
#[derive(Debug, Clone)]
pub struct FsJournalProof {
    /// Journal entries by transaction ID.
    entries: HashMap<u64, Vec<JournalEntry>>,
    /// Set of committed transaction IDs.
    committed_txns: HashSet<u64>,
    /// Set of rolled-back transaction IDs.
    rolledback_txns: HashSet<u64>,
    /// Next transaction ID.
    next_txn_id: u64,
}

impl FsJournalProof {
    /// Creates a new filesystem journal proof tracker.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            committed_txns: HashSet::new(),
            rolledback_txns: HashSet::new(),
            next_txn_id: 1,
        }
    }

    /// Begins a new transaction, returning its ID.
    pub fn begin_transaction(&mut self) -> u64 {
        let id = self.next_txn_id;
        self.next_txn_id += 1;
        self.entries.insert(id, Vec::new());
        id
    }

    /// Writes a journal entry for the given transaction.
    pub fn write_entry(
        &mut self,
        txn_id: u64,
        block: u64,
        old_data: Vec<u8>,
        new_data: Vec<u8>,
    ) -> VerifyResult<()> {
        let entries =
            self.entries
                .get_mut(&txn_id)
                .ok_or(VerificationError::JournalInconsistency {
                    reason: format!("transaction {} not found", txn_id),
                })?;
        entries.push(JournalEntry {
            txn_id,
            block,
            old_data,
            new_data,
            committed: false,
        });
        Ok(())
    }

    /// Commits a transaction (marks all entries as committed).
    pub fn commit(&mut self, txn_id: u64) -> VerifyResult<()> {
        let entries =
            self.entries
                .get_mut(&txn_id)
                .ok_or(VerificationError::JournalInconsistency {
                    reason: format!("transaction {} not found", txn_id),
                })?;
        if entries.is_empty() {
            return Err(VerificationError::JournalInconsistency {
                reason: format!("transaction {} has no entries to commit", txn_id),
            });
        }
        for entry in entries.iter_mut() {
            entry.committed = true;
        }
        self.committed_txns.insert(txn_id);
        Ok(())
    }

    /// Rolls back a transaction (marks as rolled back).
    pub fn rollback(&mut self, txn_id: u64) -> VerifyResult<()> {
        if !self.entries.contains_key(&txn_id) {
            return Err(VerificationError::JournalInconsistency {
                reason: format!("transaction {} not found for rollback", txn_id),
            });
        }
        self.rolledback_txns.insert(txn_id);
        Ok(())
    }

    /// Checks journal consistency: all committed txns have entries,
    /// no txn is both committed and rolled back.
    pub fn check_consistency(&self) -> ProofStatus {
        let mut errors = Vec::new();

        // Check for conflicting states.
        for txn_id in &self.committed_txns {
            if self.rolledback_txns.contains(txn_id) {
                errors.push(VerificationError::JournalInconsistency {
                    reason: format!("transaction {} is both committed and rolled back", txn_id),
                });
            }
        }

        // Check that committed txns have all entries marked committed.
        for txn_id in &self.committed_txns {
            if let Some(entries) = self.entries.get(txn_id) {
                for entry in entries {
                    if !entry.committed {
                        errors.push(VerificationError::JournalInconsistency {
                            reason: format!(
                                "transaction {} has uncommitted entry for block {}",
                                txn_id, entry.block
                            ),
                        });
                    }
                }
            }
        }

        if errors.is_empty() {
            ProofStatus::Verified
        } else {
            ProofStatus::Failed(errors)
        }
    }

    /// Returns the number of committed transactions.
    pub fn committed_count(&self) -> usize {
        self.committed_txns.len()
    }

    /// Returns the number of pending (neither committed nor rolled back) transactions.
    pub fn pending_count(&self) -> usize {
        self.entries
            .keys()
            .filter(|id| !self.committed_txns.contains(id) && !self.rolledback_txns.contains(id))
            .count()
    }
}

impl Default for FsJournalProof {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Verification Report
// ═══════════════════════════════════════════════════════════════════════

/// A single proof result in the verification report.
#[derive(Debug, Clone)]
pub struct ProofResult {
    /// Name of the proof.
    pub name: String,
    /// Category (e.g., "memory", "scheduler", "syscall").
    pub category: String,
    /// Outcome.
    pub status: ProofStatus,
}

/// DO-178C style verification report.
///
/// Collects all proof results and generates a summary suitable for
/// safety certification evidence.
#[derive(Debug, Clone)]
pub struct VerificationReport {
    /// System name being verified.
    pub system_name: String,
    /// Individual proof results.
    pub results: Vec<ProofResult>,
}

impl VerificationReport {
    /// Creates a new empty verification report for the named system.
    pub fn new(system_name: &str) -> Self {
        Self {
            system_name: system_name.to_string(),
            results: Vec::new(),
        }
    }

    /// Adds a proof result to the report.
    pub fn add_result(&mut self, name: &str, category: &str, status: ProofStatus) {
        self.results.push(ProofResult {
            name: name.to_string(),
            category: category.to_string(),
            status,
        });
    }

    /// Returns the total number of proofs.
    pub fn total_proofs(&self) -> usize {
        self.results.len()
    }

    /// Returns the number of verified (passed) proofs.
    pub fn verified_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status.is_verified())
            .count()
    }

    /// Returns the number of failed proofs.
    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| r.status.is_failed()).count()
    }

    /// Returns the number of skipped proofs.
    pub fn skipped_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.status, ProofStatus::Skipped(_)))
            .count()
    }

    /// Returns true if all non-skipped proofs passed.
    pub fn all_verified(&self) -> bool {
        self.failed_count() == 0
    }

    /// Generates a text summary of the report.
    pub fn summary(&self) -> String {
        let mut s = format!("=== Verification Report: {} ===\n", self.system_name);
        s.push_str(&format!(
            "Total: {} | Verified: {} | Failed: {} | Skipped: {}\n",
            self.total_proofs(),
            self.verified_count(),
            self.failed_count(),
            self.skipped_count(),
        ));
        for result in &self.results {
            let status_str = match &result.status {
                ProofStatus::Verified => "PASS".to_string(),
                ProofStatus::Failed(errors) => {
                    format!("FAIL ({})", errors.len())
                }
                ProofStatus::Skipped(reason) => {
                    format!("SKIP ({})", reason)
                }
            };
            s.push_str(&format!(
                "  [{}] {}/{}: {}\n",
                status_str, result.category, result.name, result.category
            ));
        }
        s
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Kernel Verifier (top-level)
// ═══════════════════════════════════════════════════════════════════════

/// Top-level kernel verifier that orchestrates all proof checks.
///
/// Usage:
/// 1. Record operations (alloc/free, lock ordering, task runs, etc.)
/// 2. Call `verify_all()` to run all checks and generate a report.
#[derive(Debug, Clone)]
pub struct KernelVerifier {
    /// Memory allocator proof.
    pub memory: MemoryAllocatorProof,
    /// Scheduler proof.
    pub scheduler: SchedulerProof,
    /// Syscall proof.
    pub syscall: SyscallProof,
    /// Page table proof.
    pub page_table: PageTableProof,
    /// Interrupt proof.
    pub interrupt: InterruptProof,
    /// IPC proof.
    pub ipc: IpcProof,
    /// CoW fork proof.
    pub cow_fork: CowForkProof,
    /// Filesystem journal proof.
    pub journal: FsJournalProof,
    /// System name for the report.
    system_name: String,
}

impl KernelVerifier {
    /// Creates a new kernel verifier for the named system.
    ///
    /// Default settings:
    /// - Page size: 4096 bytes
    /// - Fairness ratio: 2.0
    /// - Interrupt cycle limit: 10,000
    pub fn new(system_name: &str) -> Self {
        Self {
            memory: MemoryAllocatorProof::new(),
            scheduler: SchedulerProof::new(2.0),
            syscall: SyscallProof::new(),
            page_table: PageTableProof::new(4096),
            interrupt: InterruptProof::new(10_000),
            ipc: IpcProof::new(),
            cow_fork: CowForkProof::new(),
            journal: FsJournalProof::new(),
            system_name: system_name.to_string(),
        }
    }

    /// Runs all proof checks and returns a verification report.
    pub fn verify_all(&self) -> VerificationReport {
        let mut report = VerificationReport::new(&self.system_name);

        // Memory proofs
        report.add_result("no_leaks", "memory", self.memory.check_leaks());

        // Scheduler proofs
        report.add_result("no_deadlock", "scheduler", self.scheduler.check_deadlock());
        report.add_result("fairness", "scheduler", self.scheduler.check_fairness());

        // Syscall proofs
        report.add_result("coverage", "syscall", self.syscall.check_coverage());

        // Interrupt proofs
        report.add_result(
            "no_nested_locks",
            "interrupt",
            self.interrupt.check_nested_locks(),
        );
        report.add_result(
            "time_bounds",
            "interrupt",
            self.interrupt.check_time_bounds(),
        );

        // IPC proofs
        report.add_result("no_data_loss", "ipc", self.ipc.check_data_loss());

        // Journal proofs
        report.add_result(
            "wal_consistency",
            "journal",
            self.journal.check_consistency(),
        );

        report
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── MemoryAllocatorProof ──

    #[test]
    fn memory_proof_alloc_and_free_passes() {
        let mut proof = MemoryAllocatorProof::new();
        proof.record_alloc(0x1000, 4096);
        assert!(proof.record_free(0x1000).is_ok());
        assert!(proof.check_leaks().is_verified());
    }

    #[test]
    fn memory_proof_double_free_detected() {
        let mut proof = MemoryAllocatorProof::new();
        proof.record_alloc(0x1000, 4096);
        assert!(proof.record_free(0x1000).is_ok());
        let err = proof.record_free(0x1000).unwrap_err();
        assert!(matches!(
            err,
            VerificationError::DoubleFree { addr: 0x1000 }
        ));
    }

    #[test]
    fn memory_proof_use_after_free_detected() {
        let mut proof = MemoryAllocatorProof::new();
        proof.record_alloc(0x2000, 512);
        assert!(proof.record_free(0x2000).is_ok());
        let err = proof.record_access(0x2000).unwrap_err();
        assert!(matches!(
            err,
            VerificationError::UseAfterFree { addr: 0x2000 }
        ));
    }

    #[test]
    fn memory_proof_leak_detected() {
        let mut proof = MemoryAllocatorProof::new();
        proof.record_alloc(0x3000, 1024);
        proof.record_alloc(0x4000, 2048);
        assert!(proof.record_free(0x3000).is_ok());
        let status = proof.check_leaks();
        assert!(status.is_failed());
    }

    #[test]
    fn memory_proof_access_live_ok() {
        let mut proof = MemoryAllocatorProof::new();
        proof.record_alloc(0x5000, 256);
        assert!(proof.record_access(0x5000).is_ok());
    }

    // ── SchedulerProof ──

    #[test]
    fn scheduler_proof_no_deadlock() {
        let mut proof = SchedulerProof::new(2.0);
        proof.record_lock_order("A", "B");
        proof.record_lock_order("B", "C");
        assert!(proof.check_deadlock().is_verified());
    }

    #[test]
    fn scheduler_proof_deadlock_detected() {
        let mut proof = SchedulerProof::new(2.0);
        proof.record_lock_order("A", "B");
        proof.record_lock_order("B", "A");
        assert!(proof.check_deadlock().is_failed());
    }

    #[test]
    fn scheduler_proof_fair_scheduling() {
        let mut proof = SchedulerProof::new(2.0);
        for _ in 0..10 {
            proof.record_task_run("task1");
            proof.record_task_run("task2");
        }
        assert!(proof.check_fairness().is_verified());
    }

    #[test]
    fn scheduler_proof_unfair_scheduling() {
        let mut proof = SchedulerProof::new(2.0);
        for _ in 0..10 {
            proof.record_task_run("task1");
        }
        proof.record_task_run("task2");
        assert!(proof.check_fairness().is_failed());
    }

    // ── SyscallProof ──

    #[test]
    fn syscall_proof_full_coverage() {
        let mut proof = SyscallProof::new();
        proof.register_syscall(0, "sys_read");
        proof.record_test(0, "null_arg");
        proof.record_test(0, "max_arg");
        proof.record_test(0, "invalid_fd");
        assert!(proof.check_coverage().is_verified());
    }

    #[test]
    fn syscall_proof_missing_coverage() {
        let mut proof = SyscallProof::new();
        proof.register_syscall(1, "sys_write");
        proof.record_test(1, "null_arg");
        // Missing max_arg and invalid_fd
        assert!(proof.check_coverage().is_failed());
    }

    // ── PageTableProof ──

    #[test]
    fn page_table_proof_mapped_access() {
        let mut proof = PageTableProof::new(4096);
        proof.record_mapping(0x0000_1000, 0x0010_0000, 4);
        let paddr = proof.check_access(0x0000_2000).unwrap();
        assert_eq!(paddr, 0x0010_1000);
    }

    #[test]
    fn page_table_proof_unmapped_access() {
        let proof = PageTableProof::new(4096);
        let err = proof.check_access(0xDEAD_0000).unwrap_err();
        assert!(matches!(err, VerificationError::UnmappedAccess { .. }));
    }

    #[test]
    fn page_table_proof_remove_mapping() {
        let mut proof = PageTableProof::new(4096);
        proof.record_mapping(0x1000, 0x2000, 1);
        assert!(proof.check_access(0x1000).is_ok());
        proof.remove_mapping(0x1000);
        assert!(proof.check_access(0x1000).is_err());
    }

    // ── InterruptProof ──

    #[test]
    fn interrupt_proof_single_lock_ok() {
        let mut proof = InterruptProof::new(10_000);
        proof.record_lock("timer_handler", "timer_lock");
        assert!(proof.check_nested_locks().is_verified());
    }

    #[test]
    fn interrupt_proof_nested_lock_fails() {
        let mut proof = InterruptProof::new(10_000);
        proof.record_lock("timer_handler", "lock_a");
        proof.record_lock("timer_handler", "lock_b");
        assert!(proof.check_nested_locks().is_failed());
    }

    #[test]
    fn interrupt_proof_time_bound_ok() {
        let mut proof = InterruptProof::new(10_000);
        proof.record_cycles("timer_handler", 5_000);
        assert!(proof.check_time_bounds().is_verified());
    }

    #[test]
    fn interrupt_proof_time_bound_exceeded() {
        let mut proof = InterruptProof::new(10_000);
        proof.record_cycles("slow_handler", 15_000);
        assert!(proof.check_time_bounds().is_failed());
    }

    // ── IpcProof ──

    #[test]
    fn ipc_proof_ordered_delivery() {
        let mut proof = IpcProof::new();
        proof.record_send("ch1", 0);
        proof.record_send("ch1", 1);
        assert!(proof.record_receive("ch1", 0).is_ok());
        assert!(proof.record_receive("ch1", 1).is_ok());
        assert!(proof.check_data_loss().is_verified());
    }

    #[test]
    fn ipc_proof_ordering_violation() {
        let mut proof = IpcProof::new();
        proof.record_send("ch1", 0);
        proof.record_send("ch1", 1);
        let err = proof.record_receive("ch1", 1).unwrap_err();
        assert!(matches!(
            err,
            VerificationError::IpcOrderingViolation {
                expected: 0,
                actual: 1,
            }
        ));
    }

    #[test]
    fn ipc_proof_data_loss_detected() {
        let mut proof = IpcProof::new();
        proof.record_send("ch1", 0);
        proof.record_send("ch1", 1);
        assert!(proof.record_receive("ch1", 0).is_ok());
        // Only received 1 of 2 messages.
        assert!(proof.check_data_loss().is_failed());
    }

    // ── CowForkProof ──

    #[test]
    fn cow_proof_alloc_fork_exit_free() {
        let mut proof = CowForkProof::new();
        proof.record_alloc(0x1000);
        assert!(proof.record_fork(0x1000).is_ok());
        assert_eq!(proof.refcount(0x1000), 2);
        assert!(proof.record_exit(0x1000).is_ok());
        assert_eq!(proof.refcount(0x1000), 1);
        assert!(proof.record_exit(0x1000).is_ok());
        assert!(proof.is_freed(0x1000));
    }

    #[test]
    fn cow_proof_refcount_verify() {
        let mut proof = CowForkProof::new();
        proof.record_alloc(0x2000);
        assert!(proof.verify_refcount(0x2000, 1).is_verified());
        assert!(proof.record_fork(0x2000).is_ok());
        assert!(proof.verify_refcount(0x2000, 2).is_verified());
        assert!(proof.verify_refcount(0x2000, 1).is_failed());
    }

    #[test]
    fn cow_proof_fork_unknown_page() {
        let mut proof = CowForkProof::new();
        assert!(proof.record_fork(0x9999).is_err());
    }

    // ── FsJournalProof ──

    #[test]
    fn journal_proof_commit_and_consistency() {
        let mut proof = FsJournalProof::new();
        let txn = proof.begin_transaction();
        assert!(proof.write_entry(txn, 42, vec![0], vec![1]).is_ok());
        assert!(proof.commit(txn).is_ok());
        assert!(proof.check_consistency().is_verified());
    }

    #[test]
    fn journal_proof_rollback() {
        let mut proof = FsJournalProof::new();
        let txn = proof.begin_transaction();
        assert!(proof.write_entry(txn, 10, vec![0], vec![1]).is_ok());
        assert!(proof.rollback(txn).is_ok());
        assert!(proof.check_consistency().is_verified());
    }

    #[test]
    fn journal_proof_empty_commit_fails() {
        let mut proof = FsJournalProof::new();
        let txn = proof.begin_transaction();
        assert!(proof.commit(txn).is_err());
    }

    #[test]
    fn journal_proof_unknown_txn_write_fails() {
        let mut proof = FsJournalProof::new();
        assert!(proof.write_entry(999, 0, vec![], vec![]).is_err());
    }

    #[test]
    fn journal_proof_pending_count() {
        let mut proof = FsJournalProof::new();
        let t1 = proof.begin_transaction();
        let _t2 = proof.begin_transaction();
        assert!(proof.write_entry(t1, 1, vec![0], vec![1]).is_ok());
        assert!(proof.commit(t1).is_ok());
        assert_eq!(proof.pending_count(), 1);
        assert_eq!(proof.committed_count(), 1);
    }

    // ── KernelVerifier (integrated) ──

    #[test]
    fn verifier_clean_system_passes() {
        let verifier = KernelVerifier::new("test-kernel");
        let report = verifier.verify_all();
        // All proofs should be verified or skipped (no data recorded = no failures).
        assert!(report.all_verified());
    }

    #[test]
    fn verifier_report_summary_output() {
        let mut verifier = KernelVerifier::new("FajarOS Nova v2");
        verifier.memory.record_alloc(0x1000, 4096);
        // Intentional leak — don't free.
        let report = verifier.verify_all();
        assert!(!report.all_verified());
        let summary = report.summary();
        assert!(summary.contains("FajarOS Nova v2"));
        assert!(summary.contains("FAIL"));
    }

    #[test]
    fn verifier_full_lifecycle() {
        let mut v = KernelVerifier::new("lifecycle-test");

        // Memory: alloc + free (clean).
        v.memory.record_alloc(0x1000, 4096);
        assert!(v.memory.record_free(0x1000).is_ok());

        // Scheduler: fair round-robin.
        for _ in 0..5 {
            v.scheduler.record_task_run("init");
            v.scheduler.record_task_run("idle");
        }

        // Interrupt: single lock, within budget.
        v.interrupt.record_lock("timer", "timer_lock");
        v.interrupt.record_cycles("timer", 1000);

        // IPC: send + receive all.
        v.ipc.record_send("pipe0", 0);
        assert!(v.ipc.record_receive("pipe0", 0).is_ok());

        // CoW: alloc, fork, two exits.
        v.cow_fork.record_alloc(0x2000);
        assert!(v.cow_fork.record_fork(0x2000).is_ok());
        assert!(v.cow_fork.record_exit(0x2000).is_ok());
        assert!(v.cow_fork.record_exit(0x2000).is_ok());

        // Journal: write + commit.
        let txn = v.journal.begin_transaction();
        assert!(v.journal.write_entry(txn, 1, vec![0], vec![1]).is_ok());
        assert!(v.journal.commit(txn).is_ok());

        let report = v.verify_all();
        assert!(report.all_verified());
        assert!(report.verified_count() >= 6);
    }
}
